use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::Result;
use tokio::io::AsyncWrite;

/// Generate a split file path from a base path and a zero-based index.
///
/// Given `base = "/tmp/data.parquet"` and `index = 3`, returns
/// `/tmp/data_003.parquet`.
pub fn split_path(base: &Path, index: u32) -> PathBuf {
    let stem = base
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let ext = base.extension().and_then(|s| s.to_str()).unwrap_or("");
    let filename = if ext.is_empty() {
        format!("{stem}_{index:03}")
    } else {
        format!("{stem}_{index:03}.{ext}")
    };
    base.with_file_name(filename)
}

/// If only one split file was produced (index 0), rename `<stem>_000.<ext>`
/// back to the original output path so the user gets the clean name they asked for.
///
/// Returns `Ok(())` if the rename succeeded or if the file was already at the
/// desired path.
pub fn rename_single_split(base: &Path) -> Result<()> {
    let split_0 = split_path(base, 0);
    if split_0 != base && split_0.exists() {
        std::fs::rename(&split_0, base)?;
    }
    Ok(())
}

const LINE_BUFFER_CAPACITY: usize = 8192;

/// A splitting CSV writer that implements `tokio::io::AsyncWrite`.
///
/// Receives a CSV byte stream and distributes rows across multiple output files
/// based on row count and/or byte size thresholds. Each split file receives
/// its own copy of the header row (if headers are included in the stream).
///
/// CSV-aware row counting correctly handles quoted fields that contain
/// embedded newlines — a newline inside double quotes is not a row boundary.
pub struct SplitCsvWriter {
    base_path: PathBuf,
    max_rows: Option<u64>,
    max_bytes: Option<u64>,
    include_header: bool,

    header: Option<Vec<u8>>,
    header_captured: bool,
    line_buffer: Vec<u8>,
    in_quotes: bool,

    current_file: Option<BufWriter<File>>,
    file_index: u32,
    rows_in_file: u64,
    bytes_in_file: u64,
    total_rows: u64,
}

impl SplitCsvWriter {
    /// Create a new `SplitCsvWriter`.
    ///
    /// - `base_path`: the user-specified output path (e.g. `/tmp/data.csv`).
    ///   Split files will be named `data_000.csv`, `data_001.csv`, etc.
    /// - `max_rows`: optional maximum data rows per file.
    /// - `max_bytes`: optional maximum bytes per file.
    /// - `include_header`: whether the CSV stream starts with a header line.
    ///   When `true`, the first line is captured and replayed at the start of
    ///   each split file.
    pub fn new(
        base_path: PathBuf,
        max_rows: Option<u64>,
        max_bytes: Option<u64>,
        include_header: bool,
    ) -> Self {
        Self {
            base_path,
            max_rows,
            max_bytes,
            include_header,
            header: None,
            header_captured: false,
            line_buffer: Vec::with_capacity(LINE_BUFFER_CAPACITY),
            in_quotes: false,
            current_file: None,
            file_index: 0,
            rows_in_file: 0,
            bytes_in_file: 0,
            total_rows: 0,
        }
    }

    /// Flush and close the writer, returning `(total_rows, num_files)`.
    ///
    /// `total_rows` counts only data rows (excludes headers).
    /// `num_files` is the number of split files produced.
    pub fn finish(&mut self) -> Result<(u64, u32)> {
        if !self.line_buffer.is_empty() {
            self.flush_line()?;
        }
        if let Some(ref mut w) = self.current_file {
            w.flush()?;
        }
        self.current_file = None;

        let num_files =
            if self.file_index == 0 && self.total_rows == 0 && self.current_file.is_none() {
                // Nothing was ever written — but we might have opened file 0
                // Check if file_index advanced; if we wrote anything, file_index
                // is the index of the last file we wrote to, so count = index + 1.
                0
            } else {
                self.file_index + 1
            };

        Ok((self.total_rows, num_files))
    }

    /// Open a new split file at the current `file_index`.
    fn open_next_file(&mut self) -> io::Result<()> {
        let path = split_path(&self.base_path, self.file_index);
        let file = File::create(&path)?;
        let mut writer = BufWriter::new(file);

        if let Some(ref hdr) = self.header {
            writer.write_all(hdr)?;
            self.bytes_in_file = hdr.len() as u64;
        } else {
            self.bytes_in_file = 0;
        }

        self.current_file = Some(writer);
        self.rows_in_file = 0;
        Ok(())
    }

    /// Rotate to the next split file.
    fn rotate_file(&mut self) -> io::Result<()> {
        if let Some(ref mut w) = self.current_file {
            w.flush()?;
        }
        self.current_file = None;
        self.file_index += 1;
        self.open_next_file()
    }

    /// Check if the current file has hit a threshold and needs rotation.
    fn needs_rotation(&self) -> bool {
        if let Some(max_r) = self.max_rows {
            if self.rows_in_file >= max_r {
                return true;
            }
        }
        if let Some(max_b) = self.max_bytes {
            if self.bytes_in_file >= max_b {
                return true;
            }
        }
        false
    }

