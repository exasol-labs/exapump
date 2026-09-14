# Feature: BucketFS Credential Resolution

A BucketFS operation resolves its Basic-auth credential from command-line overrides, the resolved base connection (a profile or the BucketFS defaults, see `bucketfs/profile-integration`), and anonymous access as a last resort.

The read credential resolves in this order: `--bfs-read-password`, then the base connection's read password, then `--bfs-write-password`. A base connection built from a profile takes its read password from `bfs_read_password`, or from `bfs_write_password` when `bfs_read_password` is absent.

## Background

## Scenarios

### Scenario: Read auth prefers read_password then write_password then anonymous

* *GIVEN* a profile with `bfs_read_password = "rp"` and `bfs_write_password = "wp"`
* *WHEN* a BucketFS read operation resolves credentials
* *THEN* it MUST use `r:rp` for authentication

### Scenario: Read auth falls back to write_password

* *GIVEN* a profile with `bfs_write_password = "wp"` and no `bfs_read_password`
* *WHEN* a BucketFS read operation resolves credentials
* *THEN* the resolved read password MUST be `wp`
* *AND* this scenario MUST NOT assert the Basic-auth username sent for the read, tracked against the mismatch between `w:wp` and the `r` username that `BucketFsClient` sends

### Scenario: Read auth falls back to anonymous on public bucket

* *GIVEN* a profile with no `bfs_write_password` and no `bfs_read_password`
* *AND* the user provides no `--bfs-read-password` flag and no `--bfs-write-password` flag
* *AND* the bucket is publicly readable
* *WHEN* the user runs `exapump bucketfs ls`
* *THEN* the request MUST be sent without authentication
* *AND* the operation MUST succeed

### Scenario: Anonymous read fails on non-public bucket

* *GIVEN* a profile with no `bfs_write_password` and no `bfs_read_password`
* *AND* the bucket is not publicly readable
* *WHEN* the user runs `exapump bucketfs ls`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate access was denied
* *AND* stderr MUST suggest adding `bfs_read_password` or `bfs_write_password` to the profile

### Scenario: Write password required for write operations

* *GIVEN* a profile without `bfs_write_password`
* *AND* no `--bfs-write-password` flag is provided
* *WHEN* the user runs a BucketFS write operation (`cp` upload or `rm`)
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate that `bfs_write_password` is required for write operations

### Scenario: BucketFS defaults apply when no profile supplies a base

* *GIVEN* command-line BucketFS overrides supply a host and a BucketFS credential, and no profile supplies a base
* *WHEN* exapump resolves the BucketFS connection
* *THEN* the port MUST be `2581`
* *AND* the bucket MUST be `default`
* *AND* TLS and certificate validation MUST both be enabled

### Scenario: Read credential falls back to the --bfs-write-password flag

* *GIVEN* the base connection supplies no read password
* *AND* the user provides no `--bfs-read-password` flag
* *AND* the user provides `--bfs-write-password "wp"`
* *WHEN* a BucketFS read operation resolves credentials
* *THEN* the resolved read password MUST be `wp`

### Scenario: Configured read password outranks the write-password flag

* *GIVEN* a profile with `bfs_read_password = "rp"`
* *AND* the user provides `--bfs-write-password "wp"`
* *AND* the user provides no `--bfs-read-password` flag
* *WHEN* a BucketFS read operation resolves credentials
* *THEN* the resolved read password MUST be `rp`
