#[macro_use]
extern crate lazy_static;

pub mod emulator;
pub mod machines;
pub mod utils;
pub mod debugger;

use anyhow::Result;
use emulator::abstractions::Machine;
use machines::simplified_c64::SimplifiedC64Machine;
use std::io::Read;
use std::{fs::File, path::PathBuf};

pub fn get_file_as_byte_vec(filename: &PathBuf) -> Result<Vec<u8>> {
    let mut f = File::open(filename)?;
    let mut buffer = Vec::new();
    f.read_to_end(&mut buffer)?;
    Ok(buffer)
}

fn main() -> Result<()> {

    // let program = get_file_as_byte_vec(&PathBuf::from(r"./tests/target/add-sub-16bit.p"))?;
    let rom = get_file_as_byte_vec(&PathBuf::from(r"./rom/64c.251913-01.bin"))?;
    // let rom = get_file_as_byte_vec(&PathBuf::from(r"./rom/kernal-64c.251913-01.bin"))?;
    let kernal = &rom[8192..];
    let basic = &rom[..8192];
    let blank = [0u8; 0x2000];
    let program = [basic, &blank, kernal].concat();

    let mut be = SimplifiedC64Machine::with_program(0xa000, &program).unwrap();
    be.start();

    Ok(())
}

