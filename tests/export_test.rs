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
        .timeout(std::time::Duration::from_secs(10))
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
        .timeout(std::time::Duration::from_secs(10))
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

// --- Parquet export integration tests ---

#[test]
fn export_table_to_parquet() {
    fixtures::require_exasol!();
    let schema = setup_schema("exp_pq_tbl");
    setup_table(&schema, "test_data");

    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("output.parquet");

    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DOCKER_DSN)
        .args([
            "export",
            "--table",
            &format!("{schema}.test_data"),
            "--output",
            output.to_str().unwrap(),
            "--format",
            "parquet",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("Exported"));

    // Verify the file is valid Parquet and has rows
    let file = std::fs::File::open(&output).unwrap();
    let reader = parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(file)
        .unwrap()
        .build()
        .unwrap();
    let batches: Vec<_> = reader.map(|r| r.unwrap()).collect();
    let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
    assert_eq!(total_rows, 3);

    teardown_schema(&schema);
}

#[test]
fn export_query_to_parquet() {
    fixtures::require_exasol!();
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("query.parquet");

    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DOCKER_DSN)
        .args([
            "export",
            "--query",
            "SELECT 1 AS n, 'hello' AS msg",
            "--output",
            output.to_str().unwrap(),
            "--format",
            "parquet",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("Exported"));

    let file = std::fs::File::open(&output).unwrap();
    let reader = parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(file)
        .unwrap()
        .build()
        .unwrap();
    let batches: Vec<_> = reader.map(|r| r.unwrap()).collect();
    let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
    assert_eq!(total_rows, 1);
}

#[test]
fn export_parquet_with_compression() {
    fixtures::require_exasol!();
    let schema = setup_schema("exp_pq_comp");
    setup_table(&schema, "test_data");

    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("compressed.parquet");

    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DOCKER_DSN)
        .args([
            "export",
            "--table",
            &format!("{schema}.test_data"),
            "--output",
            output.to_str().unwrap(),
            "--format",
            "parquet",
            "--compression",
            "zstd",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("Exported"));

    // Verify the file exists and is valid Parquet
    assert!(output.exists());
    let file = std::fs::File::open(&output).unwrap();
    let reader = parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(file)
        .unwrap()
        .build()
        .unwrap();
    let batches: Vec<_> = reader.map(|r| r.unwrap()).collect();
    let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
    assert_eq!(total_rows, 3);

    teardown_schema(&schema);
}

#[test]
fn export_parquet_empty_table() {
    fixtures::require_exasol!();
    let schema = setup_schema("exp_pq_empty");

    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DOCKER_DSN)
        .args([
            "sql",
            &format!("CREATE TABLE {schema}.empty_tbl (id INT, name VARCHAR(50))"),
        ])
        .assert()
        .success();

    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("empty.parquet");

    // Exporting an empty table to Parquet succeeds and reports 0 rows.
    // Note: the underlying library returns Ok(0) without writing a file
    // when the result set is empty, so we only check the exit code and
    // stderr message — we do not attempt to read the (non-existent) file.
    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DOCKER_DSN)
        .args([
            "export",
            "--table",
            &format!("{schema}.empty_tbl"),
            "--output",
            output.to_str().unwrap(),
            "--format",
            "parquet",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("Exported 0 rows"));

    teardown_schema(&schema);
}

#[test]
fn export_parquet_split_by_rows() {
    fixtures::require_exasol!();
    let schema = setup_schema("exp_pq_split");
    setup_table(&schema, "test_data");

    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("split.parquet");

    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DOCKER_DSN)
        .args([
            "export",
            "--table",
            &format!("{schema}.test_data"),
            "--output",
            output.to_str().unwrap(),
            "--format",
            "parquet",
            "--max-rows-per-file",
            "1",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("file(s)"));

    // With max-rows-per-file=1 and 3 rows, we should get multiple files
    let split_0 = dir.path().join("split_000.parquet");
    let split_1 = dir.path().join("split_001.parquet");
    assert!(split_0.exists(), "Expected split_000.parquet to exist");
    assert!(split_1.exists(), "Expected split_001.parquet to exist");

    teardown_schema(&schema);
}

