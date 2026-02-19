use std::path::Path;

use anyhow::{bail, Result};

#[derive(Debug, PartialEq)]
pub enum FileFormat {
    Parquet,
    Csv,
}

const SUPPORTED_FORMATS: &str = ".parquet, .csv";

/// Returns an error listing supported formats when the extension is unrecognized.
pub fn detect_from_path(path: &Path) -> Result<FileFormat> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    match ext.to_ascii_lowercase().as_str() {
        "parquet" => Ok(FileFormat::Parquet),
        "csv" => Ok(FileFormat::Csv),
        _ => bail!("file format {ext:?} is not supported. Supported formats: {SUPPORTED_FORMATS}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn parquet_extension_returns_parquet() {
        let result = detect_from_path(Path::new("data.parquet"));
        assert_eq!(result.unwrap(), FileFormat::Parquet);
    }

    #[test]
    fn uppercase_parquet_extension_returns_parquet() {
        let result = detect_from_path(Path::new("data.PARQUET"));
        assert_eq!(result.unwrap(), FileFormat::Parquet);
    }

    #[test]
    fn unsupported_extension_returns_error_with_supported_formats() {
        let result = detect_from_path(Path::new("data.json"));
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("not supported"),
            "error should mention not supported: {msg}"
        );
        assert!(
            msg.contains(".parquet, .csv"),
            "error should list supported formats: {msg}"
        );
    }

    #[test]
    fn csv_extension_returns_csv() {
        let result = detect_from_path(Path::new("data.csv"));
        assert_eq!(result.unwrap(), FileFormat::Csv);
    }

    #[test]
    fn uppercase_csv_extension_returns_csv() {
        let result = detect_from_path(Path::new("data.CSV"));
        assert_eq!(result.unwrap(), FileFormat::Csv);
    }

    #[test]
    fn no_extension_returns_error() {
        let result = detect_from_path(Path::new("data"));
        assert!(result.is_err());
    }
}
