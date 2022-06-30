mod cli;
mod database;
mod models;

use crate::cli::Cli;
use crate::database::Database;
use anyhow::{Context, Result};
use clap::Parser;
use database::MailboxSummary;
use std::process::Command;

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
            println!("{}", db.add_message(mailbox.as_str(), content.as_str())?);
            post_write()?;
        }
        Cli::Clear { mailbox, read } => {
            let read_filter = if read { Some(true) } else { None };
            for message in db.clear_messages(mailbox.as_deref(), read_filter)? {
                println!("{}", message);
            }
            post_write()?;
        }
        Cli::Count { mailbox, unread } => {
            let read_filter = if unread { Some(false) } else { None };
            let count = db.count_messages(mailbox.as_deref(), read_filter)?;
            println!("{count}");
        }
        Cli::Summarize => {
            for (mailbox, MailboxSummary { count, unread }) in db.summarize_messages()? {
                println!("{mailbox}: {unread}/{count} unread");
            }
        }
        Cli::View {
            mailbox,
            mark_read,
            unread,
        } => {
            let read_filter = if unread { Some(false) } else { None };

            let messages = if mark_read {
                db.read_messages(mailbox.as_deref())?
            } else {
                db.load_messages(mailbox.as_deref(), read_filter)?
            };

            for message in messages {
                println!("{}", message);
            }

            if mark_read {
                post_write()?;
            }
        }
    };

    Ok(())
}