    /// Process a completed line (ending with `\n`). The line_buffer contains
    /// the full line including the trailing newline.
    fn flush_line(&mut self) -> io::Result<()> {
        let line = std::mem::take(&mut self.line_buffer);

        if self.include_header && !self.header_captured {
            self.header_captured = true;
            self.header = Some(line.clone());
            self.open_next_file()?;
            return Ok(());
        }

        if self.current_file.is_none() {
            self.open_next_file()?;
        }

        if self.needs_rotation() {
            self.rotate_file()?;
        }

        let writer = self.current_file.as_mut().unwrap();
        writer.write_all(&line)?;
        let line_len = line.len() as u64;
        self.bytes_in_file += line_len;
        self.rows_in_file += 1;
        self.total_rows += 1;

        Ok(())
    }

    /// Process a buffer of bytes, tracking CSV quoting state and row boundaries.
    fn process_bytes(&mut self, data: &[u8]) -> io::Result<()> {
        for &byte in data {
            self.line_buffer.push(byte);

            if self.in_quotes {
                if byte == b'"' {
                    self.in_quotes = false;
                }
            } else if byte == b'"' {
                self.in_quotes = true;
            } else if byte == b'\n' {
                self.flush_line()?;
            }
        }
        Ok(())
    }
}

// SplitCsvWriter is Unpin because it contains no self-referential fields.
impl Unpin for SplitCsvWriter {}

impl AsyncWrite for SplitCsvWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.process_bytes(buf) {
            Ok(()) => Poll::Ready(Ok(buf.len())),
            Err(e) => Poll::Ready(Err(e)),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if let Some(ref mut w) = self.current_file {
            if let Err(e) = w.flush() {
                return Poll::Ready(Err(e));
            }
        }
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // Flush any remaining partial line.
        if !self.line_buffer.is_empty() {
            if let Err(e) = self.flush_line() {
                return Poll::Ready(Err(e));
            }
        }
        if let Some(ref mut w) = self.current_file {
            if let Err(e) = w.flush() {
                return Poll::Ready(Err(e));
            }
        }
        self.current_file = None;
        Poll::Ready(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_path_with_extension() {
        let base = Path::new("/tmp/data.parquet");
        assert_eq!(split_path(base, 0), PathBuf::from("/tmp/data_000.parquet"));
        assert_eq!(split_path(base, 1), PathBuf::from("/tmp/data_001.parquet"));
        assert_eq!(split_path(base, 42), PathBuf::from("/tmp/data_042.parquet"));
    }

    #[test]
    fn split_path_csv() {
        let base = Path::new("/output/report.csv");
        assert_eq!(split_path(base, 0), PathBuf::from("/output/report_000.csv"));
        assert_eq!(split_path(base, 5), PathBuf::from("/output/report_005.csv"));
    }

    #[test]
    fn split_path_no_extension() {
        let base = Path::new("/tmp/data");
        assert_eq!(split_path(base, 0), PathBuf::from("/tmp/data_000"));
        assert_eq!(split_path(base, 10), PathBuf::from("/tmp/data_010"));
    }

    #[test]
    fn split_path_large_index() {
        let base = Path::new("/tmp/data.csv");
        assert_eq!(split_path(base, 999), PathBuf::from("/tmp/data_999.csv"));
        // Index > 999 still works (4-digit)
        assert_eq!(split_path(base, 1234), PathBuf::from("/tmp/data_1234.csv"));
    }

    #[test]
    fn rename_single_split_works() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("data.parquet");

        // Create the _000 file
        let split_0 = split_path(&base, 0);
        std::fs::write(&split_0, b"parquet data").unwrap();

        // Rename should move it to the base path
        rename_single_split(&base).unwrap();
        assert!(base.exists());
        assert!(!split_0.exists());
        assert_eq!(std::fs::read(&base).unwrap(), b"parquet data");
    }

    #[test]
    fn rename_single_split_no_file_is_ok() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("data.csv");

