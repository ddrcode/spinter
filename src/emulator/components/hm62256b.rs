use crate::emulator::abstractions::{
    Addr, Addressable, CircuitCtx, Component, Pin, PinBuilder,
    PinDirection::{self, *},
    Pins, Port, RAM,
};
use std::rc::Rc;
use std::{collections::HashMap, fmt, ops::Index};

//--------------------------------------------------------------------
// PINS

const ADDR_PINS: [usize; 15] = [10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 25, 24, 21, 23, 26];
const DATA_PINS: [usize; 8] = [11, 12, 13, 15, 16, 17, 18, 19];

/// Model of Hitachi HM62256B pins.
///
/// ```text
///               HM62256B
///            ┌──────────────┐
///    A14 --> │  1 *      28 │ <-- VCC
///    A12 --> │  2 *      27 │ <-- WE!
///     A7 --> │  3 *    * 26 │ <-- A13
///     A6 --> │  4 *    * 25 │ <-- A8
///     A5 --> │  5 *    * 24 │ <-- A9
///     A4 --> │  6 *    * 23 │ <-- A11
///     A3 --> │  7 *      22 │ --- OE!
///     A2 --> │  8 *    * 21 │ <-- A10
///     A1 --> │  9 *      20 │ <-- CS!
///     A0 --> │ 10 *    * 19 │ <-> D7
///     D0 <-> │ 11 *    * 18 │ <-> D6
///     D1 <-> │ 12 *    * 17 │ <-> D5
///     D2 <-> │ 13 *    * 16 │ <-> D4
///    GND <-- │ 14      * 15 │ <-> D3
///            └──────────────┘
///
///    * - tri-state,  ! - active on low
///
///    A0-A14:  Address input
///    D0-D7:   Data input/output
///    CS!:     Chip select input
///    WE!:     Write enable input
///    OE!:     Output enable input
///    VCC:     Power supply
///    GND:     Ground
///
///    Controlling state of data pins (D0-D7):
///    CS! OE! WE! STATE
///    H   x   x   no access
///    L   H   H   no access
///    L   L   H   read
///    L   H   L   write
///    L   L   L   write
/// ```
///
/// Links:
/// - [Data sheet](https://eater.net/datasheets/hm62256b.pdf)
///
pub struct HM62256BPins {
    pins: [Rc<Pin>; 28],
    pins_map: HashMap<String, Rc<Pin>>,
    pub data: Rc<Port<u8>>,
    pub addr: Rc<Port<u16>>,
}

impl HM62256BPins {
    pub fn new() -> Self {
        let pins: Vec<Rc<Pin>> = PinBuilder::new(28)
            .with_ids(&ADDR_PINS)
            .group("A", 0)
            .direction(Input)
            .with_ids(&DATA_PINS)
            .group("D", 0)
            .direction(Output)
            .io()
            .tri_state()
            .set(14, "VSS", Input)
            .set(20, "CS", Input)
            .set(22, "OE", Input)
            .set(27, "WE", Input)
            .set(28, "VCC", Input)
            .build()
            .iter()
            .map(|pin| Rc::new(pin.clone()))
            .collect();

        let data_pins: Vec<Rc<Pin>> = DATA_PINS.map(|id| Rc::clone(&pins[id - 1])).to_vec();
        let addr_pins: Vec<Rc<Pin>> = ADDR_PINS.map(|id| Rc::clone(&pins[id - 1])).to_vec();

        let mut pins_map: HashMap<String, Rc<Pin>> = HashMap::with_capacity(40);
        pins.iter().for_each(|pin| {
            pins_map.insert(pin.name().to_string(), Rc::clone(pin));
        });

        HM62256BPins {
            pins: pins
                .try_into()
                .unwrap_or_else(|_| panic!("Must have 28 pins")),
            data: Port::from_pins(8, data_pins),
            addr: Port::from_pins(15, addr_pins),
            pins_map,
        }
    }
}

impl Pins for HM62256BPins {
    fn pins(&self) -> &[Rc<Pin>] {
        &self.pins
    }
}

impl Index<&str> for HM62256BPins {
    type Output = Pin;

    fn index(&self, name: &str) -> &Self::Output {
        self.pins_map[name].as_ref()
    }
}

impl fmt::Debug for HM62256BPins {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!(
            "HM62256BPins: Addr: {:#04x}, Data: {:#02x}, CWO: {}{}{}",
            self.addr.read(),
            self.data.read(),
            u8::from(self["CS"].state()),
            u8::from(self["WE"].state()),
            u8::from(self["OE"].state())
        ))
    }
}

//--------------------------------------------------------------------
// LOGIC

pub struct HM62256BLogic {
    data: [u8; 1 << 15],
}

impl HM62256BLogic {
    pub fn new() -> Self {
        HM62256BLogic { data: [0; 1 << 15] }
    }

    pub fn load(&mut self, addr: Addr, data: &[u8]) {
        let a = addr as usize;
        for i in a..(a + data.len()) {
            self.data[i] = data[i - a];
        }
    }
}

impl Addressable for HM62256BLogic {
    fn read_byte(&self, addr: Addr) -> u8 {
        self.data[addr as usize]
    }

    fn write_byte(&mut self, addr: Addr, value: u8) {
        self.data[addr as usize] = value;
    }

