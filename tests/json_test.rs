mod fixtures;

use predicates::prelude::*;

#[test]
fn dry_run_shows_the_planned_table_family() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_nested_json(dir.path());

    fixtures::exapump()
        .args([
            "upload",
            json_path.to_str().unwrap(),
            "--table",
            "sales.orders",
            "--dsn",
            fixtures::DUMMY_DSN,
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"SALES\".\"ORDERS\""))
        .stdout(predicate::str::contains("\"SALES\".\"ORDERS_customer\""))
        .stdout(predicate::str::contains("\"SALES\".\"ORDERS_items_arr\""))
        .stdout(predicate::str::contains("CREATE TABLE IF NOT EXISTS"))
        .stdout(predicate::str::contains("\"_id\""))
        .stdout(predicate::str::contains("\"_parent\""))
        .stdout(predicate::str::contains("\"_pos\""))
        .stdout(predicate::str::contains("\"customer|object\""))
        .stdout(predicate::str::contains("\"items|array\""))
        .stdout(predicate::str::contains("DECIMAL(18,0)"))
        .stdout(predicate::str::contains("VARCHAR(2000000)"));
}

#[test]
fn dry_run_omits_constraint_statements() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_nested_json(dir.path());

    fixtures::exapump()
        .args([
            "upload",
            json_path.to_str().unwrap(),
            "--table",
            "sales.orders",
            "--dsn",
            fixtures::DUMMY_DSN,
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("ALTER TABLE").not())
        .stdout(predicate::str::contains("CONSTRAINT").not());
}

#[test]
fn dry_run_without_schema_prefix_shows_unqualified_names() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_flat_json(dir.path());

    fixtures::exapump()
        .args([
            "upload",
            json_path.to_str().unwrap(),
            "--table",
            "orders",
            "--dsn",
            fixtures::DUMMY_DSN,
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "CREATE TABLE IF NOT EXISTS \"ORDERS\" (",
        ))
        .stdout(predicate::str::contains("\".\"ORDERS\"").not());
}

#[test]
fn dry_run_accepts_and_ignores_the_delimiter_flag() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_flat_json(dir.path());

    fixtures::exapump()
        .args([
            "upload",
            json_path.to_str().unwrap(),
            "--table",
            "sales.orders",
            "--dsn",
            fixtures::DUMMY_DSN,
            "--delimiter",
            ";",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"SALES\".\"ORDERS\""))
        .stdout(predicate::str::contains("CREATE TABLE IF NOT EXISTS"));
}

#[test]
fn json_file_not_found() {
    fixtures::exapump()
        .args([
            "upload",
            "missing.json",
            "--table",
            "raw.missing",
            "--dsn",
            fixtures::DUMMY_DSN,
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing.json"));
}

#[test]
fn empty_json_file_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_empty_json(dir.path());

    fixtures::exapump()
        .args([
            "upload",
            json_path.to_str().unwrap(),
            "--table",
            "raw.empty",
            "--dsn",
            fixtures::DUMMY_DSN,
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("empty"));
}

#[test]
fn json_file_with_no_documents_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_empty_array_json(dir.path());

    fixtures::exapump()
        .args([
            "upload",
            json_path.to_str().unwrap(),
            "--table",
            "raw.none",
            "--dsn",
            fixtures::DUMMY_DSN,
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no documents"));
}

#[test]
fn documents_with_no_properties_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_empty_objects_json(dir.path());

    fixtures::exapump()
        .args([
            "upload",
            json_path.to_str().unwrap(),
            "--table",
            "raw.blank",
            "--dsn",
            fixtures::DUMMY_DSN,
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no column"));
}

#[test]
fn non_object_array_entry_is_rejected_with_its_position() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_non_object_entry_json(dir.path());

    fixtures::exapump()
        .args([
            "upload",
            json_path.to_str().unwrap(),
            "--table",
            "raw.scalars",
            "--dsn",
            fixtures::DUMMY_DSN,
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not an object"))
        .stderr(predicate::str::contains("2"));
}

#[test]
fn json_connection_failure() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_flat_json(dir.path());

    fixtures::exapump()
        .args([
            "upload",
            json_path.to_str().unwrap(),
            "--table",
            "sales.orders",
            "--dsn",
            "exasol://bad:bad@nowhere:9999",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::is_empty().not());
}

#[tokio::test]
async fn exasol_json_flat_import_creates_one_table() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_flat_json(dir.path());

    fixtures::exapump()
        .timeout(std::time::Duration::from_secs(60))
        .args([
            "upload",
            json_path.to_str().unwrap(),
            "--table",
            &format!("{schema}.orders"),
            "--dsn",
            fixtures::DOCKER_DSN,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "Imported 3 rows into \"{schema}\".\"ORDERS\""
        )));

    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!("SELECT TABLE_NAME FROM SYS.EXA_ALL_TABLES WHERE TABLE_SCHEMA = '{schema}'"),
        )
        .await,
        1,
        "a flat document set must create exactly one table"
    );
    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!(
                "SELECT COLUMN_NAME FROM SYS.EXA_ALL_COLUMNS \
                 WHERE COLUMN_SCHEMA = '{schema}' AND COLUMN_TABLE = 'ORDERS'"
            ),
        )
        .await,
        5,
        "expected _id plus one column per JSON property"
    );
    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!(
                "SELECT 1 FROM {schema}.\"ORDERS\" \
                 WHERE \"_id\" IS NOT NULL AND \"id\" = 1 AND \"name\" = 'Alice' \
                   AND \"score\" = 95.5 AND \"active\" = TRUE"
            ),
        )
        .await,
        1,
        "every scalar type must land in its contract column type"
    );
    assert_eq!(
        fixtures::count_rows(&mut conn, &format!("SELECT 1 FROM {schema}.\"ORDERS\"")).await,
        3,
        "expected one row per document"
    );

    let _ = conn
        .execute_update(&format!("DROP SCHEMA {schema} CASCADE"))
        .await;
}

