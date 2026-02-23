use anyhow::{bail, Result};

/// Parse a human-readable size string into bytes.
///
/// Supports base-10 units (case-insensitive):
/// - `KB` = 1,000
/// - `MB` = 1,000,000
/// - `GB` = 1,000,000,000
///
/// A plain number without suffix is treated as bytes.
pub fn parse_size(s: &str) -> Result<u64> {
    let s = s.trim();
    if s.is_empty() {
        bail!("size string is empty");
    }

    let s_upper = s.to_uppercase();

    let (num_str, multiplier) = if let Some(n) = s_upper.strip_suffix("GB") {
        (n, 1_000_000_000u64)
    } else if let Some(n) = s_upper.strip_suffix("MB") {
        (n, 1_000_000u64)
    } else if let Some(n) = s_upper.strip_suffix("KB") {
        (n, 1_000u64)
    } else {
        (s_upper.as_str(), 1u64)
    };

    let num: u64 = num_str
        .trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid size: '{s}'"))?;

    num.checked_mul(multiplier)
        .ok_or_else(|| anyhow::anyhow!("size overflow: '{s}'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_bytes() {
        assert_eq!(parse_size("1024").unwrap(), 1024);
    }

    #[test]
    fn kilobytes_uppercase() {
        assert_eq!(parse_size("500KB").unwrap(), 500_000);
    }

    #[test]
    fn kilobytes_lowercase() {
        assert_eq!(parse_size("500kb").unwrap(), 500_000);
    }

    #[test]
    fn megabytes() {
        assert_eq!(parse_size("1MB").unwrap(), 1_000_000);
    }

    #[test]
    fn megabytes_mixed_case() {
        assert_eq!(parse_size("10Mb").unwrap(), 10_000_000);
    }

    #[test]
    fn gigabytes() {
        assert_eq!(parse_size("2GB").unwrap(), 2_000_000_000);
    }

    #[test]
    fn gigabytes_lowercase() {
        assert_eq!(parse_size("2gb").unwrap(), 2_000_000_000);
    }

    #[test]
    fn with_whitespace() {
        assert_eq!(parse_size("  500KB  ").unwrap(), 500_000);
    }

    #[test]
    fn zero_bytes() {
        assert_eq!(parse_size("0").unwrap(), 0);
    }

    #[test]
    fn zero_kb() {
        assert_eq!(parse_size("0KB").unwrap(), 0);
    }

    #[test]
    fn empty_string_fails() {
        assert!(parse_size("").is_err());
    }

    #[test]
    fn invalid_number_fails() {
        assert!(parse_size("abcKB").is_err());
    }

    #[test]
    fn negative_number_fails() {
        assert!(parse_size("-1KB").is_err());
    }

    #[test]
    fn decimal_number_fails() {
        assert!(parse_size("1.5MB").is_err());
    }
}
