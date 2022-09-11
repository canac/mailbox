use self::sea_query_driver_rusqlite::RusqliteValues;
use crate::message::{Message, MessageIden, MessageState};
use crate::message_filter::MessageFilter;
use crate::new_message::NewMessage;
use anyhow::{bail, Context, Result};
use rusqlite::Connection;
use sea_query::{ColumnDef, Expr, Order, Query, SqliteQueryBuilder, Table, Value};

sea_query::sea_query_driver_rusqlite!();

pub struct Database {
    connection: Connection,
}

impl Database {
    // Create a new Database instance
    // An in-memory database is used if a database path isn't provided
    pub fn new(db_path: Option<std::path::PathBuf>) -> Result<Self> {
        let connection = match db_path {
            Some(path) => Connection::open(path),
            None => Connection::open_in_memory(),
        }
        .context("Failed to open database")?;
        let mut db = Database { connection };
        db.init()?;
        Ok(db)
    }

    // Initialize the database and create the necessary tables
    pub fn init(&mut self) -> Result<()> {
        let sql = Table::create()
            .table(MessageIden::Table)
            .if_not_exists()
            .col(
                ColumnDef::new(MessageIden::Id)
                    .integer()
                    .not_null()
                    .auto_increment()
                    .primary_key(),
            )
            .col(
                ColumnDef::new(MessageIden::Timestamp)
                    .date_time()
                    .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
            )
            .col(
                ColumnDef::new(MessageIden::Mailbox)
                    .string()
                    .not_null()
                    .extra("CHECK (LENGTH(mailbox) > 0)".to_string()),
            )
            .col(
                ColumnDef::new(MessageIden::Content)
                    .string()
                    .not_null()
                    .extra("CHECK (LENGTH(content) > 0)".to_string()),
            )
            .col(
                ColumnDef::new(MessageIden::State)
                    .integer()
                    .not_null()
                    .default(Value::Int(Some(0)))
                    .extra("CHECK (state >= 0 AND state <= 2)".to_string()),
            )
            .build(SqliteQueryBuilder);
        self.connection
            .execute(sql.as_str(), [])
            .context("Failed to create database tables")?;
        Ok(())
    }

    // Add a new message to a particular mailbox, returning the new message
    pub fn add_message(&mut self, message: NewMessage) -> Result<Message> {
        Self::validate_message(&message)?;

        let (sql, values) = Query::insert()
            .into_table(MessageIden::Table)
            .columns([
                MessageIden::Mailbox,
                MessageIden::Content,
                MessageIden::State,
            ])
            .values(vec![
                message.mailbox.into(),
                message.content.into(),
                message.state.unwrap_or(MessageState::Unread).into(),
            ])?
            .returning_all()
            .build(SqliteQueryBuilder);

        let message = self
            .connection
            .query_row(
                sql.as_str(),
                RusqliteValues::from(values).as_params().as_slice(),
                Message::from_row,
            )
            .context("Failed to add message")?;
        Ok(message)
    }

    // Load messages, applying the provided filters
    pub fn load_messages(&mut self, filter: &MessageFilter) -> Result<Vec<Message>> {
        let (sql, values) = Query::select()
            .expr(Expr::asterisk())
            .from(MessageIden::Table)
            .cond_where(filter.get_where())
            .order_by(MessageIden::Timestamp, Order::Asc)
            .build(SqliteQueryBuilder);

        let mut statement = self.connection.prepare(sql.as_str())?;
        let messages = statement
            .query_map(
                RusqliteValues::from(values).as_params().as_slice(),
                Message::from_row,
            )?
            .collect::<Result<Vec<Message>, _>>()
            .context("Failed to load messages")?;
        Ok(messages)
    }

