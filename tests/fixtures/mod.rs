use std::path::PathBuf;
use std::sync::Arc;

use arrow::array::{Float64Array, Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use assert_cmd::cargo_bin_cmd;
use assert_cmd::Command;
use parquet::arrow::ArrowWriter;

#[allow(dead_code)]
pub const DUMMY_DSN: &str = "exasol://user:pwd@host:8563";
#[allow(dead_code)]
pub const DOCKER_DSN: &str =
    "exasol://sys:exasol@localhost:8563?tls=true&validateservercertificate=0";

pub fn exapump() -> Command {
    cargo_bin_cmd!("exapump")
}

/// Creates a small Parquet file at `dir/test.parquet` with 3 columns and 3 rows.
/// Returns the path to the created file.
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
