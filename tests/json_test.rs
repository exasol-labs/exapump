mod fixtures;

use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

/// Builds an `upload` command for `path`, preloaded with `--table` and
/// `--dsn`. The caller appends whatever else the scenario needs, such as
/// `--dry-run` or a timeout, to the returned `Command`.
fn upload_json(path: &Path, table: &str, dsn: &str) -> Command {
    let mut cmd = fixtures::exapump();
    cmd.args([
        "upload",
        path.to_str().unwrap(),
        "--table",
        table,
        "--dsn",
        dsn,
    ]);
    cmd
}

#[test]
fn dry_run_shows_the_planned_table_family() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_nested_json(dir.path());

    upload_json(&json_path, "sales.orders", fixtures::DUMMY_DSN)
        .args(["--dry-run"])
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
        .stdout(predicate::str::contains("DECIMAL(19,0)"))
        .stdout(predicate::str::contains("VARCHAR(2000000)"));
}

#[test]
fn dry_run_omits_constraint_statements() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_nested_json(dir.path());

    upload_json(&json_path, "sales.orders", fixtures::DUMMY_DSN)
        .args(["--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ALTER TABLE").not())
        .stdout(predicate::str::contains("CONSTRAINT").not());
}

#[test]
fn dry_run_without_schema_prefix_shows_unqualified_names() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_flat_json(dir.path());

    upload_json(&json_path, "orders", fixtures::DUMMY_DSN)
        .args(["--dry-run"])
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

    upload_json(&json_path, "sales.orders", fixtures::DUMMY_DSN)
        .args(["--delimiter", ";", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"SALES\".\"ORDERS\""))
        .stdout(predicate::str::contains("CREATE TABLE IF NOT EXISTS"));
}

#[test]
fn json_file_not_found() {
    upload_json(
        Path::new("missing.json"),
        "raw.missing",
        fixtures::DUMMY_DSN,
    )
    .assert()
    .failure()
    .stderr(predicate::str::contains("missing.json"));
}

#[test]
fn empty_json_file_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_empty_json(dir.path());

    upload_json(&json_path, "raw.empty", fixtures::DUMMY_DSN)
        .assert()
        .failure()
        .stderr(predicate::str::contains("empty"));
}

#[test]
fn json_file_with_no_documents_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_empty_array_json(dir.path());

    upload_json(&json_path, "raw.none", fixtures::DUMMY_DSN)
        .assert()
        .failure()
        .stderr(predicate::str::contains("no documents"));
}

#[test]
fn documents_with_no_properties_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_empty_objects_json(dir.path());

    upload_json(&json_path, "raw.blank", fixtures::DUMMY_DSN)
        .assert()
        .failure()
        .stderr(predicate::str::contains("no column"));
}

#[test]
fn non_object_array_entry_is_rejected_with_its_position() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_non_object_entry_json(dir.path());

    upload_json(&json_path, "raw.scalars", fixtures::DUMMY_DSN)
        .assert()
        .failure()
        .stderr(predicate::str::contains("not an object"))
        .stderr(predicate::str::contains("2"));
}

#[test]
fn json_connection_failure() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_flat_json(dir.path());

    upload_json(&json_path, "sales.orders", "exasol://bad:bad@nowhere:9999")
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

    upload_json(
        &json_path,
        &format!("{schema}.orders"),
        fixtures::DOCKER_DSN,
    )
    .timeout(std::time::Duration::from_secs(60))
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
}

#[tokio::test]
async fn exasol_json_nested_import_creates_a_subtable_per_path() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_nested_json(dir.path());

    upload_json(
        &json_path,
        &format!("{schema}.orders"),
        fixtures::DOCKER_DSN,
    )
    .timeout(std::time::Duration::from_secs(60))
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
}

#[tokio::test]
async fn exasol_json_array_empty_in_every_document_creates_an_empty_subtable() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_all_empty_arrays_json(dir.path());

    upload_json(
        &json_path,
        &format!("{schema}.orders"),
        fixtures::DOCKER_DSN,
    )
    .timeout(std::time::Duration::from_secs(60))
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
}

#[tokio::test]
async fn exasol_json_nested_tables_carry_the_generated_key_columns() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_nested_json(dir.path());

    upload_json(
        &json_path,
        &format!("{schema}.orders"),
        fixtures::DOCKER_DSN,
    )
    .timeout(std::time::Duration::from_secs(60))
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
}

