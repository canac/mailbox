use crate::mailbox::Mailbox;
use crate::message::{Message, MessageIden, State};
use crate::message_filter::MessageFilter;
use crate::new_message::NewMessage;
use anyhow::{anyhow, bail, Context, Result};
use sea_query::{
    Alias, ColumnDef, Expr, Func, Keyword, Order, PostgresQueryBuilder, Query, QueryBuilder,
    SchemaBuilder, SqliteQueryBuilder, Table, Value,
};
use sea_query_binder::SqlxBinder;
use sqlx::any::AnyConnectOptions;
use sqlx::postgres::PgConnectOptions;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use sqlx::{query, AnyPool, Row};
use std::path::PathBuf;
use std::str::FromStr;

pub enum Engine {
    Sqlite(Option<PathBuf>),
    Postgres(String),
}

pub struct Database {
    pool: AnyPool,
    schema_builder: Box<dyn SchemaBuilder + Send + Sync>,
    query_builder: Box<dyn QueryBuilder + Send + Sync>,
}

impl Database {
    // Create a new Database instance
    // An in-memory database is used if a database path isn't provided
    pub async fn new(engine: Engine) -> Result<Self> {
        let (options, schema_builder, query_builder): (
            AnyConnectOptions,
            Box<dyn SchemaBuilder + Send + Sync>,
            Box<dyn QueryBuilder + Send + Sync>,
        ) = match engine {
            Engine::Sqlite(db_path) => {
                let path = match db_path.as_deref() {
                    Some(path) => path
                        .to_str()
                        .ok_or_else(|| anyhow!("Failed to convert database path"))?,
                    None => ":memory:",
                };
                (
                    SqliteConnectOptions::from_str(path)
                        .context("Failed to parse SQLite database path")?
                        .journal_mode(if cfg!(test) {
                            // Disable WAL during testing so that tests that write to the database
                            // and then immediately read from the database will pass
                            SqliteJournalMode::Delete
                        } else {
                            SqliteJournalMode::Wal
                        })
                        .create_if_missing(true)
                        .into(),
                    Box::new(SqliteQueryBuilder {}),
                    Box::new(SqliteQueryBuilder {}),
                )
            }
            Engine::Postgres(url) => (
                PgConnectOptions::from_str(url.as_str())
                    .context("Failed to parse PostgreSQL connection URL")?
                    .into(),
                Box::new(PostgresQueryBuilder {}),
                Box::new(PostgresQueryBuilder {}),
            ),
        };

        let pool = AnyPool::connect_with(options)
            .await
            .context("Failed to open database")?;
        let db = Database {
            pool,
            schema_builder,
            query_builder,
        };
        db.init().await?;
        Ok(db)
    }

    // Initialize the database and create the necessary tables
    pub async fn init(&self) -> Result<()> {
        if cfg!(test) {
            // Reset the database in when testing
            let sql = Table::drop()
                .table(MessageIden::Table)
                .if_exists()
                .build_any(&*self.schema_builder);
            query(&sql)
                .execute(&self.pool)
                .await
                .context("Failed to delete database table")?;
        }

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
            .build_any(&*self.schema_builder);
        query(&sql)
            .execute(&self.pool)
            .await
            .context("Failed to create database tables")?;
        Ok(())
    }

