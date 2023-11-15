use super::OperationDebug;
use std::fmt;

pub enum DebugMessage {
    CpuOperation(OperationDebug)
}

impl fmt::Display for DebugMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use DebugMessage::*;
        match self {
            CpuOperation(o) => write!(f, "{}", o)
        }
    }
}