#[tokio::test]
async fn exasol_ndjson_import_skips_blank_lines() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_ndjson(dir.path());

    upload_json(
        &json_path,
        &format!("{schema}.events"),
        fixtures::DOCKER_DSN,
    )
    .timeout(std::time::Duration::from_secs(60))
    .assert()
    .success();

    assert_eq!(
        fixtures::count_rows(&mut conn, &format!("SELECT 1 FROM {schema}.\"EVENTS\"")).await,
        3,
        "expected one row per non-empty line, with the blank line skipped"
    );
}

#[tokio::test]
async fn exasol_json_framing_is_detected_from_content_not_extension() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_ndjson_in_json_file(dir.path());

    upload_json(
        &json_path,
        &format!("{schema}.events"),
        fixtures::DOCKER_DSN,
    )
    .timeout(std::time::Duration::from_secs(60))
    .assert()
    .success();

    assert_eq!(
        fixtures::count_rows(&mut conn, &format!("SELECT 1 FROM {schema}.\"EVENTS\"")).await,
        3,
        "a .json file holding NDJSON must be read as NDJSON"
    );
}

#[tokio::test]
async fn exasol_json_mixed_scalar_types_get_alternate_columns() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_mixed_scalar_json(dir.path());

    upload_json(&json_path, &format!("{schema}.mixed"), fixtures::DOCKER_DSN)
        .timeout(std::time::Duration::from_secs(60))
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
}

#[tokio::test]
async fn exasol_json_explicit_null_stays_distinct_from_an_absent_property() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_explicit_null_json(dir.path());

    upload_json(&json_path, &format!("{schema}.nulls"), fixtures::DOCKER_DSN)
        .timeout(std::time::Duration::from_secs(60))
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
}

#[tokio::test]
async fn exasol_json_repeated_run_appends_to_existing_family() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_flat_json(dir.path());

    for _ in 0..2 {
        upload_json(
            &json_path,
            &format!("{schema}.orders"),
            fixtures::DOCKER_DSN,
        )
        .timeout(std::time::Duration::from_secs(60))
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
}

#[tokio::test]
async fn exasol_json_unqualified_table_uses_the_connection_schema() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_flat_json(dir.path());
    let dsn =
        format!("exasol://sys:exasol@localhost:8563/{schema}?tls=true&validateservercertificate=0");

    upload_json(&json_path, "orders", &dsn)
        .timeout(std::time::Duration::from_secs(60))
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("\"{schema}\".\"ORDERS\"")));

    assert_eq!(
        fixtures::count_rows(&mut conn, &format!("SELECT 1 FROM {schema}.\"ORDERS\"")).await,
        3,
        "the family must land in the connection's default schema"
    );
}

#[tokio::test]
async fn exasol_json_unqualified_table_without_a_connection_schema_fails() {
    fixtures::require_exasol!();

    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_flat_json(dir.path());

    upload_json(&json_path, "orders", fixtures::DOCKER_DSN)
        .timeout(std::time::Duration::from_secs(60))
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

    upload_json(
        &json_path,
        "NO_SUCH_SCHEMA_XYZ.orders",
        fixtures::DOCKER_DSN,
    )
    .timeout(std::time::Duration::from_secs(60))
    .assert()
    .failure()
    .stderr(predicate::str::contains(
        "failed to create \"NO_SUCH_SCHEMA_XYZ\".\"ORDERS\"",
    ))
    .stderr(predicate::str::contains("OPEN SCHEMA").not())
    .stderr(predicate::str::contains("failed to open schema").not());
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

    upload_json(
        &json_path,
        &format!("{schema}.orders"),
        fixtures::DOCKER_DSN,
    )
    .timeout(std::time::Duration::from_secs(60))
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
}

