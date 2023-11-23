use crate::emulator::{
    abstractions::{Cycles, Addr},
    cpus::{mos6502::Operand, Registers},
};
use std::fmt;

use super::disassemble;

#[derive(Debug)]
pub struct Operation {
    pub reg: Registers,
    pub opcode: u8,
    pub operand: Operand,
    pub cycle: Cycles,
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", disassemble(self, true, false))
    }
}

//--------------------------------------------------------------------
// Pins

#[derive(Debug)]
pub struct PinsState {
    pub pins: u128,
    pub width: u8,
    pub cycle: Cycles,
}

impl fmt::Display for PinsState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let width = self.width as usize;
        write!(
            f,
            "[{:08}] {:0>width$b}",
            self.cycle,
            self.pins,
        )
    }
}

//--------------------------------------------------------------------
// MemCell

#[derive(Debug)]
pub struct MemCell {
    pub addr: Addr,
    pub val: u8,
}

