mod fixtures;

use predicates::prelude::*;
use std::path::PathBuf;

/// Write a TOML config file inside a temp directory and return the path.
fn write_config(dir: &std::path::Path, content: &str) -> PathBuf {
    let config_dir = dir.join(".exapump");
    std::fs::create_dir_all(&config_dir).unwrap();
    let path = config_dir.join("config.toml");
    std::fs::write(&path, content).unwrap();
    path
}

// ──────────────────────────────────────────────
// Profile CRUD tests (CLI integration)
// ──────────────────────────────────────────────

#[test]
fn profile_list_empty() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("nonexistent_config.toml");

    fixtures::exapump()
        .env("EXAPUMP_CONFIG", config_path.to_str().unwrap())
        .args(["profile", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No profiles configured"));
}

#[test]
fn profile_list_with_profiles() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = write_config(
        dir.path(),
        r#"
[default]
host = "localhost"
port = 8563
user = "sys"
password = "exasol"

[production]
host = "prod.example.com"
port = 8563
user = "admin"
password = "secret"
"#,
    );

    fixtures::exapump()
        .env("EXAPUMP_CONFIG", config_path.to_str().unwrap())
        .args(["profile", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("default *"))
        .stdout(predicate::str::contains("production"));
}

#[test]
fn profile_show_displays_details() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = write_config(
        dir.path(),
        r#"
[default]
host = "localhost"
port = 8563
user = "sys"
password = "exasol"
tls = true
validate_certificate = false
"#,
    );

    fixtures::exapump()
        .env("EXAPUMP_CONFIG", config_path.to_str().unwrap())
        .args(["profile", "show", "default"])
        .assert()
        .success()
        .stdout(predicate::str::contains("host: localhost"))
        .stdout(predicate::str::contains("port: 8563"))
        .stdout(predicate::str::contains("user: sys"))
        .stdout(predicate::str::contains("password: ****"))
        .stdout(predicate::str::contains("tls: true"))
        .stdout(predicate::str::contains("validate_certificate: false"));
}

