use std::cell::RefCell;

pub struct DebuggerState {
    enabled: RefCell<bool>
}

impl DebuggerState {
    pub fn enabled(&self) -> bool {
        *self.enabled.borrow()
    }

    pub fn set_enabled(&self, val: bool) {
        *self.enabled.borrow_mut() = val;
    }
}

impl Default for DebuggerState {
    fn default() -> Self {
        Self { enabled: RefCell::new(true) }
    }
}
