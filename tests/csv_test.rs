mod fixtures;

use predicates::prelude::*;

#[test]
fn csv_dry_run_shows_inferred_schema() {
    let dir = tempfile::tempdir().unwrap();
    let csv_path = fixtures::create_test_csv(dir.path());

    fixtures::exapump()
        .args([
            "upload",
            csv_path.to_str().unwrap(),
            "--table",
            "test_schema.test_table",
            "--dsn",
            fixtures::DUMMY_DSN,
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Columns:"))
        .stdout(predicate::str::contains("\"id\""))
        .stdout(predicate::str::contains("\"name\""))
        .stdout(predicate::str::contains("\"score\""))
        .stdout(predicate::str::contains("CREATE TABLE"));
}

#[test]
fn csv_dry_run_with_custom_delimiter() {
    let dir = tempfile::tempdir().unwrap();
    let csv_path =
        fixtures::create_csv_with_content(dir.path(), "tab.csv", "id\tname\n1\thello\n2\tworld\n");

    fixtures::exapump()
        .args([
            "upload",
            csv_path.to_str().unwrap(),
            "--table",
            "my_table",
            "--dsn",
            fixtures::DUMMY_DSN,
            "--delimiter",
            "\t",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Columns:"))
        .stdout(predicate::str::contains("\"id\""))
        .stdout(predicate::str::contains("\"name\""))
        .stdout(predicate::str::contains("CREATE TABLE"));
}

#[test]
fn csv_dry_run_with_no_header() {
    let dir = tempfile::tempdir().unwrap();
    let csv_path =
        fixtures::create_csv_with_content(dir.path(), "noheader.csv", "1,hello\n2,world\n");

    fixtures::exapump()
        .args([
            "upload",
            csv_path.to_str().unwrap(),
            "--table",
            "my_table",
            "--dsn",
            fixtures::DUMMY_DSN,
            "--no-header",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"col_1\""))
        .stdout(predicate::str::contains("\"col_2\""))
        .stdout(predicate::str::contains("CREATE TABLE"));
}

#[test]
fn csv_file_not_found_error() {
    fixtures::exapump()
        .args([
            "upload",
            "nonexistent.csv",
            "--table",
            "test_schema.test_table",
            "--dsn",
            fixtures::DUMMY_DSN,
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("nonexistent.csv"));
}

#[test]
fn csv_dry_run_empty_file_header_only() {
    let dir = tempfile::tempdir().unwrap();
    let csv_path = fixtures::create_csv_with_content(dir.path(), "empty.csv", "id,name\n");

    fixtures::exapump()
        .args([
            "upload",
            csv_path.to_str().unwrap(),
            "--table",
            "my_table",
            "--dsn",
            fixtures::DUMMY_DSN,
            "--dry-run",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no data rows"));
}

#[test]
fn csv_import_empty_file_header_only() {
    let dir = tempfile::tempdir().unwrap();
    let csv_path = fixtures::create_csv_with_content(dir.path(), "empty.csv", "id,name\n");

    fixtures::exapump()
        .args([
            "upload",
            csv_path.to_str().unwrap(),
            "--table",
            "my_table",
            "--dsn",
            fixtures::DUMMY_DSN,
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no data rows"));
}

#[test]
fn csv_dry_run_with_custom_quote() {
    let dir = tempfile::tempdir().unwrap();
    let csv_path = fixtures::create_csv_with_content(
        dir.path(),
        "quoted.csv",
        "id,name\n1,'hello world'\n2,'foo bar'\n",
    );

    fixtures::exapump()
        .args([
            "upload",
            csv_path.to_str().unwrap(),
            "--table",
            "my_table",
            "--dsn",
            fixtures::DUMMY_DSN,
            "--quote",
            "'",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Columns:"))
        .stdout(predicate::str::contains("\"id\""))
        .stdout(predicate::str::contains("\"name\""))
        .stdout(predicate::str::contains("CREATE TABLE"));
}

#[test]
fn csv_dry_run_with_custom_escape() {
    let dir = tempfile::tempdir().unwrap();
    let csv_path = fixtures::create_csv_with_content(
        dir.path(),
        "escaped.csv",
        "id,value\n1,\"hello\\\"world\"\n2,\"foo\"\n",
    );

    fixtures::exapump()
        .args([
            "upload",
            csv_path.to_str().unwrap(),
            "--table",
            "my_table",
            "--dsn",
            fixtures::DUMMY_DSN,
            "--escape",
            "\\",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Columns:"))
        .stdout(predicate::str::contains("\"id\""))
        .stdout(predicate::str::contains("\"value\""))
        .stdout(predicate::str::contains("CREATE TABLE"));
}

#[test]
fn csv_dry_run_with_custom_null_value() {
    let dir = tempfile::tempdir().unwrap();
    let csv_path = fixtures::create_csv_with_content(
        dir.path(),
        "nulls.csv",
        "id,name\n1,alice\n2,NULL\n3,bob\n",
    );

    fixtures::exapump()
        .args([
            "upload",
            csv_path.to_str().unwrap(),
            "--table",
            "my_table",
            "--dsn",
            fixtures::DUMMY_DSN,
            "--null-value",
            "NULL",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Columns:"))
        .stdout(predicate::str::contains("\"id\""))
        .stdout(predicate::str::contains("\"name\""))
        .stdout(predicate::str::contains("CREATE TABLE"));
}

#[test]
fn csv_flags_ignored_for_parquet_dry_run() {
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
            "--delimiter",
            ";",
            "--no-header",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("CREATE TABLE"));
}

#[test]
fn csv_connection_failure() {
    let dir = tempfile::tempdir().unwrap();
    let csv_path = fixtures::create_test_csv(dir.path());

    fixtures::exapump()
        .args([
            "upload",
            csv_path.to_str().unwrap(),
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
async fn exasol_csv_import_to_existing_table() {
    fixtures::require_exasol!();

    let (mut conn, schema_name) = fixtures::setup_exasol_schema("EXAPUMP_CSV").await;

    conn.execute_update(&format!(
        "CREATE TABLE {schema_name}.CSV_EXISTING (\
            ID DECIMAL(18,0), \
            NAME VARCHAR(2000000), \
            SCORE DOUBLE\
        )"
    ))
    .await
    .unwrap();

    let dir = tempfile::tempdir().unwrap();
    let csv_path = fixtures::create_test_csv(dir.path());

    fixtures::exapump()
        .timeout(std::time::Duration::from_secs(60))
        .args([
            "upload",
            csv_path.to_str().unwrap(),
            "--table",
            &format!("{schema_name}.CSV_EXISTING"),
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
async fn exasol_csv_import_with_auto_table_creation() {
    fixtures::require_exasol!();

    let (mut conn, schema_name) = fixtures::setup_exasol_schema("EXAPUMP_CSV").await;

    let dir = tempfile::tempdir().unwrap();
    let csv_path = fixtures::create_test_csv(dir.path());

    let table_name = format!("{schema_name}.CSV_AUTO_CREATED");

    fixtures::exapump()
        .timeout(std::time::Duration::from_secs(60))
        .args([
            "upload",
            csv_path.to_str().unwrap(),
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

#[tokio::test]
async fn exasol_csv_import_prints_row_count() {
    fixtures::require_exasol!();

    let (mut conn, schema_name) = fixtures::setup_exasol_schema("EXAPUMP_CSV").await;

    let dir = tempfile::tempdir().unwrap();
    let csv_path = fixtures::create_csv_with_content(
        dir.path(),
        "test.csv",
        "id,name,value\n1,alice,3.14\n2,bob,2.72\n",
    );

    fixtures::exapump()
        .timeout(std::time::Duration::from_secs(60))
        .args([
            "upload",
            csv_path.to_str().unwrap(),
            "--table",
            &format!("{schema_name}.IMPORT_TEST"),
            "--dsn",
            fixtures::DOCKER_DSN,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported 2 rows"));

    let _ = conn
        .execute_update(&format!("DROP SCHEMA {schema_name} CASCADE"))
        .await;
}

#[tokio::test]
async fn exasol_csv_import_with_custom_delimiter() {
    fixtures::require_exasol!();

    let (mut conn, schema_name) = fixtures::setup_exasol_schema("EXAPUMP_CSV").await;

    let dir = tempfile::tempdir().unwrap();
    let csv_path =
        fixtures::create_csv_with_content(dir.path(), "tab.csv", "id\tname\n1\thello\n2\tworld\n");

    fixtures::exapump()
        .timeout(std::time::Duration::from_secs(60))
        .args([
            "upload",
            csv_path.to_str().unwrap(),
            "--table",
            &format!("{schema_name}.TAB_TEST"),
            "--dsn",
            fixtures::DOCKER_DSN,
            "--delimiter",
            "\t",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported 2 rows"));

    let _ = conn
        .execute_update(&format!("DROP SCHEMA {schema_name} CASCADE"))
        .await;
}

#[tokio::test]
async fn exasol_csv_import_with_no_header() {
    fixtures::require_exasol!();

    let (mut conn, schema_name) = fixtures::setup_exasol_schema("EXAPUMP_CSV").await;

    let dir = tempfile::tempdir().unwrap();
    let csv_path =
        fixtures::create_csv_with_content(dir.path(), "noheader.csv", "1,hello\n2,world\n");

    fixtures::exapump()
        .timeout(std::time::Duration::from_secs(60))
        .args([
            "upload",
            csv_path.to_str().unwrap(),
            "--table",
            &format!("{schema_name}.NOHEADER_TEST"),
            "--dsn",
            fixtures::DOCKER_DSN,
            "--no-header",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported 2 rows"));

    let _ = conn
        .execute_update(&format!("DROP SCHEMA {schema_name} CASCADE"))
        .await;
}

#[tokio::test]
async fn exasol_csv_flags_ignored_for_parquet() {
    fixtures::require_exasol!();

    let (mut conn, schema_name) = fixtures::setup_exasol_schema("EXAPUMP_CSV").await;

    let dir = tempfile::tempdir().unwrap();
    let parquet_path = fixtures::create_test_parquet(dir.path());

    fixtures::exapump()
        .timeout(std::time::Duration::from_secs(60))
        .args([
            "upload",
            parquet_path.to_str().unwrap(),
            "--table",
            &format!("{schema_name}.PARQUET_IGNORE_FLAGS"),
            "--dsn",
            fixtures::DOCKER_DSN,
            "--delimiter",
            ";",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported"))
        .stdout(predicate::str::contains("rows"));

    let _ = conn
        .execute_update(&format!("DROP SCHEMA {schema_name} CASCADE"))
        .await;
}