#[tokio::test]
async fn exasol_json_nested_import_creates_a_subtable_per_path() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_nested_json(dir.path());

    fixtures::exapump()
        .timeout(std::time::Duration::from_secs(60))
        .args([
            "upload",
            json_path.to_str().unwrap(),
            "--table",
            &format!("{schema}.orders"),
            "--dsn",
            fixtures::DOCKER_DSN,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "Imported 2 rows into \"{schema}\".\"ORDERS_customer\""
        )))
        .stdout(predicate::str::contains(format!(
            "Imported 3 rows into \"{schema}\".\"ORDERS_items_arr\""
        )))
        .stdout(predicate::str::contains(format!(
            "Imported 2 rows into \"{schema}\".\"ORDERS\""
        )));

    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!("SELECT TABLE_NAME FROM SYS.EXA_ALL_TABLES WHERE TABLE_SCHEMA = '{schema}'"),
        )
        .await,
        3,
        "expected a root table plus one subtable per nested path"
    );
    assert_eq!(
        fixtures::count_rows(&mut conn, &format!("SELECT 1 FROM {schema}.\"ORDERS\"")).await,
        2
    );
    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!("SELECT 1 FROM {schema}.\"ORDERS_customer\" WHERE \"tier\" = 'gold'")
        )
        .await,
        1
    );
    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!("SELECT 1 FROM {schema}.\"ORDERS_items_arr\"")
        )
        .await,
        3,
        "every array element must become a row of its own"
    );

    let _ = conn
        .execute_update(&format!("DROP SCHEMA {schema} CASCADE"))
        .await;
}

#[tokio::test]
async fn exasol_json_array_empty_in_every_document_creates_an_empty_subtable() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_all_empty_arrays_json(dir.path());

    fixtures::exapump()
        .timeout(std::time::Duration::from_secs(60))
        .args([
            "upload",
            json_path.to_str().unwrap(),
            "--table",
            &format!("{schema}.orders"),
            "--dsn",
            fixtures::DOCKER_DSN,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "Imported 0 rows into \"{schema}\".\"ORDERS_items_arr\""
        )))
        .stdout(predicate::str::contains(format!(
            "Imported 2 rows into \"{schema}\".\"ORDERS\""
        )));

    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!("SELECT TABLE_NAME FROM SYS.EXA_ALL_TABLES WHERE TABLE_SCHEMA = '{schema}'"),
        )
        .await,
        2,
        "a property that is an array in every document plans a subtable even when every array is empty"
    );
    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!("SELECT 1 FROM {schema}.\"ORDERS_items_arr\"")
        )
        .await,
        0,
        "no array element means no subtable row"
    );

    let _ = conn
        .execute_update(&format!("DROP SCHEMA {schema} CASCADE"))
        .await;
}

