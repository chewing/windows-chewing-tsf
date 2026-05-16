use error_plus::impl_context_error;

pub mod client;
pub mod messages;
pub mod named_pipe;
pub mod values;
pub mod varlink;

impl_context_error!(pub IpcError);