#[test]
fn profile_show_missing() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("nonexistent_config.toml");

    fixtures::exapump()
        .env("EXAPUMP_CONFIG", config_path.to_str().unwrap())
        .args(["profile", "show", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn profile_add_with_flags() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join(".exapump").join("config.toml");
    std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();

    fixtures::exapump()
        .env("EXAPUMP_CONFIG", config_path.to_str().unwrap())
        .args([
            "profile",
            "add",
            "prod",
            "--host",
            "prod.example.com",
            "--user",
            "admin",
            "--password",
            "s3cret",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Profile 'prod' added"));

    // Verify the config file was created and contains the profile
    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("prod"));
    assert!(content.contains("prod.example.com"));
}

#[test]
fn profile_add_default_presets() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join(".exapump").join("config.toml");
    std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();

    fixtures::exapump()
        .env("EXAPUMP_CONFIG", config_path.to_str().unwrap())
        .args(["profile", "add", "default"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Profile 'default' added"))
        .stdout(predicate::str::contains("host=localhost"))
        .stdout(predicate::str::contains("port=8563"))
        .stdout(predicate::str::contains("user=sys"));
}

#[test]
fn profile_add_partial_flags() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join(".exapump").join("config.toml");
    std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();

    fixtures::exapump()
        .env("EXAPUMP_CONFIG", config_path.to_str().unwrap())
        .args(["profile", "add", "default", "--host", "custom-host"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Profile 'default' added"))
        .stdout(predicate::str::contains("host=custom-host"));

    // Verify it still has Docker preset defaults for other fields
    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("custom-host"));
    assert!(content.contains("sys")); // user from Docker preset
}

#[test]
fn profile_add_refuses_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = write_config(
        dir.path(),
        r#"
[default]
host = "localhost"
port = 8563
user = "sys"
password = "exasol"
"#,
    );

    fixtures::exapump()
        .env("EXAPUMP_CONFIG", config_path.to_str().unwrap())
        .args(["profile", "add", "default"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn profile_remove_deletes_profile() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = write_config(
        dir.path(),
        r#"
[default]
host = "localhost"
port = 8563
user = "sys"
password = "exasol"
"#,
    );

    fixtures::exapump()
        .env("EXAPUMP_CONFIG", config_path.to_str().unwrap())
        .args(["profile", "remove", "default"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Profile 'default' removed"));

    // Verify the profile was actually removed from the file
    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(!content.contains("[default]"));
}

#[test]
fn profile_remove_missing() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("nonexistent_config.toml");

    fixtures::exapump()
        .env("EXAPUMP_CONFIG", config_path.to_str().unwrap())
        .args(["profile", "remove", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn profile_add_missing_required_fields() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join(".exapump").join("config.toml");
    std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();

    // Non-default profile without required flags should fail
    fixtures::exapump()
        .env("EXAPUMP_CONFIG", config_path.to_str().unwrap())
        .args(["profile", "add", "prod"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--host").or(predicate::str::contains("required")));
}

#[test]
fn profile_add_rejects_invalid_name() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join(".exapump").join("config.toml");
    std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();

    // Use "_start" which is invalid (starts with underscore) but won't confuse clap
    fixtures::exapump()
        .env("EXAPUMP_CONFIG", config_path.to_str().unwrap())
        .args([
            "profile",
            "add",
            "_start",
            "--host",
            "h",
            "--user",
            "u",
            "--password",
            "p",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid profile name"));
}

#[test]
fn profile_name_required_for_add() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");

    // `profile add` without a name should fail (clap handles this)
    fixtures::exapump()
        .env("EXAPUMP_CONFIG", config_path.to_str().unwrap())
        .args(["profile", "add"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

// ──────────────────────────────────────────────
// Resolution priority tests (CLI integration)
// ──────────────────────────────────────────────

#[test]
fn default_profile_auto_selected() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = write_config(
        dir.path(),
        r#"
[default]
host = "localhost"
port = 8563
user = "sys"
password = "exasol"
tls = true
validate_certificate = false
"#,
    );

    // Should fail at connection level, not at argument parsing
    fixtures::exapump()
        .env("EXAPUMP_CONFIG", config_path.to_str().unwrap())
        .env_remove("EXAPUMP_DSN")
        .args(["sql", "SELECT 1"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("connect")
                .or(predicate::str::contains("error"))
                .or(predicate::str::contains("Error")),
        );
}

#[test]
fn named_profile_via_flag() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = write_config(
        dir.path(),
        r#"
[production]
host = "prod.example.com"
port = 8563
user = "admin"
password = "secret"
tls = true
validate_certificate = true
"#,
    );

    // Should fail at connection level, not at argument parsing
    fixtures::exapump()
        .env("EXAPUMP_CONFIG", config_path.to_str().unwrap())
        .env_remove("EXAPUMP_DSN")
        .args(["sql", "--profile", "production", "SELECT 1"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("connect")
                .or(predicate::str::contains("error"))
                .or(predicate::str::contains("Error")),
        );
}

#[test]
fn dsn_overrides_profile() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = write_config(
        dir.path(),
        r#"
[default]
host = "localhost"
port = 8563
user = "sys"
password = "exasol"
"#,
    );

    // --dsn flag should take priority over default profile
    // Will fail at connection, not arg parsing
    fixtures::exapump()
        .env("EXAPUMP_CONFIG", config_path.to_str().unwrap())
        .env_remove("EXAPUMP_DSN")
        .args([
            "sql",
            "--dsn",
            "exasol://flag:pwd@somehost:8563",
            "SELECT 1",
        ])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("connect")
                .or(predicate::str::contains("error"))
                .or(predicate::str::contains("Error")),
        );
}

#[test]
fn exapump_dsn_overrides_profile() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = write_config(
        dir.path(),
        r#"
[default]
host = "localhost"
port = 8563
user = "sys"
password = "exasol"
"#,
    );

    // EXAPUMP_DSN env var should take priority over default profile
    fixtures::exapump()
        .env("EXAPUMP_CONFIG", config_path.to_str().unwrap())
        .env("EXAPUMP_DSN", "exasol://env:pwd@envhost:8563")
        .args(["sql", "SELECT 1"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("connect")
                .or(predicate::str::contains("error"))
                .or(predicate::str::contains("Error")),
        );
}

#[test]
fn missing_profile_error() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = write_config(
        dir.path(),
        r#"
[default]
host = "localhost"
port = 8563
user = "sys"
password = "exasol"
"#,
    );

    fixtures::exapump()
        .env("EXAPUMP_CONFIG", config_path.to_str().unwrap())
        .env_remove("EXAPUMP_DSN")
        .args(["sql", "--profile", "nonexistent", "SELECT 1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn no_config_no_dsn_error() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("nonexistent_config.toml");

    fixtures::exapump()
        .env("EXAPUMP_CONFIG", config_path.to_str().unwrap())
        .env_remove("EXAPUMP_DSN")
        .args(["sql", "SELECT 1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("profile add default"));
}