#[tokio::test]
async fn exasol_json_nested_tables_carry_the_generated_key_columns() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_nested_json(dir.path());

    fixtures::exapump()
        .timeout(std::time::Duration::from_secs(60))
        .args([
            "upload",
            json_path.to_str().unwrap(),
            "--table",
            &format!("{schema}.orders"),
            "--dsn",
            fixtures::DOCKER_DSN,
        ])
        .assert()
        .success();

    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!(
                "SELECT 1 FROM {schema}.\"ORDERS\" o \
                 JOIN {schema}.\"ORDERS_customer\" c ON o.\"customer|object\" = c.\"_id\" \
                 WHERE o.\"id\" = 1 AND c.\"name\" = 'Alice'"
            ),
        )
        .await,
        1,
        "customer|object must hold the _id of the matching subtable row"
    );
    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!(
                "SELECT 1 FROM {schema}.\"ORDERS_items_arr\" i \
                 JOIN {schema}.\"ORDERS\" o ON i.\"_parent\" = o.\"_id\" \
                 WHERE o.\"id\" = 1"
            ),
        )
        .await,
        2,
        "_parent must hold the _id of the parent row"
    );
    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!(
                "SELECT 1 FROM {schema}.\"ORDERS_items_arr\" i \
                 JOIN {schema}.\"ORDERS\" o ON i.\"_parent\" = o.\"_id\" \
                 WHERE o.\"id\" = 1 AND i.\"_pos\" = 0 AND i.\"sku\" = 'A1'"
            ),
        )
        .await,
        1,
        "_pos must hold the zero-based element position"
    );

    let _ = conn
        .execute_update(&format!("DROP SCHEMA {schema} CASCADE"))
        .await;
}

#[tokio::test]
async fn exasol_ndjson_import_skips_blank_lines() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_ndjson(dir.path());

    fixtures::exapump()
        .timeout(std::time::Duration::from_secs(60))
        .args([
            "upload",
            json_path.to_str().unwrap(),
            "--table",
            &format!("{schema}.events"),
            "--dsn",
            fixtures::DOCKER_DSN,
        ])
        .assert()
        .success();

    assert_eq!(
        fixtures::count_rows(&mut conn, &format!("SELECT 1 FROM {schema}.\"EVENTS\"")).await,
        3,
        "expected one row per non-empty line, with the blank line skipped"
    );

    let _ = conn
        .execute_update(&format!("DROP SCHEMA {schema} CASCADE"))
        .await;
}

#[tokio::test]
async fn exasol_json_framing_is_detected_from_content_not_extension() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_ndjson_in_json_file(dir.path());

    fixtures::exapump()
        .timeout(std::time::Duration::from_secs(60))
        .args([
            "upload",
            json_path.to_str().unwrap(),
            "--table",
            &format!("{schema}.events"),
            "--dsn",
            fixtures::DOCKER_DSN,
        ])
        .assert()
        .success();

    assert_eq!(
        fixtures::count_rows(&mut conn, &format!("SELECT 1 FROM {schema}.\"EVENTS\"")).await,
        3,
        "a .json file holding NDJSON must be read as NDJSON"
    );

    let _ = conn
        .execute_update(&format!("DROP SCHEMA {schema} CASCADE"))
        .await;
}

#[tokio::test]
async fn exasol_json_mixed_scalar_types_get_alternate_columns() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_mixed_scalar_json(dir.path());

    fixtures::exapump()
        .timeout(std::time::Duration::from_secs(60))
        .args([
            "upload",
            json_path.to_str().unwrap(),
            "--table",
            &format!("{schema}.mixed"),
            "--dsn",
            fixtures::DOCKER_DSN,
        ])
        .assert()
        .success();

    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!(
                "SELECT 1 FROM {schema}.\"MIXED\" \
                 WHERE \"code\" = 'A1' AND \"code|integer\" IS NULL"
            ),
        )
        .await,
        1,
        "a majority-type value belongs in the primary column, with the alternate NULL"
    );
    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!(
                "SELECT 1 FROM {schema}.\"MIXED\" \
                 WHERE \"code|integer\" = 42 AND \"code\" IS NULL"
            ),
        )
        .await,
        1,
        "a minority-type value belongs in its alternate column, with the primary NULL"
    );

    let _ = conn
        .execute_update(&format!("DROP SCHEMA {schema} CASCADE"))
        .await;
}

