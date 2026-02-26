# Feature: Config Profiles

Connection profiles allow users to store named sets of connection parameters in a config file at `~/.exapump/config.toml`. This eliminates the need to pass `--dsn` or set environment variables for repeated use.

## Background

The config file uses TOML format with one section per profile. Each section name is the profile name. Profile names MUST match the pattern `[a-zA-Z0-9][a-zA-Z0-9_-]*` (start with alphanumeric, then alphanumeric, underscore, or hyphen). The config file path is `~/.exapump/config.toml` on all platforms (Windows is WSL-only per project constraints).

**Default profile selection:**
- If only one profile exists, it is automatically used as the default regardless of its name.
- If multiple profiles exist, exactly one profile MUST have `default = true`. That profile is used when no `--dsn`, `EXAPUMP_DSN`, or `--profile` is provided.
- A profile named `default` has no special meaning; the `default` field controls selection.

The resolution priority (highest to lowest) is:
1. `--dsn` CLI flag
2. `EXAPUMP_DSN` environment variable (from shell or `.env` file in working directory)
3. `--profile <name>` flag (explicit profile from config file)
4. Default profile from config file (sole profile, or the one with `default = true`)

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

  [production]
  default = true
  host = "exasol-prod.example.com"
  port = 8563
  user = "admin"
  password = "s3cret"
  schema = "my_schema"
  tls = true
  validate_certificate = true
  ```
* *WHEN* exapump parses the config
* *THEN* each TOML section MUST be treated as a named profile
* *AND* each profile MUST support the fields: `host`, `port`, `user`, `password`, `schema`, `tls`, `validate_certificate`, `default`

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
