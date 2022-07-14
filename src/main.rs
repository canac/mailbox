mod cli;
mod database;
mod message;
mod message_filter;
mod message_formatter;

use crate::cli::{AddMessageState, Cli, Command};
use crate::database::Database;
use crate::message::MessageState;
use anyhow::{Context, Result};
use clap::Parser;
use message_filter::MessageFilter;
use message_formatter::{MessageFormatter, TimestampFormat};
use std::{fs::create_dir_all, vec};

fn main() -> Result<()> {
    let project_dirs = directories::ProjectDirs::from("com", "canac", "mailbox")
        .context("Couldn't determine application directory")?;
    let data_dir = project_dirs.data_local_dir();
    create_dir_all(data_dir).context("Couldn't create application directory")?;
    let mut db = Database::new(Some(data_dir.join("mailbox.db")))?;

    let cli = Cli::parse();

    let tty = atty::is(atty::Stream::Stdout);
    let default_height = 8;
    let formatter = MessageFormatter::new()
        .with_color(tty)
        .with_timestamp_format(if tty {
            TimestampFormat::Relative
        } else {
            TimestampFormat::Local
        })
        // Use slightly less than all of the available terminal space
        .with_max_lines(if !cli.full_output && tty {
            Some(
                crossterm::terminal::size().map_or(default_height, |(_, height)| {
                    std::cmp::max(default_height, (height - 4) as usize)
                }),
            )
        } else {
            None
        });

    match cli.command {
        Command::Add {
            mailbox,
            content,
            state,
        } => {
            let state = match state {
                AddMessageState::Unread => MessageState::Unread,
                AddMessageState::Read => MessageState::Read,
                AddMessageState::Archived => MessageState::Archived,
            };
            let messages = match content.as_str() {
                "-" => std::io::stdin()
                    .lines()
                    .filter_map(|result| match result {
                        Ok(line) => {
                            if line.is_empty() {
                                None
                            } else {
                                Some(line)
                            }
                        }
                        Err(_) => None,
                    })
                    .collect(),
                _ => vec![content],
            }
            .iter()
            .map(|content| db.add_message(mailbox.as_str(), content.as_str(), Some(state)))
            .collect::<Result<Vec<_>>>()?;
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
