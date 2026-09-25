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

/// Install the rustls CryptoProvider once per test process.
/// Needed because both `ring` and `aws-lc-rs` are in the dependency tree
/// (from reqwest and exarrow-rs respectively).
#[allow(dead_code)]
pub fn install_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

#[allow(dead_code)]
pub const DUMMY_DSN: &str = "exasol://user:pwd@host:8563";
#[allow(dead_code)]
pub const DOCKER_DSN: &str =
    "exasol://sys:exasol@localhost:8563?tls=true&validateservercertificate=0";

/// Panics if Exasol is not reachable at localhost:8563.
///
/// Start a local instance with:
/// `docker run -d --name exasol-test --privileged --shm-size=2g -p 8563:8563 -p 2581:2581 exasol/docker-db:2025.2.0`
#[allow(unused_macros)]
macro_rules! require_exasol {
    () => {
        use std::net::TcpStream;
        use std::time::Duration;
        let reachable =
            TcpStream::connect_timeout(&"127.0.0.1:8563".parse().unwrap(), Duration::from_secs(2))
                .is_ok();
        if !reachable {
            panic!("Exasol is not available at localhost:8563. Start it with: docker run -d --name exasol-test --privileged --shm-size=2g -p 8563:8563 -p 2581:2581 exasol/docker-db:2025.2.0");
        }
    };
}
#[allow(unused_imports)]
pub(crate) use require_exasol;

/// Extracts BucketFS write password from the running Exasol Docker container.
/// Shells out to `docker exec` to read EXAConf and decode the base64 password.
/// Panics if the container is not running or BucketFS is not configured.
/// The container name defaults to `exasol-test` but can be overridden via
/// the `EXASOL_CONTAINER` environment variable.
#[allow(dead_code)]
pub fn bfs_write_password() -> String {
    let container = std::env::var("EXASOL_CONTAINER").unwrap_or_else(|_| "exasol-test".to_string());
    let output = std::process::Command::new("docker")
        .args(["exec", &container, "cat", "/exa/etc/EXAConf"])
        .output()
        .expect("Failed to exec into exasol container");
    let exaconf = String::from_utf8(output.stdout).expect("EXAConf is not valid UTF-8");
    let line = exaconf
        .lines()
        .find(|l| l.contains("WritePasswd"))
        .expect("WritePasswd not found in EXAConf");
    let b64 = line
        .split_once('=')
        .expect("WritePasswd line has no '=' separator")
        .1
        .trim();
    use base64::Engine;
    String::from_utf8(
        base64::engine::general_purpose::STANDARD
            .decode(b64)
            .expect("WritePasswd is not valid base64"),
    )
    .expect("Decoded password is not valid UTF-8")
}

/// Panics if BucketFS is not reachable at localhost:2581.
#[allow(unused_macros)]
macro_rules! require_bucketfs {
    () => {
        use std::net::TcpStream;
        use std::time::Duration;
        let reachable =
            TcpStream::connect_timeout(&"127.0.0.1:2581".parse().unwrap(), Duration::from_secs(2))
                .is_ok();
        if !reachable {
            panic!(
                "BucketFS is not available at localhost:2581. \
                 Start with: docker run -d --name exasol-test --privileged --shm-size=2g \
                 -p 8563:8563 -p 2581:2581 exasol/docker-db:2025.2.0"
            );
        }
    };
}
#[allow(unused_imports)]
pub(crate) use require_bucketfs;

/// Creates a unique schema in Exasol and returns the connection and schema name.
#[allow(dead_code)]
pub async fn setup_exasol_schema(prefix: &str) -> (exarrow_rs::Connection, String) {
    install_crypto_provider();
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

/// Runs `sql` and returns how many rows the result holds.
///
/// Lets a test state its expectation as a predicate the database evaluates,
/// so no assertion has to decode Arrow values out of the result set.
#[allow(dead_code)]
pub async fn count_rows(conn: &mut exarrow_rs::Connection, sql: &str) -> usize {
    let result = conn
        .execute(sql)
        .await
        .unwrap_or_else(|e| panic!("query failed: {sql}: {e}"));
    let batches = result
        .fetch_all()
        .await
        .unwrap_or_else(|e| panic!("fetch failed: {sql}: {e}"));
    batches.iter().map(|b| b.num_rows()).sum()
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

/// Creates a Parquet file with a SQL-reserved keyword column (`timestamp`).
/// Returns the path to the created file.
#[allow(dead_code)]
pub fn create_parquet_with_reserved_keyword(dir: &std::path::Path) -> PathBuf {
    use arrow::array::TimestampMicrosecondArray;

    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int64, false),
        Field::new(
            "timestamp",
            DataType::Timestamp(arrow::datatypes::TimeUnit::Microsecond, None),
            false,
        ),
        Field::new("value", DataType::Float64, false),
    ]));

    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(Int64Array::from(vec![1, 2, 3])),
            Arc::new(TimestampMicrosecondArray::from(vec![
                1_000_000, 2_000_000, 3_000_000,
            ])),
            Arc::new(Float64Array::from(vec![10.0, 20.0, 30.0])),
        ],
    )
    .unwrap();

    let path = dir.join("reserved_keyword.parquet");
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

/// Writes `content` to `dir/{filename}` and returns the path.
fn write_fixture(dir: &std::path::Path, filename: &str, content: &str) -> PathBuf {
    let path = dir.join(filename);
    std::fs::write(&path, content).unwrap();
    path
}

