use crate::message::MessageState;
use serde::Deserialize;

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NewMessage {
    pub mailbox: String,
    pub content: String,
    pub state: Option<MessageState>,
}
