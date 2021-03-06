use clap::{Parser, ValueEnum};

#[derive(ValueEnum, Clone)]
pub enum AddMessageState {
    Unread,
    Read,
    Archived,
}

#[derive(ValueEnum, Clone)]
pub enum ViewMessageState {
    Unread,
    Read,
    Archived,
    Unarchived,
    All,
}

#[derive(Parser)]
pub enum Command {
    /// Add a message to a mailbox
    Add {
        /// Mailbox name
        mailbox: String,

        /// Message content ("-" to read from stdin delimited by newlines)
        content: String,

        /// Mailbox state
        #[clap(value_enum, short = 's', long, default_value = "unread")]
        state: AddMessageState,
    },

    /// View messages
    View {
        /// Only view messages in a particular mailbox
        #[clap(short = 'm', long)]
        mailbox: Option<String>,

        /// Only view messages in a particular state
        #[clap(value_enum, short = 's', long, default_value = "unread")]
        state: ViewMessageState,
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

#[derive(Parser)]
#[clap(
    name = env!("CARGO_PKG_NAME"),
    about = env!("CARGO_PKG_DESCRIPTION"),
    version = env!("CARGO_PKG_VERSION"),
    author = env!("CARGO_PKG_AUTHORS")
)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Command,

    /// Show all messages in output instead of summarizing
    #[clap(short = 'f', long, global = true)]
    pub full_output: bool,
}
