#![deny(clippy::pedantic)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::missing_errors_doc
)]

mod database;
mod message;
mod message_filter;
mod new_message;

pub use crate::database::{Database, Engine};
pub use crate::message::{Message, State};
pub use crate::message_filter::MessageFilter;
pub use crate::new_message::NewMessage;
