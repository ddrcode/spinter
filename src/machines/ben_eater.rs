use std::{thread, time::Duration};

use crossbeam_channel::TryRecvError;

use crate::emulator::{
    abstractions::{Addr, Addressable, Circuit, CircuitBuilder, Machine, PinMessage},
    components::{HM62256BLogic, Oscilator, HM62256B},
    cpus::W65C02,
    EmulatorError,
};

/// Implementation of a popular and simple W65C02-based breadboard computer designed by Ben Eater
/// Details: https://eater.net/6502
/// Work in progress (it is missing ROM and I/O, but works in the current form)
/// Address bus pin A15 is unconnected, as the machine has only 32kB of RAM (one address pin less)
/// In practice addresses pointing to the upper 32kB point in fact to the lower 32kB
pub struct BenEaterMachine {
    circuit: Circuit,
}

impl BenEaterMachine {
    pub fn new() -> Result<Self, EmulatorError> {
        BenEaterMachine::with_program(0x200, &[])
    }

    pub fn with_program(addr: Addr, data: &[u8]) -> Result<Self, EmulatorError> {
        let clock = Oscilator::new(1000);
        let mut ram = HM62256B::new(HM62256BLogic::new());
        let cpu = W65C02::new();

        // FIXME:
        // Trick: forcess the address of reset vector. (should be handled by ROM)
        ram.logic.write_byte(0xfffc & 0x7fff, 0);
        ram.logic.write_byte(0xfffd & 0x7fff, 2);
        ram.logic.load(addr, data);

        let circuit = CircuitBuilder::new()
            .add_component("X1", clock)
            .add_component("U1", cpu)
            .add_component("U6", ram)
            .link("X1", "OUT", "U1", "PHI2")
            .link("U1", "RW", "U6", "WE")
            .link_range("U1", "A", "U6", "A", 0..15)
            .link_range("U6", "D", "U1", "D", 0..8)
            .link_range("U1", "D", "U6", "D", 0..8)
            // .link_to_vcc("U1", "NMI")
            // .link_to_vcc("U1", "RDY")
            // .link_to_vcc("U1", "BE")
            .build();

        Ok(BenEaterMachine { circuit })
    }
}

impl Machine for BenEaterMachine {
    fn start(&mut self) {
        thread::sleep(Duration::from_millis(600));
        self.reset();
        loop {
            self.step();
            thread::sleep(Duration::from_millis(1));
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
        let mut threshold = 50000;
        self.circuit.tick();
        loop {
            let res = self.circuit.receiver.try_recv();
            if let Err(e) = res {
                match e {
                    TryRecvError::Empty => {
                        threshold -= 1;
                        if threshold == 0 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_machine_creation() {
        assert!(BenEaterMachine::new().is_ok());
    }

    #[test]
    fn test_simple_program() {
        let prg: &[u8] = &[
            0x40, 0x4c, 0x00, 0x02, 0x03, 0xee, 0xa9, 0x02, 0xa2, 0x00, 0xa0, 0x00, 0x18, 0x00,
            0xa0, 0x60, 0x4c, 0x11, 0x02, 0x23, 0x12, 0xa0, 0x23, 0x4c, 0xa0, 0x02, 0x4c, 0x17,
            0x02, 0x23, 0x18, 0xa0, 0x23, 0x4c, 0x4c, 0x02, 0x02, 0x65, 0xf6, 0x30, 0x10, 0x60,
            0x60, 0xf3, 0xeb, 0x70, 0x50, 0x60, 0x60, 0xe8, 0xdb, 0xb0, 0x90, 0x60, 0x60, 0xd8,
            0xda, 0xd0, 0xf0, 0x60, 0x60, 0xd7, 0x08, 0x08, 0x00, 0xa9, 0x08, 0xa2, 0x3e, 0x4e,
            0x90, 0x02, 0x18, 0x04, 0x3f, 0x6d, 0x6a, 0x02, 0x3e, 0x6e, 0xca, 0x02, 0xf3, 0xd0,
            0x3f, 0x8d, 0xad, 0x02, 0x02, 0x3e, 0x40, 0xc9, 0x20, 0x08, 0x02, 0x38, 0x04, 0x20,
            0xa0, 0x02, 0xad, 0x00, 0x02, 0x03, 0x00, 0x28,
        ];
        let m = BenEaterMachine::with_program(0x200, &prg).unwrap();
        m.step();
    }
}
