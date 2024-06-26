#![deny(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

mod backend;
mod database;
mod filter;
mod http_backend;
mod mailbox;
mod message;
mod new_message;
mod sqlite_backend;

pub use crate::backend::Backend;
pub use crate::database::{Database, MailboxInfo};
pub use crate::filter::Filter;
pub use crate::http_backend::HttpBackend;
pub use crate::mailbox::Mailbox;
pub use crate::message::{Message, State};
pub use crate::new_message::NewMessage;
pub use crate::sqlite_backend::SqliteBackend;