/// Creates `dir/orders.json`: a top-level array of three objects whose
/// properties cover every scalar type of the contract.
#[allow(dead_code)]
pub fn create_flat_json(dir: &std::path::Path) -> PathBuf {
    write_fixture(
        dir,
        "orders.json",
        r#"[
  {"id": 1, "name": "Alice", "score": 95.5, "active": true},
  {"id": 2, "name": "Bob", "score": 87.25, "active": false},
  {"id": 3, "name": "Charlie", "score": 92.5, "active": true}
]
"#,
    )
}

/// Creates `dir/orders.json`: documents carrying a nested object property
/// `customer` and a nested array property `items`.
///
/// Both nested path names sort before the literal `root`, so the root table is
/// last in family order.
#[allow(dead_code)]
pub fn create_nested_json(dir: &std::path::Path) -> PathBuf {
    write_fixture(
        dir,
        "orders.json",
        r#"[
  {"id": 1, "customer": {"name": "Alice", "tier": "gold"},
   "items": [{"sku": "A1", "qty": 2}, {"sku": "A2", "qty": 1}]},
  {"id": 2, "customer": {"name": "Bob", "tier": "silver"},
   "items": [{"sku": "B1", "qty": 5}]}
]
"#,
    )
}

/// Creates `dir/events.ndjson`: three documents, one per line, with one blank
/// line between them.
#[allow(dead_code)]
pub fn create_ndjson(dir: &std::path::Path) -> PathBuf {
    write_fixture(
        dir,
        "events.ndjson",
        "{\"id\": 1, \"kind\": \"click\"}\n\n{\"id\": 2, \"kind\": \"view\"}\n{\"id\": 3, \"kind\": \"click\"}\n",
    )
}

/// Creates `dir/events.json`: NDJSON framing behind a `.json` extension, so
/// framing detection cannot rely on the extension.
#[allow(dead_code)]
pub fn create_ndjson_in_json_file(dir: &std::path::Path) -> PathBuf {
    write_fixture(
        dir,
        "events.json",
        "{\"id\": 1, \"kind\": \"click\"}\n{\"id\": 2, \"kind\": \"view\"}\n{\"id\": 3, \"kind\": \"click\"}\n",
    )
}

/// Creates `dir/mixed.json`: the property `code` is a string in three documents
/// and an integer in the fourth.
#[allow(dead_code)]
pub fn create_mixed_scalar_json(dir: &std::path::Path) -> PathBuf {
    write_fixture(
        dir,
        "mixed.json",
        r#"[
  {"code": "A1"},
  {"code": "B2"},
  {"code": "C3"},
  {"code": 42}
]
"#,
    )
}

/// Creates `dir/nulls.json`: `note` is an explicit JSON null in the first
/// document and absent from the second.
#[allow(dead_code)]
pub fn create_explicit_null_json(dir: &std::path::Path) -> PathBuf {
    write_fixture(
        dir,
        "nulls.json",
        r#"[
  {"id": 1, "note": null},
  {"id": 2}
]
"#,
    )
}

/// Creates `dir/empty.json`: whitespace only, no document bytes at all.
#[allow(dead_code)]
pub fn create_empty_json(dir: &std::path::Path) -> PathBuf {
    write_fixture(dir, "empty.json", "  \n\t\n")
}

/// Creates `dir/none.json`: a well-formed top-level array holding no document.
#[allow(dead_code)]
pub fn create_empty_array_json(dir: &std::path::Path) -> PathBuf {
    write_fixture(dir, "none.json", "[]\n")
}

/// Creates `dir/blank.json`: every array entry is an object carrying no
/// property, so the planned family holds generated key columns only.
#[allow(dead_code)]
pub fn create_empty_objects_json(dir: &std::path::Path) -> PathBuf {
    write_fixture(dir, "blank.json", "[{}, {}, {}]\n")
}

/// Creates `dir/scalars.json`: the third array entry is the number `42` rather
/// than an object.
#[allow(dead_code)]
pub fn create_non_object_entry_json(dir: &std::path::Path) -> PathBuf {
    write_fixture(
        dir,
        "scalars.json",
        r#"[
  {"a": 1},
  {"a": 2},
  42
]
"#,
    )
}

/// Creates `dir/orders.json`: the array property `items` is present in every
/// document and empty in every document, so its subtable is planned from the
/// property alone and carries the generated key columns only.
#[allow(dead_code)]
pub fn create_all_empty_arrays_json(dir: &std::path::Path) -> PathBuf {
    write_fixture(
        dir,
        "orders.json",
        "[{\"id\": 1, \"items\": []}, {\"id\": 2, \"items\": []}]\n",
    )
}

/// Creates `dir/deep.json`: multiple levels of nesting — an object nested
/// inside an object (`customer.address`), and an object plus an array nested
/// inside each element of an array (`items[].meta`, `items[].tags`).
#[allow(dead_code)]
pub fn create_deeply_nested_json(dir: &std::path::Path) -> PathBuf {
    write_fixture(
        dir,
        "deep.json",
        r#"[
  {
    "order_id": 1,
    "customer": {
      "name": "Ada",
      "address": {"city": "Berlin", "zip": "10115"}
    },
    "items": [
      {"sku": "A1", "qty": 2, "meta": {"warehouse": "W1"}, "tags": ["red", "large"]},
      {"sku": "B2", "qty": 1, "meta": {"warehouse": "W2"}, "tags": ["blue"]}
    ]
  }
]
"#,
    )
}

/// Creates `dir/matrix.json`: an array nested inside each element of another
/// array (`matrix[][]`), with varying sub-array lengths.
#[allow(dead_code)]
pub fn create_array_of_arrays_json(dir: &std::path::Path) -> PathBuf {
    write_fixture(
        dir,
        "matrix.json",
        r#"[
  {"id": 1, "matrix": [[1, 2], [3, 4, 5]]},
  {"id": 2, "matrix": [[6]]}
]
"#,
    )
}