    // Move messages from their old state into new_state
    // Only load messages in the specified mailbox if mailbox_filter is provided
    // Only load messages in one of the specified states if states_filter is provided
    pub fn change_state(
        &mut self,
        filter: &MessageFilter,
        new_state: MessageState,
    ) -> Result<Vec<Message>> {
        let (sql, values) = Query::update()
            .table(MessageIden::Table)
            .cond_where(filter.get_where())
            .value(MessageIden::State, new_state.into())
            .returning_all()
            .build(SqliteQueryBuilder);

        let mut statement = self.connection.prepare(sql.as_str())?;
        let mut messages = statement
            .query_map(
                RusqliteValues::from(values).as_params().as_slice(),
                Message::from_row,
            )?
            .collect::<Result<Vec<Message>, _>>()
            .context("Failed to change message states")?;
        messages.sort_by_key(|message| message.timestamp);
        Ok(messages)
    }

    // Delete messages, applying the provided filters
    pub fn delete_messages(&mut self, filter: &MessageFilter) -> Result<Vec<Message>> {
        let (sql, values) = Query::delete()
            .from_table(MessageIden::Table)
            .returning_all()
            .cond_where(filter.get_where())
            .build(SqliteQueryBuilder);

        let mut statement = self.connection.prepare(sql.as_str())?;
        let mut messages = statement
            .query_map(
                RusqliteValues::from(values).as_params().as_slice(),
                Message::from_row,
            )?
            .collect::<Result<Vec<Message>, _>>()
            .context("Failed to clear messages")?;
        messages.sort_by_key(|message| message.timestamp);
        Ok(messages)
    }

