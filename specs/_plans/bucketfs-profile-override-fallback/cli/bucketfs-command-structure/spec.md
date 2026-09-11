# Feature: BucketFS Command Structure

The `bucketfs` subcommand group provides file management operations against Exasol's BucketFS distributed file system. It supports listing, uploading, downloading, and deleting files via BucketFS's HTTP/HTTPS REST API.

## Background

<!-- DELTA:CHANGED -->
BucketFS is Exasol's built-in distributed file system for storing JARs, UDF scripts, and other artifacts. It exposes an HTTP/HTTPS REST API on a configurable port (default: 2581 HTTPS). Authentication uses Basic auth with dedicated read/write passwords. The `bucketfs` subcommand group is available as `exapump bucketfs <subcommand>`.

Connection parameters come from three sources: per-command `--bfs-*` flags, a profile, and the BucketFS defaults. The BucketFS defaults are port `2581`, bucket `default`, TLS on, and certificate validation on. A profile reuses `host` (overridable via `bfs_host`) and inherits `tls`/`validate_certificate` (overridable via `bfs_tls`/`bfs_validate_certificate`).

The config file is a fallback value source, not a precondition. exapump selects one base connection in this order, then applies the `--bfs-*` flags on top of it:

* `--profile <name>` selects that profile. A name that is not in the config is an error.
* Otherwise, `--bfs-host` together with `--bfs-write-password` or `--bfs-read-password` selects the BucketFS defaults. exapump reads no profile in this case.
* Otherwise, the default profile is the base when one resolves, whether or not `--bfs-host` is given.
* Otherwise, `--bfs-host` alone selects the BucketFS defaults when the config holds zero profiles. Any other profile-resolution failure MUST propagate.
* Otherwise, the command fails and names both remedies.
<!-- /DELTA:CHANGED -->

## Scenarios

<!-- DELTA:NEW -->
### Scenario: BucketFS overrides work without a config file

* *GIVEN* no config file exists at the path exapump resolves for the config
* *AND* the user provides `--bfs-host`, `--bfs-port`, and `--bfs-write-password`
* *AND* the user provides no `--profile` flag
* *WHEN* the user runs `exapump bucketfs ls`
* *THEN* the CLI MUST build the connection from the flags and the BucketFS defaults
* *AND* the CLI MUST NOT fail with `No profiles found in config`

### Scenario: Self-sufficient overrides ignore the default profile

* *GIVEN* a config file whose single profile sets `bfs_bucket = "wrongbucket"` and `bfs_port = 9999`
* *AND* the user provides `--bfs-host` and `--bfs-write-password`
* *AND* the user provides no `--profile`, `--bfs-bucket`, or `--bfs-port` flag
* *WHEN* the bucketfs command resolves connection parameters
* *THEN* the bucket MUST be `default`
* *AND* the port MUST be `2581`

### Scenario: Named profile stays the base when overrides are self-sufficient

* *GIVEN* a config file with a profile named `bfs` that sets `bfs_validate_certificate = false`
* *AND* the user provides `--profile bfs`, `--bfs-host`, and `--bfs-write-password`
* *AND* the user provides no `--bfs-validate-certificate` flag
* *WHEN* the bucketfs command resolves connection parameters
* *THEN* the base MUST be the profile named `bfs`
* *AND* certificate validation MUST stay disabled

### Scenario: Host override alone still inherits the default profile

* *GIVEN* a config file with exactly one profile that sets `bfs_write_password`
* *AND* the user provides `--bfs-host` and no BucketFS password flag
* *AND* the user provides no `--profile` flag
* *WHEN* the bucketfs command resolves connection parameters
* *THEN* the write password MUST come from that profile

### Scenario: Host override alone works when no profile resolves

* *GIVEN* no config file exists at the path exapump resolves for the config
* *AND* the user provides `--bfs-host` and no `--profile` flag
* *WHEN* the user runs `exapump bucketfs ls`
* *THEN* the CLI MUST build the connection from the flag and the BucketFS defaults
* *AND* the CLI MUST NOT fail with `No profiles found in config`

### Scenario: Missing host and missing profile name both remedies

* *GIVEN* no config file exists at the path exapump resolves for the config
* *AND* the user provides no `--bfs-host` flag and no `--profile` flag
* *WHEN* the user runs `exapump bucketfs ls`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST name `--bfs-host` as an alternative to a profile
* *AND* stderr MUST name `exapump profile add`

### Scenario: Ambiguous default profile still fails a host-only run

* *GIVEN* a config file holding two profiles that both set `default = true`
* *AND* the user provides `--bfs-host` and no BucketFS credential flag
* *AND* the user provides no `--profile` flag
* *WHEN* the user runs `exapump bucketfs ls`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST list the conflicting profile names

### Scenario: Default profile stays the base when no override is given

* *GIVEN* a config file holding exactly one profile that sets `bfs_write_password`
* *AND* the user provides no `--profile` flag and no `--bfs-*` flag
* *WHEN* the bucketfs command resolves connection parameters
* *THEN* the base MUST be that profile
<!-- /DELTA:NEW -->
