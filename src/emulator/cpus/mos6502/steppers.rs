use std::cell::RefCell;
use std::rc::Rc;

use crate::emulator::abstractions::PinDirection;
use crate::emulator::cpus::mos6502::{AddressMode::*, OperationDef};
use crate::emulator::cpus::CpuState;
use corosensei::Coroutine;

use super::{execute_operation, Operand};

pub type Input = CpuState;
pub type Stepper = Coroutine<Input, (), StepperResult>;

pub fn get_stepper(op: &OperationDef) -> Option<Stepper> {
    use crate::emulator::cpus::mos6502::mnemonic::Mnemonic::*;

    let s = match op.address_mode {
        Implicit | Accumulator | Immediate => no_mem_stepper(op.clone()),
        Relative => branch_stepper(op.clone()),
        _ => match op.mnemonic {
            LDA | LDX | LDY | EOR | AND | ORA | ADC | SBC | CMP | CPX | CPY | BIT => {
                read_stepper(op.clone())
            }
            STA | STX | STY => write_stepper(op.clone()),
            _ => return None,
        },
    };

    Some(s)
}

pub fn read_opcode() -> Stepper {
    Coroutine::new(move |yielder, cpu: Input| {
        {
            request_opcode(&cpu);
            yielder.suspend(());
        }

        {
            read_opcode_and_inc_pc(&cpu);
            yielder.suspend(());
        }

        StepperResult::partial(true, cpu.clone())
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
///
///       #  address R/W description
///      --- ------- --- ------------------------------------------
///       1    PC     R  fetch opcode, increment PC
///       2    PC     R  fetch value, increment PC
/// ```
fn no_mem_stepper(op: OperationDef) -> Stepper {
    Coroutine::new(move |yielder, cpu: Input| {
        let mut opr = Operand::None;

        request_read_from_pc(&cpu);
        yielder.suspend(());

        let val = match op.address_mode {
            Accumulator => cpu.a(),
            Immediate => {
                cpu.inc_pc();
                let o = cpu.pins.data.read();
                opr = Operand::Byte(o);
                o
            },
            _ => 0
        };
        execute_operation(&cpu, &op, val);
        yielder.suspend(());

        StepperResult::new(false, cpu.clone(), opr)
    })
}

fn read_stepper(op: OperationDef) -> Stepper {
    Coroutine::new(move |yielder, cpu: Input| {
        let opr: Operand;

        request_read_from_pc(&cpu);
        yielder.suspend(());

        let lo = read_and_inc_pc(&cpu);
        yielder.suspend(());

        let hi = if op.address_mode == Absolute {
            request_read_from_pc(&cpu);
            yielder.suspend(());

            let hi = read_and_inc_pc(&cpu);
            yielder.suspend(());

            opr = Operand::Word(u16::from_le_bytes([lo, hi]));
            hi
        } else {
            opr = Operand::Byte(lo);
            0
        };

        request_read_from_addr(&cpu, lo, hi);
        yielder.suspend(());

        let val = cpu.pins.data.read();
        execute_operation(&cpu, &op, val);
        yielder.suspend(());

        StepperResult::new(false, cpu.clone(), opr)
    })
}

fn write_stepper(op: OperationDef) -> Stepper {
    Coroutine::new(move |yielder, cpu: Input| {
        let opr: Operand;

        request_read_from_pc(&cpu);
        yielder.suspend(());

        let lo = read_and_inc_pc(&cpu);
        yielder.suspend(());

        let hi = if op.address_mode == Absolute {
            request_read_from_pc(&cpu);
            yielder.suspend(());

            let hi = read_and_inc_pc(&cpu);
            yielder.suspend(());

            opr = Operand::Word(u16::from_le_bytes([lo, hi]));
            hi
        } else {
            opr = Operand::Byte(lo);
            0
        };

        request_write_to_addr(&cpu, lo, hi);
        yielder.suspend(());

        let val = execute_operation(&cpu, &op, 0);
        cpu.pins.data.write(val);
        yielder.suspend(());

        StepperResult::new(false, cpu.clone(), opr)
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
        request_read_from_pc(&cpu);
        yielder.suspend(());

        let shift = cpu.pins.data.read();
        let opr = Operand::Byte(shift);
        yielder.suspend(());

        let branch = execute_operation(&cpu, &op, shift) > 0;
        if !branch {
            cpu.inc_pc();
            return StepperResult::new(false, cpu.clone(), opr);
        }
        let [lo, hi] = {
            let o = shift as i8;
            (((cpu.pc() as i64 + o as i64) & 0xffff) as u16).to_le_bytes()
        };
        cpu.set_pcl(lo);
        yielder.suspend(());

        // FIXME the operation below sets SYNC pin to hi. TBC whether it should happen
        request_opcode(&cpu);
        yielder.suspend(());

        if cpu.pch() == hi {
            read_opcode_and_inc_pc(&cpu);
            yielder.suspend(());
            return StepperResult::new(true, cpu.clone(), opr);
        } else {
            // fix PC and exit, so the next cycle starts with fetching correct opcode
            cpu.set_pch(hi);
        }

        StepperResult::new(false, cpu.clone(), opr)
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

fn request_opcode(cpu: &Input) {
    let cpu = cpu;
    cpu.pins
        .set_sync(true)
        .set_data_direction(PinDirection::Input)
        .addr
        .write(cpu.pc());
}

fn read_opcode_and_inc_pc(cpu: &Input) -> u8 {
    let mut cpu = cpu;
    let opcode = cpu.pins.data.read();
    cpu.inc_pc();
    cpu.set_ir(opcode);
    cpu.pins.set_sync(false);
    opcode
}

// fn read_stepper(op: OperationDef) -> Stepper {
//     Coroutine::new(move |yielder, cpu: Input| {
//         let lo = read_and_inc_pc(&cpu);
//         yielder.suspend(());
//
//         let hi = if op.address_mode == Absolute {
//             let hi = read_and_inc_pc(&cpu);
//             yielder.suspend(());
//             hi
//         } else {
//             0
//         };
//
//         let (val, _) = read_from_addr(&cpu, lo, hi);
//         // cpu.execute(val);
//         execute_operation(&mut cpu, &op, val);
//         yielder.suspend(());
//
//         false
//     })
// }

// // ----------------------------------------------------------------------
// // Absolute addressing
//
// pub fn abs_read(cpu: &mut impl CPU) -> OpGen {
//     Box::new(Gen::new(|co| async move {
//         let lo = read_and_inc_pc(cpu);
//         co.yield_(()).await;
//
//         let hi = read_and_inc_pc(cpu);
//         co.yield_(()).await;
//
//         let (val, _) = read_from_addr(cpu, lo, hi);
//         cpu.execute(val);
//         co.yield_(()).await;
//     }))
// }
//
// pub fn abs_write(cpu: &mut impl CPU) -> OpGen {
//     Box::new(Gen::new(|co| async move {
//         let lo = read_and_inc_pc(cpu);
//         co.yield_(()).await;
//
//         let hi = read_and_inc_pc(cpu);
//         co.yield_(()).await;
//
//         let val = cpu.execute(0);
//         cpu.write_byte(addr(lo, hi), val);
//         co.yield_(()).await;
//     }))
// }
//
// pub fn abs_rmw(cpu: &mut impl CPU) -> OpGen {
//     Box::new(Gen::new(|co| async move {
//         let lo = read_and_inc_pc(cpu);
//         co.yield_(()).await;
//
//         let hi = read_and_inc_pc(cpu);
//         co.yield_(()).await;
//
//         let (val, addr) = read_from_addr(cpu, lo, hi);
//         co.yield_(()).await;
//
//         let new_val = write_and_exec(cpu, addr, val);
//         co.yield_(()).await;
//
//         cpu.write_byte(addr, new_val);
//         co.yield_(()).await;
//     }))
// }
//
// // ----------------------------------------------------------------------
// // Zer-page addressing
//
// pub fn zp_read(cpu: &mut impl CPU) -> OpGen {
//     Box::new(Gen::new(|co| async move {
//         let lo = read_and_inc_pc(cpu);
//         co.yield_(()).await;
//
//         let (val, _) = read_from_addr(cpu, lo, 0);
//         cpu.execute(val);
//         co.yield_(()).await;
//     }))
// }
//
// pub fn zp_write(cpu: &mut impl CPU) -> OpGen {
//     Box::new(Gen::new(|co| async move {
//         let lo = read_and_inc_pc(cpu);
//         co.yield_(()).await;
//
//         let val = cpu.execute(0);
//         cpu.write_byte(addr(lo, 0), val);
//         co.yield_(()).await;
//     }))
// }
//
// pub fn zp_rmw(cpu: &mut impl CPU) -> OpGen {
//     Box::new(Gen::new(|co| async move {
//         let lo = read_and_inc_pc(cpu);
//         co.yield_(()).await;
//
//         let (val, addr) = read_from_addr(cpu, lo, 0);
//         co.yield_(()).await;
//
//         let new_val = write_and_exec(cpu, addr, val);
//         co.yield_(()).await;
//
//         cpu.write_byte(addr, new_val);
//         co.yield_(()).await;
//     }))
// }

// ----------------------------------------------------------------------
// utils

#[cfg(test)]
mod tests {
    use super::*;

    // #[test]
    // fn test_stepper() {
    //     let mut stepper = nop();
    //     match stepper.resume(()) {
    //         CoroutineResult::Yield(()) => {},
    //         CoroutineResult::Return(_) => {},
    //     };
    // }
}
