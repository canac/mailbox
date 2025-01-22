#![warn(
    clippy::clone_on_ref_ptr,
    clippy::str_to_string,
    clippy::pedantic,
    clippy::nursery
)]
#![allow(clippy::future_not_send, clippy::missing_const_for_fn)]

mod cli;
mod config;
mod import;
mod message_components;
mod message_formatter;
mod truncate;
mod tui;

use crate::cli::{AddMessageState, Cli, Command, TimestampFormat};
use crate::config::Config;
use crate::import::read_messages_stdin;
use anyhow::{bail, Context, Result};
use clap::Parser;
use cli::{ConfigSubcommand, ViewMessageState};
use database::{Backend, Database, Filter, HttpBackend, NewMessage, SqliteBackend, State};
use directories::ProjectDirs;
use import::import_messages;
use message_formatter::MessageFormatter;
use std::fs::create_dir_all;
use std::io::{stdin, stdout, IsTerminal};
use std::path::PathBuf;

// Return the directories where this project stores its data
fn get_project_dirs() -> Result<ProjectDirs> {
    directories::ProjectDirs::from("com", "canac", "mailbox")
        .context("Couldn't determine application directory")
}

// Return the path of the configuration file, creating its parent directories if necessary
fn get_config_path() -> Result<PathBuf> {
    let project_dirs = get_project_dirs()?;
    let config_dir = project_dirs.config_dir();
    create_dir_all(config_dir).context("Couldn't create config directory")?;
    Ok(config_dir.join("config.toml"))
}

// Load the configuration file
fn load_config() -> Result<Option<Config>> {
    Config::load(&get_config_path()?)
}

// Open the configuration file in $EDITOR
fn edit_config() -> Result<()> {
    match std::env::var_os("EDITOR") {
        Some(editor) => {
            std::process::Command::new(&editor)
                .arg(get_config_path()?)
                .status()
                .with_context(|| format!("Failed to open editor: {}", editor.to_string_lossy()))?;
            Ok(())
        }
        None => bail!("$EDITOR environment variable isn't set"),
    }
}

// Create the message formatter
fn create_formatter(cli: &Cli) -> MessageFormatter {
    const DEFAULT_WIDTH: usize = 80;
    const DEFAULT_HEIGHT: usize = 8;

    let tty = stdout().is_terminal();
    let truncate = matches!(cli.command, Command::View { full_output, .. } if !full_output);
    let size = if truncate && tty {
        match crossterm::terminal::size() {
            Ok((width, height)) => Some((
                width as usize,
                // Use slightly less than all of the available terminal lines
                std::cmp::max(DEFAULT_HEIGHT, height.saturating_sub(4) as usize),
            )),
            Err(_) => Some((DEFAULT_WIDTH, DEFAULT_HEIGHT)),
        }
    } else {
        None
    };
    let colorize = if cli.color {
        true
    } else if cli.no_color {
        false
    } else {
        colored::control::ShouldColorize::from_env().should_colorize()
    };
    let timestamp_format = cli.timestamp_format.unwrap_or({
        if tty {
            TimestampFormat::Relative
        } else {
            TimestampFormat::Local
        }
    });
    MessageFormatter::new()
        .with_color(colorize)
        .with_timestamp_format(timestamp_format)
        .with_max_columns(size.map(|(width, _)| width))
        .with_max_lines(size.map(|(_, height)| height))
}

// Convert a ViewMessageState into the list of states that it represents
fn states_from_view_message_state(state: ViewMessageState) -> Vec<State> {
    match state {
        ViewMessageState::Unread => vec![State::Unread],
        ViewMessageState::Read => vec![State::Read],
        ViewMessageState::Archived => vec![State::Archived],
        ViewMessageState::Unarchived => {
            vec![State::Unread, State::Read]
        }
        ViewMessageState::All => vec![State::Unread, State::Read, State::Archived],
    }
}

async fn run<B: Backend + Send + Sync + 'static>(
    config: Option<Config>,
    db: Database<B>,
) -> Result<()> {
    let cli = Cli::parse();
    let formatter = create_formatter(&cli);

    match cli.command {
        Command::Add {
            mailbox,
            content,
            state,
        } => {
            let cli_state = match state {
                AddMessageState::Unread => State::Unread,
                AddMessageState::Read => State::Read,
                AddMessageState::Archived => State::Archived,
            };
            let raw_messages = vec![NewMessage {
                mailbox,
                content,
                state: Some(cli_state),
            }];
            let messages = import_messages(&db, config.as_ref(), raw_messages).await?;
            print!("{}", formatter.format_messages(&messages)?);
        }

        Command::Import { format } => {
            let messages = import_messages(
                &db,
                config.as_ref(),
                read_messages_stdin(stdin().lock(), format),
            )
            .await?;
            print!("{}", formatter.format_messages(&messages)?);
        }

        Command::View { mailbox, state, .. } => {
            let messages = db
                .load_messages(
                    Filter::new()
                        .with_mailbox_option(mailbox)
                        .with_states(states_from_view_message_state(state)),
                )
                .await?;
            print!("{}", formatter.format_messages(&messages)?);
        }

        Command::Read { mailbox } => {
            let messages = db
                .change_state(
                    Filter::new()
                        .with_mailbox_option(mailbox)
                        .with_states(vec![State::Unread]),
                    State::Read,
                )
                .await?;
            print!("{}", formatter.format_messages(&messages)?);
        }

        Command::Archive { mailbox } => {
            let messages = db
                .change_state(
                    Filter::new()
                        .with_mailbox_option(mailbox)
                        .with_states(vec![State::Unread, State::Read]),
                    State::Archived,
                )
                .await?;
            print!("{}", formatter.format_messages(&messages)?);
        }

        Command::Clear { mailbox } => {
            let messages = db
                .delete_messages(
                    Filter::new()
                        .with_mailbox_option(mailbox)
                        .with_states(vec![State::Archived]),
                )
                .await?;
            print!("{}", formatter.format_messages(&messages)?);
        }

        Command::Tui { mailbox, state } => {
            crate::tui::run(db, mailbox, states_from_view_message_state(state))?;
        }

        Command::Config { subcommand } => match subcommand {
            ConfigSubcommand::Locate => println!("{}", get_config_path()?.to_string_lossy()),
            ConfigSubcommand::Edit => edit_config()?,
        },
    };

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Fix broken pipe panics
    sigpipe::reset();

    let config = load_config()?;
    let database = config
        .as_ref()
        .map(|config| config.database.clone())
        .unwrap_or_default();
    match database {
        config::DatabaseProvider::Sqlite => {
            let project_dirs = get_project_dirs()?;
            let backend =
                SqliteBackend::new(project_dirs.data_local_dir().join("mailbox.db")).await?;
            let db = Database::new(backend);
            run(config, db).await?;
        }
        config::DatabaseProvider::Http { url, token } => {
            let backend = HttpBackend::new(url, token)?;
            let db = Database::new(backend);
            run(config, db).await?;
        }
    }

    Ok(())
}
