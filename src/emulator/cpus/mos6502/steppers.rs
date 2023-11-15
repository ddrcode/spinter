use std::cell::RefCell;
use std::rc::Rc;

use crate::emulator::cpus::mos6502::AddressMode::*;
use crate::emulator::cpus::CpuState;
use corosensei::{Coroutine, CoroutineResult};

use crate::emulator::abstractions::{Addr, PinDirection, CPU};
use crate::emulator::cpus::mos6502::{OperationDef, OPERATIONS};

use super::execute_operation;

pub type Input = Rc<RefCell<CpuState>>;
pub type Stepper = Coroutine<Input, (), bool>;

pub fn get_stepper(op: &OperationDef) -> Option<Stepper> {
    use crate::emulator::cpus::mos6502::mnemonic::Mnemonic::*;
    match op.mnemonic {
        NOP => Some(nop()),
        LDA | LDX | LDY | EOR | AND | ORA | ADC | SBC | CMP | BIT => Some(read_stepper(op.clone())),
        _ => None,
    }
}

pub fn nop() -> Stepper {
    Coroutine::new(|yielder, _input: Input| {
        yielder.suspend(());
        yielder.suspend(());
        false
    })
}

pub fn read_opcode() -> Stepper {
    Coroutine::new(move |yielder, cpu: Input| {
        {
            let cpu = cpu.borrow();
            cpu.pins
                .set_sync(true)
                .set_data_direction(PinDirection::Input)
                .addr
                .write(cpu.pc());
            println!("REQUESTING OPCODE FROM {:#04x}", cpu.pc());
            drop(cpu);
            yielder.suspend(());
        }

        {
            let mut cpu = cpu.borrow_mut();
            let data = cpu.pins.data.read();
            cpu.inc_pc();
            cpu.set_ir(data);
            drop(cpu);
            yielder.suspend(());
            println!("OPCODE RECEIVED {:#02x}", data);
        }

        false
    })
}

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

fn read_stepper(op: OperationDef) -> Stepper {
    Coroutine::new(move |yielder, cpu: Input| {
        println!("REQUESTING OPERAND, addr mode: {:?}", op.address_mode);
        request_read_from_pc(&cpu);
        yielder.suspend(());

        println!("READING OPERAND");
        let lo = read_and_inc_pc(&cpu);
        yielder.suspend(());

        let hi = if op.address_mode == Absolute {
            println!("REQUESTING OPERAND 2");
            request_read_from_pc(&cpu);
            yielder.suspend(());

            println!("READING OPERAND 2");
            let hi = read_and_inc_pc(&cpu);
            yielder.suspend(());
            hi
        } else {
            0
        };

        println!("REQUESTING DATA");
        request_read_from_addr(&cpu, lo, hi);
        yielder.suspend(());

        println!("READING DATA");
        let val = cpu.borrow().pins.data.read();
        execute_operation(&mut cpu.borrow_mut(), &op, val);
        yielder.suspend(());

        println!("KONIEC");

        false
    })
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
