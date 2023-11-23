use std::rc::Rc;

use self::macros::*;
use crate::emulator::abstractions::PinDirection;
use crate::emulator::cpus::mos6502::{AddressMode::*, Mnemonic, OperationDef};
use crate::emulator::cpus::CpuState;
use corosensei::Coroutine;

use super::{execute_operation, AddressMode, Operand};

pub type Input = Rc<CpuState>;
pub type Stepper = Coroutine<Input, (), StepperResult>;

pub fn get_stepper(op: &OperationDef) -> Option<Stepper> {
    use crate::emulator::cpus::mos6502::mnemonic::Mnemonic::*;
    let def = op.clone();

    let s = match op.address_mode {
        Implicit | Accumulator | Immediate if def.mnemonic != RTS => no_mem_stepper(def),
        Relative => branch_stepper(def),
        // ZeroPageX | ZeroPageY | AbsoluteX | AbsoluteY | Indirect | IndirectX | IndirectY => {
        //     panic!("Address mode {} not implemented!", op.address_mode);
        // }
        _ => match op.mnemonic {
            LDA | LDX | LDY | EOR | AND | ORA | ADC | SBC | CMP | CPX | CPY | BIT => {
                read_stepper(def)
            }
            STA | STX | STY => write_stepper(def),
            ASL | LSR | ROL | ROR | INC | DEC => rmw_stepper(def),
            PHA | PHP => push_stepper(def),
            PLA | PLP => pull_stepper(def),
            JMP => jmp_stepper(def),
            JSR => jsr_stepper(def),
            RTS | RTI => rts_rti_stepper(def),
            _ => return None,
        },
    };
    Some(s)
}

pub fn init_stepper() -> Stepper {
    Coroutine::new(move |yielder, cpu: Input| {
        let lo = fetch_byte_from_addr!(yielder, cpu, 0xfc, 0xff);
        cpu.set_pcl(lo);
        let hi = fetch_byte_from_addr!(yielder, cpu, 0xfd, 0xff);
        cpu.set_pch(hi);
        // cpu.set_pc(0xfce2);
        // cpu.set_pc(0xfce2);
        // yielder.suspend(());
        // yielder.suspend(());
        yielder.suspend(());
        StepperResult::partial(false, cpu)
    })
}

pub fn compensate() -> Stepper {
    Coroutine::new(move |yielder, cpu: Input| {
        yielder.suspend(());
        yielder.suspend(());
        StepperResult::partial(false, cpu)
    })
}

pub fn read_opcode() -> Stepper {
    Coroutine::new(move |yielder, cpu: Input| {
        request_opcode(Rc::clone(&cpu));
        yielder.suspend(());

        read_opcode_and_inc_pc(Rc::clone(&cpu));
        yielder.suspend(());

        StepperResult::partial(true, cpu)
    })
}

pub struct StepperResult {
    pub has_opcode: bool,
    pub cpu: Input,
    pub operand: Operand,
    pub completed: bool,
}

impl StepperResult {
    pub fn new(has_opcode: bool, cpu: Input, operand: Operand) -> Self {
        Self {
            has_opcode,
            cpu,
            operand,
            completed: true,
        }
    }

    fn partial(has_opcode: bool, cpu: Input) -> Self {
        Self {
            has_opcode,
            cpu,
            operand: Operand::None,
            completed: false,
        }
    }
}

/// Stepper for no-memory-access operations.
/// All operations with implicit and accumulator addressing modes
/// are handled by this stepper. Additionally, as the CPU for reads the next
/// byte anyway for these addressing modes, the stepper also handles the
/// immediate addressing.
///
/// ```text
/// Accumulator or implied addressing
///       #  address R/W description
///      --- ------- --- -----------------------------------------------
///       1    PC     R  fetch opcode, increment PC
///       2    PC     R  read next instruction byte (and throw it away)
///
/// Immediate addressing
///       #  address R/W description
///      --- ------- --- ------------------------------------------
///       1    PC     R  fetch opcode, increment PC
///       2    PC     R  fetch value, increment PC
/// ```
fn no_mem_stepper(op: OperationDef) -> Stepper {
    Coroutine::new(move |yielder, cpu: Input| {
        let mut opr = Operand::None;

        request_read_from_pc!(yielder, cpu);

        let val = match op.address_mode {
            Accumulator => cpu.a(),
            Immediate => {
                cpu.inc_pc();
                let o = cpu.pins.data.read();
                opr = Operand::Byte(o);
                o
            }
            _ => 0,
        };
        let res = execute_operation(&cpu, &op, val);
        if op.address_mode == Accumulator {
            cpu.set_a(res);
        }
        yielder.suspend(());

        StepperResult::new(false, cpu, opr)
    })
}

