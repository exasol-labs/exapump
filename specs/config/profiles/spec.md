# Feature: Config Profiles

Connection profiles allow users to store named sets of connection parameters in a config file at `~/.exapump/config.toml`. This eliminates the need to pass `--dsn` or set environment variables for repeated use.

## Background

Connection profiles now optionally include BucketFS connection parameters alongside the existing Exasol database fields. BucketFS reuses the profile's `host`, `tls`, and `validate_certificate` fields by default. The bucket defaults to `"default"` (Exasol's standard bucket name). Only BucketFS-specific overrides and credentials need to be added explicitly.

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
* *WHEN* exapump resolves connection parameters
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate that multiple default profiles were found
* *AND* stderr MUST list the conflicting profile names

### Scenario: Multiple profiles without default is an error

* *GIVEN* a config file exists with two or more profiles
* *AND* no profile has `default = true`
* *AND* no `--dsn` flag, no `EXAPUMP_DSN` env var, and no `--profile` flag is provided
* *WHEN* exapump resolves connection parameters
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

### Scenario: Port defaults to 8563

* *GIVEN* a profile that omits the `port` field
* *WHEN* the profile is resolved
* *THEN* the port MUST default to `8563`

### Scenario: TLS defaults to true

* *GIVEN* a profile that omits the `tls` field
* *WHEN* the profile is resolved
* *THEN* TLS MUST default to `true`

### Scenario: Validate certificate defaults to true

* *GIVEN* a profile that omits the `validate_certificate` field
* *WHEN* the profile is resolved
* *THEN* certificate validation MUST default to `true`

### Scenario: Profile builds a DSN

* *GIVEN* a profile with `host = "localhost"`, `port = 8563`, `user = "sys"`, `password = "exasol"`, `tls = true`, `validate_certificate = false`
* *WHEN* the profile is resolved for connection
* *THEN* it MUST produce a DSN string in the format `exasol://sys:exasol@localhost:8563?tls=true&validateservercertificate=0`

### Scenario: Profile with schema

* *GIVEN* a profile with `schema = "my_schema"` in addition to required fields
* *WHEN* the profile is resolved
* *THEN* the DSN MUST include the schema as `exasol://user:pwd@host:port/my_schema?tls=true&validateservercertificate=1`

### Scenario: Profile DSN maps all parameters

* *GIVEN* a profile with all fields set
* *WHEN* the profile is resolved to a DSN
* *THEN* `host` MUST map to the DSN host component
* *AND* `port` MUST map to the DSN port component
* *AND* `user` MUST map to the DSN username component
* *AND* `password` MUST map to the DSN password component
* *AND* `schema` MUST map to the DSN path component (e.g., `/my_schema`)
* *AND* `tls` MUST map to the `tls` query parameter
* *AND* `validate_certificate` MUST map to the `validateservercertificate` query parameter (`1` for true, `0` for false)

### Scenario: First profile auto-defaults

* *GIVEN* no config file exists (or config file has zero profiles)
* *WHEN* the user runs `exapump profile add <name>` without `--default`
* *THEN* the profile MUST be created with `default = true`
* *AND* stdout MUST include "(set as default)" to indicate auto-defaulting

### Scenario: Profile name validation

* *GIVEN* a profile name is provided (via `--profile` flag or `exapump profile add <name>`)
* *WHEN* the name does not match the pattern `[a-zA-Z0-9][a-zA-Z0-9_-]*`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate the profile name is invalid
* *AND* stderr MUST describe the allowed format (alphanumeric, underscore, hyphen; starts with alphanumeric)

### Scenario: Valid profile names accepted

* *GIVEN* profile names such as `default`, `my-docker`, `prod_eu`, `DB1`
* *WHEN* used as profile names
* *THEN* all MUST be accepted as valid

### Scenario: Invalid profile names rejected

* *GIVEN* profile names such as `-leading-dash`, `_leading-underscore`, `has spaces`, `special!char`, or an empty string
* *WHEN* used as profile names
* *THEN* all MUST be rejected with a validation error

### Scenario: BucketFS host falls back to profile host

* *GIVEN* a profile with `host = "myhost"` and no `bfs_host` field
* *WHEN* a BucketFS command resolves connection parameters from the profile
* *THEN* the BucketFS host MUST be `myhost`

### Scenario: BucketFS host override

* *GIVEN* a profile with `host = "dbhost"` and `bfs_host = "bfshost"`
* *WHEN* a BucketFS command resolves connection parameters from the profile
* *THEN* the BucketFS host MUST be `bfshost`