#[tokio::test]
async fn exasol_json_multi_level_nesting_creates_all_subtables() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_deeply_nested_json(dir.path());

    upload_json(&json_path, &format!("{schema}.deep"), fixtures::DOCKER_DSN)
        .timeout(std::time::Duration::from_secs(60))
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "Imported 1 rows into \"{schema}\".\"DEEP\""
        )))
        .stdout(predicate::str::contains(format!(
            "Imported 1 rows into \"{schema}\".\"DEEP_customer\""
        )))
        .stdout(predicate::str::contains(format!(
            "Imported 1 rows into \"{schema}\".\"DEEP_customer_address\""
        )))
        .stdout(predicate::str::contains(format!(
            "Imported 2 rows into \"{schema}\".\"DEEP_items_arr\""
        )))
        .stdout(predicate::str::contains(format!(
            "Imported 2 rows into \"{schema}\".\"DEEP_items_arr_meta\""
        )))
        .stdout(predicate::str::contains(format!(
            "Imported 3 rows into \"{schema}\".\"DEEP_items_arr_tags_arr\""
        )))
        .stdout(predicate::str::contains("Imported 10 rows in total"));

    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!("SELECT TABLE_NAME FROM SYS.EXA_ALL_TABLES WHERE TABLE_SCHEMA = '{schema}'"),
        )
        .await,
        6,
        "expected a root table plus one subtable per nested path at every depth"
    );

    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!(
                "SELECT 1 FROM {schema}.\"DEEP\" d \
                 JOIN {schema}.\"DEEP_customer\" c ON d.\"customer|object\" = c.\"_id\" \
                 WHERE c.\"name\" = 'Ada'"
            ),
        )
        .await,
        1,
        "customer|object must hold the _id of the matching DEEP_customer row"
    );

    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!(
                "SELECT 1 FROM {schema}.\"DEEP_customer\" c \
                 JOIN {schema}.\"DEEP_customer_address\" a ON c.\"address|object\" = a.\"_id\" \
                 WHERE a.\"city\" = 'Berlin'"
            ),
        )
        .await,
        1,
        "address|object must hold the _id of the matching DEEP_customer_address row"
    );

    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!(
                "SELECT 1 FROM {schema}.\"DEEP_items_arr\" i \
                 JOIN {schema}.\"DEEP\" d ON i.\"_parent\" = d.\"_id\""
            ),
        )
        .await,
        2,
        "_parent must link both item rows back to the root row"
    );

    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!(
                "SELECT 1 FROM {schema}.\"DEEP_items_arr\" i \
                 JOIN {schema}.\"DEEP_items_arr_meta\" m ON i.\"meta|object\" = m.\"_id\" \
                 WHERE i.\"sku\" = 'A1' AND m.\"warehouse\" = 'W1'"
            ),
        )
        .await,
        1,
        "meta|object must hold the _id of the matching DEEP_items_arr_meta row"
    );

    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!(
                "SELECT 1 FROM {schema}.\"DEEP_items_arr_tags_arr\" t \
                 JOIN {schema}.\"DEEP_items_arr\" i ON t.\"_parent\" = i.\"_id\" \
                 WHERE i.\"sku\" = 'A1'"
            ),
        )
        .await,
        2,
        "item A1 must have its 2 tags linked by _parent"
    );

    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!(
                "SELECT 1 FROM {schema}.\"DEEP_items_arr_tags_arr\" t \
                 JOIN {schema}.\"DEEP_items_arr\" i ON t.\"_parent\" = i.\"_id\" \
                 WHERE i.\"sku\" = 'B2'"
            ),
        )
        .await,
        1,
        "item B2 must have its 1 tag linked by _parent"
    );
}