        // No _000 file exists; should not error
        rename_single_split(&base).unwrap();
    }

    // --- SplitCsvWriter tests ---

    #[tokio::test]
    async fn split_csv_writer_basic_split() {
        use tokio::io::AsyncWriteExt;
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("data.csv");
        let mut writer = SplitCsvWriter::new(base.clone(), Some(3), None, true);

        let csv_data = b"id,name\n1,Alice\n2,Bob\n3,Charlie\n4,Dave\n5,Eve\n6,Frank\n";
        writer.write_all(csv_data).await.unwrap();
        let (total_rows, num_files) = writer.finish().unwrap();

        assert_eq!(total_rows, 6);
        assert_eq!(num_files, 2);

        let file0 = std::fs::read_to_string(split_path(&base, 0)).unwrap();
        let file1 = std::fs::read_to_string(split_path(&base, 1)).unwrap();
        assert!(file0.starts_with("id,name\n"));
        assert!(file1.starts_with("id,name\n"));
        // file0 should have header + 3 data rows, file1 header + 3 data rows
        assert_eq!(file0.lines().count(), 4); // header + 3 rows
        assert_eq!(file1.lines().count(), 4); // header + 3 rows
    }

    #[tokio::test]
    async fn split_csv_writer_header_in_each_file() {
        use tokio::io::AsyncWriteExt;
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("data.csv");
        let mut writer = SplitCsvWriter::new(base.clone(), Some(2), None, true);

        let csv_data = b"col_a,col_b\nX,1\nY,2\nZ,3\nW,4\n";
        writer.write_all(csv_data).await.unwrap();
        let (total_rows, num_files) = writer.finish().unwrap();

        assert_eq!(total_rows, 4);
        assert_eq!(num_files, 2);

        let file0 = std::fs::read_to_string(split_path(&base, 0)).unwrap();
        let file1 = std::fs::read_to_string(split_path(&base, 1)).unwrap();

        // Both files must start with the header
        assert!(
            file0.starts_with("col_a,col_b\n"),
            "file0 missing header: {file0}"
        );
        assert!(
            file1.starts_with("col_a,col_b\n"),
            "file1 missing header: {file1}"
        );
    }

    #[tokio::test]
    async fn split_csv_writer_no_header() {
        use tokio::io::AsyncWriteExt;
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("data.csv");
        let mut writer = SplitCsvWriter::new(base.clone(), Some(3), None, false);

        // No header line; 6 data rows
        let csv_data = b"1,Alice\n2,Bob\n3,Charlie\n4,Dave\n5,Eve\n6,Frank\n";
        writer.write_all(csv_data).await.unwrap();
        let (total_rows, num_files) = writer.finish().unwrap();

        assert_eq!(total_rows, 6);
        assert_eq!(num_files, 2);

        let file0 = std::fs::read_to_string(split_path(&base, 0)).unwrap();
        let file1 = std::fs::read_to_string(split_path(&base, 1)).unwrap();

        // No header in either file; each should have exactly 3 data rows
        assert_eq!(file0.lines().count(), 3);
        assert_eq!(file1.lines().count(), 3);

        // First file should start with data, not a header
        assert!(file0.starts_with("1,Alice\n"));
        assert!(file1.starts_with("4,Dave\n"));
    }

    #[tokio::test]
    async fn split_csv_writer_quoted_newlines() {
        use tokio::io::AsyncWriteExt;
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("data.csv");
        let mut writer = SplitCsvWriter::new(base.clone(), Some(3), None, true);

        // Row 2 contains a quoted field with an embedded newline.
        // This should be counted as 1 data row, not 2.
        let csv_data = b"id,msg,val\n1,hello,A\n2,\"hello\nworld\",B\n3,foo,C\n4,bar,D\n5,baz,E\n";
        writer.write_all(csv_data).await.unwrap();
        let (total_rows, num_files) = writer.finish().unwrap();

        // 5 data rows total; the embedded newline in row 2 should NOT split
        assert_eq!(total_rows, 5);
        assert_eq!(num_files, 2);

        let file0 = std::fs::read_to_string(split_path(&base, 0)).unwrap();
        // file0 should contain header + 3 data rows (rows 1, 2, 3)
        // Row 2 has an embedded newline, so raw line count is header(1) + row1(1) + row2(2) + row3(1) = 5
        assert!(
            file0.contains("\"hello\nworld\""),
            "Embedded newline should be preserved in output"
        );
    }

    #[tokio::test]
    async fn split_csv_writer_single_file_no_rotation() {
        use tokio::io::AsyncWriteExt;
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("data.csv");
        let mut writer = SplitCsvWriter::new(base.clone(), Some(100), None, true);

        let csv_data = b"id,name\n1,Alice\n2,Bob\n";
        writer.write_all(csv_data).await.unwrap();
        let (total_rows, num_files) = writer.finish().unwrap();

        assert_eq!(total_rows, 2);
        assert_eq!(num_files, 1);

        // Only file_000 should exist
        assert!(split_path(&base, 0).exists());
        assert!(!split_path(&base, 1).exists());
    }

    #[tokio::test]
    async fn split_csv_writer_byte_threshold() {
        use tokio::io::AsyncWriteExt;
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("data.csv");

        // Set max_bytes very low (e.g. 30 bytes) to force rotation.
        // Header "id,name\n" = 8 bytes. Each data row ~9 bytes.
        // After header (8) + 2 rows (18) = 26 bytes -> within limit.
        // After 3rd row = 35 bytes -> exceeds limit, triggering rotation.
        let mut writer = SplitCsvWriter::new(base.clone(), None, Some(30), true);

        let csv_data = b"id,name\n1,Alice\n2,Bobby\n3,Chris\n4,Danny\n";
        writer.write_all(csv_data).await.unwrap();
        let (total_rows, num_files) = writer.finish().unwrap();

        assert_eq!(total_rows, 4);
        // With 30-byte limit, we should get more than one file
        assert!(
            num_files >= 2,
            "Expected at least 2 files with byte threshold, got {num_files}"
        );

        // All files should exist
        for i in 0..num_files {
            assert!(split_path(&base, i).exists(), "Expected file {i} to exist");
        }
    }
}
