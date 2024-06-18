use crate::database::MailboxInfo;
use crate::message::{Message, State};
use crate::message_filter::MessageFilter;
use crate::new_message::NewMessage;
use anyhow::Result;
use std::future::Future;

pub trait Backend {
    fn add_messages(
        &self,
        messages: Vec<NewMessage>,
    ) -> impl Future<Output = Result<Vec<Message>>> + Send;
    fn load_messages(
        &self,
        filter: MessageFilter,
    ) -> impl Future<Output = Result<Vec<Message>>> + Send;
    fn change_state(
        &self,
        filter: MessageFilter,
        new_state: State,
    ) -> impl Future<Output = Result<Vec<Message>>> + Send;
    fn delete_messages(
        &self,
        filter: MessageFilter,
    ) -> impl Future<Output = Result<Vec<Message>>> + Send;
    fn load_mailboxes(
        &self,
        filter: MessageFilter,
    ) -> impl Future<Output = Result<Vec<MailboxInfo>>> + Send;
}