fn read_stepper(op: OperationDef) -> Stepper {
    Coroutine::new(move |yielder, cpu: Input| {
        let (lo, hi, operand) = decode_address!(yielder, cpu, op);

        request_read_from_addr(&cpu, lo, hi);
        yielder.suspend(());

        let val = cpu.pins.data.read();
        execute_operation(&cpu, &op, val);
        yielder.suspend(());

        StepperResult::new(false, cpu, operand)
    })
}

fn write_stepper(op: OperationDef) -> Stepper {
    Coroutine::new(move |yielder, cpu: Input| {
        let (lo, hi, operand) = decode_address!(yielder, cpu, op);

        request_write_to_addr(&cpu, lo, hi);
        yielder.suspend(());

        let val = execute_operation(&cpu, &op, 0);
        cpu.pins.data.write(val);
        yielder.suspend(());

        StepperResult::new(false, cpu, operand)
    })
}

/// Stepper for read-modify-write (RMW) operations
/// ```text
/// Absolute addressing
///       #  address R/W description
///      --- ------- --- ------------------------------------------
///       1    PC     R  fetch opcode, increment PC
///       2    PC     R  fetch low byte of address, increment PC
///       3    PC     R  fetch high byte of address, increment PC
///       4  address  R  read from effective address
///       5  address  W  write the value back to effective address,
///                      and do the operation on it
///       6  address  W  write the new value to effective address
/// ```
fn rmw_stepper(op: OperationDef) -> Stepper {
    Coroutine::new(move |yielder, cpu: Input| {
        let (lo, hi, operand) = decode_address!(yielder, cpu, op);

        request_read_from_addr(&cpu, lo, hi);
        yielder.suspend(());

        let mut val = cpu.pins.data.read();
        yielder.suspend(());

        request_write_to_addr(&cpu, lo, hi);
        yielder.suspend(());

        cpu.pins.data.write(val);
        val = execute_operation(&cpu, &op, val);
        yielder.suspend(());

        request_write_to_addr(&cpu, lo, hi);
        yielder.suspend(());

        cpu.pins.data.write(val);
        yielder.suspend(());

        StepperResult::new(false, cpu, operand)
    })
}

/// Coroutine for branching operations.
/// Relative addressing (BCC, BCS, BNE, BEQ, BPL, BMI, BVC, BVS)
///
///       #   address  R/W description
///      --- --------- --- ---------------------------------------------
///       1     PC      R  fetch opcode, increment PC
///       2     PC      R  fetch operand, increment PC
///       3     PC      R  Fetch opcode of next instruction,
///                        If branch is taken, add operand to PCL.
///                        Otherwise increment PC.
///       4+    PC*     R  Fetch opcode of next instruction.
///                        Fix PCH. If it did not change, increment PC.
///       5!    PC      R  Fetch opcode of next instruction,
///                        increment PC.
///
/// [Source](https://www.nesdev.org/6502_cpu.txt)
///
fn branch_stepper(op: OperationDef) -> Stepper {
    Coroutine::new(move |yielder, cpu: Input| {
        request_read_from_pc!(yielder, cpu);

        let shift = cpu.pins.data.read();
        let opr = Operand::Byte(shift);
        yielder.suspend(());

        let branch = execute_operation(&cpu, &op, shift) > 0;
        if !branch {
            cpu.inc_pc();
            // shouldn't we yield before?
            return StepperResult::new(false, cpu, opr);
        }

        let [lo, hi] = {
            let o = shift as i8;
            let pc = cpu.pc().wrapping_add(op.operand_len().into());
            (((pc as i64 + o as i64) & 0xffff) as u16).to_le_bytes()
        };
        cpu.set_pcl(lo);
        yielder.suspend(());

        request_opcode(Rc::clone(&cpu));
        yielder.suspend(());

        if cpu.pch() == hi {
            read_opcode_and_inc_pc(Rc::clone(&cpu));
            yielder.suspend(());
            return StepperResult::new(true, cpu, opr);
        } else {
            // fix PC and exit, so the next cycle starts with fetching correct opcode
            cpu.set_pch(hi);
            yielder.suspend(());
        }

        StepperResult::new(false, cpu, opr)
    })
}

//--------------------------------------------------------------------
// Stack steppers

