mod cli;
mod database;
mod message;
mod message_filter;

use crate::cli::Cli;
use crate::database::Database;
use crate::message::MessageState;
use anyhow::{Context, Result};
use clap::Parser;
use message_filter::MessageFilter;
use std::{fs::create_dir_all, vec};

fn main() -> Result<()> {
    let project_dirs = directories::ProjectDirs::from("com", "canac", "mailbox")
        .context("Couldn't determine application directory")?;
    let data_dir = project_dirs.data_local_dir();
    create_dir_all(data_dir).context("Couldn't create application directory")?;
    let mut db = Database::new(Some(data_dir.join("mailbox.db")))?;

    let cli = Cli::parse();
    match cli {
        Cli::Add {
            mailbox,
            content,
            state,
        } => {
            let state = match state {
                cli::AddMessageState::Unread => MessageState::Unread,
                cli::AddMessageState::Read => MessageState::Read,
                cli::AddMessageState::Archived => MessageState::Archived,
            };
            let contents = match content.as_str() {
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
            };
            for content in contents {
                println!(
                    "{}",
                    db.add_message(mailbox.as_str(), content.as_str(), Some(state))?
                );
            }
        }
        Cli::View { mailbox, state } => {
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
            for message in messages {
                println!("{message}");
            }
        }
        Cli::Read { mailbox } => {
            let messages = db.change_state(
                &MessageFilter::new()
                    .with_mailbox_option(mailbox)
                    .with_states(vec![MessageState::Unread]),
                MessageState::Read,
            )?;
            for message in messages {
                println!("{message}");
            }
        }
        Cli::Archive { mailbox } => {
            let messages = db.change_state(
                &MessageFilter::new()
                    .with_mailbox_option(mailbox)
                    .with_states(vec![MessageState::Unread, MessageState::Read]),
                MessageState::Archived,
            )?;
            for message in messages {
                println!("{message}");
            }
        }
        Cli::Clear { mailbox } => {
            let messages = db.delete_messages(
                &MessageFilter::new()
                    .with_mailbox_option(mailbox)
                    .with_states(vec![MessageState::Archived]),
            )?;
            for message in messages {
                println!("{message}");
            }
        }
        Cli::Summarize => {
            for summary in db.summarize_messages()? {
                println!("{summary}");
            }
        }
    };

    Ok(())
}
