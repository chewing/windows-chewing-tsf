use std::{error::Error, fmt::Display};

pub(crate) mod event_loop;
pub(crate) mod gfx;
pub(crate) mod message_box;
pub(crate) mod window;

#[derive(Debug)]
pub(crate) struct UiError(pub(crate) String);
impl Error for UiError {}
impl Display for UiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UiError: {}", self.0)
    }
}
