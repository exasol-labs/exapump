# Feature: BucketFS Connection Resolution

`exapump bucketfs <ls|cp|rm>` selects one base connection, then applies the `--bfs-*` flags on top of it. The config file is a fallback value source, not a precondition.

## Background

exapump selects the base connection in this order:

* `--profile <name>` selects that profile. A name that is not in the config is an error.
* Otherwise, `--bfs-host` together with `--bfs-write-password` or `--bfs-read-password` selects the BucketFS defaults (port `2581`, bucket `default`, TLS on, certificate validation on). exapump reads no profile in this case.
* Otherwise, the default profile is the base when one resolves, whether or not `--bfs-host` is given.
* Otherwise, `--bfs-host` alone selects the BucketFS defaults when the config holds zero profiles.

See `bucketfs/connection-resolution-errors` for what happens when none of these branches apply.

## Scenarios

### Scenario: BucketFS works with minimal profile

* *GIVEN* a profile exists with only database fields (`host`, `user`, `password`) and `bfs_write_password`
* *AND* no other BucketFS fields or flags are provided
* *WHEN* the user runs `exapump bucketfs ls --profile <name>`
* *THEN* the CLI MUST connect using the profile's `host`, port `2581`, bucket `default`, and the profile's `tls`/`validate_certificate` settings

### Scenario: BucketFS connection from profile

* *GIVEN* a profile exists with `bfs_write_password` field
* *WHEN* the user runs `exapump bucketfs ls --profile <name>`
* *THEN* the CLI MUST use BucketFS connection parameters from the profile
* *AND* the host MUST fall back to the profile's `host` field

### Scenario: BucketFS flags override profile

* *GIVEN* a profile exists with BucketFS fields
* *AND* the user provides `--bfs-host` on the command line
* *WHEN* the bucketfs command resolves connection parameters
* *THEN* the `--bfs-host` flag value MUST take precedence over the profile

### Scenario: BucketFS port defaults to 2581

* *GIVEN* BucketFS connection parameters are provided
* *AND* no `--bfs-port` flag and no `bfs_port` profile field is set
* *WHEN* the bucketfs command resolves connection parameters
* *THEN* the port MUST default to `2581` (HTTPS)

### Scenario: BucketFS overrides work without a config file

* *GIVEN* no config file exists at the path exapump resolves for the config
* *AND* the user provides `--bfs-host`, `--bfs-port`, and `--bfs-write-password`
* *AND* the user provides no `--profile` flag
* *WHEN* the user runs `exapump bucketfs ls`
* *THEN* the CLI MUST build the connection from the flags and the BucketFS defaults
* *AND* the CLI MUST NOT fail with `No profiles found in config`

### Scenario: Self-sufficient overrides ignore the default profile

* *GIVEN* a config file whose single profile sets `bfs_bucket = "wrongbucket"`, `bfs_port = 9999`, and `bfs_validate_certificate = false`
* *AND* the user provides `--bfs-host` and `--bfs-write-password`
* *AND* the user provides no `--profile`, `--bfs-bucket`, or `--bfs-port` flag
* *WHEN* the bucketfs command resolves connection parameters
* *THEN* the bucket MUST be `default`
* *AND* the port MUST be `2581`
* *AND* certificate validation MUST be enabled

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

### Scenario: Default profile stays the base when no override is given

* *GIVEN* a config file holding exactly one profile that sets `bfs_write_password`
* *AND* the user provides no `--profile` flag and no `--bfs-*` flag
* *WHEN* the bucketfs command resolves connection parameters
* *THEN* the base MUST be that profile
