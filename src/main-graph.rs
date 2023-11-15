use std::{
    cell::{OnceCell, RefCell},
    collections::HashMap,
    rc::Rc,
};

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum PinDirection {
    Input,
    Output,
}

#[derive(Clone)]
pub struct Pin {
    name: String,
    value: RefCell<bool>,
    direction: RefCell<PinDirection>,
    handler: OnceCell<Rc<RefCell<dyn PinStateChange>>>,
    inner_id: OnceCell<u32>,
}

impl Pin {
    pub fn new(name: &str, direction: PinDirection) -> Self {
        Pin {
            name: name.to_string(),
            value: RefCell::new(false),
            direction: RefCell::new(direction),
            handler: OnceCell::new(),
            inner_id: OnceCell::new(),
        }
    }

    pub fn input(name: &str) -> Self {
        Pin::new(name, PinDirection::Input)
    }

    pub fn output(name: &str) -> Self {
        Pin::new(name, PinDirection::Output)
    }

    pub fn read(&self) -> bool {
        self.state()
    }

    pub fn val(&self) -> u8 {
        self.state().into()
    }

    pub fn state(&self) -> bool {
        *self.value.borrow()
    }

    pub fn direction(&self) -> PinDirection {
        *self.direction.borrow()
    }

    pub fn write(&self, val: bool) {
        if self.is_output() {
            *self.value.borrow_mut() = val;
            if let Some(handler) = self.handler.get() {
                handler.borrow_mut().on_pin_state_change(self);
            }
        }
    }

    pub fn set_val(&self, val: bool) {
        if !self.is_output() {
            *self.value.borrow_mut() = val;
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_output(&self) -> bool {
        self.direction() == PinDirection::Output
    }

    pub fn toggle(&self) {
        if self.is_output() {
            let v = self.state();
            self.write(!v);
        }
    }

    pub(crate) fn set_inner_id(&self, id: u32) {
        let _ = self.inner_id.set(id);
    }

    pub(crate) fn set_handler(&self, handler: Rc<RefCell<dyn PinStateChange>>) {
        let _ = self
            .handler
            .set(handler)
            .map_err(|_| panic!("Handler already defined"));
    }
}

pub trait PinStateChange {
    fn on_pin_state_change(&mut self, pin: &Pin);
}

pub struct Clock {
    out: Pin,
}

impl Clock {
    pub fn new() -> Self {
        Clock {
            out: Pin::output("out"),
        }
    }

    pub fn tick(&self) {
        self.out.toggle();
    }
}

impl Component for Clock {
    fn get_pin(&self, name: &str) -> Option<&Pin> {
        match name {
            "out" => Some(&self.out),
            _ => None,
        }
    }
}

impl PinStateChange for Clock {
    fn on_pin_state_change(&mut self, pin: &Pin) {
        todo!()
    }
}

pub struct Cpu {
    phi2: Pin,
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            phi2: Pin::output("phi2"),
        }
    }

    pub fn tick(&mut self) {
        println!("CPU is ticking!");
    }
}

impl Component for Cpu {
    fn get_pin(&self, name: &str) -> Option<&Pin> {
        match name {
            "phi2" => Some(&self.phi2),
            _ => None,
        }
    }
}

impl PinStateChange for Cpu {
    fn on_pin_state_change(&mut self, pin: &Pin) {
        self.tick();
    }
}

pub trait Component: PinStateChange {
    fn get_pin(&self, name: &str) -> Option<&Pin>;
}

pub struct Circuit {
    pins: HashMap<u32, (String, String)>,
    connections: HashMap<u32, u32>,
    components: HashMap<String, Box<dyn Component>>,
}

impl Circuit {
    pub fn component(&self, name: &str) -> &Box<dyn Component> {
        self.components
            .get(name)
            .expect(&format!("Component {name} doesn't exist"))
    }
}