/// Push operations (PHA, PHP) stepper
/// ```text
///         #  address R/W description
///    --- ------- --- -----------------------------------------------
///     1    PC     R  fetch opcode, increment PC
///     2    PC     R  read next instruction byte (and throw it away)
///     3  $0100,S  W  push register on stack, decrement S
/// ```
fn push_stepper(op: OperationDef) -> Stepper {
    Coroutine::new(move |yielder, cpu: Input| {
        let _ = fetch_byte_from_pc!(yielder, cpu);

        let val = execute_operation(&cpu, &op, 0);
        request_write_to_addr(&cpu, cpu.sp(), 0x01);
        yielder.suspend(());

        cpu.pins.data.write(val);
        cpu.dec_sp();
        yielder.suspend(());

        StepperResult::new(false, cpu, Operand::None)
    })
}

/// Pull operations (PLA, PLP) stepper
/// ```text
/// scription
///    --- ------- --- -----------------------------------------------
///     1    PC     R  fetch opcode, increment PC
///     2    PC     R  read next instruction byte (and throw it away)
///     3  $0100,S  R  increment S
///     4  $0100,S  R  pull r
/// ```
fn pull_stepper(op: OperationDef) -> Stepper {
    Coroutine::new(move |yielder, cpu: Input| {
        let _ = fetch_byte_from_pc!(yielder, cpu);

        cpu.inc_sp();
        yielder.suspend(());

        request_read_from_addr(&cpu, cpu.sp(), 0x01);
        yielder.suspend(());

        let val = cpu.pins.data.read();
        execute_operation(&cpu, &op, val);
        yielder.suspend(());

        StepperResult::new(false, cpu, Operand::None)
    })
}

/// JSR stepper
/// ```text
///         #  address R/W description
///    --- ------- --- -------------------------------------------------
///     1    PC     R  fetch opcode, increment PC
///     2    PC     R  fetch low address byte, increment PC
///     3  $0100,S  R  internal operation (predecrement S?)
///     4  $0100,S  W  push PCH on stack, decrement S
///     5  $0100,S  W  push PCL on stack, decrement S
///     6    PC     R  copy low address byte to PCL, fetch high address
///                    byte to PCH
/// ```
fn jsr_stepper(_op: OperationDef) -> Stepper {
    Coroutine::new(move |yielder, cpu: Input| {
        let lo = fetch_byte_and_inc_pc!(yielder, cpu);

        push_to_stack_and_dec_sp!(yielder, cpu, cpu.pch());
        push_to_stack_and_dec_sp!(yielder, cpu, cpu.pcl());

        request_read_from_pc!(yielder, cpu);

        let hi = cpu.pins.data.read();
        cpu.set_pcl(lo);
        cpu.set_pch(hi);
        yielder.suspend(());

        StepperResult::new(false, cpu, Operand::Word(u16::from_le_bytes([lo, hi])))
    })
}

/// Steppers for return-from-subroutine (RTS) and
/// return-from-interrupt (RTI) are implemented together
/// do to similarity.
///
/// RTS stepper
/// ```text
///        #  address R/W description
///    --- ------- --- -----------------------------------------------
///     1    PC     R  fetch opcode, increment PC
///     2    PC     R  read next instruction byte (and throw it away)
///     3  $0100,S  R  increment S
///     4  $0100,S  R  pull PCL from stack, increment S
///     5  $0100,S  R  pull PCH from stack
///     6    PC     R  increment PC
/// ```
///
/// RTI stepper
/// ```text
///         #  address R/W description
///    --- ------- --- -----------------------------------------------
///     1    PC     R  fetch opcode, increment PC
///     2    PC     R  read next instruction byte (and throw it away)
///     3  $0100,S  R  increment S
///     4  $0100,S  R  pull P from stack, increment S
///     5  $0100,S  R  pull PCL from stack, increment S
///     6  $0100,S  R  pull PCH from stack
/// ```
fn rts_rti_stepper(op: OperationDef) -> Stepper {
    Coroutine::new(move |yielder, cpu: Input| {
        let _ = fetch_byte_from_pc!(yielder, cpu);

        cpu.inc_sp();
        yielder.suspend(());

        if op.mnemonic == Mnemonic::RTI {
            request_read_from_addr(&cpu, cpu.sp(), 0x01);
            yielder.suspend(());

            let p = cpu.pins.data.read();
            cpu.set_p(p);
            cpu.inc_sp();
            yielder.suspend(());
        }

        request_read_from_addr(&cpu, cpu.sp(), 0x01);
        yielder.suspend(());

        let lo = cpu.pins.data.read();
        cpu.inc_sp();
        yielder.suspend(());

        request_read_from_addr(&cpu, cpu.sp(), 0x01);
        yielder.suspend(());

        let hi = cpu.pins.data.read();
        yielder.suspend(());

        cpu.set_pcl(lo);
        cpu.set_pch(hi);
        if op.mnemonic == Mnemonic::RTS {
            cpu.inc_pc();
        }
        yielder.suspend(());

        StepperResult::new(false, cpu, Operand::None)
    })
}

