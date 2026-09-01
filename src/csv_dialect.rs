//! Detects which byte sequence terminates the records of a CSV file.
//!
//! Exasol's `IMPORT` statement takes one fixed `ROW SEPARATOR` for the whole
//! file, so exapump has to tell it the truth about the file it is sending.
//! Naming the wrong separator does not fail loudly: `LF` against a CRLF file
//! leaves a stray `\r` glued to the last column of every row, and `CRLF`
//! against an LF file makes the server read the whole file as a single record
//! that the header skip then discards.
//!
//! The scan is quote-aware, so a `\r`, a `\n` or a `\r\n` inside a quoted
//! field is data and is never mistaken for a record boundary.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use anyhow::{Context, Result};

/// Read granularity of the scan. The file is never held in memory.
const SCAN_CHUNK_SIZE: usize = 64 * 1024;

/// How the records of a CSV file end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowEndings {
    /// Every record ends with `\n`, or the file holds no record boundary at all.
    Lf,
    /// Every record ends with `\r\n`.
    Crlf,
    /// Both styles occur, so no single `ROW SEPARATOR` describes the file.
    Mixed { crlf: u64, lf: u64 },
}

/// Scan `path` and report how its records end.
///
/// `quote` and `delimiter` must match the ones the import will use, because a
/// record boundary is only a boundary outside a quoted field.
pub fn detect_row_endings(path: &Path, quote: u8, delimiter: u8) -> Result<RowEndings> {
    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut scanner = Scanner::new(quote, delimiter);
    let mut chunk = vec![0u8; SCAN_CHUNK_SIZE];

    loop {
        let read = reader
            .read(&mut chunk)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if read == 0 {
            break;
        }
        scanner.feed(&chunk[..read]);
    }

    Ok(scanner.finish())
}

/// Where the scanner is within a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    /// At the first byte of a field, where an opening quote is still possible.
    Start,
    /// Inside a field that did not open with a quote.
    Unquoted,
    /// Inside a quoted field, where delimiters and newlines are data.
    Quoted,
    /// Just past a quote inside a quoted field: a second quote is an escaped
    /// quote, anything else closes the field.
    QuoteSeen,
}

/// Counts record terminators without deciding what they mean.
struct Scanner {
    quote: u8,
    delimiter: u8,
    field: Field,
    previous: Option<u8>,
    crlf: u64,
    lf: u64,
}

impl Scanner {
    fn new(quote: u8, delimiter: u8) -> Self {
        Self {
            quote,
            delimiter,
            field: Field::Start,
            previous: None,
            crlf: 0,
            lf: 0,
        }
    }

    fn feed(&mut self, chunk: &[u8]) {
        for &byte in chunk {
            self.step(byte);
            self.previous = Some(byte);
        }
    }

    fn step(&mut self, byte: u8) {
        match self.field {
            Field::Quoted => {
                if byte == self.quote {
                    self.field = Field::QuoteSeen;
                }
            }
            Field::QuoteSeen if byte == self.quote => self.field = Field::Quoted,
            _ => self.step_outside_quotes(byte),
        }
    }

    /// A stray byte after a closing quote (`"abc"x`) drops the scanner into
    /// `Unquoted` rather than back into the quoted field, so one unbalanced
    /// quote cannot swallow the rest of the file.
    fn step_outside_quotes(&mut self, byte: u8) {
        if byte == b'\n' {
            self.count_terminator();
            self.field = Field::Start;
        } else if byte == self.delimiter {
            self.field = Field::Start;
        } else if byte == self.quote && self.field == Field::Start {
            self.field = Field::Quoted;
        } else {
            self.field = Field::Unquoted;
        }
    }

    fn count_terminator(&mut self) {
        if self.previous == Some(b'\r') {
            self.crlf += 1;
        } else {
            self.lf += 1;
        }
    }

