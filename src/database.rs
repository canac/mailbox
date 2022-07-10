use self::sea_query_driver_rusqlite::RusqliteValues;
use crate::message_filter::MessageFilter;
use crate::models::{Message, MessageIden, MessageState};
use anyhow::{Context, Result};
use rusqlite::Connection;
use sea_query::{Alias, ColumnDef, Expr, Func, Order, Query, SqliteQueryBuilder, Table, Value};

sea_query::sea_query_driver_rusqlite!();

#[derive(Debug, PartialEq, Eq)]
pub struct MailboxSummary {
    pub mailbox: String,
    pub count: i64,
    pub unread: i64,
    pub read: i64,
    pub archived: i64,
}

impl std::fmt::Display for MailboxSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use colored::*;

        write!(
            f,
            "{}: {} ({}/{}/{})",
            self.mailbox.bold().green(),
            self.count,
            self.unread.to_string().bold().red(),
            self.read,
            self.archived
        )
    }
}

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
        .context("Error opening database")?;
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
                    .extra("DEFAULT (DATETIME('now','localtime'))".to_string()),
            )
            .col(
                ColumnDef::new(MessageIden::Mailbox)
                    .string()
                    .not_null()
                    .extra("CHECK (LENGTH(mailbox) > 0)".to_string()),
            )
            .col(
                ColumnDef::new(MessageIden::Content)
                    .integer()
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
            .context("Error creating database tables")?;
        Ok(())
    }

    // Add a new message to a particular mailbox, returning the new message
    pub fn add_message(
        &mut self,
        mailbox: &str,
        content: &str,
        state: Option<MessageState>,
    ) -> Result<Message> {
        let (sql, values) = Query::insert()
            .into_table(MessageIden::Table)
            .columns([
                MessageIden::Mailbox,
                MessageIden::Content,
                MessageIden::State,
            ])
            .values(vec![
                mailbox.into(),
                content.into(),
                state.unwrap_or(MessageState::Unread).into(),
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
            .context("Error adding message")?;
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
            .context("Error loading messages")?;
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
            .context("Error changing message states")?;
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
            .context("Error clearing messages")?;
        messages.sort_by_key(|message| message.timestamp);
        Ok(messages)
    }

    // Count the number of messages in each mailbox
    pub fn summarize_messages(&mut self) -> Result<Vec<MailboxSummary>> {
        let (sql, values) = Query::select()
            .from(MessageIden::Table)
            .column(MessageIden::Mailbox)
            .expr_as(Func::count(Expr::col(MessageIden::Id)), Alias::new("count"))
            .expr_as(
                Expr::cust("COUNT(id) FILTER (WHERE state = 0)"),
                Alias::new("unread"),
            )
            .expr_as(
                Expr::cust("COUNT(id) FILTER (WHERE state = 1)"),
                Alias::new("read"),
            )
            .expr_as(
                Expr::cust("COUNT(id) FILTER (WHERE state = 2)"),
                Alias::new("archived"),
            )
            .group_by_col(MessageIden::Mailbox)
            .order_by_expr(Expr::cust("count"), Order::Desc)
            .order_by(MessageIden::Mailbox, Order::Asc)
            .build(SqliteQueryBuilder);

        let mut statement = self.connection.prepare(sql.as_str())?;
        let summaries = statement
            .query_map(RusqliteValues::from(values).as_params().as_slice(), |row| {
                Ok(MailboxSummary {
                    mailbox: row.get(0)?,
                    count: row.get(1)?,
                    unread: row.get(2)?,
                    read: row.get(3)?,
                    archived: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<MailboxSummary>, _>>()
            .context("Error summarizing messages")?;
        Ok(summaries)
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;

    fn get_populated_db() -> Result<Database> {
        let mut db = Database::new(None)?;
        db.add_message("unread", "unread1", Some(MessageState::Unread))?;
        db.add_message("unread", "unread2", Some(MessageState::Unread))?;
        db.add_message("read", "read1", Some(MessageState::Read))?;
        db.add_message("read", "read2", Some(MessageState::Read))?;
        db.add_message("read", "read3", Some(MessageState::Read))?;
        db.add_message("archived", "archive1", Some(MessageState::Archived))?;
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
        db.add_message("mailbox1", "message1", None)?;
        db.add_message("mailbox2", "message2", None)?;
        db.add_message("mailbox1", "message3", None)?;
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
        db.add_message("a", "message", None)?;
        db.add_message("ab", "message", None)?;
        db.add_message("a/b", "message", None)?;
        db.add_message("a/c", "message", None)?;
        db.add_message("a/b/c", "message", None)?;
        db.add_message("a/c/b", "message", None)?;
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

    #[test]
    fn test_summarize() -> Result<()> {
        let mut db = get_populated_db()?;
        let summary = db.summarize_messages()?;
        assert_eq!(
            summary,
            vec![
                MailboxSummary {
                    mailbox: "read".to_string(),
                    count: 3,
                    unread: 0,
                    read: 3,
                    archived: 0
                },
                MailboxSummary {
                    mailbox: "unread".to_string(),
                    count: 2,
                    unread: 2,
                    read: 0,
                    archived: 0
                },
                MailboxSummary {
                    mailbox: "archived".to_string(),
                    count: 1,
                    unread: 0,
                    read: 0,
                    archived: 1
                },
            ]
        );
        Ok(())
    }
}