//--------------------------------------------------------------------
// Individual mnemonic steppers ("other" steppers)

/// ```text
///   Absolute addressing
///         #  address R/W description
///        --- ------- --- -------------------------------------------------
///         1    PC     R  fetch opcode, increment PC
///         2    PC     R  fetch low address byte, increment PC
///         3    PC     R  copy low address byte to PCL, fetch high address
///                        byte to PCH
///
///   Absolute indirect addressing
///         #   address  R/W description
///        --- --------- --- ------------------------------------------
///         1     PC      R  fetch opcode, increment PC
///         2     PC      R  fetch pointer address low, increment PC
///         3     PC      R  fetch pointer address high, increment PC
///         4   pointer   R  fetch low address to latch
///         5  pointer+1* R  fetch PCH, copy latch to PCL
///
///        Note: * The PCH will always be fetched from the same page
///                than PCL, i.e. page boundary crossing is not handled.
/// ```
///
fn jmp_stepper(op: OperationDef) -> Stepper {
    Coroutine::new(move |yielder, cpu: Input| {
        let indirect = op.address_mode == Indirect;

        request_read_from_pc!(yielder, cpu);
        let lo = read_and_inc_pc!(yielder, cpu);

        request_read_from_pc!(yielder, cpu);

        let hi = cpu.pins.data.read();
        let mut addr = u16::from_le_bytes([lo, hi]);
        if indirect {
            cpu.inc_pc();
        } else {
            cpu.set_pc(addr);
        }
        yielder.suspend(());

        if indirect {
            request_read_from_addr(&cpu, lo, hi);
            yielder.suspend(());

            let final_lo = cpu.pins.data.read();
            yielder.suspend(());

            // page boundary crossing not allowed
            request_read_from_addr(&cpu, lo.wrapping_add(1), hi);
            yielder.suspend(());

            let final_hi = cpu.pins.data.read();
            addr = u16::from_le_bytes([final_lo, final_hi]);
            cpu.set_pc(addr);
            yielder.suspend(());
        }

        StepperResult::new(false, cpu, Operand::Word(addr))
    })
}

//--------------------------------------------------------------------
// Utils

fn request_read_from_pc(cpu_ref: &Input) {
    let cpu = cpu_ref;
    cpu.pins
        .set_data_direction(PinDirection::Input)
        .addr
        .write(cpu.pc());
}

fn request_read_from_addr(cpu: &Input, lo: u8, hi: u8) {
    let addr = u16::from_le_bytes([lo, hi]);
    cpu.pins
        .set_data_direction(PinDirection::Input)
        .addr
        .write(addr);
}

fn read_and_inc_pc(cpu: &Input) -> u8 {
    let val = cpu.pins.data.read();
    cpu.inc_pc();
    val
}

fn request_write_to_addr(cpu: &Input, lo: u8, hi: u8) {
    let addr = u16::from_le_bytes([lo, hi]);
    cpu.pins
        .set_data_direction(PinDirection::Output)
        .addr
        .write(addr);
}

fn request_opcode(cpu: Input) {
    cpu.pins.set_sync(true);
    cpu.pins.set_data_direction(PinDirection::Input);
    cpu.pins.addr.write(cpu.pc());
}

fn read_opcode_and_inc_pc(cpu: Input) -> u8 {
    let opcode = cpu.pins.data.read();
    cpu.inc_pc();
    cpu.set_ir(opcode);
    cpu.pins.set_sync(false);
    opcode
}

fn get_addr_operand(am: &AddressMode, lo: u8, hi: u8) -> Operand {
    match am {
        Absolute | AbsoluteX | AbsoluteY | Indirect => Operand::Word(u16::from_le_bytes([lo, hi])),
        ZeroPage | ZeroPageX | ZeroPageY | Relative | IndirectX | IndirectY => Operand::Byte(lo),
        Immediate | Implicit | Accumulator => Operand::None,
    }
}

