# Feature: Config Profiles

Connection profiles allow users to store named sets of connection parameters in a config file at `~/.exapump/config.toml`. This eliminates the need to pass `--dsn` or set environment variables for repeated use.

## Background

<!-- DELTA:CHANGED -->
Connection profiles optionally include BucketFS connection parameters alongside the Exasol database fields. BucketFS reuses the profile's `host`, `tls`, and `validate_certificate` fields by default. Only BucketFS-specific overrides and credentials need to be added explicitly.

A BucketFS connection does not require a profile. When command-line overrides supply a host and a BucketFS credential, exapump builds the connection from those overrides and the BucketFS defaults. The BucketFS defaults are port `2581`, bucket `default`, TLS on, and certificate validation on. `default` is Exasol's standard bucket name.

The read credential resolves in this order: `--bfs-read-password`, then the base connection's read password, then `--bfs-write-password`. A base connection built from a profile takes its read password from `bfs_read_password`, or from `bfs_write_password` when `bfs_read_password` is absent.
<!-- /DELTA:CHANGED -->

## Scenarios

<!-- DELTA:NEW -->
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
<!-- /DELTA:NEW -->

<!-- DELTA:CHANGED -->
### Scenario: Multiple default profiles is an error

* *GIVEN* a config file exists with multiple profiles
* *AND* more than one profile has `default = true`
* *WHEN* exapump resolves connection parameters from the config
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate that multiple default profiles were found
* *AND* stderr MUST list the conflicting profile names

### Scenario: Multiple profiles without default is an error

* *GIVEN* a config file exists with two or more profiles
* *AND* no profile has `default = true`
* *AND* no `--dsn` flag, no `EXAPUMP_DSN` env var, and no `--profile` flag is provided
* *WHEN* exapump resolves connection parameters from the config
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate that no default profile is set
* *AND* stderr MUST suggest adding `default = true` to one profile
<!-- /DELTA:CHANGED -->

<!-- DELTA:CHANGED -->
### Scenario: Read auth falls back to write_password

* *GIVEN* a profile with `bfs_write_password = "wp"` and no `bfs_read_password`
* *WHEN* a BucketFS read operation resolves credentials
* *THEN* the resolved read password MUST be `wp`
* *AND* the Basic-auth username sent for the read is out of scope for this plan, tracked against the mismatch between `w:wp` and the `r` username that `BucketFsClient` sends
<!-- /DELTA:CHANGED -->

<!-- DELTA:CHANGED -->
### Scenario: Read auth falls back to anonymous on public bucket

* *GIVEN* a profile with no `bfs_write_password` and no `bfs_read_password`
* *AND* the user provides no `--bfs-read-password` flag and no `--bfs-write-password` flag
* *AND* the bucket is publicly readable
* *WHEN* the user runs `exapump bucketfs ls`
* *THEN* the request MUST be sent without authentication
* *AND* the operation MUST succeed
<!-- /DELTA:CHANGED -->
