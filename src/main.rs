#[macro_use]
extern crate lazy_static;

pub mod debugger;
pub mod emulator;
pub mod machines;
pub mod utils;

use anyhow::Result;
use debugger::{ CliDebugger, Debugger, DebuggerConfig };
use emulator::abstractions::Machine;
use machines::simplified_c64::SimplifiedC64Machine;
use std::io::Read;
use std::rc::Rc;
use std::{fs::File, path::PathBuf};

pub fn get_file_as_byte_vec(filename: &PathBuf) -> Result<Vec<u8>> {
    let mut f = File::open(filename)?;
    let mut buffer = Vec::new();
    f.read_to_end(&mut buffer)?;
    Ok(buffer)
}

fn main() -> Result<()> {
    // let program = get_file_as_byte_vec(&PathBuf::from(r"./tests/target/cmp.p"))?;
    // let rom = get_file_as_byte_vec(&PathBuf::from(r"./rom/64c.251913-01.bin"))?;
    let rom = get_file_as_byte_vec(&PathBuf::from(r"./rom/kernal-64c.251913-01.bin"))?;
    let kernal = &rom[8192..];
    let basic = &rom[..8192];
    let blank = [0u8; 0x2000];
    let program = [basic, &blank, kernal].concat();

    let debugger = Rc::new(CliDebugger::new(DebuggerConfig {
        show_operations: false,
        show_pins_state: true,
    }));
    // debugger.init_mem(0x200, &program);
    debugger.enable();

    // let mut be = SimplifiedC64Machine::with_program(0x0200, &program)?;
    let mut be = SimplifiedC64Machine::with_program_and_debugger(
        0xa000,
        // 0x0200,
        &program,
        Rc::clone(&debugger) as Rc<dyn Debugger>
    )?;
    be.start();
    debugger.print_screen_memory(0x0400, 40, 25);
    // debugger.mem_dump(0..1024);

    Ok(())
}
