use super::{CircuitCtx, Pin, PinMessage};

pub trait Component: Send {
    fn ctx(&self) -> &CircuitCtx;
    fn attach(&mut self, env: CircuitCtx);
    fn init(&mut self) {
        println!("Initializng {}", self.ctx().component_name);
    }
    fn on_pin_state_change(&mut self, pin_name: &str, val: bool);
    fn write(&self, pin: &str, val: bool) {
        self.ctx()
            .sender
            .send(PinMessage::new(&self.ctx().component_name, pin, val))
            .unwrap();
    }
    fn get_pin(&self, name: &str) -> Option<&Pin>;
}
