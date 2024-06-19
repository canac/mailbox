use crate::database::MailboxInfo;
use crate::filter::Filter;
use crate::message::{Message, State};
use crate::new_message::NewMessage;
use anyhow::Result;
use std::future::Future;

pub trait Backend {
    fn add_messages(
        &self,
        messages: Vec<NewMessage>,
    ) -> impl Future<Output = Result<Vec<Message>>> + Send;
    fn load_messages(&self, filter: Filter) -> impl Future<Output = Result<Vec<Message>>> + Send;
    fn change_state(
        &self,
        filter: Filter,
        new_state: State,
    ) -> impl Future<Output = Result<Vec<Message>>> + Send;
    fn delete_messages(&self, filter: Filter) -> impl Future<Output = Result<Vec<Message>>> + Send;
    fn load_mailboxes(
        &self,
        filter: Filter,
    ) -> impl Future<Output = Result<Vec<MailboxInfo>>> + Send;
}
