use self::sea_query_driver_rusqlite::RusqliteValues;
use crate::models::{Message, MessageIden};
use anyhow::{Context, Result};
use rusqlite::Connection;
use sea_query::{Alias, ColumnDef, Expr, Func, Order, Query, SqliteQueryBuilder, Table, Value};
use std::collections::HashMap;

sea_query::sea_query_driver_rusqlite!();

pub struct Database {
    connection: Connection,
}

pub struct MailboxSummary {
    pub count: i64,
    pub unread: i64,
}

impl Database {
    // Create a new Database instance
    pub fn new() -> Result<Self> {
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
            .col(ColumnDef::new(MessageIden::Mailbox).string().not_null())
            .col(ColumnDef::new(MessageIden::Content).integer().not_null())
            .col(
                ColumnDef::new(MessageIden::Read)
                    .integer()
                    .not_null()
                    .default(Value::Int(Some(false.into()))),
            )
            .build(SqliteQueryBuilder);

        let connection = Connection::open("mailbox.db").context("Error opening database")?;
        connection
            .execute(sql.as_str(), [])
            .context("Error creating database tables")?;
        Ok(Database { connection })
    }

    // Add a new message to a particular mailbox, returning the new message
    pub fn add_message(&mut self, mailbox: &str, content: &str) -> Result<Message> {
        let (sql, values) = Query::insert()
            .into_table(MessageIden::Table)
            .columns([MessageIden::Mailbox, MessageIden::Content])
            .values(vec![mailbox.into(), content.into()])?
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

    // Load messages, optionally applying the provided mailbox and read filters
    pub fn load_messages(
        &mut self,
        mailbox_filter: Option<&str>,
        read_filter: Option<bool>,
    ) -> Result<Vec<Message>> {
        let (sql, values) = Query::select()
            .expr(Expr::asterisk())
            .from(MessageIden::Table)
            .and_where_option(
                mailbox_filter.map(|mailbox| Expr::col(MessageIden::Mailbox).eq(mailbox)),
            )
            .and_where_option(read_filter.map(|read| Expr::col(MessageIden::Read).eq(read)))
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

    // Mark messages as read, optionally applying the provided mailbox filter
    pub fn read_messages(&mut self, mailbox_filter: Option<&str>) -> Result<Vec<Message>> {
        let (sql, values) = Query::update()
            .table(MessageIden::Table)
            .and_where_option(
                mailbox_filter.map(|mailbox| Expr::col(MessageIden::Mailbox).eq(mailbox)),
            )
            .value(MessageIden::Read, true.into())
            .returning_all()
            .build(SqliteQueryBuilder);

        let mut statement = self.connection.prepare(sql.as_str())?;
        let mut messages = statement
            .query_map(
                RusqliteValues::from(values).as_params().as_slice(),
                Message::from_row,
            )?
            .collect::<Result<Vec<Message>, _>>()
            .context("Error reading messages")?;
        messages.sort_by_key(|message| message.timestamp);
        Ok(messages)
    }

    // Delete messages, optionally applying the provided mailbox and read filters
    pub fn clear_messages(
        &mut self,
        mailbox_filter: Option<&str>,
        read_filter: Option<bool>,
    ) -> Result<Vec<Message>> {
        let (sql, values) = Query::delete()
            .from_table(MessageIden::Table)
            .returning_all()
            .and_where_option(
                mailbox_filter.map(|mailbox| Expr::col(MessageIden::Mailbox).eq(mailbox)),
            )
            .and_where_option(read_filter.map(|read| Expr::col(MessageIden::Read).eq(read)))
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

    // Count messages, optionally applying the provided mailbox and read filters
    pub fn count_messages(
        &mut self,
        mailbox_filter: Option<&str>,
        read_filter: Option<bool>,
    ) -> Result<i64> {
        let (sql, values) = Query::select()
            .from(MessageIden::Table)
            .expr_as(Func::count(Expr::col(MessageIden::Id)), Alias::new("count"))
            .and_where_option(
                mailbox_filter.map(|mailbox| Expr::col(MessageIden::Mailbox).eq(mailbox)),
            )
            .and_where_option(read_filter.map(|read| Expr::col(MessageIden::Read).eq(read)))
            .build(SqliteQueryBuilder);

        let mut statement = self.connection.prepare(sql.as_str())?;
        let count: i64 = statement
            .query_row(RusqliteValues::from(values).as_params().as_slice(), |row| {
                row.get(0)
            })
            .context("Error counting messages")?;
        Ok(count)
    }

    // Count the number of messages in each mailbox
    pub fn summarize_messages(&mut self) -> Result<HashMap<String, MailboxSummary>> {
        let (sql, values) = Query::select()
            .from(MessageIden::Table)
            .column(MessageIden::Mailbox)
            .expr_as(Func::count(Expr::col(MessageIden::Id)), Alias::new("count"))
            .expr_as(
                Expr::cust("COUNT(id) FILTER (WHERE read = FALSE)"),
                Alias::new("unread"),
            )
            .group_by_col(MessageIden::Mailbox)
            .build(SqliteQueryBuilder);

        let mut statement = self.connection.prepare(sql.as_str())?;
        let summaries = statement
            .query_map(RusqliteValues::from(values).as_params().as_slice(), |row| {
                Ok((
                    row.get(0)?,
                    MailboxSummary {
                        count: row.get(1)?,
                        unread: row.get(2)?,
                    },
                ))
            })?
            .collect::<Result<HashMap<String, MailboxSummary>, _>>()
            .context("Error summarizing messages")?;
        Ok(summaries)
    }
}
