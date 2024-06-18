use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[clap(about, version, author)]
pub struct Cli {
    /// The port that the HTTP server will listen on
    #[clap(short = 'p', long, default_value = "8080", env = "PORT")]
    pub port: u16,

    /// Accept connections from the local network, i.e. bind to 0.0.0.0 instead of 127.0.0.1
    #[clap(short = 'e', long)]
    pub expose: bool,

    /// Require all requests to have an "Authorization: Bearer" header containing this token
    #[clap(long, env = "MAILBOX_AUTH_TOKEN")]
    pub token: Option<String>,

    /// SQLite mailbox database filename
    #[allow(clippy::doc_markdown)]
    #[clap(short = 'f', long, default_value = "mailbox.db")]
    pub db_file: PathBuf,
}
