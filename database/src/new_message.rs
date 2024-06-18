use crate::mailbox::Mailbox;
use crate::message::State;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NewMessage {
    pub mailbox: Mailbox,
    pub content: String,
    pub state: Option<State>,
}
