use std::rc::Rc;
use crate::debugger::{Debugger, NullDebugger};
use super::{Pin, PinStateChange};

pub trait Component: PinStateChange {
    fn get_pin(&self, name: &str) -> Option<&Pin>;
    fn set_debugger(&mut self, _debugger: Rc<dyn Debugger>) {}
}

pub trait ComponentLogic {
    fn debugger(&self) -> &dyn Debugger {
        &NullDebugger
    }
}
