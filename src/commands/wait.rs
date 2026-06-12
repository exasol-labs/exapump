use std::net::{TcpStream, ToSocketAddrs};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::cli::WaitArgs;

const POLL_INTERVAL_SECS: u64 = 5;
const TCP_CONNECT_TIMEOUT_SECS: u64 = 2;
const DIAGNOSTIC_INTERVAL_SECS: u64 = 30;

const EXIT_CONFIG_ERROR: i32 = 1;
const EXIT_TIMEOUT: i32 = 2;
const EXIT_CONTAINER_FAILURE: i32 = 3;

/// Poll Exasol until TCP port is open and `SELECT 1` succeeds, then exit 0.
pub async fn run(args: WaitArgs) -> anyhow::Result<()> {
    let dsn = match args.conn.resolve_dsn() {
        Ok(dsn) => dsn,
        Err(error) => {
            eprintln!("No connection info available: {error}");
            std::process::exit(EXIT_CONFIG_ERROR);
        }
    };

    let (host, port) = match parse_host_port(&dsn) {
        Ok(host_port) => host_port,
        Err(error) => {
            eprintln!("Failed to parse connection info from DSN: {error}");
            std::process::exit(EXIT_CONFIG_ERROR);
        }
    };

    let container = args.container.as_deref();
    let start = Instant::now();

    if let Some(name) = container {
        if !check_container_liveness(name) {
            eprintln!("Container '{name}' is not running");
            std::process::exit(EXIT_CONTAINER_FAILURE);
        }
    }

    let mut last_diagnostic_secs = 0;

    wait_for_tcp(
        &host,
        port,
        args.timeout_secs,
        container,
        start,
        &mut last_diagnostic_secs,
    );

    wait_for_sql(
        &args,
        args.timeout_secs,
        container,
        start,
        &mut last_diagnostic_secs,
    )
    .await;

    println!("Exasol is ready! (total {}s)", start.elapsed().as_secs());
    Ok(())
}

/// Phase 1: poll the TCP port until it accepts a connection. Enforces the
/// overall deadline (exit 2) and the optional container liveness guard (exit 3).
fn wait_for_tcp(
    host: &str,
    port: u16,
    timeout_secs: u64,
    container: Option<&str>,
    start: Instant,
    last_diagnostic_secs: &mut u64,
) {
    eprintln!("--- Phase 1: Waiting for TCP port {host}:{port} ---");

    loop {
        enforce_deadline(timeout_secs, start);
        enforce_container_liveness(container);

        if tcp_port_open(host, port) {
            eprintln!(
                "TCP port {host}:{port} is open (after {}s)",
                start.elapsed().as_secs()
            );
            return;
        }

        report_progress(start, last_diagnostic_secs);
        sleep_before_retry(timeout_secs, start);
    }
}

/// Phase 2: poll `SELECT 1` over the configured transport until it succeeds.
/// Enforces the overall deadline (exit 2) and the container guard (exit 3).
async fn wait_for_sql(
    args: &WaitArgs,
    timeout_secs: u64,
    container: Option<&str>,
    start: Instant,
    last_diagnostic_secs: &mut u64,
) {
    eprintln!("--- Phase 2: Waiting for SQL (SELECT 1) ---");

    loop {
        enforce_deadline(timeout_secs, start);
        enforce_container_liveness(container);

        if select_one_succeeds(args).await {
            return;
        }

        report_progress(start, last_diagnostic_secs);
        sleep_before_retry(timeout_secs, start);
    }
}

/// Attempt a single `SELECT 1` against the database, returning whether it
/// completed successfully. Connection and query errors are treated as
/// "not ready yet" so the caller retries.
async fn select_one_succeeds(args: &WaitArgs) -> bool {
    match args.conn.connect().await {
        Ok(mut conn) => conn.execute("SELECT 1").await.is_ok(),
        Err(_) => false,
    }
}

/// Attempt a single TCP connection to `host:port` with a bounded timeout,
/// returning whether the port accepted the connection.
fn tcp_port_open(host: &str, port: u16) -> bool {
    let addrs = match (host, port).to_socket_addrs() {
        Ok(addrs) => addrs,
        Err(_) => return false,
    };

    let timeout = Duration::from_secs(TCP_CONNECT_TIMEOUT_SECS);
    for addr in addrs {
        if TcpStream::connect_timeout(&addr, timeout).is_ok() {
            return true;
        }
    }
    false
}

/// Exit with code 2 if the overall elapsed time has reached the deadline.
fn enforce_deadline(timeout_secs: u64, start: Instant) {
    let elapsed = start.elapsed().as_secs();
    if elapsed >= timeout_secs {
        eprintln!("Timed out after {elapsed}s waiting for Exasol to become ready");
        std::process::exit(EXIT_TIMEOUT);
    }
}

/// When a container is named, exit with code 3 (after dumping recent logs) if
/// the container is no longer running.
fn enforce_container_liveness(container: Option<&str>) {
    if let Some(name) = container {
        if !check_container_liveness(name) {
            eprintln!("Container '{name}' stopped unexpectedly");
            dump_container_logs(name);
            std::process::exit(EXIT_CONTAINER_FAILURE);
        }
    }
}

