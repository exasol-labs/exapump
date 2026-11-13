mod fixtures;

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use arrow::array::{Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;

const ROWS: i64 = 50;

/// Byte log of one relayed connection: client-to-server and server-to-client.
type ConnLog = Arc<Mutex<(Vec<u8>, Vec<u8>)>>;

/// TCP relay in front of the Docker database that records every byte it forwards.
///
/// exarrow-rs opens the transfer tunnel to the same host and port as the login,
/// so pointing the DSN at the relay routes both connections through it.
struct RecordingRelay {
    port: u16,
    conns: Arc<Mutex<Vec<ConnLog>>>,
}

impl RecordingRelay {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let conns: Arc<Mutex<Vec<ConnLog>>> = Arc::default();
        let registry = Arc::clone(&conns);
        thread::spawn(move || {
            for client in listener.incoming().flatten() {
                let Ok(server) = TcpStream::connect("127.0.0.1:8563") else {
                    continue;
                };
                let log: ConnLog = Arc::default();
                registry.lock().unwrap().push(Arc::clone(&log));
                pump(
                    client.try_clone().unwrap(),
                    server.try_clone().unwrap(),
                    Arc::clone(&log),
                    true,
                );
                pump(server, client, log, false);
            }
        });
        Self { port, conns }
    }

    fn dsn(&self) -> String {
        format!(
            "exasol://sys:exasol@127.0.0.1:{}?tls=true&validateservercertificate=0",
            self.port
        )
    }

    fn connection_count(&self) -> usize {
        self.conns.lock().unwrap().len()
    }

    fn any_connection_contains(&self, needle: &[u8]) -> bool {
        self.conns.lock().unwrap().iter().any(|log| {
            let log = log.lock().unwrap();
            contains(&log.0, needle) || contains(&log.1, needle)
        })
    }
}

fn pump(mut from: TcpStream, mut to: TcpStream, log: ConnLog, client_to_server: bool) {
    thread::spawn(move || {
        let mut buf = [0u8; 16384];
        while let Ok(n) = from.read(&mut buf) {
            if n == 0 {
                break;
            }
            {
                let mut log = log.lock().unwrap();
                let sink = if client_to_server {
                    &mut log.0
                } else {
                    &mut log.1
                };
                sink.extend_from_slice(&buf[..n]);
            }
            if to.write_all(&buf[..n]).is_err() {
                break;
            }
        }
        let _ = to.shutdown(Shutdown::Write);
    });
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn marker(tag: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("TLSMARKER-{tag}-{nanos}")
}

fn write_marker_csv(dir: &std::path::Path, marker: &str) -> std::path::PathBuf {
    let mut content = String::from("id,note\n");
    for i in 1..=ROWS {
        content.push_str(&format!("{i},{marker}-{i}\n"));
    }
    fixtures::create_csv_with_content(dir, "marker.csv", &content)
}

fn write_marker_parquet(dir: &std::path::Path, marker: &str) -> std::path::PathBuf {
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int64, false),
        Field::new("note", DataType::Utf8, false),
    ]));
    let ids: Vec<i64> = (1..=ROWS).collect();
    let notes: Vec<String> = ids.iter().map(|i| format!("{marker}-{i}")).collect();
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            Arc::new(Int64Array::from(ids)),
            Arc::new(StringArray::from(notes)),
        ],
    )
    .unwrap();
    let path = dir.join("marker.parquet");
    let mut writer =
        ArrowWriter::try_new(std::fs::File::create(&path).unwrap(), schema, None).unwrap();
    writer.write(&batch).unwrap();
    writer.close().unwrap();
    path
}

/// Fills `<schema>.SRC` with ROWS marker rows over a direct login that bypasses the relay.
async fn create_marker_table(conn: &mut exarrow_rs::Connection, schema: &str, marker: &str) {
    conn.execute_update(&format!(
        "CREATE TABLE {schema}.SRC (\"id\" DECIMAL(18,0), \"note\" VARCHAR(200))"
    ))
    .await
    .unwrap();
    let values: Vec<String> = (1..=ROWS)
        .map(|i| format!("({i}, '{marker}-{i}')"))
        .collect();
    conn.execute_update(&format!(
        "INSERT INTO {schema}.SRC VALUES {}",
        values.join(", ")
    ))
    .await
    .unwrap();
}

fn assert_transfer_used_tls(relay: &RecordingRelay, marker: &str) {
    assert!(
        relay.connection_count() >= 2,
        "expected the login and the transfer tunnel to pass the relay, saw {} connection(s)",
        relay.connection_count()
    );
    assert!(
        !relay.any_connection_contains(marker.as_bytes()),
        "the transfer did not use TLS: row values appeared on the wire as plain text"
    );
}

/// Scenario: Upload uses TLS for the data transfer when the connection uses TLS
#[tokio::test]
async fn upload_csv_uses_tls_for_the_data_transfer() {
    fixtures::require_exasol!();
    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_TLS_UP_CSV").await;
    let dir = tempfile::tempdir().unwrap();
    let marker = marker("upcsv");
    let csv = write_marker_csv(dir.path(), &marker);
    let relay = RecordingRelay::start();

    fixtures::exapump()
        .timeout(std::time::Duration::from_secs(60))
        .args([
            "upload",
            csv.to_str().unwrap(),
            "--table",
            &format!("{schema}.T"),
        ])
        .args(["--dsn", &relay.dsn()])
        .assert()
        .success()
        .stdout(predicates::str::contains(format!("Imported {ROWS} rows")));

    assert_transfer_used_tls(&relay, &marker);
    let _ = conn
        .execute_update(&format!("DROP SCHEMA {schema} CASCADE"))
        .await;
}

