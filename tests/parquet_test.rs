mod fixtures;

use predicates::prelude::*;

#[test]
fn dry_run_shows_inferred_schema() {
    let dir = tempfile::tempdir().unwrap();
    let parquet_path = fixtures::create_test_parquet(dir.path());

    fixtures::exapump()
        .args([
            "upload",
            parquet_path.to_str().unwrap(),
            "--table",
            "test_schema.test_table",
            "--dsn",
            fixtures::DUMMY_DSN,
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Columns:"))
        .stdout(predicate::str::contains("ID"))
        .stdout(predicate::str::contains("NAME"))
        .stdout(predicate::str::contains("SCORE"))
        .stdout(predicate::str::contains("CREATE TABLE"));
}

#[test]
fn file_not_found_error() {
    fixtures::exapump()
        .args([
            "upload",
            "nonexistent.parquet",
            "--table",
            "test_schema.test_table",
            "--dsn",
            fixtures::DUMMY_DSN,
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("nonexistent.parquet"));
}

#[test]
fn unsupported_file_extension() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = dir.path().join("data.json");
    std::fs::write(&json_path, r#"{"a":1}"#).unwrap();

    fixtures::exapump()
        .args([
            "upload",
            json_path.to_str().unwrap(),
            "--table",
            "test_schema.test_table",
            "--dsn",
            fixtures::DUMMY_DSN,
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not supported"))
        .stderr(predicate::str::contains(".parquet, .csv"));
}

#[test]
fn connection_failure() {
    let dir = tempfile::tempdir().unwrap();
    let parquet_path = fixtures::create_test_parquet(dir.path());

    fixtures::exapump()
        .args([
            "upload",
            parquet_path.to_str().unwrap(),
            "--table",
            "test_schema.test_table",
            "--dsn",
            "exasol://bad:bad@nowhere:9999",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::is_empty().not());
}

#[tokio::test]
async fn exasol_parquet_import_to_existing_table() {
    fixtures::require_exasol!();

    let (mut conn, schema_name) = fixtures::setup_exasol_schema("EXAPUMP_PQ").await;

    conn.execute_update(&format!(
        "CREATE TABLE {schema_name}.UPLOAD_EXISTING (\
            ID DECIMAL(18,0), \
            NAME VARCHAR(2000000), \
            SCORE DOUBLE\
        )"
    ))
    .await
    .unwrap();

    let dir = tempfile::tempdir().unwrap();
    let parquet_path = fixtures::create_test_parquet(dir.path());

    fixtures::exapump()
        .timeout(std::time::Duration::from_secs(60))
        .args([
            "upload",
            parquet_path.to_str().unwrap(),
            "--table",
            &format!("{schema_name}.UPLOAD_EXISTING"),
            "--dsn",
            fixtures::DOCKER_DSN,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported"))
        .stdout(predicate::str::contains("rows"));

    let _ = conn
        .execute_update(&format!("DROP SCHEMA {schema_name} CASCADE"))
        .await;
}

#[tokio::test]
async fn exasol_parquet_import_with_auto_table_creation() {
    fixtures::require_exasol!();

    let (mut conn, schema_name) = fixtures::setup_exasol_schema("EXAPUMP_PQ").await;

    let dir = tempfile::tempdir().unwrap();
    let parquet_path = fixtures::create_test_parquet(dir.path());

    let table_name = format!("{schema_name}.AUTO_CREATED");

    fixtures::exapump()
        .timeout(std::time::Duration::from_secs(60))
        .args([
            "upload",
            parquet_path.to_str().unwrap(),
            "--table",
            &table_name,
            "--dsn",
            fixtures::DOCKER_DSN,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported"))
        .stdout(predicate::str::contains("rows"));

    let _ = conn
        .execute_update(&format!("DROP SCHEMA {schema_name} CASCADE"))
        .await;
}
