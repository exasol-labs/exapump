mod fixtures;

use predicates::prelude::*;
use std::path::PathBuf;

fn unique_prefix() -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    format!(
        "test_{}_{}_{}/",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
        std::process::id(),
        seq,
    )
}

fn write_bfs_config(dir: &std::path::Path, write_password: &str) -> PathBuf {
    let config_dir = dir.join(".exapump");
    std::fs::create_dir_all(&config_dir).unwrap();
    let path = config_dir.join("config.toml");
    std::fs::write(
        &path,
        format!(
            r#"
[bfs]
host = "localhost"
port = 8563
user = "sys"
password = "exasol"
tls = true
validate_certificate = false
bfs_write_password = "{write_password}"
bfs_tls = true
bfs_validate_certificate = false
"#
        ),
    )
    .unwrap();
    path
}

fn bfs_cmd(config_path: &std::path::Path) -> assert_cmd::Command {
    let mut cmd = fixtures::exapump();
    cmd.env("EXAPUMP_CONFIG", config_path.to_str().unwrap());
    cmd
}

fn cleanup_path(config_path: &std::path::Path, path: &str) {
    let _ = bfs_cmd(config_path)
        .args(["bucketfs", "rm", path, "--profile", "bfs"])
        .ok();
}

#[test]
fn overrides_only_lists_bucket_without_config() {
    fixtures::require_bucketfs!();
    let write_pw = fixtures::bfs_write_password();
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("no-such-config.toml");

    bfs_cmd(&config_path)
        .args([
            "bucketfs",
            "ls",
            "--bfs-host",
            "localhost",
            "--bfs-port",
            "2581",
            "--bfs-write-password",
            &write_pw,
            "--bfs-validate-certificate",
            "false",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("No profiles found in config").not());
}

#[test]
fn overrides_only_ignores_default_profile_bucket() {
    fixtures::require_bucketfs!();
    let write_pw = fixtures::bfs_write_password();
    let dir = tempfile::tempdir().unwrap();
    let config_dir = dir.path().join(".exapump");
    std::fs::create_dir_all(&config_dir).unwrap();
    let config_path = config_dir.join("config.toml");
    std::fs::write(
        &config_path,
        r#"
[bfs]
host = "localhost"
user = "sys"
password = "exasol"
bfs_bucket = "wrongbucket"
bfs_port = 9999
bfs_validate_certificate = false
"#,
    )
    .unwrap();

    // Neither --bfs-bucket nor --bfs-port is given, so the profile's
    // `wrongbucket` and port 9999 would break the run if it were the base.
    bfs_cmd(&config_path)
        .args([
            "bucketfs",
            "ls",
            "--bfs-host",
            "localhost",
            "--bfs-write-password",
            &write_pw,
            "--bfs-validate-certificate",
            "false",
        ])
        .assert()
        .success();
}

#[test]
fn host_override_inherits_default_profile_password() {
    fixtures::require_bucketfs!();
    let write_pw = fixtures::bfs_write_password();
    let dir = tempfile::tempdir().unwrap();
    let config_path = write_bfs_config(dir.path(), &write_pw);
    let prefix = unique_prefix();

    let src_file = dir.path().join("host_inherit.txt");
    std::fs::write(&src_file, "host override inheritance test\n").unwrap();
    let remote_path = format!("{prefix}host_inherit.txt");

    // No password flag and no --profile: the upload can only authenticate if
    // the default profile still supplies `bfs_write_password`.
    bfs_cmd(&config_path)
        .args([
            "bucketfs",
            "cp",
            src_file.to_str().unwrap(),
            &remote_path,
            "--bfs-host",
            "localhost",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("Uploaded"));

    cleanup_path(&config_path, &remote_path);
}

#[test]
fn named_profile_stays_base_with_overrides() {
    fixtures::require_bucketfs!();
    let write_pw = fixtures::bfs_write_password();
    let dir = tempfile::tempdir().unwrap();
    let config_path = write_bfs_config(dir.path(), &write_pw);

    // No --bfs-validate-certificate flag: the run reaches the self-signed
    // container only if the named profile stays the base and keeps
    // `bfs_validate_certificate = false`.
    bfs_cmd(&config_path)
        .args([
            "bucketfs",
            "ls",
            "--profile",
            "bfs",
            "--bfs-host",
            "localhost",
            "--bfs-write-password",
            &write_pw,
        ])
        .assert()
        .success();
}

#[test]
fn default_profile_lists_bucket_without_profile_flag() {
    fixtures::require_bucketfs!();
    let write_pw = fixtures::bfs_write_password();
    let dir = tempfile::tempdir().unwrap();
    let config_path = write_bfs_config(dir.path(), &write_pw);

    bfs_cmd(&config_path)
        .args(["bucketfs", "ls"])
        .assert()
        .success();
}

#[test]
fn list_bucket_root() {
    fixtures::require_bucketfs!();
    let write_pw = fixtures::bfs_write_password();
    let dir = tempfile::tempdir().unwrap();
    let config_path = write_bfs_config(dir.path(), &write_pw);
    let prefix = unique_prefix();

    // Upload a file so the bucket is guaranteed non-empty
    let src_file = dir.path().join("list_test.txt");
    std::fs::write(&src_file, "list test").unwrap();
    let remote_path = format!("{prefix}list_test.txt");

    bfs_cmd(&config_path)
        .args([
            "bucketfs",
            "cp",
            src_file.to_str().unwrap(),
            &remote_path,
            "--profile",
            "bfs",
        ])
        .assert()
        .success();

    bfs_cmd(&config_path)
        .args(["bucketfs", "ls", "--profile", "bfs"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());

    cleanup_path(&config_path, &remote_path);
}

#[test]
fn upload_and_download_roundtrip() {
    fixtures::require_bucketfs!();
    let write_pw = fixtures::bfs_write_password();
    let dir = tempfile::tempdir().unwrap();
    let config_path = write_bfs_config(dir.path(), &write_pw);
    let prefix = unique_prefix();

    let upload_content = "roundtrip test content\n";
    let src_file = dir.path().join("upload.txt");
    std::fs::write(&src_file, upload_content).unwrap();

    let remote_path = format!("{prefix}upload.txt");

    bfs_cmd(&config_path)
        .args([
            "bucketfs",
            "cp",
            src_file.to_str().unwrap(),
            &remote_path,
            "--profile",
            "bfs",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("Uploaded"));

    let dst_file = dir.path().join("download.txt");
    bfs_cmd(&config_path)
        .args([
            "bucketfs",
            "cp",
            &remote_path,
            dst_file.to_str().unwrap(),
            "--profile",
            "bfs",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("Downloaded"));

    let downloaded = std::fs::read_to_string(&dst_file).unwrap();
    assert_eq!(downloaded, upload_content);

    cleanup_path(&config_path, &remote_path);
}

#[test]
fn upload_preserves_filename() {
    fixtures::require_bucketfs!();
    let write_pw = fixtures::bfs_write_password();
    let dir = tempfile::tempdir().unwrap();
    let config_path = write_bfs_config(dir.path(), &write_pw);
    let prefix = unique_prefix();

    let src_file = dir.path().join("test.txt");
    std::fs::write(&src_file, "filename test").unwrap();

    bfs_cmd(&config_path)
        .args([
            "bucketfs",
            "cp",
            src_file.to_str().unwrap(),
            &prefix,
            "--profile",
            "bfs",
        ])
        .assert()
        .success();

    // BucketFS has eventual consistency — wait for the file to become visible
    std::thread::sleep(std::time::Duration::from_secs(3));

    bfs_cmd(&config_path)
        .args(["bucketfs", "ls", &prefix, "--profile", "bfs"])
        .assert()
        .success()
        .stdout(predicate::str::contains("test.txt"));

    let remote_path = format!("{prefix}test.txt");
    cleanup_path(&config_path, &remote_path);
}

#[test]
fn delete_file() {
    fixtures::require_bucketfs!();
    let write_pw = fixtures::bfs_write_password();
    let dir = tempfile::tempdir().unwrap();
    let config_path = write_bfs_config(dir.path(), &write_pw);
    let prefix = unique_prefix();

    let src_file = dir.path().join("to_delete.txt");
    std::fs::write(&src_file, "delete me").unwrap();

    let remote_path = format!("{prefix}to_delete.txt");
    bfs_cmd(&config_path)
        .args([
            "bucketfs",
            "cp",
            src_file.to_str().unwrap(),
            &remote_path,
            "--profile",
            "bfs",
        ])
        .assert()
        .success();

    bfs_cmd(&config_path)
        .args(["bucketfs", "rm", &remote_path, "--profile", "bfs"])
        .assert()
        .success()
        .stderr(predicate::str::contains("Deleted"));

    // BucketFS has eventual consistency — wait for delete to propagate
    std::thread::sleep(std::time::Duration::from_secs(3));

    bfs_cmd(&config_path)
        .args(["bucketfs", "ls", &prefix, "--profile", "bfs"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("Not Found")));
}

#[test]
fn upload_source_not_found() {
    fixtures::require_bucketfs!();
    let write_pw = fixtures::bfs_write_password();
    let dir = tempfile::tempdir().unwrap();
    let config_path = write_bfs_config(dir.path(), &write_pw);

    bfs_cmd(&config_path)
        .args([
            "bucketfs",
            "cp",
            "/nonexistent/file.txt",
            "some/dest.txt",
            "--profile",
            "bfs",
        ])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("not found")
                .or(predicate::str::contains("File not found"))
                .or(predicate::str::contains("not reachable")),
        );
}

#[test]
fn download_file_not_found() {
    fixtures::require_bucketfs!();
    let write_pw = fixtures::bfs_write_password();
    let dir = tempfile::tempdir().unwrap();
    let config_path = write_bfs_config(dir.path(), &write_pw);

    let dst_file = dir.path().join("should_not_exist.txt");
    bfs_cmd(&config_path)
        .args([
            "bucketfs",
            "cp",
            "nonexistent_path_12345/no_file.txt",
            dst_file.to_str().unwrap(),
            "--profile",
            "bfs",
        ])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("not found").or(predicate::str::contains("File not found")),
        );
}

#[test]
fn upload_accepts_bfs_uri_destination() {
    fixtures::require_bucketfs!();
    let write_pw = fixtures::bfs_write_password();
    let dir = tempfile::tempdir().unwrap();
    let config_path = write_bfs_config(dir.path(), &write_pw);
    let prefix = unique_prefix();

    let upload_content = "bfs_uri upload test content\n";
    let src_file = dir.path().join("bfs_uri_upload.txt");
    std::fs::write(&src_file, upload_content).unwrap();

    let plain_path = format!("{prefix}bfs_uri_upload.txt");
    let bfs_uri_dest = format!("bfs://default/{plain_path}");

    // Upload using a bfs:// URI as destination
    bfs_cmd(&config_path)
        .args([
            "bucketfs",
            "cp",
            src_file.to_str().unwrap(),
            &bfs_uri_dest,
            "--profile",
            "bfs",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("Uploaded"));

    // Verify file is downloadable by its plain path
    let dst_file = dir.path().join("bfs_uri_upload_dl.txt");
    bfs_cmd(&config_path)
        .args([
            "bucketfs",
            "cp",
            &plain_path,
            dst_file.to_str().unwrap(),
            "--profile",
            "bfs",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("Downloaded"));

    let downloaded = std::fs::read_to_string(&dst_file).unwrap();
    assert_eq!(downloaded, upload_content);

    cleanup_path(&config_path, &plain_path);
}

#[test]
fn download_accepts_bfs_uri_source() {
    fixtures::require_bucketfs!();
    let write_pw = fixtures::bfs_write_password();
    let dir = tempfile::tempdir().unwrap();
    let config_path = write_bfs_config(dir.path(), &write_pw);
    let prefix = unique_prefix();

    let upload_content = "bfs_uri download test content\n";
    let src_file = dir.path().join("bfs_uri_download.txt");
    std::fs::write(&src_file, upload_content).unwrap();

    let plain_path = format!("{prefix}bfs_uri_download.txt");

    // Upload using plain path
    bfs_cmd(&config_path)
        .args([
            "bucketfs",
            "cp",
            src_file.to_str().unwrap(),
            &plain_path,
            "--profile",
            "bfs",
        ])
        .assert()
        .success();

    // Download using a bfs:// URI as source
    let bfs_uri_src = format!("bfs://default/{plain_path}");
    let dst_file = dir.path().join("bfs_uri_download_result.txt");
    bfs_cmd(&config_path)
        .args([
            "bucketfs",
            "cp",
            &bfs_uri_src,
            dst_file.to_str().unwrap(),
            "--profile",
            "bfs",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("Downloaded"));

    let downloaded = std::fs::read_to_string(&dst_file).unwrap();
    assert_eq!(downloaded, upload_content);

    cleanup_path(&config_path, &plain_path);
}

#[test]
fn upload_accepts_bfss_uri_destination() {
    fixtures::require_bucketfs!();
    let write_pw = fixtures::bfs_write_password();
    let dir = tempfile::tempdir().unwrap();
    let config_path = write_bfs_config(dir.path(), &write_pw);
    let prefix = unique_prefix();

    let upload_content = "bfss_uri upload test content\n";
    let src_file = dir.path().join("bfss_uri_upload.txt");
    std::fs::write(&src_file, upload_content).unwrap();

    let plain_path = format!("{prefix}bfss_uri_upload.txt");
    let bfss_uri_dest = format!("bfss://default/{plain_path}");

    // Upload using a bfss:// URI as destination
    bfs_cmd(&config_path)
        .args([
            "bucketfs",
            "cp",
            src_file.to_str().unwrap(),
            &bfss_uri_dest,
            "--profile",
            "bfs",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("Uploaded"));

    // Verify file is downloadable by its plain path
    let dst_file = dir.path().join("bfss_uri_upload_dl.txt");
    bfs_cmd(&config_path)
        .args([
            "bucketfs",
            "cp",
            &plain_path,
            dst_file.to_str().unwrap(),
            "--profile",
            "bfs",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("Downloaded"));

    let downloaded = std::fs::read_to_string(&dst_file).unwrap();
    assert_eq!(downloaded, upload_content);

    cleanup_path(&config_path, &plain_path);
}

#[test]
fn download_accepts_bfss_uri_source() {
    fixtures::require_bucketfs!();
    let write_pw = fixtures::bfs_write_password();
    let dir = tempfile::tempdir().unwrap();
    let config_path = write_bfs_config(dir.path(), &write_pw);
    let prefix = unique_prefix();

    let upload_content = "bfss_uri download test content\n";
    let src_file = dir.path().join("bfss_uri_download.txt");
    std::fs::write(&src_file, upload_content).unwrap();

    let plain_path = format!("{prefix}bfss_uri_download.txt");

    // Upload using plain path
    bfs_cmd(&config_path)
        .args([
            "bucketfs",
            "cp",
            src_file.to_str().unwrap(),
            &plain_path,
            "--profile",
            "bfs",
        ])
        .assert()
        .success();

    // Download using a bfss:// URI as source
    let bfss_uri_src = format!("bfss://default/{plain_path}");
    let dst_file = dir.path().join("bfss_uri_download_result.txt");
    bfs_cmd(&config_path)
        .args([
            "bucketfs",
            "cp",
            &bfss_uri_src,
            dst_file.to_str().unwrap(),
            "--profile",
            "bfs",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("Downloaded"));

    let downloaded = std::fs::read_to_string(&dst_file).unwrap();
    assert_eq!(downloaded, upload_content);

    cleanup_path(&config_path, &plain_path);
}

#[test]
fn bfss_uri_infers_tls_overrides_profile_false() {
    fixtures::require_bucketfs!();
    let write_pw = fixtures::bfs_write_password();
    let dir = tempfile::tempdir().unwrap();

    // Write a config with bfs_tls = false to verify bfss:// URI inference overrides it
    let config_dir = dir.path().join(".exapump");
    std::fs::create_dir_all(&config_dir).unwrap();
    let config_path = config_dir.join("config.toml");
    std::fs::write(
        &config_path,
        format!(
            r#"
[bfs]
host = "localhost"
port = 8563
user = "sys"
password = "exasol"
tls = true
validate_certificate = false
bfs_write_password = "{write_pw}"
bfs_tls = false
bfs_validate_certificate = false
"#
        ),
    )
    .unwrap();

    let prefix = unique_prefix();
    let src_file = dir.path().join("bfss_tls_infer.txt");
    std::fs::write(&src_file, "bfss tls inference test\n").unwrap();

    let plain_path = format!("{prefix}bfss_tls_infer.txt");
    let bfss_uri_dest = format!("bfss://default/{plain_path}");

    // bfss:// URI must infer TLS and succeed despite profile having bfs_tls = false
    bfs_cmd(&config_path)
        .args([
            "bucketfs",
            "cp",
            src_file.to_str().unwrap(),
            &bfss_uri_dest,
            "--profile",
            "bfs",
        ])
        .assert()
        .success();

    cleanup_path(&config_path, &plain_path);
}

#[test]
fn bfss_uri_tls_overridden_by_explicit_flag() {
    fixtures::require_bucketfs!();
    let write_pw = fixtures::bfs_write_password();
    let dir = tempfile::tempdir().unwrap();
    let config_path = write_bfs_config(dir.path(), &write_pw);
    let prefix = unique_prefix();

    let src_file = dir.path().join("bfss_flag_override.txt");
    std::fs::write(&src_file, "bfss explicit flag test\n").unwrap();

    let plain_path = format!("{prefix}bfss_flag_override.txt");
    let bfss_uri_dest = format!("bfss://default/{plain_path}");

    // Explicit --bfs-tls false disables TLS even for a bfss:// URI, so port 2581 (TLS-only) must reject the connection
    bfs_cmd(&config_path)
        .args([
            "bucketfs",
            "cp",
            src_file.to_str().unwrap(),
            &bfss_uri_dest,
            "--profile",
            "bfs",
            "--bfs-tls",
            "false",
        ])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("not reachable")
                .or(predicate::str::contains("error sending request")),
        );
}
