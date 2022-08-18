mod cli;
mod config;
mod database;
mod import;
mod message;
mod message_components;
mod message_filter;
mod message_formatter;
mod new_message;
mod truncate;

use crate::cli::{AddMessageState, Cli, Command, TimestampFormat};
use crate::config::Config;
use crate::database::Database;
use crate::import::read_messages_stdin;
use crate::message::MessageState;
use anyhow::{Context, Result};
use clap::Parser;
use import::import_messages;
use message_filter::MessageFilter;
use message_formatter::MessageFormatter;
use new_message::NewMessage;
use std::fs::create_dir_all;
use std::io::stdin;

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
fn create_formatter(cli: &Cli) -> Result<MessageFormatter> {
    let tty = atty::is(atty::Stream::Stdout);
    const DEFAULT_WIDTH: usize = 80;
    const DEFAULT_HEIGHT: usize = 8;
    let size = if !cli.full_output && tty {
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
    let timestamp_format = cli.timestamp_format.unwrap_or({
        if tty {
            TimestampFormat::Relative
        } else {
            TimestampFormat::Local
        }
    });
    Ok(MessageFormatter::new()
        .with_color(colored::control::SHOULD_COLORIZE.should_colorize())
        .with_timestamp_format(timestamp_format)
        .with_max_columns(size.map(|(width, _)| width))
        .with_max_lines(size.map(|(_, height)| height)))
}

fn main() -> Result<()> {
    let mut db = load_database()?;
    let config = load_config()?;
    let cli = Cli::parse();
    let formatter = create_formatter(&cli)?;

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
            let raw_messages = vec![NewMessage {
                mailbox,
                content,
                state: Some(cli_state),
            }];
            let messages = import_messages(&mut db, &config, raw_messages)?;
            print!("{}", formatter.format_messages(&messages))
        }

        Command::Import { format } => {
            let messages = import_messages(
                &mut db,
                &config,
                read_messages_stdin(stdin().lock(), format),
            )?;
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