#[test]
fn export_parquet_split_single_file_rename() {
    fixtures::require_exasol!();
    let schema = setup_schema("exp_pq_single");
    setup_table(&schema, "test_data");

    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("data.parquet");

    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DOCKER_DSN)
        .args([
            "export",
            "--table",
            &format!("{schema}.test_data"),
            "--output",
            output.to_str().unwrap(),
            "--format",
            "parquet",
            "--max-rows-per-file",
            "9999",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("file(s)"));

    // With a high row limit, only one file should be produced
    // and it should use the original name (no _000 suffix)
    assert!(
        output.exists(),
        "Expected original filename data.parquet to exist"
    );
    let split_0 = dir.path().join("data_000.parquet");
    assert!(
        !split_0.exists(),
        "Expected data_000.parquet to NOT exist (should be renamed to data.parquet)"
    );

    teardown_schema(&schema);
}

#[test]
fn export_parquet_csv_options_ignored() {
    fixtures::require_exasol!();
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("ignore_opts.parquet");

    // CSV-specific options like --delimiter should be silently ignored for Parquet
    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DOCKER_DSN)
        .args([
            "export",
            "--query",
            "SELECT 1 AS n",
            "--output",
            output.to_str().unwrap(),
            "--format",
            "parquet",
            "--delimiter",
            "\t",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("Exported"));

    assert!(output.exists());
}

// --- CSV split integration tests ---

#[test]
fn export_csv_split_by_rows() {
    fixtures::require_exasol!();
    let schema = setup_schema("exp_csv_split");
    setup_table(&schema, "test_data");

    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("split.csv");

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
            "--max-rows-per-file",
            "1",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("file(s)"));

    // With max-rows-per-file=1 and 3 data rows, we should get 3 split files
    let split_0 = dir.path().join("split_000.csv");
    let split_1 = dir.path().join("split_001.csv");
    let split_2 = dir.path().join("split_002.csv");
    assert!(split_0.exists(), "Expected split_000.csv to exist");
    assert!(split_1.exists(), "Expected split_001.csv to exist");
    assert!(split_2.exists(), "Expected split_002.csv to exist");

    // Each file should have a header + 1 data row
    let content0 = std::fs::read_to_string(&split_0).unwrap();
    let content1 = std::fs::read_to_string(&split_1).unwrap();
    let content2 = std::fs::read_to_string(&split_2).unwrap();

    // Header line should be present in each file
    for (i, content) in [&content0, &content1, &content2].iter().enumerate() {
        let line_count = content.lines().count();
        assert_eq!(
            line_count, 2,
            "File {i} should have header + 1 data row, got {line_count} lines"
        );
    }

    teardown_schema(&schema);
}

#[test]
fn export_csv_split_single_file_rename() {
    fixtures::require_exasol!();
    let schema = setup_schema("exp_csv_single");
    setup_table(&schema, "test_data");

    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("data.csv");

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
            "--max-rows-per-file",
            "9999",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("file(s)"));

    // With a high row limit, only one file should be produced
    // and it should use the original name (no _000 suffix)
    assert!(
        output.exists(),
        "Expected original filename data.csv to exist"
    );
    let split_0 = dir.path().join("data_000.csv");
    assert!(
        !split_0.exists(),
        "Expected data_000.csv to NOT exist (should be renamed to data.csv)"
    );

    teardown_schema(&schema);
}

#[test]
fn export_csv_split_with_no_header() {
    fixtures::require_exasol!();
    let schema = setup_schema("exp_csv_noh");
    setup_table(&schema, "test_data");

    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("noh.csv");

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
            "--no-header",
            "--max-rows-per-file",
            "2",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("file(s)"));

    // With --no-header and max-rows-per-file=2, 3 rows should split into 2 files
    let split_0 = dir.path().join("noh_000.csv");
    let split_1 = dir.path().join("noh_001.csv");
    assert!(split_0.exists(), "Expected noh_000.csv to exist");
    assert!(split_1.exists(), "Expected noh_001.csv to exist");

    let content0 = std::fs::read_to_string(&split_0).unwrap();
    let content1 = std::fs::read_to_string(&split_1).unwrap();

    // With --no-header, no file should contain a header line.
    // File 0 should have 2 data rows, file 1 should have 1 data row.
    assert_eq!(
        content0.lines().count(),
        2,
        "File 0 should have 2 data rows"
    );
    assert_eq!(content1.lines().count(), 1, "File 1 should have 1 data row");

    // Verify no header-like content (column names) in either file
    assert!(
        !content0.to_uppercase().contains("ID,NAME,SCORE")
            && !content0.to_uppercase().contains("ID\t"),
        "File 0 should not contain header"
    );

    teardown_schema(&schema);
}
