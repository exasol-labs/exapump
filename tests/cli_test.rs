mod fixtures;

use predicates::prelude::*;

#[test]
fn display_top_level_help() {
    fixtures::exapump()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "The simplest path from file to Exasol table",
        ))
        .stdout(predicate::str::contains("upload"))
        .stdout(predicate::str::contains("sql"))
        .stdout(predicate::str::contains("export"))
        .stdout(predicate::str::contains("interactive"))
        .stdout(predicate::str::contains("profile"))
        .stdout(predicate::str::contains("bucketfs"))
        .stdout(predicate::str::contains("--help"))
        .stdout(predicate::str::contains("--version"));
}

#[test]
fn display_version() {
    fixtures::exapump()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("exapump"))
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn no_arguments_shows_help() {
    fixtures::exapump()
        .assert()
        .code(2)
        .stdout(predicate::str::contains("upload"))
        .stdout(predicate::str::contains("sql"))
        .stdout(predicate::str::contains("export"))
        .stdout(predicate::str::contains("interactive"))
        .stdout(predicate::str::contains("bucketfs"));
}

#[test]
fn upload_help_shows_all_arguments() {
    fixtures::exapump()
        .args(["upload", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("<FILES>"))
        .stdout(predicate::str::contains("--table"))
        .stdout(predicate::str::contains("--dsn"))
        .stdout(predicate::str::contains("--profile"))
        .stdout(predicate::str::contains("--dry-run"))
        .stdout(predicate::str::contains("--delimiter"))
        .stdout(predicate::str::contains("--no-header"))
        .stdout(predicate::str::contains("--quote"))
        .stdout(predicate::str::contains("--escape"))
        .stdout(predicate::str::contains("--null-value"));
}

#[test]
fn csv_flags_shown_with_defaults_in_help() {
    fixtures::exapump()
        .args(["upload", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("--delimiter").and(predicate::str::contains("[default: ,]")),
        )
        .stdout(predicate::str::contains("--quote").and(predicate::str::contains("[default: \"]")))
        .stdout(
            predicate::str::contains("--null-value")
                .and(predicate::str::contains("[default: \"\"]")),
        );
}

#[test]
fn missing_required_arguments() {
    fixtures::exapump()
        .arg("upload")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn dsn_from_environment_variable() {
    let dir = tempfile::tempdir().unwrap();
    let parquet_path = fixtures::create_test_parquet(dir.path());

    fixtures::exapump()
        .env("EXAPUMP_DSN", "exasol://env:pwd@host:8563")
        .args([
            "upload",
            parquet_path.to_str().unwrap(),
            "--table",
            "my_schema.my_table",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("CREATE TABLE"));
}

#[test]
fn dsn_flag_overrides_environment_variable() {
    let dir = tempfile::tempdir().unwrap();
    let parquet_path = fixtures::create_test_parquet(dir.path());

    fixtures::exapump()
        .env("EXAPUMP_DSN", "exasol://env:pwd@host:8563")
        .args([
            "upload",
            parquet_path.to_str().unwrap(),
            "--table",
            "my_schema.my_table",
            "--dsn",
            "exasol://flag:pwd@host:8563",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("CREATE TABLE"));
}

// --- SQL subcommand tests ---

#[test]
fn sql_help_shows_all_arguments() {
    fixtures::exapump()
        .args(["sql", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("[SQL]"))
        .stdout(predicate::str::contains("--dsn"))
        .stdout(predicate::str::contains("--profile"))
        .stdout(predicate::str::contains("--format"))
        .stdout(predicate::str::contains("csv"))
        .stdout(predicate::str::contains("json"));
}

#[test]
fn sql_missing_dsn() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("nonexistent_config.toml");

    fixtures::exapump()
        .env("EXAPUMP_CONFIG", config_path.to_str().unwrap())
        .env_remove("EXAPUMP_DSN")
        .args(["sql", "SELECT 1"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("No profiles found in config")
                .or(predicate::str::contains("--dsn"))
                .or(predicate::str::contains("No connection")),
        );
}

#[test]
fn sql_dsn_from_environment_variable() {
    // This will fail to connect but should get past argument parsing
    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DUMMY_DSN)
        .args(["sql", "SELECT 1"])
        .assert()
        .failure() // fails at connection, not at argument parsing
        .stderr(
            predicate::str::contains("connect")
                .or(predicate::str::contains("error"))
                .or(predicate::str::contains("Error")),
        );
}

#[test]
fn sql_empty_stdin_produces_error() {
    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DUMMY_DSN)
        .arg("sql")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No SQL statements to execute"));
}

#[test]
fn sql_stdin_pipe() {
    // Pipe SQL via stdin — will fail at connection, not at argument parsing
    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DUMMY_DSN)
        .arg("sql")
        .write_stdin("SELECT 1")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("connect")
                .or(predicate::str::contains("error"))
                .or(predicate::str::contains("Error")),
        );
}

#[test]
fn sql_stdin_dash() {
    // Use "-" to explicitly read from stdin
    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DUMMY_DSN)
        .args(["sql", "-"])
        .write_stdin("SELECT 1")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("connect")
                .or(predicate::str::contains("error"))
                .or(predicate::str::contains("Error")),
        );
}

