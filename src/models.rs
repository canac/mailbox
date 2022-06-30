use rusqlite::{Result, Row};
use sea_query::enum_def;

#[enum_def]
pub struct Message {
    pub id: i32,
    pub timestamp: chrono::NaiveDateTime,
    pub mailbox: String,
    pub content: String,
    pub read: bool,
}

impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let unread_marker = if self.read { " " } else { "*" };
        write!(
            f,
            "{unread_marker} {} [{}] @ {}",
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
            read: row.get(4)?,
        })
    }
}
