use corosensei::CoroutineResult;
use std::cell::RefCell;
use std::rc::Rc;

use crate::debugger::OperationDebug;
use crate::emulator::abstractions::{CPUCycles, Component, Pin, PinStateChange, Pins};
use crate::emulator::cpus::mos6502::{
    get_stepper, init_stepper, mnemonic_from_opcode, read_opcode, Operand, OperationDef, Stepper,
    OPERATIONS,
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
        W65C02 {
            pins,
            logic,
        }
    }
}

impl Component for W65C02 {
    fn get_pin(&self, name: &str) -> Option<&Pin> {
        self.pins.by_name(name)
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
    state: RefCell<CpuState>,
}

impl W65C02Logic {
    pub fn new(pins: Rc<W65C02_Pins>) -> Self {
        let logic = W65C02Logic {
            state: RefCell::new(CpuState::new(pins)),
            stepper: RefCell::new(init_stepper()),
            cycles: RefCell::new(0),
        };

        logic
    }

    pub fn tick(&self, phase: bool) {
        let v = self.stepper.borrow_mut().resume(self.state.borrow().clone());
        match v {
            CoroutineResult::Yield(()) => {}
            CoroutineResult::Return(res) => {
                *self.state.borrow_mut() = res.cpu.into();
                if res.completed {
                    self.debug(&res.operand);
                }
                *self.stepper.borrow_mut() = if res.has_opcode {
                    let op = self.decode_op(&self.state.borrow().ir());
                    let s = get_stepper(&op);
                    if s.is_none() {
                        panic!(
                            "There is no stepper for current IR: {:#02x} (mnemonic: {:?})",
                            self.state.borrow().ir(),
                            mnemonic_from_opcode(self.state.borrow().ir())
                        );
                    }
                    s.unwrap().into()
                } else {
                    if phase != false {
                        panic!("New instruction must start with phase high");
                    }
                    read_opcode().into()
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
                self.state.borrow().pc()
            ),
        }
    }

    fn advance_cycles(&self) {
        let val = self.cycles.borrow().wrapping_add(1);
        *self.cycles.borrow_mut() = val;
    }

    fn debug(&self, operand: &Operand) {
        let cpu = &self.state;
        let s = OperationDebug {
            reg: cpu.borrow().regs().clone(),
            opcode: cpu.borrow().ir(),
            operand: operand.clone(),
            cycle: *self.cycles.borrow(),
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
