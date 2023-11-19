use std::{time::Duration, thread};

use crate::{
    emulator::abstractions::{Component, Pin, Tickable, CircuitCtx, },
    utils::if_else,
};
use gametime::{Frequency, FrequencyTicker};

pub struct Oscilator {
    pub pin: Pin,
    ticker: FrequencyTicker,
    ctx: CircuitCtx
}

impl Oscilator {
    pub fn new(khz: u64) -> Self {
        Oscilator {
            pin: Pin::output("OUT"),
            ticker: Frequency::from_khz(khz).ticker(),
            ctx: Default::default()
        }
    }
}

impl Tickable for Oscilator {
    fn tick(&self) {
        self.pin.toggle().unwrap();
    }
}

impl Component for Oscilator {
    fn get_pin(&self, name: &str) -> Option<&Pin> {
        if_else(name == "OUT", Some(&self.pin), None)
    }

    fn ctx(&self) -> &CircuitCtx {
        &self.ctx
    }

    fn attach(&mut self, ctx: CircuitCtx) {
        self.ctx = ctx;
    }

    fn on_pin_state_change(&mut self, _pin_name: &str, _val: bool) {
        // no input pins
    }

    fn init(&mut self) {
        loop {
        thread::sleep(Duration::from_millis(900));
        println!("Oscilator tick");
        self.tick();
        }
    }
}

unsafe impl Send for Oscilator {}
