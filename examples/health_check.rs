//! Health check for Exasol Docker container.
//!
//! Connects via WebSocket (same path the tests use) and runs `SELECT 1`.
//! Reads connection details from environment variables:
//! - `EXASOL_HOST` (default: `localhost`)
//! - `EXASOL_PORT` (default: `8563`)
//! - `EXASOL_USER` (default: `sys`)
//! - `EXASOL_PASSWORD` (default: `exasol`)
//!
//! Exits 0 if ready, or if `REQUIRE_EXASOL` is not set and Exasol is unreachable.
//! Exits 1 if `REQUIRE_EXASOL` is set and Exasol is unreachable. 10 s timeout.

use std::time::Duration;

#[tokio::main]
async fn main() {
    let host = std::env::var("EXASOL_HOST").unwrap_or_else(|_| "localhost".into());
    let port = std::env::var("EXASOL_PORT").unwrap_or_else(|_| "8563".into());
    let user = std::env::var("EXASOL_USER").unwrap_or_else(|_| "sys".into());
    let password = std::env::var("EXASOL_PASSWORD").unwrap_or_else(|_| "exasol".into());

    let dsn =
        format!("exasol://{user}:{password}@{host}:{port}?tls=true&validateservercertificate=0");

    match tokio::time::timeout(Duration::from_secs(10), check(&dsn)).await {
        Ok(Ok(())) => {
            println!("Exasol is ready");
        }
        Ok(Err(e)) => {
            if std::env::var("REQUIRE_EXASOL").is_ok() {
                eprintln!("Connection failed: {e}");
                std::process::exit(1);
            }
            eprintln!("Skipping: Exasol not available ({e})");
        }
        Err(_) => {
            if std::env::var("REQUIRE_EXASOL").is_ok() {
                eprintln!("Timed out after 10s");
                std::process::exit(1);
            }
            eprintln!("Skipping: Exasol not available (timeout)");
        }
    }
}

async fn check(dsn: &str) -> anyhow::Result<()> {
    let driver = exarrow_rs::Driver::new();
    let db = driver.open(dsn)?;
    let mut conn = db.connect().await?;
    conn.execute_update("SELECT 1").await?;
    Ok(())
}
