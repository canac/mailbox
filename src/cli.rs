use clap::Parser;

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
    },

    /// Clear messages from one or all mailboxes
    Clear {
        /// Mailbox name, all mailboxes if absent
        mailbox: Option<String>,

        /// Only clear read messages
        #[clap(short = 'r', long)]
        read: bool,
    },

    /// Count messages in one or all mailboxes
    Count {
        /// Mailbox name, all mailboxes if absent
        mailbox: Option<String>,

        /// Only count unread messages
        #[clap(short = 'u', long)]
        unread: bool,
    },

    /// Summarize all mailboxes
    Summarize,

    /// View messages in one or all mailboxes
    View {
        /// Mailbox name, all mailboxes if absent
        mailbox: Option<String>,

        /// Only view unread messages
        #[clap(short = 'u', long)]
        unread: bool,

        /// Mark viewed messages as read
        #[clap(short = 'r', long, requires = "unread")]
        mark_read: bool,
    },
}
