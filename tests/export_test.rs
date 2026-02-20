mod fixtures;

use predicates::prelude::*;

/// Helper to create and populate a test table via SQL.
fn setup_table(schema: &str, table: &str) {
    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DOCKER_DSN)
        .args([
            "sql",
            &format!(
                "CREATE TABLE {schema}.{table} (id INT, name VARCHAR(50), score DOUBLE); \
                 INSERT INTO {schema}.{table} VALUES (1, 'Alice', 95.5); \
                 INSERT INTO {schema}.{table} VALUES (2, 'Bob', 87.0); \
                 INSERT INTO {schema}.{table} VALUES (3, 'Charlie', 92.3);"
            ),
        ])
        .assert()
        .success();
}

fn setup_schema(prefix: &str) -> String {
    let schema = format!(
        "{prefix}_{}_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
        std::process::id(),
    );
    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DOCKER_DSN)
        .args(["sql", &format!("CREATE SCHEMA IF NOT EXISTS {schema}")])
        .assert()
        .success();
    schema
}

fn teardown_schema(schema: &str) {
    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DOCKER_DSN)
        .args(["sql", &format!("DROP SCHEMA IF EXISTS {schema} CASCADE")])
        .assert()
        .success();
}

#[test]
fn export_table_to_csv() {
    fixtures::require_exasol!();
    let schema = setup_schema("exp_tbl");
    setup_table(&schema, "test_data");

    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("output.csv");

    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DOCKER_DSN)
        .args([
            "export",
            "--table",
            &format!("{schema}.test_data"),
            "--output",
            output.to_str().unwrap(),
            "--format",
            "csv",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("Exported 4 rows"));

    let content = std::fs::read_to_string(&output).unwrap();
    assert!(content.contains("Alice"));
    assert!(content.contains("Bob"));
    assert!(content.contains("Charlie"));

    teardown_schema(&schema);
}

#[test]
fn export_query_to_csv() {
    fixtures::require_exasol!();
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("query.csv");

    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DOCKER_DSN)
        .args([
            "export",
            "--query",
            "SELECT 1 AS n, 2 AS m",
            "--output",
            output.to_str().unwrap(),
            "--format",
            "csv",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("Exported 2 rows"));

    let content = std::fs::read_to_string(&output).unwrap();
    assert!(content.contains("1"));
    assert!(content.contains("2"));
}

#[test]
fn export_with_no_header() {
    fixtures::require_exasol!();
    let schema = setup_schema("exp_noh");
    setup_table(&schema, "test_data");

    let dir = tempfile::tempdir().unwrap();
    let with_header = dir.path().join("with_header.csv");
    let without_header = dir.path().join("without_header.csv");

    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DOCKER_DSN)
        .args([
            "export",
            "--table",
            &format!("{schema}.test_data"),
            "--output",
            with_header.to_str().unwrap(),
            "--format",
            "csv",
        ])
        .assert()
        .success();

    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DOCKER_DSN)
        .args([
            "export",
            "--table",
            &format!("{schema}.test_data"),
            "--output",
            without_header.to_str().unwrap(),
            "--format",
            "csv",
            "--no-header",
        ])
        .assert()
        .success();

    let with_h = std::fs::read_to_string(&with_header).unwrap();
    let without_h = std::fs::read_to_string(&without_header).unwrap();

    // The with-header version should have more lines than without
    let with_lines: Vec<&str> = with_h.lines().collect();
    let without_lines: Vec<&str> = without_h.lines().collect();
    assert!(
        with_lines.len() > without_lines.len(),
        "Expected with-header ({}) to have more lines than without-header ({})",
        with_lines.len(),
        without_lines.len(),
    );

    teardown_schema(&schema);
}

#[test]
fn export_with_custom_delimiter() {
    fixtures::require_exasol!();
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("tab.csv");

    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DOCKER_DSN)
        .args([
            "export",
            "--query",
            "SELECT 1 AS a, 2 AS b",
            "--output",
            output.to_str().unwrap(),
            "--format",
            "csv",
            "--delimiter",
            "\t",
        ])
        .assert()
        .success();

    let content = std::fs::read_to_string(&output).unwrap();
    assert!(content.contains('\t'), "Expected tab-separated output");
}

#[test]
fn export_empty_table() {
    fixtures::require_exasol!();
    let schema = setup_schema("exp_empty");

    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DOCKER_DSN)
        .args([
            "sql",
            &format!("CREATE TABLE {schema}.empty_tbl (id INT, name VARCHAR(50))"),
        ])
        .assert()
        .success();

    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("empty.csv");

    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DOCKER_DSN)
        .args([
            "export",
            "--table",
            &format!("{schema}.empty_tbl"),
            "--output",
            output.to_str().unwrap(),
            "--format",
            "csv",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("Exported 1 rows"));

    teardown_schema(&schema);
}

#[test]
fn export_with_custom_quote() {
    fixtures::require_exasol!();
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("quoted.csv");

    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DOCKER_DSN)
        .args([
            "export",
            "--query",
            "SELECT 'hello,world' AS greeting",
            "--output",
            output.to_str().unwrap(),
            "--format",
            "csv",
            "--quote",
            "|",
        ])
        .assert()
        .success();

    let content = std::fs::read_to_string(&output).unwrap();
    assert!(
        content.contains("|hello,world|"),
        "Expected pipe-quoted value in output, got: {content}"
    );
}

#[test]
fn export_with_custom_null_value() {
    fixtures::require_exasol!();
    let schema = setup_schema("exp_null");

    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DOCKER_DSN)
        .args([
            "sql",
            &format!(
                "CREATE TABLE {schema}.nulls (id INT, name VARCHAR(50)); \
                 INSERT INTO {schema}.nulls VALUES (1, 'Alice'); \
                 INSERT INTO {schema}.nulls VALUES (2, NULL);"
            ),
        ])
        .assert()
        .success();

    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("nulls.csv");

    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DOCKER_DSN)
        .args([
            "export",
            "--table",
            &format!("{schema}.nulls"),
            "--output",
            output.to_str().unwrap(),
            "--format",
            "csv",
            "--null-value",
            "N/A",
        ])
        .assert()
        .success();

    let content = std::fs::read_to_string(&output).unwrap();
    assert!(content.contains("N/A"), "Expected N/A for NULL values");

    teardown_schema(&schema);
}

#[test]
fn export_table_not_found() {
    fixtures::require_exasol!();

    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("notfound.csv");

    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DOCKER_DSN)
        .args([
            "export",
            "--table",
            "nonexistent_schema_xyz.nonexistent_table",
            "--output",
            output.to_str().unwrap(),
            "--format",
            "csv",
        ])
        .assert()
        .failure();
}

#[test]
fn export_query_error() {
    fixtures::require_exasol!();

    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("error.csv");

    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DOCKER_DSN)
        .args([
            "export",
            "--query",
            "SELECT * FROM nonexistent_table_xyz_abc",
            "--output",
            output.to_str().unwrap(),
            "--format",
            "csv",
        ])
        .assert()
        .failure();
}

#[test]
fn export_output_not_writable() {
    fixtures::require_exasol!();

    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DOCKER_DSN)
        .args([
            "export",
            "--query",
            "SELECT 1 AS n",
            "--output",
            "/nonexistent_dir_xyz/output.csv",
            "--format",
            "csv",
        ])
        .assert()
        .failure();
}
