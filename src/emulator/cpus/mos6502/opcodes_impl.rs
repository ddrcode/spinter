use super::{AddressMode::*, Mnemonic::*, Operation, OperationDef};
use crate::emulator::cpus::CpuState;
use std::num::Wrapping;

// OMG this is so terribly ugly!
pub fn execute_operation(cpu: &CpuState, op: &OperationDef, val: u8) -> u8 {
    match &*op.fn_name {
        "op_arithmetic" => op_arithmetic(cpu, op, val),
        "op_bit" => op_bit(cpu, op, val),
        "op_bitwise" => op_bitwise(cpu, op, val),
        "op_branch" => op_branch(cpu, op, val),
        //     "op_brk" => op_brk(op, machine),
        "op_compare" => op_compare(cpu, op, val),
        "op_flag" => op_flag(cpu, op, val),
        //     "op_incdec_mem" => op_incdec_mem(op, machine),
        //     "op_incdec_reg" => op_incdec_reg(op, machine),
        //     "op_jmp" => op_jmp(op, machine),
        //     "op_jsr" => op_jsr(op, machine),
        "op_load" => op_load(cpu, op, val),
        //     "op_nop" => op_nop(op, machine),
        //     "op_pla" => op_pla(op, machine),
        //     "op_plp" => op_plp(op, machine),
        //     "op_push" => op_push(op, machine),
        //     "op_rotate" => op_rotate(op, machine),
        //     "op_rti" => op_rti(op, machine),
        //     "op_rts" => op_rts(op, machine),
        //     "op_shift" => op_shift(op, machine),
        "op_store" => op_store(cpu, op, val),
        "op_transfer" => op_transfer(cpu, op, val),
        _ => panic!("Unidentified function name {}", op.fn_name),
    }
}

// ----------------------------------------------------------------------
// helpers

// fn get_val(op: &Operation, machine: &impl Machine) -> Option<u8> {
//     if let Some(addr) = op.address {
//         Some(machine.get_byte(addr))
//     } else if op.def.address_mode == Immediate {
//         op.operand.as_ref().unwrap().get_byte()
//     } else if op.def.address_mode == Accumulator {
//         Some(machine.A8())
//     } else {
//         None
//     }
// }
//
// fn set_val(val: u8, op: &Operation, machine: &mut impl Machine) {
//     if let Some(addr) = op.address {
//         machine.set_byte(addr, val)
//     } else if op.def.address_mode == Accumulator {
//         machine.set_A(val)
//     } else {
//         panic!("Can't set value for address mode {}", op.def.address_mode)
//     };
// }
//
// fn store_byte(val: u8, op: &Operation, machine: &mut impl Machine) -> u8 {
//     machine.set_byte(op.address.unwrap(), val);
//     op.def.cycles
// }
//
fn set_flags(flags: &str, vals: &[bool], cpu: &CpuState) {
    let chars = String::from(flags);
    if chars.len() != vals.len() {
        panic!("Incorrect args length in set_flags")
    };
    for (i, ch) in chars.chars().enumerate() {
        let val = vals[i];
        match ch {
            'C' => cpu.set_carry(val),
            'Z' => cpu.set_zero(val),
            'I' => cpu.set_interrupt_disable(val),
            'D' => cpu.set_decimal_mode(val),
            'B' => cpu.set_break_command(val),
            'V' => cpu.set_overflow(val),
            'N' => cpu.set_negative(val),
            _ => panic!("Unrecognized flag symbol: {}", ch),
        };
    }
}

fn set_nz_flags(val: u8, cpu: &CpuState) {
    cpu.set_negative(neg(val));
    cpu.set_zero(zero(val));
}

fn neg(val: u8) -> bool {
    val & 0x80 > 0
}
fn zero(val: u8) -> bool {
    val == 0
}

// see https://www.righto.com/2012/12/the-6502-overflow-flag-explained.html
fn overflow(in1: u8, in2: u8, result: u8) -> bool {
    ((in1 ^ result) & (in2 ^ result) & 0x80) > 0
}

// // ----------------------------------------------------------------------
// // implementation of operations
//
// // https://www.righto.com/2012/12/the-6502-overflow-flag-explained.html
// // http://retro.hansotten.nl/uploads/mag6502/sbc_tsx_txs_instructions.pdf
// // TODO compute cycles for page cross
fn op_arithmetic(cpu: &CpuState, op: &OperationDef, val: u8) -> u8 {
    let a = cpu.a();
    let val = match op.mnemonic {
        ADC => val,
        SBC => !val,
        _ => panic!("{} is not an arithmetic operation", op.mnemonic),
    };
    let sum = u16::from(cpu.a()) + u16::from(cpu.carry()) + u16::from(val);
    cpu.set_a((sum & 0xff) as u8);
    let res = cpu.a();
    set_flags(
        "NZCV",
        &[neg(res), zero(res), sum > 0xff, overflow(a, val, res)],
        cpu,
    );
    res
}

