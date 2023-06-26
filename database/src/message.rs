use anyhow::anyhow;
use sea_query::{enum_def, Value};
use serde::Deserialize;
use sqlx::{any::AnyRow, FromRow, Row};
use std::fmt::{self, Display, Formatter};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Unread,
    Read,
    Archived,
}

impl Display for State {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                State::Unread => "*",
                State::Read => " ",
                State::Archived => "-",
            }
        )
    }
}

impl TryFrom<i32> for State {
    type Error = anyhow::Error;

    fn try_from(value: i32) -> anyhow::Result<Self> {
        match value {
            0 => Ok(State::Unread),
            1 => Ok(State::Read),
            2 => Ok(State::Archived),
            _ => Err(anyhow!("Invalid message state {}", value)),
        }
    }
}

impl From<State> for i32 {
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
    pub state: State,
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
