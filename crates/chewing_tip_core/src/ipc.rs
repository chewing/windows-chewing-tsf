use std::{error::Error, fmt::Display};

pub mod client;
pub mod messages;
pub mod named_pipe;
pub mod varlink;

#[derive(Debug)]
pub struct IpcError(String);

impl Error for IpcError {}
impl Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IpcError: {}", self.0)
    }
}