    fn address_width(&self) -> u16 {
        15
    }
}

impl RAM for HM62256BLogic {}

//--------------------------------------------------------------------
// MAIN STRUCT

pub struct HM62256B<T: Addressable> {
    pub pins: Rc<HM62256BPins>,
    pub logic: T,
    ctx: CircuitCtx,
}

impl<T: RAM> HM62256B<T> {
    pub fn new(logic: T) -> Self {
        let pins = Rc::new(HM62256BPins::new());
        HM62256B {
            pins,
            logic,
            ctx: Default::default(),
        }
    }

    fn is_enabled(&self) -> bool {
        self.pins["CS"].low()
    }

    fn can_write(&self) -> bool {
        self.is_enabled() && self.pins["WE"].low()
    }

    fn can_read(&self) -> bool {
        self.is_enabled() && !self.can_write() && self.pins["OE"].low()
    }

    fn write_byte(&mut self) {
        if self.can_write() {
            let addr = self.pins.addr.read();
            let byte = self.pins.data.read();
            self.logic.write_byte(addr, byte);
        }
    }

    fn read_byte(&self) {
        if self.can_read() {
            let addr = self.pins.addr.read();
            let byte = self.logic.read_byte(addr);
            self.pins.data.write(byte);
        }
    }

    fn set_enable(&self, val: bool) {
        self.pins
            .pins
            .iter()
            .filter(|p| p.tri_state())
            .for_each(|p| {
                p.set_enable(val).unwrap();
            });
    }

    fn set_data_direction(&self, val: bool) {
        self.pins
            .data
            .set_direction(PinDirection::from(val))
            .unwrap();
    }
}

impl<T: RAM + 'static> Component for HM62256B<T> {
    fn get_pin(&self, name: &str) -> Option<&Pin> {
        self.pins.by_name(name)
    }

    fn ctx(&self) -> &CircuitCtx {
        &self.ctx
    }

    fn attach(&mut self, ctx: CircuitCtx) {
        self.ctx = ctx;
    }

    fn on_pin_state_change(&mut self, pin_name: &str, val: bool) {
        match pin_name {
            "CS" => self.set_enable(!val),
            "WE" => self.set_data_direction(val),
            "OE" => self.set_data_direction(!val || self.pins["WE"].high()),
            _ => {
                if let Some(gn) = self.pins[pin_name].group_name() {
                    match gn.as_str() {
                        "A" => self.read_byte(),
                        "D" => self.write_byte(),
                        _ => {}
                    }
                }
            }
        }
    }
}

unsafe impl<T: RAM> Send for HM62256B<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emulator::abstractions::PinDirection::{self, *};

    type MEM = HM62256B<HM62256BLogic>;

    fn set_input(mem: &mut MEM, pin_name: &str, val: bool) {
        mem.pins[pin_name].set_val(val);
        mem.on_pin_state_change(pin_name, val);
    }

    fn set_state(mem: &mut MEM, cs: bool, we: bool, oe: bool) {
        set_input(mem, "CS", cs);
        set_input(mem, "WE", we);
        set_input(mem, "OE", oe);
    }

    fn test_state(mem: &MEM, enabled: bool, can_write: bool, can_read: bool) {
        assert_eq!(enabled, mem.is_enabled());
        assert_eq!(can_write, mem.can_write());
        assert_eq!(can_read, mem.can_read());
    }

    fn test_directions(mem: &MEM, dir: PinDirection) {
        for i in 0..8 {
            assert_eq!(dir, mem.pins[&format!("D{i}")].direction());
        }
    }

    #[test]
    fn test_structure() {
        let mem = HM62256B::new(HM62256BLogic::new());
        mem.pins.pins.iter().for_each(|pin| {
            assert!(pin.id().is_some());
        });
    }

    #[test]
    fn test_enablement() {
        let mut mem = HM62256B::new(HM62256BLogic::new());

        // CS high - component disabled
        for (we, oe) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            set_state(&mut mem, true, we > 0, oe > 0);
            test_state(&mem, false, false, false);
        }

        set_state(&mut mem, false, false, false);
        test_state(&mem, true, true, false);

        set_state(&mut mem, false, true, false);
        test_state(&mem, true, false, true);

        set_state(&mut mem, false, false, true);
        test_state(&mem, true, true, false);

        set_state(&mut mem, false, true, true);
        test_state(&mem, true, false, false);
    }

    #[test]
    fn test_read() {
        let mut mem = HM62256B::new(HM62256BLogic::new());
        mem.logic.write_byte(0x21, 0xff); // addr: 0b100001
        set_state(&mut mem, false, true, false);
        set_input(&mut mem, "A0", true);
        set_input(&mut mem, "A5", true);
        test_directions(&mem, Output);
        assert_eq!(mem.pins.data.read(), 0xff);
    }

    #[test]
    fn test_write() {
        let mut mem = HM62256B::new(HM62256BLogic::new());
        set_state(&mut mem, false, false, true);
        set_input(&mut mem, "A1", true);
        set_input(&mut mem, "A2", true);
        set_input(&mut mem, "D0", true);
        set_input(&mut mem, "D1", true);
        test_directions(&mem, Input);
        assert_eq!(mem.logic.read_byte(0b110), 0b11);
    }
}
