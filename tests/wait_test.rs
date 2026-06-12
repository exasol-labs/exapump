mod fixtures;

use predicates::prelude::*;

// --- wait subcommand tests ---

#[test]
fn wait_help_shows_all_arguments() {
    fixtures::exapump()
        .args(["wait", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--dsn"))
        .stdout(predicate::str::contains("--profile"))
        .stdout(predicate::str::contains("--container"))
        .stdout(predicate::str::contains("--timeout-secs"))
        .stdout(predicate::str::contains("1500"));
}

#[test]
fn wait_succeeds_when_database_ready() {
    fixtures::require_exasol!();

    fixtures::exapump()
        .args(["wait", "--dsn", fixtures::DOCKER_DSN])
        .assert()
        .code(0)
        .stdout(predicate::str::is_match("(?i)ready").unwrap());
}

#[test]
fn wait_succeeds_before_timeout() {
    fixtures::require_exasol!();
    fixtures::exapump()
        .args([
            "wait",
            "--dsn",
            fixtures::DOCKER_DSN,
            "--timeout-secs",
            "60",
        ])
        .assert()
        .success()
        .code(0);
}

#[test]
fn wait_times_out_when_sql_never_ready() {
    fixtures::exapump()
        .args([
            "wait",
            "--dsn",
            "exasol://sys:exasol@localhost:9999?validateservercertificate=0",
            "--timeout-secs",
            "1",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::is_match("(?i)timed? out").unwrap());
}

#[test]
fn wait_fails_when_container_absent() {
    fixtures::require_exasol!();

    fixtures::exapump()
        .args([
            "wait",
            "--dsn",
            fixtures::DOCKER_DSN,
            "--container",
            "does-not-exist-xyz",
        ])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("not running"));
}

#[test]
fn wait_skips_docker_without_container_flag() {
    fixtures::exapump()
        .args([
            "wait",
            "--dsn",
            "exasol://sys:exasol@localhost:9999?validateservercertificate=0",
            "--timeout-secs",
            "1",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::is_match("(?i)timed out|timeout").unwrap())
        .stderr(predicate::str::contains("not running").not());
}

#[test]
fn wait_resolves_dsn_from_env() {
    fixtures::require_exasol!();

    fixtures::exapump()
        .env("EXAPUMP_DSN", fixtures::DOCKER_DSN)
        .args(["wait", "--timeout-secs", "30"])
        .assert()
        .code(0);
}

#[test]
fn wait_fails_without_connection_info() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("nonexistent_config.toml");

    fixtures::exapump()
        .env("EXAPUMP_CONFIG", config_path.to_str().unwrap())
        .env_remove("EXAPUMP_DSN")
        .arg("wait")
        .assert()
        .code(1)
        .stderr(
            predicate::str::contains("No connection")
                .or(predicate::str::contains("connection info"))
                .or(predicate::str::contains("No profiles"))
                .or(predicate::str::contains("--dsn")),
        );
}

#[test]
fn wait_reports_phase_and_elapsed_progress() {
    fixtures::require_exasol!();

    fixtures::exapump()
        .args(["wait", "--dsn", fixtures::DOCKER_DSN])
        .assert()
        .code(0)
        .stderr(predicate::str::contains("Phase 1"))
        .stderr(predicate::str::contains("Phase 2"));
}

#[test]
#[ignore = "requires container to be stopped mid-poll; run manually against a real Exasol Docker container"]
fn wait_dumps_logs_when_container_crashes() {
    // intentionally empty: covered by manual testing against a live container
}