/// Scenario: Upload uses TLS for the data transfer when the connection uses TLS
#[tokio::test]
async fn upload_parquet_uses_tls_for_the_data_transfer() {
    fixtures::require_exasol!();
    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_TLS_UP_PQ").await;
    let dir = tempfile::tempdir().unwrap();
    let marker = marker("uppq");
    let parquet = write_marker_parquet(dir.path(), &marker);
    let relay = RecordingRelay::start();

    fixtures::exapump()
        .timeout(std::time::Duration::from_secs(60))
        .args([
            "upload",
            parquet.to_str().unwrap(),
            "--table",
            &format!("{schema}.T"),
        ])
        .args(["--dsn", &relay.dsn()])
        .assert()
        .success()
        .stdout(predicates::str::contains(format!("Imported {ROWS} rows")));

    assert_transfer_used_tls(&relay, &marker);
    let _ = conn
        .execute_update(&format!("DROP SCHEMA {schema} CASCADE"))
        .await;
}

/// Runs one export of `<schema>.SRC` through a fresh relay and checks both the
/// wire and the output file.
async fn export_through_relay(prefix: &str, format: &str, extra: &[&str]) {
    let (mut conn, schema) = fixtures::setup_exasol_schema(prefix).await;
    let marker = marker(prefix);
    create_marker_table(&mut conn, &schema, &marker).await;
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join(format!("out.{format}"));
    let relay = RecordingRelay::start();

    fixtures::exapump()
        .timeout(std::time::Duration::from_secs(60))
        .args(["export", "--table", &format!("{schema}.SRC")])
        .args(["--format", format, "--output", output.to_str().unwrap()])
        .args(extra)
        .args(["--dsn", &relay.dsn()])
        .assert()
        .success();

    assert_transfer_used_tls(&relay, &marker);
    assert_eq!(
        exported_marker_rows(dir.path(), &marker),
        ROWS as usize,
        "the exported file(s) must hold every row"
    );
    let _ = conn
        .execute_update(&format!("DROP SCHEMA {schema} CASCADE"))
        .await;
}

/// Counts the marker values across every CSV or Parquet file in `dir`.
fn exported_marker_rows(dir: &std::path::Path, marker: &str) -> usize {
    let mut count = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "parquet") {
            let file = std::fs::File::open(&path).unwrap();
            let reader =
                parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(file)
                    .unwrap()
                    .build()
                    .unwrap();
            for batch in reader {
                let batch = batch.unwrap();
                let column = batch.column_by_name("note").unwrap();
                let notes = column.as_any().downcast_ref::<StringArray>().unwrap();
                count += notes
                    .iter()
                    .flatten()
                    .filter(|n| n.starts_with(marker))
                    .count();
            }
        } else {
            let text = std::fs::read_to_string(&path).unwrap();
            count += text.matches(marker).count();
        }
    }
    count
}

/// Scenario: Export uses TLS for the data transfer when the connection uses TLS
#[tokio::test]
async fn export_csv_uses_tls_for_the_data_transfer() {
    fixtures::require_exasol!();
    export_through_relay("EXAPUMP_TLS_EX_CSV", "csv", &[]).await;
}

/// Scenario: Export uses TLS for the data transfer when the connection uses TLS
#[tokio::test]
async fn export_parquet_uses_tls_for_the_data_transfer() {
    fixtures::require_exasol!();
    export_through_relay("EXAPUMP_TLS_EX_PQ", "parquet", &[]).await;
}

/// Scenario: Export uses TLS for the data transfer when the connection uses TLS
#[tokio::test]
async fn export_split_parquet_uses_tls_for_the_data_transfer() {
    fixtures::require_exasol!();
    export_through_relay(
        "EXAPUMP_TLS_EX_PQSPLIT",
        "parquet",
        &["--max-rows-per-file", "20"],
    )
    .await;
}

/// Scenario: Export uses TLS for the data transfer when the connection uses TLS
#[tokio::test]
async fn export_split_parquet_of_an_empty_result_uses_tls_for_the_data_transfer() {
    fixtures::require_exasol!();
    let (mut conn, schema) = fixtures::setup_exasol_schema("EXAPUMP_TLS_EX_PQEMPTY").await;
    conn.execute_update(&format!(
        "CREATE TABLE {schema}.SRC (\"id\" DECIMAL(18,0), \"note\" VARCHAR(200))"
    ))
    .await
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("out.parquet");
    let relay = RecordingRelay::start();

    // An empty split export writes its single file through a second, fallback transfer.
    fixtures::exapump()
        .timeout(std::time::Duration::from_secs(60))
        .args(["export", "--table", &format!("{schema}.SRC")])
        .args(["--format", "parquet", "--output", output.to_str().unwrap()])
        .args(["--max-rows-per-file", "20"])
        .args(["--dsn", &relay.dsn()])
        .assert()
        .success();

    // With no rows to look for, a plain HTTP exchange on any connection shows a tunnel without TLS.
    assert!(
        relay.connection_count() >= 2,
        "expected the login and the transfer tunnel to pass the relay, saw {} connection(s)",
        relay.connection_count()
    );
    assert!(
        !relay.any_connection_contains(b"HTTP/1.1"),
        "the transfer did not use TLS: an HTTP exchange appeared on the wire as plain text"
    );
    assert_eq!(exported_marker_rows(dir.path(), "TLSMARKER"), 0);
    let _ = conn
        .execute_update(&format!("DROP SCHEMA {schema} CASCADE"))
        .await;
}