#[tokio::test]
async fn exasol_json_explicit_null_stays_distinct_from_an_absent_property() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_explicit_null_json(dir.path());

    fixtures::exapump()
        .timeout(std::time::Duration::from_secs(60))
        .args([
            "upload",
            json_path.to_str().unwrap(),
            "--table",
            &format!("{schema}.nulls"),
            "--dsn",
            fixtures::DOCKER_DSN,
        ])
        .assert()
        .success();

    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!("SELECT 1 FROM {schema}.\"NULLS\" WHERE \"id\" = 1 AND \"note|n\" = TRUE"),
        )
        .await,
        1,
        "the mask must be TRUE for the explicit null"
    );
    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!("SELECT 1 FROM {schema}.\"NULLS\" WHERE \"id\" = 2 AND \"note|n\" = FALSE"),
        )
        .await,
        1,
        "the mask must be FALSE for the absent property"
    );

    let _ = conn
        .execute_update(&format!("DROP SCHEMA {schema} CASCADE"))
        .await;
}

#[tokio::test]
async fn exasol_json_repeated_run_appends_to_existing_family() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_flat_json(dir.path());

    for _ in 0..2 {
        fixtures::exapump()
            .timeout(std::time::Duration::from_secs(60))
            .args([
                "upload",
                json_path.to_str().unwrap(),
                "--table",
                &format!("{schema}.orders"),
                "--dsn",
                fixtures::DOCKER_DSN,
            ])
            .assert()
            .success()
            .stderr(predicate::str::contains("_id"))
            .stderr(predicate::str::contains("run"));
    }

    assert_eq!(
        fixtures::count_rows(&mut conn, &format!("SELECT 1 FROM {schema}.\"ORDERS\"")).await,
        6,
        "a repeated run must append to the existing family"
    );

    let _ = conn
        .execute_update(&format!("DROP SCHEMA {schema} CASCADE"))
        .await;
}

#[tokio::test]
async fn exasol_json_unqualified_table_uses_the_connection_schema() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_flat_json(dir.path());
    let dsn =
        format!("exasol://sys:exasol@localhost:8563/{schema}?tls=true&validateservercertificate=0");

    fixtures::exapump()
        .timeout(std::time::Duration::from_secs(60))
        .args([
            "upload",
            json_path.to_str().unwrap(),
            "--table",
            "orders",
            "--dsn",
            &dsn,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("\"{schema}\".\"ORDERS\"")));

    assert_eq!(
        fixtures::count_rows(&mut conn, &format!("SELECT 1 FROM {schema}.\"ORDERS\"")).await,
        3,
        "the family must land in the connection's default schema"
    );

    let _ = conn
        .execute_update(&format!("DROP SCHEMA {schema} CASCADE"))
        .await;
}

#[tokio::test]
async fn exasol_json_unqualified_table_without_a_connection_schema_fails() {
    fixtures::require_exasol!();

    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_flat_json(dir.path());

    fixtures::exapump()
        .timeout(std::time::Duration::from_secs(60))
        .args([
            "upload",
            json_path.to_str().unwrap(),
            "--table",
            "orders",
            "--dsn",
            fixtures::DOCKER_DSN,
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("schema"))
        .stderr(predicate::str::contains("--table"));
}

#[tokio::test]
async fn exasol_json_missing_target_schema_fails() {
    fixtures::require_exasol!();

    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_flat_json(dir.path());

    fixtures::exapump()
        .timeout(std::time::Duration::from_secs(60))
        .args([
            "upload",
            json_path.to_str().unwrap(),
            "--table",
            "NO_SUCH_SCHEMA_XYZ.orders",
            "--dsn",
            fixtures::DOCKER_DSN,
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed to open schema"))
        .stderr(predicate::str::contains("NO_SUCH_SCHEMA_XYZ"));
}

#[tokio::test]
async fn exasol_json_partial_family_failure_reports_loaded_tables() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    conn.execute_update(&format!("CREATE TABLE {schema}.\"ORDERS\" (\"_id\" DATE)"))
        .await
        .unwrap();

    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_nested_json(dir.path());

    fixtures::exapump()
        .timeout(std::time::Duration::from_secs(60))
        .args([
            "upload",
            json_path.to_str().unwrap(),
            "--table",
            &format!("{schema}.orders"),
            "--dsn",
            fixtures::DOCKER_DSN,
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains(format!(
            "Imported 2 rows into \"{schema}\".\"ORDERS_customer\""
        )))
        .stdout(predicate::str::contains(format!(
            "Imported 3 rows into \"{schema}\".\"ORDERS_items_arr\""
        )))
        .stderr(predicate::str::contains(format!("\"{schema}\".\"ORDERS\"")));

    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!("SELECT 1 FROM {schema}.\"ORDERS_customer\"")
        )
        .await,
        2,
        "the tables loaded before the failure must not be rolled back"
    );

    let _ = conn
        .execute_update(&format!("DROP SCHEMA {schema} CASCADE"))
        .await;
}