/// Emit a diagnostic line if at least the diagnostic interval has elapsed
/// since the last one.
fn report_progress(start: Instant, last_diagnostic_secs: &mut u64) {
    let elapsed = start.elapsed().as_secs();
    if elapsed >= *last_diagnostic_secs + DIAGNOSTIC_INTERVAL_SECS {
        eprintln!("  Still waiting... ({elapsed}s elapsed)");
        *last_diagnostic_secs = elapsed;
    }
}

fn sleep_before_retry(timeout_secs: u64, start: Instant) {
    enforce_deadline(timeout_secs, start);
    let remaining = timeout_secs.saturating_sub(start.elapsed().as_secs());
    let sleep_secs = remaining.min(POLL_INTERVAL_SECS);
    std::thread::sleep(Duration::from_secs(sleep_secs));
}

/// Returns whether a container with the given name appears in `docker ps`.
fn check_container_liveness(name: &str) -> bool {
    let output = Command::new("docker")
        .args(["ps", "--format", "{{.Names}}"])
        .output();

    match output {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .lines()
            .any(|line| line.trim() == name),
        _ => false,
    }
}

/// Print the container's recent Docker logs to stderr (best effort).
fn dump_container_logs(name: &str) {
    let output = Command::new("docker")
        .args(["logs", "--tail", "50", name])
        .output();

    if let Ok(output) = output {
        eprintln!("--- docker logs --tail 50 {name} ---");
        eprint!("{}", String::from_utf8_lossy(&output.stdout));
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
    }
}

/// Extract the host and port from a DSN of the form
/// `scheme://[user[:pwd]@]host:port[/path][?query]`. The port is required.
fn parse_host_port(dsn: &str) -> anyhow::Result<(String, u16)> {
    let after_scheme = dsn.split_once("://").map(|(_, rest)| rest).unwrap_or(dsn);

    let authority = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(after_scheme);

    let host_port = match authority.rsplit_once('@') {
        Some((_, host_port)) => host_port,
        None => authority,
    };

    let (host, port_str) = split_host_port(host_port)
        .ok_or_else(|| anyhow::anyhow!("DSN '{dsn}' is missing a host and port"))?;

    if host.is_empty() {
        anyhow::bail!("DSN '{dsn}' is missing a host");
    }

    let port: u16 = port_str
        .parse()
        .map_err(|_| anyhow::anyhow!("DSN '{dsn}' has an invalid port '{port_str}'"))?;

    Ok((host.to_string(), port))
}

/// Split a `host:port` authority into its parts, handling bracketed IPv6
/// literals (`[::1]:8563`). Returns `None` when no port is present.
fn split_host_port(host_port: &str) -> Option<(&str, &str)> {
    if let Some(rest) = host_port.strip_prefix('[') {
        let (host, after) = rest.split_once(']')?;
        let port = after.strip_prefix(':')?;
        return Some((host, port));
    }

    host_port.rsplit_once(':')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_host_and_port_from_full_dsn() {
        let dsn = "exasol://sys:exasol@localhost:8563?tls=true&validateservercertificate=0";
        assert_eq!(
            parse_host_port(dsn).unwrap(),
            ("localhost".to_string(), 8563)
        );
    }

    #[test]
    fn parses_dsn_without_credentials() {
        let dsn = "exasol://db.example.com:1234";
        assert_eq!(
            parse_host_port(dsn).unwrap(),
            ("db.example.com".to_string(), 1234)
        );
    }

    #[test]
    fn password_containing_at_sign_uses_rightmost_separator() {
        let dsn = "exasol://user:p@ss@host:8563";
        assert_eq!(parse_host_port(dsn).unwrap(), ("host".to_string(), 8563));
    }

    #[test]
    fn ignores_query_string_when_deriving_port() {
        let dsn = "exasol://user:pwd@host:8563?transport=websocket";
        assert_eq!(parse_host_port(dsn).unwrap(), ("host".to_string(), 8563));
    }

    #[test]
    fn ignores_trailing_path_segment() {
        let dsn = "exasol://user:pwd@host:8563/EXA_DB";
        assert_eq!(parse_host_port(dsn).unwrap(), ("host".to_string(), 8563));
    }

    #[test]
    fn parses_bracketed_ipv6_host() {
        let dsn = "exasol://user:pwd@[::1]:8563?tls=true";
        assert_eq!(parse_host_port(dsn).unwrap(), ("::1".to_string(), 8563));
    }

    #[test]
    fn rejects_dsn_without_port() {
        let dsn = "exasol://user:pwd@host";
        assert!(parse_host_port(dsn).is_err());
    }

    #[test]
    fn rejects_dsn_with_non_numeric_port() {
        let dsn = "exasol://user:pwd@host:notaport";
        assert!(parse_host_port(dsn).is_err());
    }

    #[test]
    fn rejects_dsn_with_out_of_range_port() {
        let dsn = "exasol://user:pwd@host:70000";
        assert!(parse_host_port(dsn).is_err());
    }

    #[test]
    fn rejects_dsn_with_empty_host() {
        let dsn = "exasol://user:pwd@:8563";
        assert!(parse_host_port(dsn).is_err());
    }
}
