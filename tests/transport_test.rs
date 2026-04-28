mod fixtures;

use predicates::prelude::*;

#[test]
fn sql_select_one_native_transport_against_docker() {
    fixtures::require_exasol!();
    fixtures::exapump()
        .args([
            "sql",
            "SELECT 1",
            "--dsn",
            fixtures::DOCKER_DSN,
            "--transport",
            "native",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("1"));
}

#[test]
fn sql_select_one_websocket_transport_against_docker() {
    fixtures::require_exasol!();
    fixtures::exapump()
        .args([
            "sql",
            "SELECT 1",
            "--dsn",
            fixtures::DOCKER_DSN,
            "--transport",
            "websocket",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("1"));
}
