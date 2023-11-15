use corosensei::CoroutineResult;
use std::{cell::RefCell, rc::Rc};

use crate::emulator::abstractions::{Addr, CPUCycles, CircuitCtx, Component, Pin, Pins, CPU};
use crate::emulator::cpus::mos6502::{get_stepper, read_opcode, OperationDef, Stepper, OPERATIONS};

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
            }
            _ => {}
        };
    }

    fn init(&mut self) {
        self.logic.state.borrow_mut().reg.pc = 0x200;
    }
}

pub struct W65C02Logic {
    stepper: Option<Stepper>,
    cycles: CPUCycles,
    state: Rc<RefCell<CpuState>>,
}

impl W65C02Logic {
    pub fn new(pins: Rc<W65C02_Pins>) -> Self {
        let logic = W65C02Logic {
            state: Rc::new(RefCell::new(CpuState {
                reg: Default::default(),
                pins,
            })),
            stepper: Some(read_opcode()),
            cycles: 0,
        };

        logic
    }

    pub fn tick(&mut self) {
        if self.stepper.is_none() {
            panic!("There is no stepper for current IR: {:02x}", self.state.borrow().ir());
        }
        let v = self
            .stepper
            .as_mut()
            .unwrap()
            .resume(Rc::clone(&self.state));
        match v {
            CoroutineResult::Yield(()) => {}
            CoroutineResult::Return(res) => {
                self.stepper = if res {
                    let op = self.decode_op(&self.state.borrow().ir());
                    get_stepper(&op)
                } else {
                    Some(read_opcode())
                }
            }
        }
        self.advance_cycles();
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

    fn advance_cycles(&mut self) {
        self.cycles = self.cycles.wrapping_add(1);
    }
}

unsafe impl Send for W65C02 {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emulator::cpus::mos6502::Stepper;

    fn create_stepper() -> Stepper {
        corosensei::Coroutine::new(|yielder, _input| {
            for _ in 0..3 {
                yielder.suspend(());
            }
            false
        })
    }

    #[test]
    fn test_steps() {
        let mut cpu = W65C02::new();
        cpu.logic.stepper = Some(create_stepper());

        assert_eq!(0, cpu.logic.cycles);
        cpu.logic.tick();
        assert_eq!(1, cpu.logic.cycles);
        cpu.logic.tick();
        assert_eq!(2, cpu.logic.cycles);
        cpu.logic.tick();
        assert_eq!(3, cpu.logic.cycles);
    }

    // #[test]
    // fn test_steps_with_clock_signal() {
    //     let clock = Pin::output();
    //     let cpu = W65C02::new();
    //     (*cpu.logic.borrow_mut()).stepper = Some(create_stepper());
    //     Pin::link(&clock, &cpu.pins.by_name("PHI2").unwrap()).unwrap();
    //
    //     assert_eq!(0, cpu.logic.borrow().cycles());
    //     clock.toggle();
    //     assert_eq!(1, cpu.logic.borrow().cycles());
    //     clock.toggle();
    //     assert_eq!(2, cpu.logic.borrow().cycles());
    //     clock.toggle();
    //     assert_eq!(3, cpu.logic.borrow().cycles());
    // }

    #[test]
    fn test_with_real_stepper() {
        let mut cpu = W65C02::new();
        let opdef = OPERATIONS.get(&0xad).unwrap(); // LDA, absolute
        cpu.logic.stepper = get_stepper(opdef);

        assert_eq!(0, cpu.logic.cycles);
        cpu.logic.tick();
        assert_eq!(1, cpu.logic.cycles);
        cpu.logic.tick();
        assert_eq!(2, cpu.logic.cycles);
        cpu.logic.tick();
        assert_eq!(3, cpu.logic.cycles);
    }
}
