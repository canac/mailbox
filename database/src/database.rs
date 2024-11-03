use crate::filter::Filter;
use crate::mailbox::Mailbox;
use crate::message::{Message, State};
use crate::new_message::NewMessage;
use crate::Backend;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

fn validate_message(message: &NewMessage) -> Result<()> {
    if message.content.is_empty() {
        bail!("content must not be empty");
    }

    Ok(())
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MailboxInfo {
    pub name: Mailbox,
    pub message_count: usize,
}

pub struct Database<B: Backend + Sized> {
    backend: B,
}

impl<B: Backend + Sized> Database<B> {
    // Create a new Database that uses the provided backend
    #[must_use]
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    // Add multiple new messages, returning the new messages
    pub async fn add_messages(&self, messages: Vec<NewMessage>) -> Result<Vec<Message>> {
        for message in &messages {
            validate_message(message)?;
        }

        self.backend.add_messages(messages).await
    }

    // Load all messages that match the filter
    pub async fn load_messages(&self, filter: Filter) -> Result<Vec<Message>> {
        self.backend.load_messages(filter).await
    }

    // Move messages that match the filter from their old state into new_state, returning the
    // modified messages
    pub async fn change_state(&self, filter: Filter, new_state: State) -> Result<Vec<Message>> {
        self.backend.change_state(filter, new_state).await
    }

    // Delete messages that match the filter, returning the deleted messages
    pub async fn delete_messages(&self, filter: Filter) -> Result<Vec<Message>> {
        self.backend.delete_messages(filter).await
    }

    // Given all messages that match the filter, determine the names and sizes of all mailboxes
    // used by those messages
    pub async fn load_mailboxes(&self, filter: Filter) -> Result<Vec<MailboxInfo>> {
        self.backend.load_mailboxes(filter).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate() {
        assert!(validate_message(&NewMessage {
            mailbox: "mailbox".try_into().unwrap(),
            content: String::new(),
            state: None,
        })
        .is_err());

        assert!(validate_message(&NewMessage {
            mailbox: "mailbox".try_into().unwrap(),
            content: String::from("message"),
            state: None,
        })
        .is_ok());
    }
}
