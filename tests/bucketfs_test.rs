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
