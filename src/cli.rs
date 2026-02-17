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
}
