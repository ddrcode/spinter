use corosensei::CoroutineResult;
use std::{cell::RefCell, rc::Rc};

use crate::debugger::OperationDebug;
use crate::emulator::abstractions::{CPUCycles, CircuitCtx, Component, Pin, Pins};
use crate::emulator::cpus::mos6502::{
    get_stepper, read_opcode, Operand, OperationDef, Stepper, OPERATIONS, mnemonic_from_opcode,
};

use super::{CpuState, W65C02_Pins};
// use genawaiter::{rc::gen, rc::Gen, yield_};

pub struct W65C02 {
    pub pins: Rc<W65C02_Pins>,
    logic: W65C02Logic,
    ctx: CircuitCtx,
}

impl W65C02 {
    pub fn new() -> Self {
        let pins = Rc::new(W65C02_Pins::new());
        let logic = W65C02Logic::new(Rc::clone(&pins));
        W65C02 {
            pins,
            logic,
            ctx: Default::default(),
        }
    }
}

impl Component for W65C02 {
    fn get_pin(&self, name: &str) -> Option<&Pin> {
        self.pins.by_name(name)
    }

    fn ctx(&self) -> &CircuitCtx {
        &self.ctx
    }

    fn attach(&mut self, ctx: CircuitCtx) {
        self.ctx = ctx;
    }

    fn on_pin_state_change(&mut self, pin_name: &str, val: bool) {
        match pin_name {
            "PHI2" => {
                self.pins["PHI1O"].write(!val).unwrap();
                self.pins["PHI2O"].write(val).unwrap();
                self.logic.tick();
                if val {
                    self.logic.advance_cycles();
                }
            }
            _ => {}
        };
    }

    fn init(&mut self) {
        self.logic.state.set_pc(0x200);
    }
}

//--------------------------------------------------------------------
// W65C02Logic

pub struct W65C02Logic {
    stepper: Stepper,
    cycles: CPUCycles,
    state: CpuState,
}

impl W65C02Logic {
    pub fn new(pins: Rc<W65C02_Pins>) -> Self {
        let logic = W65C02Logic {
            state: CpuState::new(pins),
            stepper: read_opcode(),
            cycles: 0,
        };

        logic
    }

    pub fn tick(&mut self) {
        let v = self.stepper.resume(self.state.clone());
        match v {
            CoroutineResult::Yield(()) => {}
            CoroutineResult::Return(res) => {
                self.state = res.cpu;
                if res.completed {
                    self.debug(&res.operand);
                }
                self.stepper = if res.has_opcode {
                    let op = self.decode_op(&self.state.ir());
                    let s = get_stepper(&op);
                    if s.is_none() {
                        panic!(
                            "There is no stepper for current IR: {:#02x} (mnemonic: {:?})",
                            self.state.ir(),
                            mnemonic_from_opcode(self.state.ir())
                        );
                    }
                    s.unwrap()
                } else {
                    read_opcode()
                }
            }
        }
    }

    fn decode_op(&self, opcode: &u8) -> OperationDef {
        match OPERATIONS.get(&opcode) {
            Some(op) => op.clone(),
            None => panic!(
                "Opcode {:#04x} not found at address {:#06x}",
                opcode,
                self.state.pc()
            ),
        }
    }

    fn advance_cycles(&mut self) {
        self.cycles = self.cycles.wrapping_add(1);
    }

    fn debug(&self, operand: &Operand) {
        let cpu = &self.state;
        let s = OperationDebug {
            reg: cpu.regs().clone(),
            opcode: cpu.ir(),
            operand: operand.clone(),
            cycle: self.cycles,
        };
        println!("{}", s);
    }
}

unsafe impl Send for W65C02 {}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::emulator::cpus::mos6502::Stepper;
//
//     fn create_stepper() -> Stepper {
//         corosensei::Coroutine::new(|yielder, _input| {
//             for _ in 0..3 {
//                 yielder.suspend(());
//             }
//             StepperResult::new()
//         })
//     }
//
//     #[test]
//     fn test_steps() {
//         let mut cpu = W65C02::new();
//         cpu.logic.stepper = Some(create_stepper());
//
//         assert_eq!(0, cpu.logic.cycles);
//         cpu.logic.tick();
//         assert_eq!(1, cpu.logic.cycles);
//         cpu.logic.tick();
//         assert_eq!(2, cpu.logic.cycles);
//         cpu.logic.tick();
//         assert_eq!(3, cpu.logic.cycles);
//     }
//
//     #[test]
//     fn test_with_real_stepper() {
//         let mut cpu = W65C02::new();
//         let opdef = OPERATIONS.get(&0xad).unwrap(); // LDA, absolute
//         cpu.logic.stepper = get_stepper(opdef);
//
//         assert_eq!(0, cpu.logic.cycles);
//         cpu.logic.tick();
//         assert_eq!(1, cpu.logic.cycles);
//         cpu.logic.tick();
//         assert_eq!(2, cpu.logic.cycles);
//         cpu.logic.tick();
//         assert_eq!(3, cpu.logic.cycles);
//     }
// }
