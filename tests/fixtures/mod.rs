use std::path::PathBuf;
use std::sync::Arc;

use arrow::array::{Float64Array, Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use assert_cmd::cargo_bin_cmd;
use assert_cmd::Command;
use parquet::arrow::ArrowWriter;

#[allow(dead_code)]
static SCHEMA_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[allow(dead_code)]
pub const DUMMY_DSN: &str = "exasol://user:pwd@host:8563";
#[allow(dead_code)]
pub const DOCKER_DSN: &str =
    "exasol://sys:exasol@localhost:8563?tls=true&validateservercertificate=0";

/// Skips the test if Exasol is not reachable (unless `REQUIRE_EXASOL` is set).
///
/// When `REQUIRE_EXASOL` is set (CI), panics on unreachable Exasol so failures
/// are caught. When unset (local dev), the test is silently skipped so
/// `cargo test` works without Docker.
#[allow(unused_macros)]
macro_rules! require_exasol {
    () => {
        use std::net::TcpStream;
        use std::time::Duration;
        let reachable =
            TcpStream::connect_timeout(&"127.0.0.1:8563".parse().unwrap(), Duration::from_secs(2))
                .is_ok();
        if !reachable {
            if std::env::var("REQUIRE_EXASOL").is_ok() {
                panic!("REQUIRE_EXASOL is set but Exasol is not available at localhost:8563");
            }
            eprintln!("Skipping: Exasol not available at localhost:8563");
            return;
        }
    };
}
#[allow(unused_imports)]
pub(crate) use require_exasol;

/// Creates a unique schema in Exasol and returns the connection and schema name.
#[allow(dead_code)]
pub async fn setup_exasol_schema(prefix: &str) -> (exarrow_rs::Connection, String) {
    let seq = SCHEMA_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let schema_name = format!(
        "{prefix}_{}_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
        seq,
    );
    let driver = exarrow_rs::Driver::new();
    let db = driver.open(DOCKER_DSN).unwrap();
    let mut conn = db.connect().await.unwrap();
    conn.execute_update(&format!("CREATE SCHEMA IF NOT EXISTS {schema_name}"))
        .await
        .unwrap();
    (conn, schema_name)
}

pub fn exapump() -> Command {
    cargo_bin_cmd!("exapump")
}

/// Creates a small Parquet file at `dir/test.parquet` with 3 columns and 3 rows.
/// Returns the path to the created file.
#[allow(dead_code)]
pub fn create_test_parquet(dir: &std::path::Path) -> PathBuf {
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int64, false),
        Field::new("name", DataType::Utf8, true),
        Field::new("score", DataType::Float64, false),
    ]));

    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(Int64Array::from(vec![1, 2, 3])),
            Arc::new(StringArray::from(vec![
                Some("Alice"),
                Some("Bob"),
                Some("Charlie"),
            ])),
            Arc::new(Float64Array::from(vec![95.5, 87.0, 92.3])),
        ],
    )
    .unwrap();

    let path = dir.join("test.parquet");
    let file = std::fs::File::create(&path).unwrap();
    let mut writer = ArrowWriter::try_new(file, schema, None).unwrap();
    writer.write(&batch).unwrap();
    writer.close().unwrap();
    path
}

/// Creates a small CSV file at `dir/test.csv` with 3 columns and 3 rows.
/// Returns the path to the created file.
#[allow(dead_code)]
pub fn create_test_csv(dir: &std::path::Path) -> PathBuf {
    let path = dir.join("test.csv");
    std::fs::write(
        &path,
        "id,name,score\n1,Alice,95.5\n2,Bob,87.0\n3,Charlie,92.3\n",
    )
    .unwrap();
    path
}

/// Creates a CSV file with custom content at `dir/{filename}`.
/// Returns the path to the created file.
#[allow(dead_code)]
pub fn create_csv_with_content(dir: &std::path::Path, filename: &str, content: &str) -> PathBuf {
    let path = dir.join(filename);
    std::fs::write(&path, content).unwrap();
    path
}