#[tokio::test]
async fn exasol_json_array_of_arrays_creates_nested_subtables() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_array_of_arrays_json(dir.path());

    upload_json(&json_path, &format!("{schema}.mat"), fixtures::DOCKER_DSN)
        .timeout(std::time::Duration::from_secs(60))
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "Imported 2 rows into \"{schema}\".\"MAT\""
        )))
        .stdout(predicate::str::contains(format!(
            "Imported 3 rows into \"{schema}\".\"MAT_matrix_arr\""
        )))
        .stdout(predicate::str::contains(format!(
            "Imported 6 rows into \"{schema}\".\"MAT_matrix_arr_value_arr\""
        )))
        .stdout(predicate::str::contains("Imported 11 rows in total"));

    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!("SELECT TABLE_NAME FROM SYS.EXA_ALL_TABLES WHERE TABLE_SCHEMA = '{schema}'"),
        )
        .await,
        3,
        "expected a root table plus one subtable per array-nesting depth"
    );

    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!("SELECT 1 FROM {schema}.\"MAT\" WHERE \"id\" = 1 AND \"matrix|array\" = 2")
        )
        .await,
        1,
        "matrix|array on id=1 must report its 2 sub-arrays"
    );
    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!("SELECT 1 FROM {schema}.\"MAT\" WHERE \"id\" = 2 AND \"matrix|array\" = 1")
        )
        .await,
        1,
        "matrix|array on id=2 must report its 1 sub-array"
    );

    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!(
                "SELECT 1 FROM {schema}.\"MAT_matrix_arr\" a \
                 JOIN {schema}.\"MAT\" m ON a.\"_parent\" = m.\"_id\" \
                 WHERE m.\"id\" = 1 AND a.\"_pos\" = 0 AND a.\"_value|array\" = 2"
            ),
        )
        .await,
        1,
        "the [1,2] sub-array (id=1, pos=0) must report 2 leaf values"
    );
    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!(
                "SELECT 1 FROM {schema}.\"MAT_matrix_arr\" a \
                 JOIN {schema}.\"MAT\" m ON a.\"_parent\" = m.\"_id\" \
                 WHERE m.\"id\" = 1 AND a.\"_pos\" = 1 AND a.\"_value|array\" = 3"
            ),
        )
        .await,
        1,
        "the [3,4,5] sub-array (id=1, pos=1) must report 3 leaf values"
    );
    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!(
                "SELECT 1 FROM {schema}.\"MAT_matrix_arr\" a \
                 JOIN {schema}.\"MAT\" m ON a.\"_parent\" = m.\"_id\" \
                 WHERE m.\"id\" = 2 AND a.\"_pos\" = 0 AND a.\"_value|array\" = 1"
            ),
        )
        .await,
        1,
        "the [6] sub-array (id=2, pos=0) must report 1 leaf value"
    );

    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!(
                "SELECT 1 FROM {schema}.\"MAT_matrix_arr_value_arr\" v \
                 JOIN {schema}.\"MAT_matrix_arr\" a ON v.\"_parent\" = a.\"_id\" \
                 JOIN {schema}.\"MAT\" m ON a.\"_parent\" = m.\"_id\" \
                 WHERE m.\"id\" = 1 AND a.\"_pos\" = 0"
            ),
        )
        .await,
        2,
        "the [1,2] sub-array must have 2 leaf value rows linked by _parent"
    );
    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!(
                "SELECT 1 FROM {schema}.\"MAT_matrix_arr_value_arr\" v \
                 JOIN {schema}.\"MAT_matrix_arr\" a ON v.\"_parent\" = a.\"_id\" \
                 JOIN {schema}.\"MAT\" m ON a.\"_parent\" = m.\"_id\" \
                 WHERE m.\"id\" = 1 AND a.\"_pos\" = 1"
            ),
        )
        .await,
        3,
        "the [3,4,5] sub-array must have 3 leaf value rows linked by _parent"
    );
    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!(
                "SELECT 1 FROM {schema}.\"MAT_matrix_arr_value_arr\" v \
                 JOIN {schema}.\"MAT_matrix_arr\" a ON v.\"_parent\" = a.\"_id\" \
                 JOIN {schema}.\"MAT\" m ON a.\"_parent\" = m.\"_id\" \
                 WHERE m.\"id\" = 2"
            ),
        )
        .await,
        1,
        "the [6] sub-array must have 1 leaf value row linked by _parent"
    );
}

#[tokio::test]
async fn exasol_json_reordered_properties_keep_each_value_in_its_column() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let (first, second) = fixtures::create_reordered_property_files(dir.path());

    for path in [&first, &second] {
        upload_json(path, &format!("{schema}.reorder"), fixtures::DOCKER_DSN)
            .timeout(std::time::Duration::from_secs(60))
            .assert()
            .success();
    }

    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!("SELECT 1 FROM {schema}.\"REORDER\" WHERE \"a\" = 'x2' AND \"b\" = 'y2'"),
        )
        .await,
        1,
        "the second file lists the same two properties in the other order, so an import that \
         places values by position swaps them between two same-typed columns"
    );
    assert_eq!(
        fixtures::count_rows(&mut conn, &format!("SELECT 1 FROM {schema}.\"REORDER\"")).await,
        2,
        "each file must contribute one row"
    );
}

#[tokio::test]
async fn exasol_json_second_file_with_an_unknown_column_fails() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let (first, second) = fixtures::create_unknown_column_files(dir.path());

    upload_json(&first, &format!("{schema}.mismatch"), fixtures::DOCKER_DSN)
        .timeout(std::time::Duration::from_secs(60))
        .assert()
        .success();

    upload_json(&second, &format!("{schema}.mismatch"), fixtures::DOCKER_DSN)
        .timeout(std::time::Duration::from_secs(60))
        .assert()
        .failure()
        .stderr(predicate::str::contains(format!(
            "\"{schema}\".\"MISMATCH\""
        )))
        .stderr(predicate::str::contains("\"c\""));

    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!("SELECT 1 FROM {schema}.\"MISMATCH\" WHERE \"a\" = 1 AND \"b\" = 'from_b'"),
        )
        .await,
        1,
        "the row the first file loaded must survive the failed second upload unchanged"
    );
    assert_eq!(
        fixtures::count_rows(&mut conn, &format!("SELECT 1 FROM {schema}.\"MISMATCH\"")).await,
        1,
        "a batch naming a column the table does not hold must load no row at all"
    );
}

