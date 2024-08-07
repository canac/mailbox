use crate::Mailbox;
use anyhow::anyhow;
use sea_query::{enum_def, Value};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Unread,
    Read,
    Archived,
}

impl Display for State {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            State::Unread => "unread",
            State::Read => "read",
            State::Archived => "archived",
        })
    }
}

impl TryFrom<u32> for State {
    type Error = anyhow::Error;

    fn try_from(value: u32) -> anyhow::Result<Self> {
        match value {
            0 => Ok(State::Unread),
            1 => Ok(State::Read),
            2 => Ok(State::Archived),
            _ => Err(anyhow!("Invalid message state {}", value)),
        }
    }
}

impl FromStr for State {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "unread" => Ok(State::Unread),
            "read" => Ok(State::Read),
            "archived" => Ok(State::Archived),
            _ => Err(anyhow!("Invalid message state {}", value)),
        }
    }
}

impl From<State> for u32 {
    fn from(value: State) -> Self {
        match value {
            State::Unread => 0,
            State::Read => 1,
            State::Archived => 2,
        }
    }
}

impl From<State> for Value {
    fn from(value: State) -> Value {
        Value::Unsigned(Some(value.into()))
    }
}

pub type Id = u32;

#[derive(Clone, Deserialize, Serialize, sqlx::FromRow)]
#[enum_def]
pub struct Message {
    pub id: Id,
    pub timestamp: chrono::NaiveDateTime,
    #[sqlx(try_from = "String")]
    pub mailbox: Mailbox,
    pub content: String,
    #[sqlx(try_from = "u32")]
    pub state: State,
}
