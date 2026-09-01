use clap::{Args, Parser, Subcommand};

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
    /// Export an Exasol table or query result to a file
    Export(ExportArgs),
    /// Start an interactive SQL session
    Interactive(InteractiveArgs),
    /// Manage connection profiles
    Profile(crate::commands::profile::ProfileArgs),
    /// Interact with BucketFS (list, copy, delete files)
    Bucketfs(BucketFsArgs),
    /// Wait until Exasol is ready
    Wait(WaitArgs),
}

#[derive(clap::Args)]
pub struct SqlArgs {
    /// SQL statement to execute (reads from stdin if omitted or if '-' is given)
    pub sql: Option<String>,

    #[command(flatten)]
    pub conn: crate::connection::ConnectionArgs,

    /// Output format for SELECT results (to run SQL held in a file, pipe it in:
    /// `exapump sql - < query.sql`)
    #[arg(short, long, value_parser = OutputFormatParser, default_value = "csv")]
    pub format: OutputFormat,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum OutputFormat {
    Csv,
    Json,
}

/// clap's enum parser for `--format`, with one extra sentence for the value
/// that gets typed most often by mistake.
///
/// `-f` reads as "file", so `exapump sql -f query.sql` is a common first
/// attempt. Plain clap answers `invalid value 'query.sql' for '--format'`,
/// which names the wrong flag as the problem and leaves the user no closer to
/// running the file.
#[derive(Clone)]
pub struct OutputFormatParser;

impl clap::builder::TypedValueParser for OutputFormatParser {
    type Value = OutputFormat;

    fn parse_ref(
        &self,
        cmd: &clap::Command,
        arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        clap::builder::EnumValueParser::<OutputFormat>::new()
            .parse_ref(cmd, arg, value)
            .map_err(|err| file_mistaken_for_format(cmd, value).unwrap_or(err))
    }

    fn possible_values(
        &self,
    ) -> Option<Box<dyn Iterator<Item = clap::builder::PossibleValue> + '_>> {
        Some(Box::new(
            <OutputFormat as clap::ValueEnum>::value_variants()
                .iter()
                .filter_map(clap::ValueEnum::to_possible_value),
        ))
    }
}

/// Replace the `--format` rejection when the value looks like a file name.
///
/// Returns `None` for anything else — a plain typo such as `--format jsonl` is
/// better served by clap's own message.
fn file_mistaken_for_format(cmd: &clap::Command, value: &std::ffi::OsStr) -> Option<clap::Error> {
    let value = value.to_str()?;
    if !looks_like_a_file_name(value) {
        return None;
    }

    Some(cmd.clone().error(
        clap::error::ErrorKind::InvalidValue,
        format!(
            "invalid value '{value}' for '--format <FORMAT>' [possible values: csv, json]\n\n  \
             -f/--format picks the output format, not an input file.\n  \
             To run the SQL held in {value}, pipe it in: exapump sql - < {value}"
        ),
    ))
}