struct CircuitPinHandler<'a>(&'a Circuit);
impl PinStateChange for CircuitPinHandler<'_> {
    fn on_pin_state_change(&mut self, pin: &Pin) {
        let id = pin.inner_id.get().unwrap();

        let (component_id, reader_pin_name) = {
            let circuit = &self.0;
            println!("Changing pin: {}, {}", pin.name, id);
            let reader_id = circuit.connections[id];
            circuit.pins[&reader_id].clone()
        };

        let rpin = {
            let c = &self.0.components[&component_id];
            let p = c.get_pin(&reader_pin_name).unwrap();
            p.set_val(pin.state());
            p.clone()
        };

        // let component = self.0.components.get_mut(&component_id).unwrap();
        // println!(
        //     "Reader pin: {}, {}",
        //     rpin.name(),
        //     rpin.inner_id.get().unwrap()
        // );
        //
        // println!("Updating compoent {}", component_id);
        // component.on_pin_state_change(&rpin);
    }
}

struct CircuitBuilder {
    components: Option<HashMap<String, Box<dyn Component>>>,
    pins: HashMap<u32, (String, String)>,
    last_pin_id: u32,
    connections: HashMap<u32, u32>,
}

impl CircuitBuilder {
    pub fn new() -> Self {
        CircuitBuilder {
            components: Some(HashMap::new()),
            pins: HashMap::new(),
            last_pin_id: 0,
            connections: HashMap::new(),
        }
    }

    pub fn add_component(&mut self, name: &str, cmp: impl Component + 'static) -> &mut Self {
        self.components
            .as_mut()
            .unwrap()
            .insert(name.to_string(), Box::new(cmp));
        self
    }

    fn add_pin(&mut self, component_name: &str, pin_name: &str) -> u32 {
        self.pins.insert(
            self.last_pin_id,
            (component_name.to_string(), pin_name.to_string()),
        );
        self.last_pin_id += 1;
        self.last_pin_id - 1
    }

    fn add_connection(&mut self, writer_id: u32, reader_id: u32) {
        self.connections.insert(writer_id, reader_id);
    }

    pub fn link(
        &mut self,
        writer_name: &str,
        writer_pin_name: &str,
        reader_name: &str,
        reader_pin_name: &str,
    ) -> &mut Self {
        let writer_id = self.add_pin(writer_name, writer_pin_name);
        let reader_id = self.add_pin(reader_name, reader_pin_name);
        self.add_connection(writer_id, reader_id);

        self
    }

    pub fn build(&mut self) -> Circuit {
        let c = Circuit {
            pins: self.pins.clone(),
            connections: self.connections.clone(),
            components: self.components.take().unwrap(),
        };

        // let h: CircuitPinHandler<'static> = CircuitPinHandler(&c);
        // let handler = Rc::new(RefCell::new(h));
        //
        // c.connections.iter().for_each(|(key, rkey)| {
        //     let data = &c.pins[key];
        //     let component = &c.components[&data.0];
        //     let pin = component.get_pin(&data.1).unwrap();
        //     pin.set_handler(Rc::clone(&handler) as Rc<RefCell<dyn PinStateChange>>);
        //     pin.set_inner_id(*key);
        //
        //     let data = &c.pins[rkey];
        //     let component = &c.components[&data.0];
        //     let pin = component.get_pin(&data.1).unwrap();
        //     pin.set_inner_id(*rkey);
        // });

        c
    }
}

fn init<'a>(c: &'a mut Circuit) -> &'a Circuit {
    // let h: CircuitPinHandler<'static> = CircuitPinHandler(&c);
    let h = CircuitPinHandler(c);
    let handler = Rc::new(RefCell::new(h));

    c.connections.iter().for_each(|(key, rkey)| {
        let data = &c.pins[key];
        let component = &c.components[&data.0];
        let pin = component.get_pin(&data.1).unwrap();
        let x = Rc::clone(&handler);
        pin.set_handler(x);
        pin.set_inner_id(*key);

        let data = &c.pins[rkey];
        let component = &c.components[&data.0];
        let pin = component.get_pin(&data.1).unwrap();
        pin.set_inner_id(*rkey);
    });

    c
}

fn main() {
    let mut c = CircuitBuilder::new();
    c.add_component("X1", Clock::new())
        .add_component("U1", Cpu::new())
        .link("X1", "out", "U1", "phi2");

    let mut circuit = c.build();
    init(&mut circuit).component("X1").get_pin("out").unwrap().toggle();
}
