use corosensei::CoroutineResult;
use std::cell::RefCell;
use std::rc::Rc;

use crate::debugger::{DebugMessage, Debugger, NullDebugger, Operation, PinsState};
use crate::emulator::abstractions::{
    CPUCycles, Component, ComponentLogic, Pin, PinStateChange, Pins,
};
use crate::emulator::cpus::mos6502::{
    compensate, get_stepper, init_stepper, mnemonic_from_opcode, read_opcode, Operand,
    OperationDef, Stepper, OPERATIONS,
};

use super::{CpuState, W65C02_Pins};
// use genawaiter::{rc::gen, rc::Gen, yield_};

pub struct W65C02 {
    pub pins: Rc<W65C02_Pins>,
    logic: W65C02Logic,
}

impl W65C02 {
    pub fn new() -> Self {
        let pins = Rc::new(W65C02_Pins::new());
        let logic = W65C02Logic::new(Rc::clone(&pins));
        W65C02 { pins, logic }
    }
}

impl Component for W65C02 {
    fn get_pin(&self, name: &str) -> Option<&Pin> {
        self.pins.by_name(name)
    }

    fn set_debugger(&mut self, debugger: Rc<dyn Debugger>) {
        self.logic.debugger = debugger;
    }
}

impl PinStateChange for W65C02 {
    fn on_state_change(&self, pin: &Pin) {
        let val = pin.state();
        match pin.name().as_str() {
            "PHI2" => {
                self.pins["PHI1O"].write(!val).unwrap();
                self.pins["PHI2O"].write(val).unwrap();
                self.logic.tick(val);
                if val {
                    self.logic.advance_cycles();
                }
            }
            _ => {}
        };
    }
}

//--------------------------------------------------------------------
// W65C02Logic

pub struct W65C02Logic {
    stepper: RefCell<Stepper>,
    cycles: RefCell<CPUCycles>,
    state: Rc<CpuState>,
    debugger: Rc<dyn Debugger>,
}

impl W65C02Logic {
    pub fn new(pins: Rc<W65C02_Pins>) -> Self {
        let logic = W65C02Logic {
            state: Rc::new(CpuState::new(pins)),
            stepper: RefCell::new(init_stepper()),
            cycles: RefCell::new(0),
            debugger: Rc::new(NullDebugger),
        };

        logic
    }

    pub fn tick(&self, phase: bool) {
        let v = self.stepper.borrow_mut().resume(Rc::clone(&self.state));
        match v {
            CoroutineResult::Yield(()) => {
                if phase == false {
                    self.debug_pins();
                }
            }
            CoroutineResult::Return(res) => {
                if phase == false {
                    self.debug_pins();
                }
                if res.completed {
                    self.debug_operation(&res.operand);
                }
                *self.stepper.borrow_mut() = if res.has_opcode {
                    let op = self.decode_op(&self.state.ir());
                    let s = get_stepper(&op);
                    if s.is_none() {
                        panic!(
                            "There is no stepper for current IR: {:#02x} (mnemonic: {:?})",
                            self.state.ir(),
                            mnemonic_from_opcode(self.state.ir())
                        );
                    }
                    s.unwrap().into()
                } else {
                    if phase != false {
                        println!("Compensating");
                        compensate().into()
                        // panic!("New instruction must start with phase high");
                    } else {
                        read_opcode().into()
                    }
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

    fn advance_cycles(&self) {
        let val = self.cycles.borrow().wrapping_add(1);
        *self.cycles.borrow_mut() = val;
    }

    fn debug_operation(&self, operand: &Operand) {
        if self.debugger.enabled() {
            let cpu = &self.state;
            let operation = Operation {
                reg: cpu.regs().clone(),
                opcode: cpu.ir(),
                operand: operand.clone(),
                cycle: *self.cycles.borrow(),
            };
            self.debugger.debug(DebugMessage::CpuOperation(operation));
        }
    }

    fn debug_pins(&self) {
        if self.debugger.enabled() {
            self.debugger.debug(DebugMessage::PinsState(PinsState {
                pins: self.state.pins.into_u128(),
                width: 40,
                cycle: *self.cycles.borrow(),
            }));
        }
    }
}

impl ComponentLogic for W65C02Logic {
    fn debugger(&self) -> &dyn Debugger {
        self.debugger.as_ref()
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