    // Return an error if the new message invalid
    fn validate_message(message: &NewMessage) -> Result<()> {
        if message.content.is_empty() {
            bail!("content must not be empty");
        }
        if message.mailbox.is_empty() {
            bail!("mailbox must not be empty");
        }
        if message.mailbox.starts_with('/') {
            bail!("mailbox must not start with /");
        }
        if message.mailbox.ends_with('/') {
            bail!("mailbox must not end with /");
        }
        if message.mailbox.contains("//") {
            bail!("mailbox must not contain //");
        }
        if message.mailbox.contains('*') {
            bail!("mailbox must not contain *");
        }
        if message.mailbox.contains('?') {
            bail!("mailbox must not contain ?");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;

    fn add_message(
        db: &mut Database,
        mailbox: &str,
        content: &str,
        state: Option<MessageState>,
    ) -> Result<()> {
        db.add_message(NewMessage {
            mailbox: mailbox.to_string(),
            content: content.to_string(),
            state,
        })?;
        Ok(())
    }

    fn get_populated_db() -> Result<Database> {
        let mut db = Database::new(None)?;
        add_message(&mut db, "unread", "unread1", Some(MessageState::Unread))?;
        add_message(&mut db, "unread", "unread2", Some(MessageState::Unread))?;
        add_message(&mut db, "read", "read1", Some(MessageState::Read))?;
        add_message(&mut db, "read", "read2", Some(MessageState::Read))?;
        add_message(&mut db, "read", "read3", Some(MessageState::Read))?;
        add_message(
            &mut db,
            "archived",
            "archive1",
            Some(MessageState::Archived),
        )?;
        Ok(db)
    }

    #[test]
    fn test_create() -> Result<()> {
        Database::new(None)?;
        Ok(())
    }

    #[test]
    fn test_add() -> Result<()> {
        let mut db = Database::new(None)?;
        add_message(&mut db, "mailbox1", "message1", None)?;
        add_message(&mut db, "mailbox2", "message2", None)?;
        add_message(&mut db, "mailbox1", "message3", None)?;
        assert_eq!(db.load_messages(&MessageFilter::new())?.len(), 3);

        let messages = db.load_messages(&MessageFilter::new().with_mailbox("mailbox1"))?;
        assert_eq!(messages[0].mailbox, "mailbox1");
        assert_eq!(messages[0].content, "message1");
        assert_eq!(messages[1].mailbox, "mailbox1");
        assert_eq!(messages[1].content, "message3");
        assert_eq!(messages.len(), 2);

        let messages = db.load_messages(&MessageFilter::new().with_mailbox("mailbox2"))?;
        assert_eq!(messages[0].mailbox, "mailbox2");
        assert_eq!(messages[0].content, "message2");
        assert_eq!(messages.len(), 1);

        Ok(())
    }

    #[test]
    fn test_add_invalid() -> Result<()> {
        let mut db = Database::new(None)?;
        assert!(add_message(&mut db, "mailbox", "", None).is_err());
        assert!(add_message(&mut db, "", "message", None).is_err());
        assert!(add_message(&mut db, "mailbox/", "message", None).is_err());
        assert!(add_message(&mut db, "/mailbox", "message", None).is_err());
        assert!(add_message(&mut db, "parent//child", "message", None).is_err());
        assert!(add_message(&mut db, "parent/*", "message", None).is_err());
        assert!(add_message(&mut db, "parent/?", "message", None).is_err());
        Ok(())
    }

    #[test]
    fn test_load() -> Result<()> {
        let mut db = get_populated_db()?;
        assert_eq!(db.load_messages(&MessageFilter::new())?.len(), 6);
        Ok(())
    }

    #[test]
    fn test_load_with_mailbox_filter() -> Result<()> {
        let mut db = get_populated_db()?;
        assert_eq!(
            db.load_messages(&MessageFilter::new().with_mailbox("unread"))?
                .len(),
            2
        );
        Ok(())
    }

    #[test]
    fn test_load_with_states_filter() -> Result<()> {
        let mut db = get_populated_db()?;
        assert_eq!(
            db.load_messages(
                &MessageFilter::new().with_states(vec![MessageState::Read, MessageState::Archived])
            )?
            .len(),
            4
        );
        Ok(())
    }

    #[test]
    fn test_load_with_sub_mailbox_filters() -> Result<()> {
        let mut db = get_populated_db()?;
        add_message(&mut db, "a", "message", None)?;
        add_message(&mut db, "ab", "message", None)?;
        add_message(&mut db, "a/b", "message", None)?;
        add_message(&mut db, "a/c", "message", None)?;
        add_message(&mut db, "a/b/c", "message", None)?;
        add_message(&mut db, "a/c/b", "message", None)?;
        assert_eq!(
            db.load_messages(&MessageFilter::new().with_mailbox("a"))?
                .len(),
            5
        );
        assert_eq!(
            db.load_messages(&MessageFilter::new().with_mailbox("a/b"))?
                .len(),
            2
        );
        assert_eq!(
            db.load_messages(&MessageFilter::new().with_mailbox("a/b/c"))?
                .len(),
            1
        );
        Ok(())
    }

    #[test]
    fn test_read() -> Result<()> {
        let mut db = get_populated_db()?;
        db.change_state(
            &MessageFilter::new().with_states(vec![MessageState::Unread]),
            MessageState::Read,
        )?;
        assert_eq!(
            db.load_messages(&MessageFilter::new().with_states(vec![MessageState::Read]))?
                .len(),
            5
        );
        Ok(())
    }

    #[test]
    fn test_archive() -> Result<()> {
        let mut db = get_populated_db()?;
        db.change_state(
            &MessageFilter::new().with_states(vec![MessageState::Unread, MessageState::Read]),
            MessageState::Archived,
        )?;
        assert_eq!(
            db.load_messages(&MessageFilter::new().with_states(vec![MessageState::Archived]))?
                .len(),
            6
        );
        Ok(())
    }

    #[test]
    fn test_delete() -> Result<()> {
        let mut db = get_populated_db()?;
        db.delete_messages(
            &MessageFilter::new().with_states(vec![MessageState::Unread, MessageState::Read]),
        )?;
        assert_eq!(db.load_messages(&MessageFilter::new())?.len(), 1);
        Ok(())
    }
}
