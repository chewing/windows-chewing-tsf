use error_plus::impl_context_error;

pub(crate) mod event_loop;
pub(crate) mod gfx;
pub(crate) mod message_box;
pub(crate) mod window;

impl_context_error!(pub UiError);
