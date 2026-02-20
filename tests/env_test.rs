mod fixtures;

use predicates::prelude::*;

#[test]
fn env_file_provides_dsn() {
    let dir = tempfile::tempdir().unwrap();
    // Write .env file
    std::fs::write(
        dir.path().join(".env"),
        "EXAPUMP_DSN=exasol://env:pwd@envhost:8563\n",
    )
    .unwrap();
    // Create a test parquet for dry-run
    let parquet_path = fixtures::create_test_parquet(dir.path());

    fixtures::exapump()
        .current_dir(dir.path())
        .args([
            "upload",
            parquet_path.to_str().unwrap(),
            "--table",
            "test_table",
            "--dry-run",
        ])
        .env_remove("EXAPUMP_DSN") // ensure only .env file provides it
        .assert()
        .success()
        .stdout(predicate::str::contains("CREATE TABLE"));
}

#[test]
fn env_file_missing_is_not_error() {
    let dir = tempfile::tempdir().unwrap();
    fixtures::exapump()
        .current_dir(dir.path())
        .env_remove("EXAPUMP_DSN")
        .arg("--help")
        .assert()
        .success();
}

#[test]
fn shell_env_overrides_env_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".env"),
        "EXAPUMP_DSN=exasol://file:pwd@filehost:8563\n",
    )
    .unwrap();
    let parquet_path = fixtures::create_test_parquet(dir.path());

    // Shell env should win over .env file — both values work for dry-run
    fixtures::exapump()
        .current_dir(dir.path())
        .env("EXAPUMP_DSN", "exasol://shell:pwd@shellhost:8563")
        .args([
            "upload",
            parquet_path.to_str().unwrap(),
            "--table",
            "test_table",
            "--dry-run",
        ])
        .assert()
        .success();
}

#[test]
fn cli_flag_overrides_env_file_and_shell() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".env"),
        "EXAPUMP_DSN=exasol://file:pwd@filehost:8563\n",
    )
    .unwrap();
    let parquet_path = fixtures::create_test_parquet(dir.path());

    fixtures::exapump()
        .current_dir(dir.path())
        .env("EXAPUMP_DSN", "exasol://shell:pwd@shellhost:8563")
        .args([
            "upload",
            parquet_path.to_str().unwrap(),
            "--table",
            "test_table",
            "--dsn",
            "exasol://flag:pwd@flaghost:8563",
            "--dry-run",
        ])
        .assert()
        .success();
}