    fn finish(self) -> RowEndings {
        match (self.crlf, self.lf) {
            (0, _) => RowEndings::Lf,
            (_, 0) => RowEndings::Crlf,
            (crlf, lf) => RowEndings::Mixed { crlf, lf },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUOTE: u8 = b'"';
    const COMMA: u8 = b',';
    const SINGLE_QUOTE: u8 = 39;
    const TAB: u8 = b'\t';

    fn scan(content: &[u8]) -> RowEndings {
        scan_with(content, QUOTE, COMMA)
    }

    fn scan_with(content: &[u8], quote: u8, delimiter: u8) -> RowEndings {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scan.csv");
        std::fs::write(&path, content).unwrap();
        detect_row_endings(&path, quote, delimiter).unwrap()
    }

    #[test]
    fn lf_file_reports_lf() {
        assert_eq!(scan(b"id,name\n1,Alice\n2,Bob\n"), RowEndings::Lf);
    }

    #[test]
    fn crlf_file_reports_crlf() {
        assert_eq!(scan(b"id,name\r\n1,Alice\r\n2,Bob\r\n"), RowEndings::Crlf);
    }

    #[test]
    fn crlf_file_without_a_final_newline_reports_crlf() {
        assert_eq!(scan(b"id,name\r\n1,Alice\r\n2,Bob"), RowEndings::Crlf);
    }

    #[test]
    fn mixed_file_reports_both_counts() {
        let endings = scan(b"id,name\r\n1,Alice\r\n2,Bob\n3,Cy\r\n");
        assert_eq!(endings, RowEndings::Mixed { crlf: 3, lf: 1 });
    }

    #[test]
    fn empty_file_reports_lf() {
        assert_eq!(scan(b""), RowEndings::Lf);
    }

    #[test]
    fn single_unterminated_record_reports_lf() {
        assert_eq!(scan(b"id,name"), RowEndings::Lf);
    }

    #[test]
    fn a_lone_cr_inside_a_quoted_field_is_data_not_a_terminator() {
        // The file is LF-terminated; the `\r` belongs to the value.
        assert_eq!(scan(b"id,note\n1,\"has\rcr\"\n"), RowEndings::Lf);
    }

    #[test]
    fn a_crlf_inside_a_quoted_field_is_data_not_a_terminator() {
        // Every record ends with a bare `\n`, so the embedded `\r\n` must not
        // make this look like a CRLF file.
        assert_eq!(
            scan(b"id,note\n1,\"two\r\nlines\"\n2,plain\n"),
            RowEndings::Lf
        );
    }

    #[test]
    fn a_quoted_newline_does_not_end_a_record_in_a_crlf_file() {
        let endings = scan(b"id,note\r\n1,\"two\nlines\"\r\n2,plain\r\n");
        assert_eq!(endings, RowEndings::Crlf);
    }

    #[test]
    fn a_lone_cr_inside_an_unquoted_field_is_data_not_a_terminator() {
        // A `\r` that is not followed by `\n` is never treated as a separator.
        assert_eq!(scan(b"id,note\n1,has\rcr\n"), RowEndings::Lf);
    }

    #[test]
    fn a_doubled_quote_keeps_the_field_open() {
        let endings = scan(b"id,note\r\n1,\"say \"\"hi\"\"\r\nagain\"\r\n");
        assert_eq!(endings, RowEndings::Crlf);
    }

    #[test]
    fn a_quote_in_the_middle_of_an_unquoted_field_does_not_open_a_quote() {
        assert_eq!(scan(b"id,note\n1,6\" pipe\n2,plain\n"), RowEndings::Lf);
    }

    #[test]
    fn a_stray_byte_after_a_closing_quote_does_not_swallow_the_file() {
        assert_eq!(
            scan(b"id,note\r\n1,\"quoted\"x\r\n2,plain\r\n"),
            RowEndings::Crlf
        );
    }

    #[test]
    fn the_configured_quote_character_is_honoured() {
        // With `'` as the quote character the `\r\n` inside the field is data.
        assert_eq!(
            scan_with(b"id,note\n1,'two\r\nlines'\n", SINGLE_QUOTE, COMMA),
            RowEndings::Lf
        );
    }

    #[test]
    fn the_configured_delimiter_is_honoured() {
        // A tab-separated CRLF file: the comma is an ordinary character.
        assert_eq!(
            scan_with(b"id\tnote\r\n1\ta,b\r\n", QUOTE, TAB),
            RowEndings::Crlf
        );
    }

    #[test]
    fn an_empty_trailing_field_before_crlf_is_still_crlf() {
        assert_eq!(scan(b"id,note\r\n1,\r\n"), RowEndings::Crlf);
    }

    #[test]
    fn a_record_boundary_is_found_across_a_chunk_boundary() {
        // Force the `\r` and the `\n` of one terminator into separate reads.
        let mut content = vec![b'a'; SCAN_CHUNK_SIZE - 1];
        content.extend_from_slice(b"\r\nb\r\n");
        assert_eq!(scan(&content), RowEndings::Crlf);
    }

    #[test]
    fn a_missing_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("absent.csv");
        let err = detect_row_endings(&missing, QUOTE, COMMA).unwrap_err();
        assert!(
            err.to_string().contains("absent.csv"),
            "error should name the file: {err}"
        );
    }
}
