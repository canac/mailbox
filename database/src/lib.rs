#![deny(clippy::pedantic)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::missing_errors_doc
)]

mod backend;
mod database;
mod http_backend;
mod mailbox;
mod message;
mod message_filter;
mod new_message;
mod sqlite_backend;

pub use crate::backend::Backend;
pub use crate::database::{Database, MailboxInfo};
pub use crate::http_backend::HttpBackend;
pub use crate::mailbox::Mailbox;
pub use crate::message::{Message, State};
pub use crate::message_filter::MessageFilter;
pub use crate::new_message::NewMessage;
pub use crate::sqlite_backend::SqliteBackend;