//--------------------------------------------------------------------
// Macros

mod macros {

    // 1-step macros (single yield)

    macro_rules! request_read_from_pc {
        ($yielder: ident, $cpu: ident) => {
            request_read_from_pc(&$cpu);
            $yielder.suspend(());
        };
    }

    macro_rules! read_and_inc_pc {
        ($yielder: ident, $cpu: ident) => {{
            let val = read_and_inc_pc(&$cpu);
            $yielder.suspend(());
            val
        }};
    }

    // 2-step macros (double yield)

    macro_rules! fetch_byte_from_pc {
        ($yielder: ident, $cpu: ident) => {{
            request_read_from_pc!($yielder, $cpu);
            let val = $cpu.pins.data.read();
            $yielder.suspend(());
            val
        }};
    }

    macro_rules! fetch_byte_and_inc_pc {
        ($yielder: ident, $cpu: ident) => {{
            request_read_from_pc!($yielder, $cpu);
            read_and_inc_pc!($yielder, $cpu)
        }};
    }

    macro_rules! fetch_byte_from_addr {
        ($yielder: ident, $cpu: ident, $lo: expr, $hi: expr) => {{
            request_read_from_addr(&$cpu, $lo, $hi);
            $yielder.suspend(());
            let val = $cpu.pins.data.read();
            $yielder.suspend(());
            val
        }};
    }

    macro_rules! push_to_stack_and_dec_sp {
        ($yielder: ident, $cpu: ident, $val: expr) => {
            request_write_to_addr(&$cpu, $cpu.sp(), 0x01);
            $yielder.suspend(());

            $cpu.pins.data.write($val);
            $cpu.dec_sp();
            $yielder.suspend(());
        };
    }

    // multi-step macros
    macro_rules! decode_address {
        ($yielder: ident, $cpu: ident, $op: ident) => {{
            let lo = fetch_byte_and_inc_pc!($yielder, $cpu);
            let mut hi = 0u8;

            let (new_lo, new_hi) = match $op.address_mode {
                ZeroPage => (lo, 0),
                ZeroPageX => (lo.wrapping_add($cpu.x()), 0),
                ZeroPageY => (lo.wrapping_add($cpu.y()), 0),
                Absolute => {
                    hi = fetch_byte_and_inc_pc!($yielder, $cpu);
                    (lo, hi)
                }
                AbsoluteX | AbsoluteY => {
                    hi = fetch_byte_and_inc_pc!($yielder, $cpu);
                    let addr = u16::from_le_bytes([lo, hi]).wrapping_add(
                        if $op.address_mode == AbsoluteX {
                            $cpu.x().into()
                        } else {
                            $cpu.y().into()
                        },
                    );
                    let res = addr.to_le_bytes();
                    if res[1] != hi {
                        // additional read in case of crossing page boundary
                        // to follow real processor behaviour
                        fetch_byte_from_addr!($yielder, $cpu, res[0], hi);
                    }
                    res.into()
                }
                IndirectX => {
                    let pointer = lo.wrapping_add($cpu.x());

                    let addr_lo = fetch_byte_from_addr!($yielder, $cpu, pointer, 0);
                    let addr_hi = fetch_byte_from_addr!($yielder, $cpu, pointer.wrapping_add(1), 0);
                    (addr_lo, addr_hi)
                }
                IndirectY => {
                    let addr_lo = fetch_byte_from_addr!($yielder, $cpu, lo, 0);
                    let addr_hi = fetch_byte_from_addr!($yielder, $cpu, lo.wrapping_add(1), 0);
                    let res = u16::from_le_bytes([addr_lo, addr_hi])
                        .wrapping_add($cpu.y().into())
                        .to_le_bytes();
                    if res[1] != addr_hi {
                        // additional read in case of crossing page boundary
                        fetch_byte_from_addr!($yielder, $cpu, res[0], addr_hi);
                    }
                    res.into()
                }
                _ => panic!(
                    "Can't decode address for {:?} address mode",
                    $op.address_mode
                ),
            };

            (new_lo, new_hi, get_addr_operand(&$op.address_mode, lo, hi))
        }};
    }

    pub(super) use decode_address;
    pub(super) use fetch_byte_and_inc_pc;
    pub(super) use fetch_byte_from_addr;
    pub(super) use fetch_byte_from_pc;
    pub(super) use push_to_stack_and_dec_sp;
    pub(super) use read_and_inc_pc;
    pub(super) use request_read_from_pc;
}