#[tokio::test]
async fn exasol_json_second_file_omitting_a_column_loads_null() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let (first, second) = fixtures::create_omitted_column_files(dir.path());

    for path in [&first, &second] {
        upload_json(path, &format!("{schema}.optional"), fixtures::DOCKER_DSN)
            .timeout(std::time::Duration::from_secs(60))
            .assert()
            .success();
    }

    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!("SELECT 1 FROM {schema}.\"OPTIONAL\" WHERE \"a\" = 3 AND \"b\" IS NULL"),
        )
        .await,
        1,
        "a column the second file never names must load NULL, not fail the import"
    );
    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!(
                "SELECT 1 FROM {schema}.\"OPTIONAL\" \
                 WHERE (\"a\" = 1 AND \"b\" = 'x') OR (\"a\" = 2 AND \"b\" = 'y')"
            ),
        )
        .await,
        2,
        "the rows the first file loaded must keep their original values"
    );
    assert_eq!(
        fixtures::count_rows(&mut conn, &format!("SELECT 1 FROM {schema}.\"OPTIONAL\"")).await,
        3,
        "expected two rows from the first file and one from the second"
    );
}

#[test]
fn colliding_table_names_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_colliding_paths_json(dir.path());

    for extra in [&[][..], &["--dry-run"][..]] {
        upload_json(&json_path, "raw.col", fixtures::DUMMY_DSN)
            .args(extra)
            .assert()
            .failure()
            .stderr(predicate::str::contains("COL_customer_address"))
            .stderr(predicate::str::contains(
                "customer.address and customer_address",
            ));
    }
}

#[test]
fn nested_object_with_no_properties_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_hollow_object_json(dir.path());

    upload_json(&json_path, "raw.hollow", fixtures::DUMMY_DSN)
        .assert()
        .failure()
        .stderr(predicate::str::contains("no column"));
}

#[tokio::test]
async fn exasol_jsonl_extension_imports_one_row_per_line() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let jsonl_path = fixtures::create_jsonl(dir.path());

    upload_json(
        &jsonl_path,
        &format!("{schema}.events"),
        fixtures::DOCKER_DSN,
    )
    .timeout(std::time::Duration::from_secs(60))
    .assert()
    .success()
    .stderr(predicate::str::contains("not supported").not());

    assert_eq!(
        fixtures::count_rows(&mut conn, &format!("SELECT 1 FROM {schema}.\"EVENTS\"")).await,
        3,
        "the .jsonl extension must import one row per non-empty line"
    );
}

#[test]
fn single_multi_line_json_object_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_multi_line_object_json(dir.path());

    upload_json(&json_path, "raw.single", fixtures::DUMMY_DSN)
        .assert()
        .failure()
        .stderr(predicate::str::contains("JSON array of objects"))
        .stderr(predicate::str::contains("one JSON object per line"));
}

#[test]
fn malformed_ndjson_line_is_reported_without_the_framing_diagnosis() {
    let dir = tempfile::tempdir().unwrap();
    let ndjson_path = fixtures::create_malformed_ndjson(dir.path());

    upload_json(&ndjson_path, "raw.malformed", fixtures::DUMMY_DSN)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Line 2"))
        .stderr(predicate::str::contains("neither a JSON array of objects").not());
}

#[tokio::test]
async fn exasol_json_nineteen_digit_integer_imports_unchanged() {
    fixtures::require_exasol!();

    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_JSON").await;
    let dir = tempfile::tempdir().unwrap();
    let json_path = fixtures::create_wide_integer_json(dir.path());

    upload_json(&json_path, &format!("{schema}.ids"), fixtures::DOCKER_DSN)
        .timeout(std::time::Duration::from_secs(60))
        .assert()
        .success();

    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!(
                "SELECT 1 FROM SYS.EXA_ALL_COLUMNS \
                 WHERE COLUMN_SCHEMA = '{schema}' AND COLUMN_TABLE = 'IDS' \
                   AND COLUMN_NAME = 'id' AND COLUMN_TYPE = 'DECIMAL(19,0)'"
            ),
        )
        .await,
        1,
        "an integer column must be wide enough for the whole 64-bit signed range"
    );
    assert_eq!(
        fixtures::count_rows(
            &mut conn,
            &format!("SELECT 1 FROM {schema}.\"IDS\" WHERE \"id\" = 1234567890123456789"),
        )
        .await,
        1,
        "a 19-digit integer must survive the load unchanged"
    );
}