    // Add a new message to a particular mailbox, returning the new message
    pub async fn add_message(&self, message: NewMessage) -> Result<Message> {
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
                message.state.unwrap_or(State::Unread).into(),
            ])?
            .returning_all()
            .build_any_sqlx(&*self.query_builder);

        let message = sqlx::query_as_with::<_, Message, _>(&sql, values)
            .fetch_one(&self.pool)
            .await
            .context("Failed to add message")?;
        Ok(message)
    }

    // Load messages, applying the provided filters
    pub async fn load_messages(&self, filter: MessageFilter) -> Result<Vec<Message>> {
        let (sql, values) = Query::select()
            .expr(Expr::asterisk())
            .from(MessageIden::Table)
            .cond_where(filter.get_where())
            .order_by(MessageIden::Timestamp, Order::Desc)
            .order_by(MessageIden::Id, Order::Desc)
            .build_any_sqlx(&*self.query_builder);

        let messages = sqlx::query_as_with::<_, Message, _>(&sql, values)
            .fetch_all(&self.pool)
            .await
            .context("Failed to load messages")?;
        Ok(messages)
    }

    // Move messages from their old state into new_state
    // Only load messages in the specified mailbox if mailbox_filter is provided
    // Only load messages in one of the specified states if states_filter is provided
    pub async fn change_state(
        &self,
        filter: MessageFilter,
        new_state: State,
    ) -> Result<Vec<Message>> {
        let (sql, values) = Query::update()
            .table(MessageIden::Table)
            .cond_where(filter.get_where())
            .value::<_, i32>(MessageIden::State, new_state.into())
            .returning_all()
            .build_any_sqlx(&*self.query_builder);

        let mut messages = sqlx::query_as_with::<_, Message, _>(&sql, values)
            .fetch_all(&self.pool)
            .await
            .context("Failed to change message states")?;
        // Sort the messages manually since SQLite doesn't support sorting RETURNING results
        messages.sort_by_key(|message| -message.timestamp.timestamp());
        Ok(messages)
    }

    // Delete messages, applying the provided filters
    pub async fn delete_messages(&self, filter: MessageFilter) -> Result<Vec<Message>> {
        let (sql, values) = Query::delete()
            .from_table(MessageIden::Table)
            .returning_all()
            .cond_where(filter.get_where())
            .build_any_sqlx(&*self.query_builder);

        let mut messages = sqlx::query_as_with::<_, Message, _>(&sql, values)
            .fetch_all(&self.pool)
            .await
            .context("Failed to clear messages")?;
        // Sort the messages manually since SQLite doesn't support sorting RETURNING results
        messages.sort_by_key(|message| -message.timestamp.timestamp());
        Ok(messages)
    }

    // Load the names of all used mailboxes
    pub async fn load_mailboxes(&self, filter: MessageFilter) -> Result<Vec<(Mailbox, usize)>> {
        let (sql, values) = Query::select()
            .from(MessageIden::Table)
            .column(MessageIden::Mailbox)
            .cond_where(filter.get_where())
            .expr_as(Func::count(Expr::col(MessageIden::Id)), Alias::new("count"))
            .group_by_col(MessageIden::Mailbox)
            .order_by(MessageIden::Mailbox, Order::Asc)
            .distinct()
            .build_any_sqlx(&*self.query_builder);
        let rows = sqlx::query_with(&sql, values)
            .fetch_all(&self.pool)
            .await
            .context("Failed to load mailboxes")?;
        let mailboxes = rows
            .iter()
            .map(|row| {
                let mailbox: String = row.try_get("mailbox")?;
                let count: i64 = row.try_get("count")?;
                Ok((mailbox.try_into()?, count as usize))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(mailboxes)
    }

    // Return an error if the new message invalid
    fn validate_message(message: &NewMessage) -> Result<()> {
        if message.content.is_empty() {
            bail!("content must not be empty");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rstest::*;
    use rstest_reuse::{self, apply, template};

    use super::*;

    // Return a unique database file path for each test
    fn get_db_path() -> PathBuf {
        let dir = std::env::temp_dir().join("mailbox");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("mailbox.db")
    }

    // Return a unique database URL for each test
    fn get_db_url() -> String {
        std::env::var_os("TEST_DB_URL")
            .map(|url| url.into_string().unwrap())
            .unwrap_or(format!("postgresql://postgres@localhost/mailbox-test"))
    }

    async fn add_message(
        db: &Database,
        mailbox: &str,
        content: &str,
        state: impl Into<Option<State>>,
    ) -> Result<()> {
        db.add_message(NewMessage {
            mailbox: mailbox.try_into()?,
            content: content.to_string(),
            state: state.into(),
        })
        .await?;
        Ok(())
    }

    async fn get_populated_db(engine: Engine) -> Result<Database> {
        let db = Database::new(engine).await?;
        add_message(&db, "unread", "unread1", State::Unread).await?;
        add_message(&db, "unread", "unread2", State::Unread).await?;
        add_message(&db, "read", "read1", State::Read).await?;
        add_message(&db, "read", "read2", State::Read).await?;
        add_message(&db, "read", "read3", State::Read).await?;
        add_message(&db, "archived", "archive1", State::Archived).await?;
        Ok(db)
    }

    #[template]
    #[rstest]
    #[case::sqlite_memory(Engine::Sqlite(None))]
    #[case::sqlite_file(Engine::Sqlite(Some(PathBuf::from(get_db_path()))))]
    #[case::postgres(Engine::Postgres(get_db_url()))]
    async fn engines(#[case] engine: Engine) {}

    #[apply(engines)]
    #[tokio::test]
    async fn test_create(engine: Engine) {
        assert!(Database::new(engine).await.is_ok());
    }

    #[apply(engines)]
    #[tokio::test]
    async fn test_add(engine: Engine) -> Result<()> {
        let db = Database::new(engine).await?;
        add_message(&db, "mailbox1", "message1", None).await?;
        add_message(&db, "mailbox2", "message2", None).await?;
        add_message(&db, "mailbox1", "message3", None).await?;
        assert_eq!(db.load_messages(MessageFilter::new()).await?.len(), 3);

        let messages = db
            .load_messages(MessageFilter::new().with_mailbox("mailbox1".try_into()?))
            .await?;
        assert_eq!(messages[0].mailbox.as_ref(), "mailbox1");
        assert_eq!(messages[0].content, "message3");
        assert_eq!(messages[1].mailbox.as_ref(), "mailbox1");
        assert_eq!(messages[1].content, "message1");
        assert_eq!(messages.len(), 2);

        let messages = db
            .load_messages(MessageFilter::new().with_mailbox("mailbox2".try_into()?))
            .await?;
        assert_eq!(messages[0].mailbox.as_ref(), "mailbox2");
        assert_eq!(messages[0].content, "message2");
        assert_eq!(messages.len(), 1);

        Ok(())
    }

    #[apply(engines)]
    #[tokio::test]
    async fn test_add_invalid(engine: Engine) -> Result<()> {
        let db = Database::new(engine).await?;
        assert!(add_message(&db, "mailbox", "", None).await.is_err());
        assert!(add_message(&db, "", "message", None).await.is_err());
        Ok(())
    }

    #[apply(engines)]
    #[tokio::test]
    async fn test_load(engine: Engine) -> Result<()> {
        let db = get_populated_db(engine).await?;
        assert_eq!(db.load_messages(MessageFilter::new()).await?.len(), 6);
        Ok(())
    }

    #[apply(engines)]
    #[tokio::test]
    async fn test_load_with_mailbox_filter(engine: Engine) -> Result<()> {
        let db = get_populated_db(engine).await?;
        assert_eq!(
            db.load_messages(MessageFilter::new().with_mailbox("unread".try_into()?))
                .await?
                .len(),
            2
        );
        Ok(())
    }

    #[apply(engines)]
    #[tokio::test]
    async fn test_load_with_states_filter(engine: Engine) -> Result<()> {
        let db = get_populated_db(engine).await?;
        assert_eq!(
            db.load_messages(MessageFilter::new().with_states(vec![State::Read, State::Archived]))
                .await?
                .len(),
            4
        );
        Ok(())
    }

    #[apply(engines)]
    #[tokio::test]
    async fn test_load_with_sub_mailbox_filters(engine: Engine) -> Result<()> {
        let db = Database::new(engine).await?;
        add_message(&db, "a", "message", None).await?;
        add_message(&db, "ab", "message", None).await?;
        add_message(&db, "a/b", "message", None).await?;
        add_message(&db, "a/c", "message", None).await?;
        add_message(&db, "a/b/c", "message", None).await?;
        add_message(&db, "a/c/b", "message", None).await?;
        assert_eq!(
            db.load_messages(MessageFilter::new().with_mailbox("a".try_into()?))
                .await?
                .len(),
            5
        );
        assert_eq!(
            db.load_messages(MessageFilter::new().with_mailbox("a/b".try_into()?))
                .await?
                .len(),
            2
        );
        assert_eq!(
            db.load_messages(MessageFilter::new().with_mailbox("a/b/c".try_into()?))
                .await?
                .len(),
            1
        );
        Ok(())
    }

    #[apply(engines)]
    #[tokio::test]
    async fn test_read(engine: Engine) -> Result<()> {
        let db = get_populated_db(engine).await?;
        db.change_state(
            MessageFilter::new().with_states(vec![State::Unread]),
            State::Read,
        )
        .await?;
        assert_eq!(
            db.load_messages(MessageFilter::new().with_states(vec![State::Read]))
                .await?
                .len(),
            5
        );
        Ok(())
    }

    #[apply(engines)]
    #[tokio::test]
    async fn test_archive(engine: Engine) -> Result<()> {
        let db = get_populated_db(engine).await?;
        assert_eq!(
            db.change_state(
                MessageFilter::new().with_states(vec![State::Unread, State::Read]),
                State::Archived,
            )
            .await?
            .len(),
            5
        );
        assert_eq!(
            db.load_messages(MessageFilter::new().with_states(vec![State::Archived]))
                .await?
                .len(),
            6
        );
        Ok(())
    }

    #[apply(engines)]
    #[tokio::test]
    async fn test_delete(engine: Engine) -> Result<()> {
        let db = get_populated_db(engine).await?;
        assert_eq!(
            db.delete_messages(MessageFilter::new().with_states(vec![State::Unread, State::Read]))
                .await?
                .len(),
            5
        );
        assert_eq!(db.load_messages(MessageFilter::new()).await?.len(), 1);
        Ok(())
    }

    #[apply(engines)]
    #[tokio::test]
    async fn test_load_mailboxes(engine: Engine) -> Result<()> {
        let db = get_populated_db(engine).await?;
        assert_eq!(
            db.load_mailboxes(MessageFilter::new()).await?,
            vec![
                ("archived".try_into()?, 1),
                ("read".try_into()?, 3),
                ("unread".try_into()?, 2),
            ]
        );
        Ok(())
    }

    #[apply(engines)]
    #[tokio::test]
    async fn test_load_mailboxes_with_filter(engine: Engine) -> Result<()> {
        let db = get_populated_db(engine).await?;
        assert_eq!(
            db.load_mailboxes(MessageFilter::new().with_states(vec![State::Unread]))
                .await?,
            vec![("unread".try_into()?, 2)]
        );
        Ok(())
    }
}
