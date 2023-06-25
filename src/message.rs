use anyhow::anyhow;
use clap::ValueEnum;
use sea_query::{enum_def, Value};
use serde::Deserialize;
use sqlx::{any::AnyRow, FromRow, Row};
use std::fmt::{self, Display, Formatter};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum MessageState {
    Unread,
    Read,
    Archived,
}

impl Display for MessageState {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                MessageState::Unread => "*",
                MessageState::Read => " ",
                MessageState::Archived => "-",
            }
        )
    }
}

impl TryFrom<i32> for MessageState {
    type Error = anyhow::Error;

    fn try_from(value: i32) -> anyhow::Result<Self> {
        match value {
            0 => Ok(MessageState::Unread),
            1 => Ok(MessageState::Read),
            2 => Ok(MessageState::Archived),
            _ => Err(anyhow!("Invalid message state {}", value)),
        }
    }
}

impl From<MessageState> for i32 {
    fn from(value: MessageState) -> Self {
        match value {
            MessageState::Unread => 0,
            MessageState::Read => 1,
            MessageState::Archived => 2,
        }
    }
}

impl From<MessageState> for Value {
    fn from(value: MessageState) -> Value {
        Value::Int(Some(value.into()))
    }
}

#[derive(Clone)]
#[enum_def]
pub struct Message {
    pub id: i32,
    pub timestamp: chrono::NaiveDateTime,
    pub mailbox: String,
    pub content: String,
    pub state: MessageState,
}

impl FromRow<'_, AnyRow> for Message {
    fn from_row(row: &AnyRow) -> sqlx::Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            timestamp: row.try_get("timestamp")?,
            mailbox: row.try_get("mailbox")?,
            content: row.try_get("content")?,
            state: row.try_get::<i32, _>("state")?.try_into().unwrap(),
        })
    }
}
