use crate::database::MailboxInfo;
use crate::message::{Message, MessageIden, State};
use crate::message_filter::MessageFilter;
use crate::new_message::NewMessage;
use crate::Backend;
use anyhow::{Context, Result};
use sea_query::{
    Alias, ColumnDef, Expr, Func, Keyword, Order, Query, SqliteQueryBuilder, Table, Value,
};
use sea_query_binder::SqlxBinder;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use sqlx::{query, Row, SqlitePool};
use std::fs::create_dir_all;
use std::path::PathBuf;

pub struct SqliteBackend {
    pool: SqlitePool,
}

impl SqliteBackend {
    // Create a new SqliteBackend instance
    pub async fn new(db_path: PathBuf) -> Result<Self> {
        if let Some(directory) = db_path.parent() {
            create_dir_all(directory)
                .context("Failed to create SQLite database parent directory")?;
        }
        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .journal_mode(SqliteJournalMode::Wal)
            .create_if_missing(true);

        let pool = SqlitePool::connect_with(options)
            .await
            .context("Failed to open database")?;
        let backend = SqliteBackend { pool };
        backend.init().await?;
        Ok(backend)
    }

    // Create a new SqliteBackend instance for testing that is backed by a uniquely temporary file
    #[cfg(any(test, feature = "test-utils"))]
    pub async fn new_test() -> Result<Self> {
        use std::env::temp_dir;
        use std::sync::atomic::{AtomicU32, Ordering};

        static INDEX: AtomicU32 = AtomicU32::new(0);

        let db_dir = temp_dir().join("mailbox");
        create_dir_all(db_dir.clone())
            .context("Failed to create SQLite database parent directory")?;
        let db_path = db_dir.join(format!(
            "mailbox-{}.db",
            INDEX.fetch_add(1, Ordering::Relaxed)
        ));

        let options = SqliteConnectOptions::new()
            .filename(db_path)
            // Disable WAL during testing so that tests that write to the database
            // and then immediately read from the database will pass
            .journal_mode(SqliteJournalMode::Delete)
            .create_if_missing(true);

        let pool = SqlitePool::connect_with(options)
            .await
            .context("Failed to open database")?;
        let backend = SqliteBackend { pool };

        // Reset the database
        let sql = Table::drop()
            .table(MessageIden::Table)
            .if_exists()
            .build(SqliteQueryBuilder);
        query(&sql)
            .execute(&backend.pool)
            .await
            .context("Failed to delete database table")?;

        backend.init().await?;

        Ok(backend)
    }

    // Initialize the database and create the necessary tables
    pub async fn init(&self) -> Result<()> {
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
                    .default(Keyword::CurrentTimestamp),
            )
            .col(
                ColumnDef::new(MessageIden::Mailbox)
                    .string()
                    .not_null()
                    .extra(String::from("CHECK (LENGTH(mailbox) > 0)")),
            )
            .col(
                ColumnDef::new(MessageIden::Content)
                    .string()
                    .not_null()
                    .extra(String::from("CHECK (LENGTH(content) > 0)")),
            )
            .col(
                ColumnDef::new(MessageIden::State)
                    .integer()
                    .not_null()
                    .default(Value::Int(Some(0)))
                    .extra(String::from("CHECK (state >= 0 AND state <= 2)")),
            )
            .build(SqliteQueryBuilder);
        query(&sql)
            .execute(&self.pool)
            .await
            .context("Failed to create database tables")?;
        Ok(())
    }
}

impl Backend for SqliteBackend {
    async fn add_messages(&self, messages: Vec<NewMessage>) -> Result<Vec<Message>> {
        if messages.is_empty() {
            // The SQL query will be malformed if there are no messages to add, so bail
            return Ok(vec![]);
        }

        let mut statement = Query::insert();
        statement.into_table(MessageIden::Table).columns([
            MessageIden::Mailbox,
            MessageIden::Content,
            MessageIden::State,
        ]);
        // Add the messages in reverse order so that the first message in the batch will appear
        // first when the messages are loaded
        for message in messages.into_iter().rev() {
            statement.values(vec![
                message.mailbox.into(),
                message.content.into(),
                message.state.unwrap_or(State::Unread).into(),
            ])?;
        }
        let (sql, values) = statement.returning_all().build_sqlx(SqliteQueryBuilder);

        let mut messages = sqlx::query_as_with::<_, Message, _>(&sql, values)
            .fetch_all(&self.pool)
            .await
            .context("Failed to add messages")?;
        // Reverse the messages back to the order from the input
        messages.reverse();
        Ok(messages)
    }

