use super::W65C02_Pins;
use crate::utils::bool_to_bit;
use std::{cell::RefCell, fmt, rc::Rc};

//--------------------------------------------------------------------
// Registers

#[derive(Debug, Default, Clone)]
pub struct Registers {
    /// Stores currently processed instruction. Can't be set by any operation.
    pub(crate) ir: RefCell<u8>,
    pub(crate) a: RefCell<u8>,
    pub(crate) x: RefCell<u8>,
    pub(crate) y: RefCell<u8>,
    pub(crate) pc: RefCell<u16>,
    pub(crate) sp: RefCell<u8>, // stack pointer
    pub(crate) s: RefCell<u8>,
}

impl fmt::Display for Registers {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "PC:${:04x}  A:${:02x}  X:${:02x}  Y:${:02x}  SP:${:02x}  S:%{:08b}",
            *self.pc.borrow(),
            *self.a.borrow(),
            *self.x.borrow(),
            *self.y.borrow(),
            *self.sp.borrow(),
            *self.s.borrow()
        )
    }
}

//--------------------------------------------------------------------
// CPU State

#[derive(Clone)]
pub struct CpuState {
    reg: Registers,
    pub pins: Rc<W65C02_Pins>,
}

impl CpuState {
    pub fn new(pins: Rc<W65C02_Pins>) -> Self {
        Self {
            pins,
            reg: Default::default()
        }
    }

    #[inline]
    pub fn inc_pc(&self) {
        self.set_pc(self.pc().wrapping_add(1));
    }

    #[inline]
    pub fn a(&self) -> u8 {
        *self.reg.a.borrow()
    }

    #[inline]
    pub fn x(&self) -> u8 {
        *self.reg.x.borrow()
    }

    #[inline]
    pub fn y(&self) -> u8 {
        *self.reg.y.borrow()
    }

    #[inline]
    pub fn s(&self) -> u8 {
        *self.reg.s.borrow()
    }

    #[inline]
    pub fn sp(&self) -> u8 {
        *self.reg.sp.borrow()
    }

    #[inline]
    pub fn pc(&self) -> u16 {
        *self.reg.pc.borrow()
    }

    #[inline]
    pub fn pcl(&self) -> u8 {
        (*self.reg.pc.borrow() & 0x00ff) as u8
    }

    #[inline]
    pub fn pch(&self) -> u8 {
        (*self.reg.pc.borrow() >> 8) as u8
    }

    #[inline]
    pub fn set_pc(&self, val: u16) {
        *self.reg.pc.borrow_mut() = val;
    }

    #[inline]
    pub fn set_pcl(&self, val: u8) {
        let addr = (*self.reg.pc.borrow() & 0xff00) | u16::from(val);
        self.set_pc(addr);
    }

    #[inline]
    pub fn set_pch(&self, val: u8) {
        let addr = (*self.reg.pc.borrow() & 0x00ff) | (u16::from(val) << 8);
        self.set_pc(addr);
    }

    #[inline]
    pub fn ir(&self) -> u8 {
        *self.reg.ir.borrow()
    }

    #[inline]
    pub fn set_a(&self, val: u8) {
        *self.reg.a.borrow_mut() = val;
    }

    #[inline]
    pub fn set_x(&self, val: u8) {
        *self.reg.x.borrow_mut() = val;
    }

    #[inline]
    pub fn set_y(&self, val: u8) {
        *self.reg.y.borrow_mut() = val;
    }

    #[inline]
    pub fn set_sp(&self, val: u8) {
        *self.reg.sp.borrow_mut() = val;
    }

    #[inline]
    pub fn set_ir(&self, val: u8) {
        *self.reg.ir.borrow_mut() = val;
    }

    #[inline]
    pub fn carry(&self) -> bool {
        (self.s() & 1) > 0
    }

    #[inline]
    pub fn negative(&self) -> bool {
        (self.s() & 0b1000_0000) > 0
    }

    #[inline]
    pub fn zero(&self) -> bool {
        (self.s() & 0b0000_0010) > 0
    }

    #[inline]
    pub fn overflow(&self) -> bool {
        (self.s() & 0b0100_0000) > 0
    }

    #[inline]
    pub fn interrupt_disable(&self) -> bool {
        (self.s() & 0b0000_0100) > 0
    }

    #[inline]
    pub fn decimal_mode(&self) -> bool {
        (self.s() & 0b0000_1000) > 0
    }

    #[inline]
    pub fn break_command(&self) -> bool {
        (self.s() & 0b0001_0000) > 0
    }

    pub fn set_negative(&self, val: bool) {
        *self.reg.s.borrow_mut() = self.s() & 0b0111_1111 | bool_to_bit(&val, 7);
    }

    pub fn set_zero(&self, val: bool) {
        *self.reg.s.borrow_mut() = self.s() & 0b1111_1101 | bool_to_bit(&val, 1)
    }

    pub fn set_carry(&self, val: bool) {
        *self.reg.s.borrow_mut() = self.s() & 0b1111_1110 | bool_to_bit(&val, 0)
    }

    pub fn set_overflow(&self, val: bool) {
        *self.reg.s.borrow_mut() = self.s() & 0b1011_1111 | bool_to_bit(&val, 6)
    }

    pub fn set_interrupt_disable(&self, val: bool) {
        *self.reg.s.borrow_mut() = self.s() & 0b1111_1011 | bool_to_bit(&val, 2)
    }

    pub fn set_decimal_mode(&self, val: bool) {
        *self.reg.s.borrow_mut() = self.s() & 0b1111_0111 | bool_to_bit(&val, 3)
    }

    pub fn set_break_command(&self, val: bool) {
        *self.reg.s.borrow_mut() = self.s() & 0b1110_1111 | bool_to_bit(&val, 4)
    }

    pub fn regs(&self) -> &Registers {
        &self.reg
    }
}
