#[macro_use]
extern crate lazy_static;

pub mod emulator;
pub mod machines;
pub mod utils;

use anyhow::Result;
use emulator::abstractions::Machine;
use machines::ben_eater::BenEaterMachine;
use std::io::Read;
use std::{fs::File, path::PathBuf};

pub fn get_file_as_byte_vec(filename: &PathBuf) -> Result<Vec<u8>> {
    let mut f = File::open(filename)?;
    let mut buffer = Vec::new();
    f.read_to_end(&mut buffer)?;
    Ok(buffer)
}

fn main() -> Result<()> {

    let program = get_file_as_byte_vec(&PathBuf::from(r"./tests/all-opcodes.p"))?;

    let mut be = BenEaterMachine::with_program(0x200, &program).unwrap();
    be.start();

    Ok(())
}

