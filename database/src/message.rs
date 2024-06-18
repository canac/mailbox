use anyhow::anyhow;
use sea_query::{enum_def, Value};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqliteRow, FromRow, Row};
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use crate::Mailbox;

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

pub type Id = i32;

#[derive(Clone, Deserialize, Serialize)]
#[enum_def]
pub struct Message {
    pub id: Id,
    pub timestamp: chrono::NaiveDateTime,
    pub mailbox: Mailbox,
    pub content: String,
    pub state: State,
}

impl FromRow<'_, SqliteRow> for Message {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            timestamp: row.try_get("timestamp")?,
            mailbox: row.try_get::<String, _>("mailbox")?.try_into().unwrap(),
            content: row.try_get("content")?,
            state: row.try_get::<i32, _>("state")?.try_into().unwrap(),
        })
    }
}
