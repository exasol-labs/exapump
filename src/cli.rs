use clap::{Parser, Subcommand};

/// The simplest path from file to Exasol table — import, export, and SQL in one command.
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
    /// Execute SQL statements against Exasol
    Sql(SqlArgs),
}

#[derive(clap::Args)]
pub struct SqlArgs {
    /// SQL statement to execute (reads from stdin if omitted or if '-' is given)
    pub sql: Option<String>,

    #[command(flatten)]
    pub conn: crate::connection::ConnectionArgs,

    /// Output format for SELECT results
    #[arg(short, long, value_enum, default_value_t = OutputFormat::Csv)]
    pub format: OutputFormat,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum OutputFormat {
    Csv,
    Json,
}

#[derive(clap::Args)]
pub struct UploadArgs {
    /// Files to upload
    #[arg(required = true)]
    pub files: Vec<String>,

    /// Target table name (e.g., schema.table)
    #[arg(short, long)]
    pub table: String,

    #[command(flatten)]
    pub conn: crate::connection::ConnectionArgs,

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
