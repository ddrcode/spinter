use crate::debugger::DebugMessage;

use super::Component;
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ops::{Range};
use std::thread;
use std::time::Duration;

const CIRCUIT_MESSAGE_CAP: usize = 100;
const COMPONENT_MESSAGE_CAP: usize = 100;

//--------------------------------------------------------------------
// PinMessage
#[derive(Default, Debug)]
pub struct PinMessage {
    pub component: String,
    pub pin: String,
    pub val: bool,
}

impl PinMessage {
    pub fn new(component: &str, pin: &str, val: bool) -> Self {
        PinMessage {
            component: component.to_string(),
            pin: pin.to_string(),
            val,
        }
    }
}
//--------------------------------------------------------------------
// CircuitCtx

#[derive(Clone, Debug)]
pub struct CircuitCtx {
    pub(crate) component_name: String,
    pub(crate) sender: Sender<PinMessage>,
    pub(crate) receiver: Receiver<PinMessage>,
}

impl CircuitCtx {
    pub fn new(
        component_name: &str,
        sender: Sender<PinMessage>,
        receiver: Receiver<PinMessage>,
    ) -> Self {
        CircuitCtx {
            component_name: component_name.to_string(),
            sender,
            receiver,
        }
    }

    pub fn debug(&self, msg: DebugMessage) {
        println!("{}", msg);
    }
}

impl Default for CircuitCtx {
    fn default() -> Self {
        let (sender, receiver) = unbounded();
        Self {
            component_name: Default::default(),
            sender,
            receiver,
        }
    }
}

//--------------------------------------------------------------------
// CircuitBuilder

pub struct CircuitBuilder {
    components: HashMap<String, Box<dyn Component>>,
    links: HashMap<String, HashMap<String, HashSet<(String, String)>>>,
}

impl CircuitBuilder {
    pub fn new() -> Self {
        CircuitBuilder {
            components: HashMap::new(),
            links: HashMap::new(),
        }
    }

    pub fn add_component(&mut self, name: &str, comp: impl Component + 'static) -> &mut Self {
        self.components.insert(name.to_string(), Box::new(comp));

        self
    }

    pub fn link(
        &mut self,
        writer_name: &str,
        writer_pin_name: &str,
        reader_name: &str,
        reader_pin_name: &str,
    ) -> &mut Self {
        if !self.links.contains_key(writer_name) {
            self.links.insert(writer_name.to_string(), HashMap::new());
        }

        if !self.links[writer_name].contains_key(writer_pin_name) {
            self.links
                .get_mut(writer_name)
                .unwrap()
                .insert(writer_pin_name.to_string(), HashSet::new());
        }

        self.links
            .get_mut(writer_name)
            .unwrap()
            .get_mut(writer_pin_name)
            .unwrap()
            .insert((reader_name.to_string(), reader_pin_name.to_string()));

        self
    }

    pub fn link_range(
        &mut self,
        writer_name: &str,
        writer_pin_prefix: &str,
        reader_name: &str,
        reader_name_prefix: &str,
        range: Range<u8>,
    ) -> &mut Self {
        for i in range {
            self.link(
                &writer_name,
                &format!("{}{}", writer_pin_prefix, i),
                reader_name,
                &format!("{}{}", reader_name_prefix, i),
            );
        }
        self
    }

    pub fn build(&mut self) -> Circuit {
        let mut components: HashMap<String, CircuitNode> =
            HashMap::with_capacity(self.components.len());
        let (sender, receiver) = bounded::<PinMessage>(CIRCUIT_MESSAGE_CAP);

        for (name, mut comp) in self.components.drain() {
            let (s, r) = bounded::<PinMessage>(COMPONENT_MESSAGE_CAP);
            let ctx = CircuitCtx::new(&name, sender.clone(), r);
            let links = self.links.remove(&name).unwrap_or(HashMap::new());

            for pin in links.keys() {
                comp.get_pin(pin).unwrap().set_context(ctx.clone());
            }

            let tid = thread::Builder::new()
                .name(name.clone())
                .spawn(move || {
                    comp.attach(ctx);
                    thread::sleep(Duration::from_millis(500));
                    comp.init();
                    loop {
                        let msg_res = comp.ctx().receiver.recv();
                        if let Err(e) = msg_res {
                            println!(
                                "Reading msg failed in {} with {:?}",
                                comp.ctx().component_name,
                                e
                            );
                        }
                        let msg = msg_res.unwrap();
                        let pin = comp.get_pin(&msg.pin).unwrap();
                        if pin.state() != msg.val {
                            pin.set_val(msg.val);
                            comp.on_pin_state_change(&msg.pin, msg.val);
                        }
                    }
                })
                .unwrap();

            components.insert(
                name.to_string(),
                CircuitNode {
                    sender: s,
                    links,
                    thread_handle: Some(tid),
                },
            );
        }

        Circuit {
            components,
            receiver,
            sender,
            state: RefCell::new(false),
        }
    }
}

//--------------------------------------------------------------------
// Circuit

pub struct CircuitNode {
    pub(crate) sender: Sender<PinMessage>,
    pub(crate) links: HashMap<String, HashSet<(String, String)>>,
    pub(crate) thread_handle: Option<thread::JoinHandle<()>>,
}

pub struct Circuit {
    pub(crate) components: HashMap<String, CircuitNode>,
    pub(crate) receiver: Receiver<PinMessage>,
    pub(crate) sender: Sender<PinMessage>,
    // FIXME use Oscilator (X1) instead
    state: RefCell<bool>,
}

impl Circuit {
    pub fn tick(&self) {
        let val = *self.state.borrow();
        self.sender.send(PinMessage::new("X1", "OUT", val)).unwrap();
        *self.state.borrow_mut() = !val;
    }
}