### Scenario: BucketFS bucket defaults to default

* *GIVEN* a profile with no `bfs_bucket` field
* *AND* no `--bfs-bucket` flag is provided
* *WHEN* a BucketFS command resolves connection parameters
* *THEN* the bucket MUST default to `default`

### Scenario: BucketFS bucket override

* *GIVEN* a profile with `bfs_bucket = "custom"`
* *WHEN* a BucketFS command resolves connection parameters from the profile
* *THEN* the bucket MUST be `custom`

### Scenario: BucketFS TLS falls back to profile TLS

* *GIVEN* a profile with `tls = true` and no `bfs_tls` field
* *WHEN* a BucketFS command resolves connection parameters from the profile
* *THEN* the BucketFS connection MUST use TLS

### Scenario: BucketFS TLS override

* *GIVEN* a profile with `tls = true` and `bfs_tls = false`
* *WHEN* a BucketFS command resolves connection parameters from the profile
* *THEN* the BucketFS connection MUST NOT use TLS

### Scenario: BucketFS validate_certificate falls back to profile

* *GIVEN* a profile with `validate_certificate = false` and no `bfs_validate_certificate` field
* *WHEN* a BucketFS command resolves connection parameters from the profile
* *THEN* TLS certificate validation for BucketFS MUST be skipped

### Scenario: BucketFS validate_certificate override

* *GIVEN* a profile with `validate_certificate = false` and `bfs_validate_certificate = true`
* *WHEN* a BucketFS command resolves connection parameters from the profile
* *THEN* TLS certificate validation for BucketFS MUST be enabled

### Scenario: Profile with BucketFS builds connection URL

* *GIVEN* a profile with `host = "myhost"`, `tls = true`, `bfs_write_password = "write"`
* *AND* no `bfs_host`, `bfs_port`, or `bfs_bucket` overrides
* *WHEN* a BucketFS command resolves connection parameters from the profile
* *THEN* it MUST connect to `https://myhost:2581/default/`

### Scenario: BucketFS port defaults in profile

* *GIVEN* a profile with `host` set but no `bfs_port`
* *WHEN* the BucketFS connection is resolved
* *THEN* the port MUST default to `2581`

### Scenario: Profile add includes BucketFS fields

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump profile add myprofile --host h --user u --password p --bfs-write-password w`
* *THEN* the profile MUST be saved with both database and BucketFS fields

### Scenario: Profile show displays BucketFS fields

* *GIVEN* a profile exists with BucketFS fields
* *WHEN* the user runs `exapump profile show <name>`
* *THEN* the output MUST include the BucketFS fields that are set
* *AND* the `bfs_write_password` and `bfs_read_password` MUST be masked (shown as `***`)

### Scenario: Docker preset excludes BucketFS

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump profile add docker`
* *THEN* the docker preset MUST NOT include BucketFS fields
* *AND* the profile MUST still be valid for database operations

### Scenario: Read auth prefers read_password then write_password then anonymous

* *GIVEN* a profile with `bfs_read_password = "rp"` and `bfs_write_password = "wp"`
* *WHEN* a BucketFS read operation resolves credentials
* *THEN* it MUST use `r:rp` for authentication

### Scenario: Read auth falls back to write_password

* *GIVEN* a profile with `bfs_write_password = "wp"` and no `bfs_read_password`
* *WHEN* a BucketFS read operation resolves credentials
* *THEN* it MUST use `w:wp` for authentication

### Scenario: Read auth falls back to anonymous on public bucket

* *GIVEN* a profile with no `bfs_write_password` and no `bfs_read_password`
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

### Scenario: Profile certificate_fingerprint is appended to DSN

* *GIVEN* a profile named `pinned` with `host = "exa.example.com"`, `user = "u"`, `password = "p"`, `tls = true`, `validate_certificate = false`, `certificate_fingerprint = "deadbeef"`
* *WHEN* exapump generates the DSN for the profile
* *THEN* the DSN MUST contain `certificate_fingerprint=deadbeef` as a query parameter
* *AND* the DSN MUST also contain `tls=true` and `validateservercertificate=0`

### Scenario: Profile without certificate_fingerprint omits the parameter

* *GIVEN* a profile named `nopin` with no `certificate_fingerprint` field
* *WHEN* exapump generates the DSN for the profile
* *THEN* the DSN MUST NOT contain a `certificate_fingerprint` parameter
