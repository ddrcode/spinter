use std::cell::RefCell;
use std::rc::Rc;

use crate::debugger::OperationDebug;
use crate::emulator::cpus::mos6502::AddressMode::*;
use crate::emulator::cpus::CpuState;
use corosensei::{Coroutine, CoroutineResult};

use crate::emulator::abstractions::{Addr, PinDirection, CPU};
use crate::emulator::cpus::mos6502::{OperationDef, OPERATIONS};

use super::{execute_operation, Operand};

pub type Input = Rc<RefCell<CpuState>>;
pub type Stepper = Coroutine<Input, (), bool>;

pub fn get_stepper(op: &OperationDef) -> Option<Stepper> {
    use crate::emulator::cpus::mos6502::mnemonic::Mnemonic::*;
    match op.mnemonic {
        LDA | LDX | LDY | EOR | AND | ORA | ADC | SBC | CMP | CPX | CPY | BIT => {
            Some(read_stepper(op.clone()))
        }
        STA | STX | STY => Some(write_stepper(op.clone())),
        BCC | BCS | BNE | BEQ | BPL | BMI | BVC | BVS => Some(branch_stepper(op.clone())),
        _ => None,
    }
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

        true
    })
}

fn read_stepper(op: OperationDef) -> Stepper {
    Coroutine::new(move |yielder, cpu: Input| {
        let opr: Option<Operand>;

        request_read_from_pc(&cpu);
        yielder.suspend(());

        let lo = read_and_inc_pc(&cpu);
        yielder.suspend(());

        let hi = if op.address_mode == Absolute {
            request_read_from_pc(&cpu);
            yielder.suspend(());

            let hi = read_and_inc_pc(&cpu);
            yielder.suspend(());

            opr = Some(Operand::Word(u16::from_le_bytes([lo, hi])));
            hi
        } else {
            opr = Some(Operand::Byte(lo));
            0
        };

        request_read_from_addr(&cpu, lo, hi);
        yielder.suspend(());

        let val = cpu.borrow().pins.data.read();
        execute_operation(&mut cpu.borrow_mut(), &op, val);
        yielder.suspend(());

        debug(&cpu, opr);
        false
    })
}

fn write_stepper(op: OperationDef) -> Stepper {
    Coroutine::new(move |yielder, cpu: Input| {
        let opr: Option<Operand>;

        request_read_from_pc(&cpu);
        yielder.suspend(());

        let lo = read_and_inc_pc(&cpu);
        yielder.suspend(());

        let hi = if op.address_mode == Absolute {
            request_read_from_pc(&cpu);
            yielder.suspend(());

            let hi = read_and_inc_pc(&cpu);
            yielder.suspend(());

            opr = Some(Operand::Word(u16::from_le_bytes([lo, hi])));
            hi
        } else {
            opr = Some(Operand::Byte(lo));
            0
        };

        request_write_to_addr(&cpu, lo, hi);
        yielder.suspend(());

        let val = execute_operation(&mut cpu.borrow_mut(), &op, 0);
        cpu.borrow().pins.data.write(val);
        yielder.suspend(());

        debug(&cpu, opr);
        false
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

        let shift = cpu.borrow().pins.data.read();
        yielder.suspend(());

        let branch = execute_operation(&mut cpu.borrow_mut(), &op, shift) > 0;
        if !branch {
            cpu.borrow_mut().inc_pc();
            return false;
        }
        let [lo, hi] = {
            let o = shift as i8;
            (((cpu.borrow().pc() as i64 + o as i64) & 0xffff) as u16).to_le_bytes()
        };
        cpu.borrow_mut().set_pcl(lo);
        yielder.suspend(());

        // FIXME the operation below sets SYNC pin to hi. TBC whether it should happen
        request_opcode(&cpu);
        yielder.suspend(());

        if cpu.borrow().pch() == hi {
            read_opcode_and_inc_pc(&cpu);
            yielder.suspend(());
            return true;
        } else {
            // fix PC and exit, so the next cycle starts with fetching correct opcode
            cpu.borrow_mut().set_pch(hi);
        }

        false
    })
}

//--------------------------------------------------------------------
// Utils

fn request_read_from_pc(cpu_ref: &Input) {
    let cpu = cpu_ref.borrow();
    cpu.pins
        .set_data_direction(PinDirection::Input)
        .addr
        .write(cpu.pc());
}

fn request_read_from_addr(cpu: &Input, lo: u8, hi: u8) {
    let addr = u16::from_le_bytes([lo, hi]);
    cpu.borrow()
        .pins
        .set_data_direction(PinDirection::Input)
        .addr
        .write(addr);
}

fn read_and_inc_pc(cpu: &Input) -> u8 {
    let val = cpu.borrow().pins.data.read();
    cpu.borrow_mut().inc_pc();
    val
}

fn request_write_to_addr(cpu: &Input, lo: u8, hi: u8) {
    let addr = u16::from_le_bytes([lo, hi]);
    cpu.borrow()
        .pins
        .set_data_direction(PinDirection::Output)
        .addr
        .write(addr);
}

fn request_opcode(cpu: &Input) {
    let cpu = cpu.borrow();
    cpu.pins
        .set_sync(true)
        .set_data_direction(PinDirection::Input)
        .addr
        .write(cpu.pc());
}

fn read_opcode_and_inc_pc(cpu: &Input) -> u8 {
    let mut cpu = cpu.borrow_mut();
    let opcode = cpu.pins.data.read();
    cpu.inc_pc();
    cpu.set_ir(opcode);
    cpu.pins.set_sync(false);
    opcode
}

fn debug(cpu: &Input, operand: Option<Operand>) {
    let s = OperationDebug {
        reg: cpu.borrow().reg.clone(),
        opcode: cpu.borrow().ir(),
        operand,
        cycle: 0,
    };
    println!("{}", s);
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
//         // cpu.borrow_mut().execute(val);
//         execute_operation(&mut cpu.borrow_mut(), &op, val);
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
