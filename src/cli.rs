use clap::{Parser, ValueEnum};

#[derive(Clone, ValueEnum)]
pub enum AddMessageState {
    Unread,
    Read,
    Archived,
}

#[derive(Clone, ValueEnum)]
pub enum ImportMessageFormat {
    Json,
    Tsv,
}

#[derive(Clone, ValueEnum)]
pub enum ViewMessageState {
    Unread,
    Read,
    Archived,
    Unarchived,
    All,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum TimestampFormat {
    Relative,
    Local,
    Utc,
}

#[derive(Parser)]
pub enum ConfigSubcommand {
    /// Show the location of the config file
    Locate,

    /// Open the config file in $EDITOR
    Edit,
}

#[derive(Parser)]
pub enum Command {
    /// Add a message to a mailbox
    Add {
        /// Mailbox name
        mailbox: String,

        /// Message content
        content: String,

        /// Mailbox state
        #[clap(value_enum, short = 's', long, default_value = "unread")]
        state: AddMessageState,
    },

    /// Add multiple messages
    Import {
        /// Import format
        #[clap(value_enum, long, default_value = "tsv")]
        format: ImportMessageFormat,
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

    /// Manage the configuration
    Config {
        #[clap(subcommand)]
        subcommand: ConfigSubcommand,
    },
}

#[derive(Parser)]
#[clap(about, version, author)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Command,

    /// Show all messages in output instead of summarizing
    #[clap(short = 'f', long, global = true)]
    pub full_output: bool,

    /// Enable color even when terminal is not a TTY
    #[clap(long, global = true)]
    pub color: bool,

    /// Disable color even when terminal is a TTY
    #[clap(long, global = true, conflicts_with = "color")]
    pub no_color: bool,

    /// Choose the timestamp format to use (defaults to relative with a TTY and UTC otherwise)
    #[clap(value_enum, long, global = true)]
    pub timestamp_format: Option<TimestampFormat>,
}
