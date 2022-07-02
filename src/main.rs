mod cli;
mod database;
mod models;

use crate::cli::Cli;
use crate::database::Database;
use crate::models::MessageState;
use anyhow::{Context, Result};
use clap::Parser;
use database::MailboxSummary;
use std::{process::Command, vec};

// Execute MAILBOX_POST_WRITE_CMD
fn post_write() -> Result<()> {
    if let Some(cmd) = std::env::var_os("MAILBOX_POST_WRITE_CMD") {
        if cmd.len() == 0 {
            return Ok(());
        }

        return Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .output()
            .map(|_| ())
            .context("Error running post write command");
    }

    Ok(())
}

fn main() -> Result<()> {
    let mut db = Database::new()?;

    let cli = Cli::parse();
    match cli {
        Cli::Add { mailbox, content } => {
            println!(
                "{}",
                db.add_message(mailbox.as_str(), content.as_str(), None)?
            );
            post_write()?;
        }
        Cli::View { mailbox, state } => {
            let states = match state {
                cli::MessageState::Unread => vec![MessageState::Unread],
                cli::MessageState::Read => vec![MessageState::Read],
                cli::MessageState::Archived => vec![MessageState::Archived],
                cli::MessageState::Unarchived => {
                    vec![MessageState::Unread, MessageState::Read]
                }
                cli::MessageState::All => vec![
                    MessageState::Unread,
                    MessageState::Read,
                    MessageState::Archived,
                ],
            };
            let messages = db.load_messages(mailbox.as_deref(), Some(states))?;
            for message in messages {
                println!("{}", message);
            }
        }
        Cli::Read { mailbox } => {
            let messages = db.change_state(
                mailbox.as_deref(),
                Some(vec![MessageState::Unread]),
                MessageState::Read,
            )?;
            for message in messages {
                println!("{}", message);
            }
            post_write()?;
        }
        Cli::Archive { mailbox } => {
            let messages = db.change_state(
                mailbox.as_deref(),
                Some(vec![MessageState::Unread, MessageState::Read]),
                MessageState::Archived,
            )?;
            for message in messages {
                println!("{}", message);
            }
            post_write()?;
        }
        Cli::Clear { mailbox } => {
            let messages =
                db.delete_messages(mailbox.as_deref(), Some(vec![MessageState::Archived]))?;
            for message in messages {
                println!("{}", message);
            }
            post_write()?;
        }
        Cli::Summarize => {
            let summaries = db.summarize_messages()?;
            for MailboxSummary {
                mailbox,
                count,
                unread,
                read,
                archived,
            } in summaries
            {
                println!("{mailbox}: {count} ({unread}/{read}/{archived})");
            }
        }
    };

    Ok(())
}