#[test]
fn sql_default_format_is_csv() {
    // sql --help should show csv as default
    fixtures::exapump()
        .args(["sql", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("[default: csv]"));
}

// --- Export subcommand tests ---

#[test]
fn export_help_shows_all_arguments() {
    fixtures::exapump()
        .args(["export", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--table"))
        .stdout(predicate::str::contains("--query"))
        .stdout(predicate::str::contains("--output"))
        .stdout(predicate::str::contains("--format"))
        .stdout(predicate::str::contains("--dsn"))
        .stdout(predicate::str::contains("--profile"))
        .stdout(predicate::str::contains("--delimiter"))
        .stdout(predicate::str::contains("--quote"))
        .stdout(predicate::str::contains("--no-header"))
        .stdout(predicate::str::contains("--null-value"))
        .stdout(predicate::str::contains("--compression"))
        .stdout(predicate::str::contains("--max-rows-per-file"))
        .stdout(predicate::str::contains("--max-file-size"));
}

#[test]
fn export_missing_required_arguments() {
    fixtures::exapump()
        .arg("export")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn export_table_and_query_mutually_exclusive() {
    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DUMMY_DSN)
        .args([
            "export",
            "--table",
            "schema.table",
            "--query",
            "SELECT 1",
            "--output",
            "/tmp/test.csv",
            "--format",
            "csv",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn export_either_table_or_query_required() {
    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DUMMY_DSN)
        .args(["export", "--output", "/tmp/test.csv", "--format", "csv"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn export_format_is_required() {
    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DUMMY_DSN)
        .args([
            "export",
            "--table",
            "schema.table",
            "--output",
            "/tmp/test.csv",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn export_dsn_from_environment_variable() {
    // Will fail at connection, not at argument parsing
    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DUMMY_DSN)
        .args([
            "export",
            "--table",
            "schema.table",
            "--output",
            "/tmp/test.csv",
            "--format",
            "csv",
        ])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("connect")
                .or(predicate::str::contains("error"))
                .or(predicate::str::contains("Error")),
        );
}

#[test]
fn export_dsn_flag_overrides_environment_variable() {
    fixtures::exapump()
        .env("EXAPUMP_DSN", "exasol://env:pwd@host:8563")
        .args([
            "export",
            "--table",
            "schema.table",
            "--output",
            "/tmp/test.csv",
            "--format",
            "csv",
            "--dsn",
            "exasol://flag:pwd@host:8563",
        ])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("connect")
                .or(predicate::str::contains("error"))
                .or(predicate::str::contains("Error")),
        );
}

#[test]
fn export_format_accepts_parquet() {
    // --format parquet is accepted (will fail at connection, not arg parsing)
    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DUMMY_DSN)
        .args([
            "export",
            "--table",
            "schema.table",
            "--output",
            "/tmp/test.parquet",
            "--format",
            "parquet",
        ])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("connect")
                .or(predicate::str::contains("error"))
                .or(predicate::str::contains("Error")),
        );
}

#[test]
fn export_compression_rejected_for_csv() {
    // --compression with --format csv should fail with a descriptive error
    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DUMMY_DSN)
        .args([
            "export",
            "--table",
            "schema.table",
            "--output",
            "/tmp/test.csv",
            "--format",
            "csv",
            "--compression",
            "snappy",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("compression").and(predicate::str::contains("Parquet")));
}

#[test]
fn export_invalid_compression_rejected() {
    // An invalid compression value should be rejected by clap with valid values listed
    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DUMMY_DSN)
        .args([
            "export",
            "--table",
            "schema.table",
            "--output",
            "/tmp/test.parquet",
            "--format",
            "parquet",
            "--compression",
            "brotli",
        ])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("invalid value")
                .or(predicate::str::contains("possible values")),
        );
}

// --- Interactive subcommand tests ---

#[test]
fn interactive_help_shows_all_arguments() {
    fixtures::exapump()
        .args(["interactive", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--dsn"))
        .stdout(predicate::str::contains("--profile"))
        .stdout(
            predicate::str::contains("interactive SQL session")
                .or(predicate::str::contains("Interactive")),
        );
}

#[test]
fn interactive_missing_dsn() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("nonexistent_config.toml");

    fixtures::exapump()
        .env("EXAPUMP_CONFIG", config_path.to_str().unwrap())
        .env_remove("EXAPUMP_DSN")
        .args(["interactive"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("No profiles found in config")
                .or(predicate::str::contains("--dsn"))
                .or(predicate::str::contains("No connection")),
        );
}

// --- Profile subcommand tests ---

#[test]
fn profile_help_shows_subcommands() {
    fixtures::exapump()
        .args(["profile", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("show"))
        .stdout(predicate::str::contains("add"))
        .stdout(predicate::str::contains("remove"));
}

// --- BucketFS subcommand tests ---

#[test]
fn bucketfs_help_shows_subcommands_and_flags() {
    fixtures::exapump()
        .args(["bucketfs", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ls"))
        .stdout(predicate::str::contains("cp"))
        .stdout(predicate::str::contains("rm"));

    // Connection flags are on each subcommand
    fixtures::exapump()
        .args(["bucketfs", "ls", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--profile"))
        .stdout(predicate::str::contains("--bfs-host"))
        .stdout(predicate::str::contains("--bfs-port"))
        .stdout(predicate::str::contains("--bfs-bucket"))
        .stdout(predicate::str::contains("--bfs-write-password"))
        .stdout(predicate::str::contains("--bfs-read-password"));
}

#[test]
fn bucketfs_ls_help_shows_path_and_recursive() {
    fixtures::exapump()
        .args(["bucketfs", "ls", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("[PATH]"))
        .stdout(predicate::str::contains("--recursive"));
}

#[test]
fn bucketfs_cp_help_shows_source_and_destination() {
    fixtures::exapump()
        .args(["bucketfs", "cp", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("<SOURCE>"))
        .stdout(predicate::str::contains("<DESTINATION>"));
}

#[test]
fn bucketfs_rm_help_shows_path() {
    fixtures::exapump()
        .args(["bucketfs", "rm", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("<PATH>"));
}

#[test]
fn certificate_fingerprint_flag_in_help_for_all_commands() {
    for cmd in ["upload", "export", "sql", "interactive"] {
        fixtures::exapump()
            .args([cmd, "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("--certificate-fingerprint"))
            .stdout(predicate::str::contains("SHA-256"));
    }
}

#[test]
fn profile_add_help_includes_certificate_fingerprint() {
    fixtures::exapump()
        .args(["profile", "add", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--certificate-fingerprint"));
}

#[test]
fn profile_add_help_includes_bucketfs_flags() {
    fixtures::exapump()
        .args(["profile", "add", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--bfs-host"))
        .stdout(predicate::str::contains("--bfs-port"))
        .stdout(predicate::str::contains("--bfs-bucket"))
        .stdout(predicate::str::contains("--bfs-write-password"))
        .stdout(predicate::str::contains("--bfs-read-password"))
        .stdout(predicate::str::contains("--bfs-tls"))
        .stdout(predicate::str::contains("--bfs-validate-certificate"));
}