    async fn load_messages(&self, filter: MessageFilter) -> Result<Vec<Message>> {
        let (sql, values) = Query::select()
            .expr(Expr::asterisk())
            .from(MessageIden::Table)
            .cond_where(filter.get_where())
            .order_by(MessageIden::Id, Order::Desc)
            .build_sqlx(SqliteQueryBuilder);

        let messages = sqlx::query_as_with::<_, Message, _>(&sql, values)
            .fetch_all(&self.pool)
            .await
            .context("Failed to load messages")?;
        Ok(messages)
    }

    async fn change_state(&self, filter: MessageFilter, new_state: State) -> Result<Vec<Message>> {
        let (sql, values) = Query::update()
            .table(MessageIden::Table)
            .cond_where(filter.get_where())
            .value::<_, i32>(MessageIden::State, new_state.into())
            .returning_all()
            .build_sqlx(SqliteQueryBuilder);

        let mut messages = sqlx::query_as_with::<_, Message, _>(&sql, values)
            .fetch_all(&self.pool)
            .await
            .context("Failed to change message states")?;
        // Sort the messages manually since SQLite doesn't support sorting RETURNING results
        messages.sort_by_key(|message| -message.timestamp.timestamp());
        Ok(messages)
    }

    async fn delete_messages(&self, filter: MessageFilter) -> Result<Vec<Message>> {
        let (sql, values) = Query::delete()
            .from_table(MessageIden::Table)
            .returning_all()
            .cond_where(filter.get_where())
            .build_sqlx(SqliteQueryBuilder);

        let mut messages = sqlx::query_as_with::<_, Message, _>(&sql, values)
            .fetch_all(&self.pool)
            .await
            .context("Failed to clear messages")?;
        // Sort the messages manually since SQLite doesn't support sorting RETURNING results
        messages.sort_by_key(|message| -message.timestamp.timestamp());
        Ok(messages)
    }

