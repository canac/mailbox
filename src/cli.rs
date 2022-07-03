use crate::models;
use clap::{Parser, ValueEnum};

#[derive(ValueEnum, Clone)]
pub enum MessageState {
    Unread,
    Read,
    Archived,
    Unarchived,
    All,
}

#[derive(Parser)]
#[clap(
    name = env!("CARGO_PKG_NAME"),
    about = env!("CARGO_PKG_DESCRIPTION"),
    version = env!("CARGO_PKG_VERSION"),
    author = env!("CARGO_PKG_AUTHORS")
)]
pub enum Cli {
    /// Add a message to a mailbox
    Add {
        /// Mailbox name
        mailbox: String,

        /// Message content
        content: String,

        /// Mailbox state
        #[clap(value_enum, short = 's', long, default_value = "unread")]
        state: models::MessageState,
    },

    /// View messages
    View {
        /// Only view messages in a particular mailbox
        #[clap(short = 'm', long)]
        mailbox: Option<String>,

        /// Only view messages in a particular state
        #[clap(value_enum, short = 's', long, default_value = "unread")]
        state: MessageState,
    },

    /// Mark unread messages as read
    Read {
        /// Only read messages in a particular mailbox
        #[clap(short = 'm', long)]
        mailbox: Option<String>,
    },

    /// Archive all read and unread messages
    Archive {
        /// Only archive messages in a particular mailbox
        #[clap(short = 'm', long)]
        mailbox: Option<String>,
    },

    /// Permanently clear archived messages
    Clear {
        /// Only clear archived messages in a particular mailbox
        #[clap(short = 'm', long)]
        mailbox: Option<String>,
    },

    /// Summarize all mailboxes
    Summarize,
}
