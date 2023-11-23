use super::{DebugMessage, DebuggerState};
use crate::emulator::abstractions::{Addr, Cycles};
use std::{cell::RefCell, collections::HashMap, ops::Range, rc::Rc};

pub trait Debugger {
    fn debug(&self, msg: DebugMessage);
    fn enable(&self);
    fn disable(&self);
    fn enabled(&self) -> bool;
}

#[derive(Default)]
pub struct NullDebugger;

impl NullDebugger {
    pub fn as_rc() -> Rc<Self> {
        Rc::new(NullDebugger)
    }
}

impl Debugger for NullDebugger {
    fn debug(&self, _msg: DebugMessage) {}
    fn enable(&self) {}
    fn disable(&self) {}

    fn enabled(&self) -> bool {
        false
    }
}

pub struct CliDebugger {
    state: DebuggerState,
    memory: RefCell<HashMap<Addr, u8>>,
}

impl Default for CliDebugger {
    fn default() -> Self {
        Self {
            state: DebuggerState::default(),
            memory: RefCell::new(HashMap::with_capacity(1 << 16)),
        }
    }
}

impl Debugger for CliDebugger {
    fn debug(&self, msg: DebugMessage) {
        if self.enabled() {
            match msg {
                // DebugMessage::CpuOperation(o) => println!("{o}"),
                DebugMessage::MemCellUpdate(m) => {
                    self.memory.borrow_mut().insert(m.addr, m.val);
                }
                _ => (),
            }
        }
    }

    fn enable(&self) {
        self.state.set_enabled(true);
    }

    fn disable(&self) {
        self.state.set_enabled(false);
    }

    fn enabled(&self) -> bool {
        self.state.enabled()
    }
}

impl CliDebugger {
    pub fn print_screen_memory(&self, addr: Addr, width: usize, height: usize) {
        let char_set: Vec<char> =
            "@abcdefghijklmnopqrstuvwxyz[£]↑← !\"#$%&'()*+,-./0123456789:;<=>?\
         -ABCDEFGHIJKLMNOPQRSTUVWXYZ[£]↑← !\"#$%&'()*+,-./0123456789:;<=>?\
         @ABCDEFGHIJKLMNOPQRSTUVWXYZ[£]↑← !\"#$%&'()*+,-./0123456789:;<=>?\
         -abcdefghijklmnopqrstuvwxyz[£]↑← !\"#$%&'()*+,-./0123456789:;<=>?"
                .chars()
                .collect();
        let ua = addr as usize;
        let mem = self.memory.borrow();
        let mut n = 0;

        println!();
        println!("{}", " ".repeat(44));
        print!("{}", "  ");
        for i in ua..(ua + width * height) {
            let sc = *mem.get(&(i as Addr)).unwrap_or(&0);
            print!("{}", format!("{}", char_set[sc as usize]));
            n += 1;
            if n % 40 == 0 {
                print!("{}", "  ");
                println!();
                print!("{}", "  ");
            }
        }
        println!("{}", " ".repeat(42));
        println!("              ");
    }

    pub fn mem_dump(&self, range: Range<Addr>) {
        let mem = self.memory.borrow();
        // let v: Vec<u8> = Vec::with_capacity((range.end - range.start).into());
        println!();
        for i in range {
            if i % 16 == 0 {
                println!();
                print!("{i:04x}: ");
            }
            else if i % 4 == 0 {
                print!("| ");
            }
            let cell = mem.get(&(i as Addr)).map_or(0, |v| *v);
            print!("{cell:02x} ");
        }
        println!();
    }

    pub fn init_mem(&self, addr: Addr, data: &[u8]) {
        let mut mem = self.memory.borrow_mut();
        let s = usize::from(addr);
        for i in s..(s+data.len()) {
            mem.insert(i as Addr, data[i-s]);
        }
    }
}