fn op_bit(cpu: &CpuState, op: &OperationDef, val: u8) -> u8 {
    set_flags(
        "NZV",
        &[neg(val), zero(val & cpu.a()), val & 0b01000000 > 0],
        cpu,
    );
    val
}

fn op_branch(cpu: &CpuState, op: &OperationDef, _val: u8) -> u8 {
    let branch: bool = match op.mnemonic {
        BCC => !cpu.carry(),
        BCS => cpu.carry(),
        BNE => !cpu.zero(),
        BEQ => cpu.zero(),
        BPL => !cpu.negative(),
        BMI => cpu.negative(),
        BVC => !cpu.overflow(),
        BVS => cpu.overflow(),
        _ => panic!("{} is not a branch operation", op.mnemonic),
    };

    // BVC always takes 3 cycles (see https://c64os.com/post/6502instructions)
    // op.def.cycles + if op.def.mnemonic == BVC { 1 } else { 0 }

    branch.into()
}

// // see https://www.c64-wiki.com/wiki/BRK
// fn op_brk(op: &Operation, machine: &mut impl Machine) -> u8 {
//     machine.set_PC(machine.PC().wrapping_add(2));
//     set_flags("B", &[true], machine);
//     machine.irq();
//     op.def.cycles
// }
//
// TODO add cycle for page change
fn op_compare(cpu: &CpuState, op: &OperationDef, val: u8) -> u8 {
    let reg = match op.mnemonic {
        CMP => cpu.a(),
        CPX => cpu.x(),
        CPY => cpu.y(),
        _ => panic!("{} is not a compare operation", op.mnemonic),
    };
    let diff = reg.wrapping_sub(val);
    set_flags("NZC", &[neg(diff), reg == val, reg >= val], cpu);
    diff
}

// fn op_incdec_mem(op: &Operation, machine: &mut impl Machine) -> u8 {
//     let mut val = Wrapping(get_val(op, machine).unwrap());
//     match op.def.mnemonic {
//         DEC => val -= 1,
//         INC => val += 1,
//         _ => panic!("{} is not a inc/dec (mem) operation", op.def.mnemonic),
//     };
//     set_val(val.0, op, machine);
//     set_nz_flags(val.0, machine);
//     op.def.cycles
// }
//
// fn op_incdec_reg(op: &Operation, machine: &mut impl Machine) -> u8 {
//     match op.def.mnemonic {
//         DEX => machine.cpu_mut().registers.x -= 1,
//         DEY => machine.cpu_mut().registers.y -= 1,
//         INX => machine.cpu_mut().registers.x += 1,
//         INY => machine.cpu_mut().registers.y += 1,
//         _ => panic!("{} is not a inc/dec operation", op.def.mnemonic),
//     };
//     let val = match op.def.mnemonic {
//         DEX | INX => machine.X8(),
//         DEY | INY => machine.Y8(),
//         _ => panic!("{} is not a inc/dec operation", op.def.mnemonic),
//     };
//     set_nz_flags(val, machine);
//     op.def.cycles
// }
//
// TODO add cycle for page change
fn op_bitwise(cpu: &CpuState, op: &OperationDef, val: u8) -> u8 {
    match op.mnemonic {
        AND => cpu.set_a(cpu.a() & val),
        ORA => cpu.set_a(cpu.a() | val),
        EOR => cpu.set_a(cpu.a() ^ val),
        _ => panic!("{} is not a bitwise operation", op.mnemonic),
    };
    let a = cpu.a();
    set_nz_flags(a, cpu);
    a
}

fn op_flag(cpu: &CpuState, op: &OperationDef, _val: u8) -> u8 {
    match op.mnemonic {
        CLC => cpu.set_carry(false),
        SEC => cpu.set_carry(true),
        CLI => cpu.set_interrupt_disable(false),
        SEI => cpu.set_interrupt_disable(true),
        CLD => cpu.set_decimal_mode(false),
        SED => cpu.set_decimal_mode(true),
        CLV => cpu.set_overflow(false),
        _ => panic!("{} is not a flag set/unset operation", op.mnemonic),
    };
    0
}

