use super::{Operation, PinsState, MemCell};
use std::fmt;

#[derive(Debug)]
#[non_exhaustive]
pub enum DebugMessage {
    CpuOperation(Operation),
    PinsState(PinsState),
    MemCellUpdate(MemCell)
}

impl fmt::Display for DebugMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use DebugMessage::*;
        match self {
            CpuOperation(o) => write!(f, "{o}"),
            PinsState(state) => write!(f, "{state}"),
            _ => write!(f, "{:?}", self),
        }
    }
}
