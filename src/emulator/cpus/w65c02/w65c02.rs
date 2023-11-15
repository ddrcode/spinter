use corosensei::CoroutineResult;
use std::{cell::RefCell, rc::Rc};

use crate::emulator::abstractions::{Addr, CPUCycles, CircuitCtx, Component, Pin, Pins, CPU};
use crate::emulator::cpus::mos6502::{get_stepper, read_opcode, OperationDef, Stepper, OPERATIONS};
use crate::utils::bool_to_bit;

use super::W65C02_Pins;
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

#[derive(Debug, Default)]
pub struct Registers {
    /// Stores currently processed instruction. Can't be set by any operation.
    pub ir: u8,

    // actual registers
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub pc: u16,
    pub sp: u8, // stack pointer
    pub s: u8,
}

pub struct CpuState {
    pub reg: Registers,
    pub pins: Rc<W65C02_Pins>,
}

impl CpuState {
    pub fn inc_pc(&mut self) {
        self.reg.pc = self.reg.pc.wrapping_add(1);
    }

    pub fn execute(&mut self, _val: u8) -> u8 {
        0
    }

    pub fn a(&self) -> u8 {
        self.reg.a
    }

    pub fn pc(&self) -> u16 {
        self.reg.pc
    }

    pub fn ir(&self) -> u8 {
        self.reg.ir
    }

    pub fn carry(&self) -> bool {
        (self.reg.s & 1) > 0
    }

    pub fn set_a(&mut self, val: u8) {
        self.reg.a = val;
    }

    pub fn set_x(&mut self, val: u8) {
        self.reg.x = val;
    }

    pub fn set_y(&mut self, val: u8) {
        self.reg.y = val;
    }

    pub fn set_ir(&mut self, val: u8) {
        self.reg.ir = val;
    }

    pub fn set_negative(&mut self, val: bool) {
        self.reg.s = self.reg.s & 0b0111_1111 | bool_to_bit(&val, 7);
    }

    pub fn set_zero(&mut self, val: bool) {
        self.reg.s = self.reg.s & 0b1111_1101 | bool_to_bit(&val, 1)
    }

    pub fn set_carry(&mut self, val: bool) {
        self.reg.s = self.reg.s & 0b1111_1110 | bool_to_bit(&val, 0)
    }

    pub fn set_overflow(&mut self, val: bool) {
        self.reg.s = self.reg.s & 0b1011_1111 | bool_to_bit(&val, 6)
    }

    pub fn set_interrupt_disable(&mut self, val: bool) {
        self.reg.s = self.reg.s & 0b1111_1011 | bool_to_bit(&val, 2)
    }

    pub fn set_decimal_mode(&mut self, val: bool) {
        self.reg.s = self.reg.s & 0b1111_0111 | bool_to_bit(&val, 3)
    }

    pub fn set_break_command(&mut self, val: bool) {
        self.reg.s = self.reg.s & 0b1110_1111 | bool_to_bit(&val, 4)
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
                reg: Registers::default(),
                pins,
            })),
            stepper: None,
            cycles: 0,
        };

        logic
    }

    pub fn tick(&mut self) {
        if self.stepper.is_none() {
            self.stepper = if self.state.borrow().pins["SYNC"].low() {
                Some(read_opcode())
            } else {
                self.state.borrow().pins["SYNC"].set_low().unwrap();
                let op = self.decode_op(&self.state.borrow().ir());
                get_stepper(&op)
            }
        }
        // let cpu = Rc::clone(&self.self_ref);
        let v = self
            .stepper
            .as_mut()
            .unwrap()
            .resume(Rc::clone(&self.state));
        match v {
            CoroutineResult::Yield(()) => {}
            CoroutineResult::Return(_) => {
                self.stepper = None;
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
                self.pc()
            ),
        }
    }
}

impl CPU for W65C02Logic {
    fn cycles(&self) -> CPUCycles {
        self.cycles
    }

    fn advance_cycles(&mut self) {
        self.cycles = self.cycles.wrapping_add(1);
    }

    fn execute(&mut self, _val: u8) -> u8 {
        todo!()
    }

    fn pc(&self) -> Addr {
        self.state.borrow().reg.pc
    }

    fn inc_pc(&mut self) {
        self.state.borrow_mut().inc_pc();
    }
}

unsafe impl Send for W65C02 {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emulator::{abstractions::Pin, cpus::mos6502::Stepper};

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

        assert_eq!(0, cpu.logic.cycles());
        cpu.logic.tick();
        assert_eq!(1, cpu.logic.cycles());
        cpu.logic.tick();
        assert_eq!(2, cpu.logic.cycles());
        cpu.logic.tick();
        assert_eq!(3, cpu.logic.cycles());
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

        assert_eq!(0, cpu.logic.cycles());
        cpu.logic.tick();
        assert_eq!(1, cpu.logic.cycles());
        cpu.logic.tick();
        assert_eq!(2, cpu.logic.cycles());
        cpu.logic.tick();
        assert_eq!(3, cpu.logic.cycles());
    }
}
