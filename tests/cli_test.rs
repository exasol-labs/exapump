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
        .stdout(predicate::str::contains("0.2.0"));
}

#[test]
fn no_arguments_shows_help() {
    fixtures::exapump()
        .assert()
        .code(2)
        .stdout(predicate::str::contains("upload"))
        .stdout(predicate::str::contains("sql"))
        .stdout(predicate::str::contains("export"));
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
            predicate::str::contains("--null-value").and(predicate::str::contains("[default: ]")),
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
        .stdout(predicate::str::contains("--format"))
        .stdout(predicate::str::contains("csv"))
        .stdout(predicate::str::contains("json"));
}

#[test]
fn sql_missing_dsn() {
    fixtures::exapump()
        .args(["sql", "SELECT 1"])
        .env_remove("EXAPUMP_DSN")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--dsn").or(predicate::str::contains("required")));
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
        .stdout(predicate::str::contains("--delimiter"))
        .stdout(predicate::str::contains("--quote"))
        .stdout(predicate::str::contains("--no-header"))
        .stdout(predicate::str::contains("--null-value"));
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
