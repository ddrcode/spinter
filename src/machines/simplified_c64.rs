use std::{thread, time::Duration};

use crossbeam_channel::TryRecvError;

use crate::emulator::{
    abstractions::{Addr, Addressable, Circuit, CircuitBuilder, Machine, PinMessage},
    components::{W24512ALogic, Oscilator, W24512A},
    cpus::W65C02,
    EmulatorError,
};

/// Implementation of a popular and simple W65C02-based breadboard computer designed by Ben Eater
/// Details: https://eater.net/6502
/// Work in progress (it is missing ROM and I/O, but works in the current form)
/// Address bus pin A15 is unconnected, as the machine has only 32kB of RAM (one address pin less)
/// In practice addresses pointing to the upper 32kB point in fact to the lower 32kB
pub struct SimplifiedC64Machine {
    circuit: Circuit,
}

impl SimplifiedC64Machine {
    pub fn new() -> Result<Self, EmulatorError> {
        SimplifiedC64Machine::with_program(0x200, &[])
    }

    pub fn with_program(addr: Addr, data: &[u8]) -> Result<Self, EmulatorError> {
        let clock = Oscilator::new(1000);
        let mut ram = W24512A::new(W24512ALogic::new());
        let cpu = W65C02::new();

        // FIXME:
        // Trick: forcess the address of reset vector. (should be handled by ROM)
        // ram.logic.write_byte(0xfffc, 0);
        // ram.logic.write_byte(0xfffd, 2);
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
            .build();

        Ok(SimplifiedC64Machine { circuit })
    }
}

impl Machine for SimplifiedC64Machine {
    fn start(&mut self) {
        thread::sleep(Duration::from_millis(500));
        self.reset();
        loop {
            self.step();
            thread::sleep(Duration::from_micros(100));
        }
    }

    fn stop(&mut self) {
        // let _ = self.circuit.write_to_pin("U1", "VCC", false);
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

    fn step(&self) {
        let mut threshold = if *self.circuit.state.borrow() { 2000 } else { 5000 };
        self.circuit.tick();
        loop {
            let res = self.circuit.receiver.try_recv();
            if let Err(e) = res {
                match e {
                    TryRecvError::Empty => {
                        threshold -= 1;
                        if threshold == 0 {//|| !*self.circuit.state.borrow() {
                            break;
                        }
                        continue;
                    }
                    TryRecvError::Disconnected => {
                        panic!("Channel disconnected");
                    }
                }
            }
            let msg = res.unwrap();
            if let Some(links) = &self.circuit.components[&msg.component].links.get(&msg.pin) {
                for (comp, pin) in links.iter() {
                    self.circuit.components[comp]
                        .sender
                        .send(PinMessage::new(comp, pin, msg.val))
                        .unwrap();
                }
            }
        }
    }
}

