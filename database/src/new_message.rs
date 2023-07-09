use crate::mailbox::Mailbox;
use crate::message::State;
use serde::Deserialize;

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NewMessage {
    pub mailbox: Mailbox,
    pub content: String,
    pub state: Option<State>,
}
