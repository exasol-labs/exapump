# Feature: Config Profiles

Connection profiles allow users to store named sets of connection parameters in a config file at `~/.exapump/config.toml`. This eliminates the need to pass `--dsn` or set environment variables for repeated use.

## Background

A config file holds zero or more named profiles. exapump resolves one profile as the base for a command from, in order: an explicit `--profile` flag, a single `default = true` profile, or the sole profile when only one exists. `--dsn` and `EXAPUMP_DSN` override profile resolution entirely. See `config/dsn-generation` for how a resolved profile becomes a DSN, and `config/profile-validation` for name rules and file permissions.

## Scenarios

### Scenario: Config file format

* *GIVEN* a config file at `~/.exapump/config.toml`
* *AND* the file contains:
```toml
[local]
host = "localhost"
port = 8563
user = "sys"
password = "exasol"
tls = true
validate_certificate = false
bfs_write_password = "bucketfs_write_pw"

[production]
default = true
host = "exasol-prod.example.com"
port = 8563
user = "admin"
password = "s3cret"
schema = "my_schema"
tls = true
validate_certificate = true
certificate_fingerprint = "1a2b3c4d5e6f7890abcdef1234567890abcdef1234567890abcdef1234567890"
bfs_host = "bfs-node.example.com"
bfs_port = 6583
bfs_bucket = "data"
bfs_write_password = "w_secret"
bfs_read_password = "r_secret"
bfs_tls = false
bfs_validate_certificate = false
```
* *WHEN* exapump parses the config
* *THEN* each profile MUST support the optional fields: `certificate_fingerprint`, `bfs_host`, `bfs_port`, `bfs_bucket`, `bfs_write_password`, `bfs_read_password`, `bfs_tls`, `bfs_validate_certificate`
* *AND* profiles without any of these optional fields MUST still be valid
* *AND* BucketFS fields MUST NOT affect DSN generation for database connections
* *AND* `certificate_fingerprint` MUST NOT affect BucketFS connection construction

### Scenario: Default profile auto-selected

* *GIVEN* a config file exists with multiple profiles
* *AND* exactly one profile has `default = true`
* *AND* no `--dsn` flag, no `EXAPUMP_DSN` env var, and no `--profile` flag is provided
* *WHEN* exapump resolves connection parameters
* *THEN* the profile with `default = true` MUST be used automatically

### Scenario: Single profile is auto-default

* *GIVEN* a config file exists with exactly one profile (any name)
* *AND* the profile does not have `default = true`
* *AND* no `--dsn` flag, no `EXAPUMP_DSN` env var, and no `--profile` flag is provided
* *WHEN* exapump resolves connection parameters
* *THEN* that profile MUST be used automatically

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

### Scenario: Named profile via --profile flag

* *GIVEN* a config file exists with multiple profiles
* *AND* the user provides `--profile production` (or `-p production`)
* *WHEN* exapump resolves connection parameters
* *THEN* the `production` profile MUST be used

### Scenario: DSN overrides profile

* *GIVEN* a config file exists with a default profile
* *AND* the user provides `--dsn exasol://flag:pwd@host:8563`
* *WHEN* exapump resolves connection parameters
* *THEN* the `--dsn` flag MUST take precedence over the config file profile

### Scenario: EXAPUMP_DSN overrides profile

* *GIVEN* a config file exists with a default profile
* *AND* `EXAPUMP_DSN` is set (via shell environment or `.env` file)
* *WHEN* exapump resolves connection parameters
* *THEN* the `EXAPUMP_DSN` value MUST take precedence over the config file profile

### Scenario: Missing profile error

* *GIVEN* a config file exists but does not contain a profile named `staging`
* *AND* the user provides `--profile staging`
* *WHEN* exapump resolves connection parameters
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate the profile `staging` was not found
* *AND* stderr SHOULD list the available profiles

### Scenario: No config file and no DSN

* *GIVEN* no config file exists at `~/.exapump/config.toml`
* *AND* no `--dsn` flag, no `EXAPUMP_DSN` env var is provided
* *AND* no `--profile` flag is provided
* *WHEN* exapump resolves connection parameters
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST suggest running `exapump profile add default` to get started
