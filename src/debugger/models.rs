use std::fmt;
use crate::emulator::{cpus::{mos6502::Operand, Registers}, abstractions::Cycles};

use super::disassemble;


#[derive(Debug)]
pub struct OperationDebug {
    pub reg: Registers,
    pub opcode: u8,
    pub operand: Operand,
    pub cycle: Cycles
}

impl fmt::Display for OperationDebug {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", disassemble(self, true, false))
    }
}