fn looks_like_a_file_name(value: &str) -> bool {
    value.contains('/') || value.contains('\\') || std::path::Path::new(value).extension().is_some()
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ExportFormat {
    Csv,
    Parquet,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum Compression {
    Snappy,
    Gzip,
    Lz4,
    Zstd,
    None,
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

/// Largest `--timeout` value whose seconds-to-milliseconds conversion in
/// `build_csv_options` (`secs * 1000`) cannot overflow `u64`.
const MAX_TIMEOUT_SECONDS: u64 = u64::MAX / 1000;

#[derive(clap::Args)]
pub struct ExportArgs {
    /// Table to export (e.g., schema.table)
    #[arg(
        short,
        long,
        conflicts_with = "query",
        required_unless_present = "query"
    )]
    pub table: Option<String>,

    /// SQL query to export results from
    #[arg(
        short,
        long,
        conflicts_with = "table",
        required_unless_present = "table"
    )]
    pub query: Option<String>,

    /// Output file path
    #[arg(short, long)]
    pub output: String,

    /// Export format
    #[arg(short, long, value_enum)]
    pub format: ExportFormat,

    #[command(flatten)]
    pub conn: crate::connection::ConnectionArgs,

    /// CSV field delimiter
    #[arg(long, default_value_t = ',')]
    pub delimiter: char,

    /// CSV quoting character
    #[arg(long, default_value_t = '"')]
    pub quote: char,

    /// Exclude header row from output
    #[arg(long)]
    pub no_header: bool,

    /// String to represent NULL values
    #[arg(long, default_value = "")]
    pub null_value: String,

    /// Client-side export deadline in seconds (CSV format only)
    ///
    /// On a split export (--max-rows-per-file or --max-file-size) the deadline
    /// bounds only the download phase; the file writing that follows it runs
    /// unbounded.
    #[arg(long, value_name = "SECONDS", value_parser = clap::value_parser!(u64).range(1..=MAX_TIMEOUT_SECONDS))]
    pub timeout: Option<u64>,

    /// Compression codec (Parquet only)
    #[arg(long, value_enum)]
    pub compression: Option<Compression>,

    /// Maximum number of rows per output file (enables file splitting)
    #[arg(long)]
    pub max_rows_per_file: Option<u64>,

    /// Maximum file size per output file, e.g. 500KB, 1MB, 2GB (enables file splitting)
    #[arg(long)]
    pub max_file_size: Option<String>,
}

#[derive(Args)]
pub struct InteractiveArgs {
    #[command(flatten)]
    pub conn: crate::connection::ConnectionArgs,
}

#[derive(clap::Args)]
pub struct BucketFsArgs {
    #[command(subcommand)]
    pub command: BucketfsCommands,
}

#[derive(clap::Args, Clone)]
pub struct BfsConnectionOverrides {
    /// Connection profile name
    #[arg(long)]
    pub profile: Option<String>,

    /// BucketFS host override
    #[arg(long)]
    pub bfs_host: Option<String>,

    /// BucketFS port override
    #[arg(long)]
    pub bfs_port: Option<u16>,

    /// BucketFS bucket override
    #[arg(long)]
    pub bfs_bucket: Option<String>,

    /// BucketFS write password override
    #[arg(long)]
    pub bfs_write_password: Option<String>,

    /// BucketFS read password override
    #[arg(long)]
    pub bfs_read_password: Option<String>,

    /// BucketFS TLS override
    #[arg(long)]
    pub bfs_tls: Option<bool>,

    /// BucketFS certificate validation override
    #[arg(long)]
    pub bfs_validate_certificate: Option<bool>,
}

#[derive(clap::Args)]
pub struct WaitArgs {
    #[command(flatten)]
    pub conn: crate::connection::ConnectionArgs,

    /// Docker container name to monitor (optional; skips Docker checks when omitted)
    #[arg(long)]
    pub container: Option<String>,

    /// Maximum seconds to wait before timing out
    #[arg(long, default_value_t = 1500)]
    pub timeout_secs: u64,
}

#[derive(Subcommand)]
pub enum BucketfsCommands {
    /// List files in a bucket
    Ls {
        /// Path within the bucket to list
        path: Option<String>,

        /// List files recursively
        #[arg(short, long)]
        recursive: bool,

        #[command(flatten)]
        conn: BfsConnectionOverrides,
    },
    /// Copy files to/from BucketFS (direction auto-detected)
    Cp {
        /// Source path (local file or BucketFS path)
        source: String,

        /// Destination path (BucketFS path or local file)
        destination: String,

        #[command(flatten)]
        conn: BfsConnectionOverrides,
    },
    /// Delete a file from BucketFS
    Rm {
        /// Path of the file to delete
        path: String,

        #[command(flatten)]
        conn: BfsConnectionOverrides,
    },
}