    async fn load_mailboxes(&self, filter: MessageFilter) -> Result<Vec<MailboxInfo>> {
        let (sql, values) = Query::select()
            .from(MessageIden::Table)
            .column(MessageIden::Mailbox)
            .cond_where(filter.get_where())
            .expr_as(Func::count(Expr::col(MessageIden::Id)), Alias::new("count"))
            .group_by_col(MessageIden::Mailbox)
            .order_by(MessageIden::Mailbox, Order::Asc)
            .distinct()
            .build_sqlx(SqliteQueryBuilder);
        let rows = sqlx::query_with(&sql, values)
            .fetch_all(&self.pool)
            .await
            .context("Failed to load mailboxes")?;
        let mailboxes = rows
            .iter()
            .map(|row| {
                let mailbox: String = row.try_get("mailbox")?;
                let count: i64 = row.try_get("count")?;
                Ok(MailboxInfo {
                    name: mailbox.try_into()?,
                    message_count: count as usize,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(mailboxes)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    // Helper for creating a NewMessage from its parts
    fn make_message(
        mailbox: &str,
        content: &str,
        state: impl Into<Option<State>>,
    ) -> Result<NewMessage> {
        Ok(NewMessage {
            mailbox: mailbox.try_into()?,
            content: content.to_string(),
            state: state.into(),
        })
    }

    // Create an Sqlite backend containing several existing messages
    async fn get_populated_backend() -> Result<SqliteBackend> {
        let backend = SqliteBackend::new_test().await?;
        backend
            .add_messages(vec![
                make_message("unread", "unread1", State::Unread)?,
                make_message("unread", "unread2", State::Unread)?,
                make_message("read", "read1", State::Read)?,
                make_message("read", "read2", State::Read)?,
                make_message("read", "read3", State::Read)?,
                make_message("archived", "archive1", State::Archived)?,
            ])
            .await?;
        Ok(backend)
    }

    #[tokio::test]
    async fn test_create() {
        assert!(SqliteBackend::new_test().await.is_ok());
    }

    #[tokio::test]
    async fn test_add_many() -> Result<()> {
        let backend = SqliteBackend::new_test().await?;
        let messages = backend
            .add_messages(vec![
                make_message("mailbox2", "message2", None)?,
                make_message("mailbox1", "message1", None)?,
                make_message("mailbox1", "message3", None)?,
            ])
            .await?;
        assert_eq!(
            messages
                .into_iter()
                .map(|message| message.content)
                .collect::<Vec<_>>(),
            vec!["message2", "message1", "message3"]
        );
        assert_eq!(backend.load_messages(MessageFilter::new()).await?.len(), 3);

        let messages = backend
            .load_messages(MessageFilter::new().with_mailbox("mailbox1".try_into()?))
            .await?;
        assert_eq!(messages[0].mailbox.as_ref(), "mailbox1");
        assert_eq!(messages[0].content, "message1");
        assert_eq!(messages[1].mailbox.as_ref(), "mailbox1");
        assert_eq!(messages[1].content, "message3");
        assert_eq!(messages.len(), 2);

        let messages = backend
            .load_messages(MessageFilter::new().with_mailbox("mailbox2".try_into()?))
            .await?;
        assert_eq!(messages[0].mailbox.as_ref(), "mailbox2");
        assert_eq!(messages[0].content, "message2");
        assert_eq!(messages.len(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_add_zero() -> Result<()> {
        let backend = SqliteBackend::new_test().await?;
        backend.add_messages(vec![]).await?;
        assert_eq!(backend.load_messages(MessageFilter::new()).await?.len(), 0);
        Ok(())
    }

    #[tokio::test]
    async fn test_add_invalid() -> Result<()> {
        let backend = SqliteBackend::new_test().await?;
        assert!(backend
            .add_messages(vec![make_message("mailbox", "", None)?])
            .await
            .is_err());
        Ok(())
    }

    #[tokio::test]
    async fn test_load() -> Result<()> {
        let backend = get_populated_backend().await?;
        assert_eq!(backend.load_messages(MessageFilter::new()).await?.len(), 6);
        Ok(())
    }

    #[tokio::test]
    async fn test_load_with_mailbox_filter() -> Result<()> {
        let backend = get_populated_backend().await?;
        assert_eq!(
            backend
                .load_messages(MessageFilter::new().with_mailbox("unread".try_into()?))
                .await?
                .len(),
            2
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_load_with_states_filter() -> Result<()> {
        let backend = get_populated_backend().await?;
        assert_eq!(
            backend
                .load_messages(MessageFilter::new().with_states(vec![State::Read, State::Archived]))
                .await?
                .len(),
            4
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_load_with_sub_mailbox_filters() -> Result<()> {
        let backend = SqliteBackend::new_test().await?;
        backend
            .add_messages(vec![
                make_message("a", "message", None)?,
                make_message("ab", "message", None)?,
                make_message("a/b", "message", None)?,
                make_message("a/c", "message", None)?,
                make_message("a/b/c", "message", None)?,
                make_message("a/c/b", "message", None)?,
            ])
            .await?;
        assert_eq!(
            backend
                .load_messages(MessageFilter::new().with_mailbox("a".try_into()?))
                .await?
                .len(),
            5
        );
        assert_eq!(
            backend
                .load_messages(MessageFilter::new().with_mailbox("a/b".try_into()?))
                .await?
                .len(),
            2
        );
        assert_eq!(
            backend
                .load_messages(MessageFilter::new().with_mailbox("a/b/c".try_into()?))
                .await?
                .len(),
            1
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_read() -> Result<()> {
        let backend = get_populated_backend().await?;
        backend
            .change_state(
                MessageFilter::new().with_states(vec![State::Unread]),
                State::Read,
            )
            .await?;
        assert_eq!(
            backend
                .load_messages(MessageFilter::new().with_states(vec![State::Read]))
                .await?
                .len(),
            5
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_archive() -> Result<()> {
        let backend = get_populated_backend().await?;
        assert_eq!(
            backend
                .change_state(
                    MessageFilter::new().with_states(vec![State::Unread, State::Read]),
                    State::Archived,
                )
                .await?
                .len(),
            5
        );
        assert_eq!(
            backend
                .load_messages(MessageFilter::new().with_states(vec![State::Archived]))
                .await?
                .len(),
            6
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_delete() -> Result<()> {
        let backend = get_populated_backend().await?;
        assert_eq!(
            backend
                .delete_messages(MessageFilter::new().with_states(vec![State::Unread, State::Read]))
                .await?
                .len(),
            5
        );
        assert_eq!(backend.load_messages(MessageFilter::new()).await?.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn test_load_mailboxes() -> Result<()> {
        let backend = get_populated_backend().await?;
        assert_eq!(
            backend.load_mailboxes(MessageFilter::new()).await?,
            vec![
                MailboxInfo {
                    name: "archived".try_into()?,
                    message_count: 1
                },
                MailboxInfo {
                    name: "read".try_into()?,
                    message_count: 3
                },
                MailboxInfo {
                    name: "unread".try_into()?,
                    message_count: 2
                },
            ]
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_load_mailboxes_with_filter() -> Result<()> {
        let backend = get_populated_backend().await?;
        assert_eq!(
            backend
                .load_mailboxes(MessageFilter::new().with_states(vec![State::Unread]))
                .await?,
            vec![MailboxInfo {
                name: "unread".try_into()?,
                message_count: 2
            }]
        );
        Ok(())
    }
}
