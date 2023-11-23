use crate::{
    emulator::cpus::mos6502::{AddressMode::*, Operand, OperationDef, OPERATIONS},
    utils::if_else,
};
use std::fmt::Write;

use super::Operation;

pub fn disassemble(op: &Operation, verbose: bool, next_op: bool) -> String {
    let mut out = format!("[{:08}] ", op.cycle);
    let def = {
        let o = OPERATIONS.get(&op.opcode);
        if o.is_none() {
            return format!("ERROR! Unknown opcode {:#02x}", op.opcode);
        }
        o.unwrap()
    };
    let addr_correction = if_else(next_op, 0u16, def.len().into());
    let addr = op.reg.pc.borrow().wrapping_sub(addr_correction);
    let val = operand_to_bytes_string(&op.operand);
    let opstr = op_to_string(&def, &op.operand);
    let _ = write!(
        &mut out,
        "{:04x}: {:02x} {} | {}",
        addr, op.opcode, val, opstr
    );
    if verbose && !next_op {
        // can't have cpu state for non-executed op
        let _ = write!(&mut out, "{} ->  {}", " ".repeat(13 - opstr.len()), op.reg);
    }
    out
}

fn op_to_string(def: &OperationDef, operand: &Operand) -> String {
    let m = def.mnemonic.to_string();
    let o = operand.to_string();
    match def.address_mode {
        Implicit => format!("{}", m),
        Accumulator => format!("{} A", m),
        Immediate => format!("{} #${}", m, o),
        Relative => format!("{} ${}", m, o),
        ZeroPage => format!("{} ${}", m, o),
        ZeroPageX => format!("{} ${},X", m, o),
        ZeroPageY => format!("{} ${},Y", m, o),
        Absolute => format!("{} ${}", m, o),
        AbsoluteX => format!("{} ${},X", m, o),
        AbsoluteY => format!("{} ${},Y", m, o),
        Indirect => format!("{} (${})", m, o),
        IndirectX => format!("{} (${},X)", m, o),
        IndirectY => format!("{} (${}),Y", m, o),
    }
}

fn operand_to_bytes_string(operand: &Operand) -> String {
    let blank = "  ";
    let (lo, hi) = match operand {
        Operand::None => (blank.to_string(), blank.to_string()),
        Operand::Byte(x) => (format!("{:02x}", x), blank.to_string()),
        Operand::Word(x) => (format!("{:02x}", x & 0xff), format!("{:02x}", x >> 8)),
    };
    format!("{lo} {hi}")
}
