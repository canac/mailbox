mod cli;
mod config;
mod database;
mod message;
mod message_components;
mod message_filter;
mod message_formatter;
mod truncate;

use crate::cli::{AddMessageState, Cli, Command};
use crate::config::{Config, Override};
use crate::database::Database;
use crate::message::MessageState;
use anyhow::{Context, Result};
use clap::Parser;
use message::Message;
use message_filter::MessageFilter;
use message_formatter::{MessageFormatter, TimestampFormat};
use serde::Deserialize;
use std::io::stdin;
use std::{fs::create_dir_all, vec};

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportMessageState {
    Unread,
    Read,
    Archived,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportedMessage {
    mailbox: String,
    content: String,
    state: Option<ImportMessageState>,
}

// Load the database connection
fn load_database() -> Result<Database> {
    let project_dirs = directories::ProjectDirs::from("com", "canac", "mailbox")
        .context("Couldn't determine application directory")?;
    let data_dir = project_dirs.data_local_dir();
    create_dir_all(data_dir).context("Couldn't create application directory")?;
    Database::new(Some(data_dir.join("mailbox.db")))
}

// Load the configuration file
fn load_config() -> Result<Option<Config>> {
    Ok(match std::env::var_os("MAILBOX_CONFIG") {
        Some(config_path) => Some(Config::load(config_path.into())?),
        None => None,
    })
}

// Create the message formatter
fn create_formatter(full_output: bool) -> Result<MessageFormatter> {
    let tty = atty::is(atty::Stream::Stdout);
    const DEFAULT_WIDTH: usize = 80;
    const DEFAULT_HEIGHT: usize = 8;
    let size = if !full_output && tty {
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
    Ok(MessageFormatter::new()
        .with_color(tty)
        .with_timestamp_format(if tty {
            TimestampFormat::Relative
        } else {
            TimestampFormat::Local
        })
        .with_max_columns(size.map(|(width, _)| width))
        .with_max_lines(size.map(|(_, height)| height)))
}

// Import messages into the database using the provided config to potentially
// override their initial state
fn import_messages(
    db: &mut Database,
    config: &Option<Config>,
    messages: Vec<(String, String, Option<MessageState>)>,
) -> Result<Vec<Message>> {
    messages
        .into_iter()
        .filter_map(|(mailbox, content, state)| {
            let overridden_state = config
                .as_ref()
                .and_then(|config| config.get_override(&mailbox));
            let state = match overridden_state {
                Some(Override::Unread) => Some(MessageState::Unread),
                Some(Override::Read) => Some(MessageState::Read),
                Some(Override::Archived) => Some(MessageState::Archived),
                // Skip adding this message entirely
                Some(Override::Ignored) => return None,
                None => state,
            };
            Some(db.add_message(&mailbox, &content, state))
        })
        .collect()
}

// Import messages as lines of JSON from stdin
fn read_messages_stdin() -> Vec<(String, String, Option<MessageState>)> {
    stdin()
        .lines()
        .filter_map(|result| match result {
            Ok(line) => {
                if line.is_empty() {
                    None
                } else {
                    let parse_result = serde_json::from_str::<ImportedMessage>(&line)
                        .context("Error parsing line as JSON");
                    match parse_result {
                        Ok(message) => {
                            let state = message.state.map(|state| match state {
                                ImportMessageState::Unread => MessageState::Unread,
                                ImportMessageState::Read => MessageState::Read,
                                ImportMessageState::Archived => MessageState::Archived,
                            });
                            Some((message.mailbox, message.content, state))
                        }
                        Err(err) => {
                            // Print an error but continue attempting to parse the other lines
                            eprintln!("{:?}", err);
                            None
                        }
                    }
                }
            }
            Err(_) => None,
        })
        .collect::<Vec<_>>()
}

fn main() -> Result<()> {
    let mut db = load_database()?;
    let config = load_config()?;
    let cli = Cli::parse();
    let formatter = create_formatter(cli.full_output)?;

    match cli.command {
        Command::Add {
            mailbox,
            content,
            state,
        } => {
            let cli_state = match state {
                AddMessageState::Unread => MessageState::Unread,
                AddMessageState::Read => MessageState::Read,
                AddMessageState::Archived => MessageState::Archived,
            };
            let raw_messages = vec![(mailbox, content, Some(cli_state))];
            let messages = import_messages(&mut db, &config, raw_messages)?;
            print!("{}", formatter.format_messages(&messages))
        }

        Command::Import => {
            let messages = import_messages(&mut db, &config, read_messages_stdin())?;
            print!("{}", formatter.format_messages(&messages))
        }

        Command::View { mailbox, state } => {
            let states = match state {
                cli::ViewMessageState::Unread => vec![MessageState::Unread],
                cli::ViewMessageState::Read => vec![MessageState::Read],
                cli::ViewMessageState::Archived => vec![MessageState::Archived],
                cli::ViewMessageState::Unarchived => {
                    vec![MessageState::Unread, MessageState::Read]
                }
                cli::ViewMessageState::All => vec![
                    MessageState::Unread,
                    MessageState::Read,
                    MessageState::Archived,
                ],
            };
            let messages = db.load_messages(
                &MessageFilter::new()
                    .with_mailbox_option(mailbox)
                    .with_states(states),
            )?;
            print!("{}", formatter.format_messages(&messages))
        }

        Command::Read { mailbox } => {
            let messages = db.change_state(
                &MessageFilter::new()
                    .with_mailbox_option(mailbox)
                    .with_states(vec![MessageState::Unread]),
                MessageState::Read,
            )?;
            print!("{}", formatter.format_messages(&messages))
        }

        Command::Archive { mailbox } => {
            let messages = db.change_state(
                &MessageFilter::new()
                    .with_mailbox_option(mailbox)
                    .with_states(vec![MessageState::Unread, MessageState::Read]),
                MessageState::Archived,
            )?;
            print!("{}", formatter.format_messages(&messages))
        }

        Command::Clear { mailbox } => {
            let messages = db.delete_messages(
                &MessageFilter::new()
                    .with_mailbox_option(mailbox)
                    .with_states(vec![MessageState::Archived]),
            )?;
            print!("{}", formatter.format_messages(&messages))
        }

        Command::Summarize => {
            for summary in db.summarize_messages()? {
                println!("{summary}");
            }
        }
    };

    Ok(())
}
