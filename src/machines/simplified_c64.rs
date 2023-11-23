use std::{
    cell::RefCell,
    rc::Rc,
    thread,
    time::{Duration, Instant},
};

use crossbeam_channel::RecvTimeoutError;

use crate::{emulator::{
    abstractions::{Addr, Addressable, Circuit, CircuitBuilder, Machine},
    components::{Oscilator, W24512ALogic, W24512A},
    cpus::W65C02,
    EmulatorError,
}, debugger::{Debugger, NullDebugger}};

/// Implementation of a popular and simple W65C02-based breadboard computer designed by Ben Eater
/// Details: https://eater.net/6502
/// Work in progress (it is missing ROM and I/O, but works in the current form)
/// Address bus pin A15 is unconnected, as the machine has only 32kB of RAM (one address pin less)
/// In practice addresses pointing to the upper 32kB point in fact to the lower 32kB
pub struct SimplifiedC64Machine {
    circuit: Rc<Circuit>,
}

impl SimplifiedC64Machine {
    pub fn new() -> Result<Self, EmulatorError> {
        SimplifiedC64Machine::with_program_and_debugger(0x200, &[], Rc::new(NullDebugger))
    }

    pub fn with_program_and_debugger(addr: Addr, data: &[u8], debugger: Rc<dyn Debugger>) -> Result<Self, EmulatorError> {
        let clock = Oscilator::new(1000);
        let mut ram = W24512A::new(W24512ALogic::new());
        let cpu = W65C02::new();

        // FIXME:
        // Trick: forcess the address of reset vector. (should be handled by ROM)
        ram.logic.write_byte(0xfffd, 0xfc);
        ram.logic.write_byte(0xfffc, 0xe2);
        ram.logic.load(addr, data);

        let circuit = CircuitBuilder::new()
            .add_component("X1", clock)
            .add_component("U1", cpu)
            .add_component("U6", ram)
            .link("X1", "OUT", "U1", "PHI2")
            .link("U1", "RW", "U6", "WE")
            .link_range("U1", "A", "U6", "A", 0..16)
            .link_range("U6", "D", "U1", "D", 0..8)
            .link_range("U1", "D", "U6", "D", 0..8)
            // .link_to_vcc("U1", "NMI")
            // .link_to_vcc("U1", "RDY")
            // .link_to_vcc("U1", "BE")
            .set_debugger(debugger)
            .build()?;

        Ok(SimplifiedC64Machine { circuit })
    }
}

impl Machine for SimplifiedC64Machine {
    fn start(&mut self) {
        self.reset();
        // for _ in 0..3_500_000 {
        for _ in 0..55 {
            self.step();
        }
    }

    fn stop(&mut self) {
        self.circuit.write_to_pin("U1", "VCC", false).unwrap();
    }

    // W65C02 requires two cycles in high state on pin 40 (RST) to initialize or reset
    // Then, after start, first 7 cycles are initialization steps
    fn reset(&mut self) {
        // let _ = self.circuit.write_to_pin("U1", "RST", true);
        // self.step();
        // self.step();
        // let _ = self.circuit.write_to_pin("U1", "RST", false);
        // for _ in 0..7 {
        //     self.step();
        // }
    }

    fn step(&mut self) {
        self.circuit.with_pin("X1", "OUT", |pin| {
            pin.toggle().unwrap();
        });
    }
}
