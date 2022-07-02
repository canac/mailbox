use anyhow::anyhow;
use rusqlite::{Result, Row};
use sea_query::{enum_def, Value};

pub enum MessageState {
    Unread,
    Read,
    Archived,
}

impl TryFrom<i64> for MessageState {
    type Error = anyhow::Error;

    fn try_from(value: i64) -> anyhow::Result<Self> {
        match value {
            0 => Ok(MessageState::Unread),
            1 => Ok(MessageState::Read),
            2 => Ok(MessageState::Archived),
            _ => Err(anyhow!("Invalid message state {}", value)),
        }
    }
}

impl From<MessageState> for i64 {
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
        Value::BigInt(Some(value.into()))
    }
}

#[enum_def]
pub struct Message {
    pub id: i32,
    pub timestamp: chrono::NaiveDateTime,
    pub mailbox: String,
    pub content: String,
    pub state: MessageState,
}

impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let marker = match self.state {
            MessageState::Unread => "*",
            MessageState::Read => " ",
            MessageState::Archived => "-",
        };
        write!(
            f,
            "{marker} {} [{}] @ {}",
            self.content, self.mailbox, self.timestamp
        )
    }
}

impl Message {
    pub fn from_row(row: &Row) -> Result<Message> {
        Ok(Message {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            mailbox: row.get(2)?,
            content: row.get(3)?,
            state: row.get::<_, i64>(4)?.try_into().unwrap(),
        })
    }
}
