use clap::{Parser, Subcommand};

/// The simplest path from file to Exasol table.
#[derive(Parser)]
#[command(name = "exapump", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Upload files to an Exasol table
    Upload(UploadArgs),
}

#[derive(clap::Args)]
pub struct UploadArgs {
    /// Files to upload
    #[arg(required = true)]
    pub files: Vec<String>,

    /// Target table name (e.g., schema.table)
    #[arg(short, long)]
    pub table: String,

    /// Connection string (e.g., exasol://user:pwd@host:port)
    #[arg(short, long, env = "EXAPUMP_DSN")]
    pub dsn: String,

    /// Preview inferred schema without loading data
    #[arg(long)]
    pub dry_run: bool,

    /// CSV field delimiter
    #[arg(long, default_value_t = ',')]
    pub delimiter: char,

    /// Treat the first row as data, not a header
    #[arg(long)]
    pub no_header: bool,

    /// CSV quoting character
    #[arg(long, default_value_t = '"')]
    pub quote: char,

    /// CSV escape character
    #[arg(long)]
    pub escape: Option<char>,

    /// String to interpret as NULL
    #[arg(long, default_value = "")]
    pub null_value: String,
}