// fn op_jmp(op: &Operation, machine: &mut impl Machine) -> u8 {
//     machine.set_PC(op.address.unwrap());
//     op.def.cycles
// }
//
// fn op_jsr(op: &Operation, machine: &mut impl Machine) -> u8 {
//     let pc = machine.PC().wrapping_sub(1);
//     machine.push((pc >> 8) as u8);
//     machine.push((pc & 0x00ff) as u8);
//     machine.set_PC(op.address.unwrap());
//     op.def.cycles
// }
//
// // FIXME add cycle for crossing page boundary
fn op_load(cpu: &CpuState, op: &OperationDef, val: u8) -> u8 {
    match op.mnemonic {
        LDA => cpu.set_a(val),
        LDX => cpu.set_x(val),
        LDY => cpu.set_y(val),
        _ => panic!("{} is not a load operation", op.mnemonic),
    };
    set_nz_flags(val, cpu);
    val
}
//
// fn op_nop(op: &Operation, _machine: &mut impl Machine) -> u8 {
//     op.def.cycles
// }
//
// fn op_pla(op: &Operation, machine: &mut impl Machine) -> u8 {
//     let val = machine.pop();
//     machine.set_A(val);
//     set_nz_flags(val, machine);
//     op.def.cycles
// }
//
// fn op_plp(op: &Operation, machine: &mut impl Machine) -> u8 {
//     let val = machine.pop();
//     machine.cpu_mut().registers.status = ProcessorStatus::from(val);
//     op.def.cycles
// }
//
// fn op_push(op: &Operation, machine: &mut impl Machine) -> u8 {
//     let val: u8 = match op.def.mnemonic {
//         PHA => machine.A8(),
//         PHP => machine.P().into(),
//         _ => panic!("{} is not a push operation", op.def.mnemonic),
//     };
//     let addr = machine.stack_addr();
//     machine.set_byte(addr, val);
//     machine.cpu_mut().registers.stack -= 1;
//     op.def.cycles
// }
//
// fn op_rotate(op: &Operation, machine: &mut impl Machine) -> u8 {
//     let val = get_val(op, machine).unwrap();
//     let c = if machine.P().carry { 0xff } else { 0 };
//     let (new_val, mask, carry) = match op.def.mnemonic {
//         ROL => (val << 1, c & 1, val & 0b10000000 > 0),
//         ROR => (val >> 1, c & 0b10000000, val & 1 > 0),
//         _ => panic!("{} is not a rotate operation", op.def.mnemonic),
//     };
//     set_val(new_val | mask, op, machine);
//     set_flags(
//         "NZC",
//         &[neg(new_val | mask), zero(new_val | mask), carry],
//         machine,
//     );
//     op.def.cycles
// }
//
// fn op_rti(op: &Operation, machine: &mut impl Machine) -> u8 {
//     machine.cpu_mut().registers.status = ProcessorStatus::from(machine.pop());
//     machine.cpu_mut().registers.counter = machine.pop() as u16 | ((machine.pop() as u16) << 8);
//     op.def.cycles
// }
//
// fn op_rts(op: &Operation, machine: &mut impl Machine) -> u8 {
//     let lo = machine.pop() as u16;
//     let hi = machine.pop() as u16;
//     machine.set_PC((lo | hi << 8).wrapping_add(1));
//     op.def.cycles
// }
//
// fn op_shift(op: &Operation, machine: &mut impl Machine) -> u8 {
//     let val = get_val(op, machine).unwrap();
//     let (res, carry) = match op.def.mnemonic {
//         ASL => ((Wrapping(val) << 1).0, val & 0b10000000 > 0),
//         LSR => (val >> 1, val & 1 > 0),
//         _ => panic!("{} is not a shift operation", op.def.mnemonic),
//     };
//     set_val(res, op, machine);
//     set_flags("NZC", &[neg(res), zero(res), carry], machine);
//     op.def.cycles
// }

fn op_store(cpu: &CpuState, op: &OperationDef, _val: u8) -> u8 {
    match op.mnemonic {
        STA => cpu.a(),
        STX => cpu.x(),
        STY => cpu.y(),
        _ => panic!("{} is not a store operation", op.mnemonic),
    }
}

fn op_transfer(cpu: &CpuState, op: &OperationDef, _val: u8) -> u8 {
    match op.mnemonic {
        TAX => cpu.set_x(cpu.a()),
        TAY => cpu.set_y(cpu.a()),
        TXA => cpu.set_a(cpu.x()),
        TYA => cpu.set_a(cpu.y()),
        TXS => cpu.set_sp(cpu.x()),
        TSX => cpu.set_x(cpu.sp()),
        _ => panic!("{} is not a transfer operation", op.mnemonic),
    };
    if op.mnemonic != TXS {
        // TXS doesn't change any flag
        set_nz_flags(cpu.a(), cpu);
    }
    0
}
//
// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     #[test]
//     fn test_utils() {
//         assert!(neg(0x80));
//         assert!(zero(0));
//     }
// }
