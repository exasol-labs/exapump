# Feature: Config Profile Validation

Profile names, certificate pinning, and the config file's own permissions are validated independently of connection resolution (see `config/profiles`) and DSN generation (see `config/dsn-generation`).

## Background

## Scenarios

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

### Scenario: Profile certificate_fingerprint is appended to DSN

* *GIVEN* a profile named `pinned` with `host = "exa.example.com"`, `user = "u"`, `password = "p"`, `tls = true`, `validate_certificate = false`, `certificate_fingerprint = "deadbeef"`
* *WHEN* exapump generates the DSN for the profile
* *THEN* the DSN MUST contain `certificate_fingerprint=deadbeef` as a query parameter
* *AND* the DSN MUST also contain `tls=true` and `validateservercertificate=0`

### Scenario: Profile without certificate_fingerprint omits the parameter

* *GIVEN* a profile named `nopin` with no `certificate_fingerprint` field
* *WHEN* exapump generates the DSN for the profile
* *THEN* the DSN MUST NOT contain a `certificate_fingerprint` parameter

### Scenario: Broad saved config file permissions warn on unix

* *GIVEN* exapump is running on a unix-like operating system
* *AND* `~/.exapump/config.toml` has group or other permission bits set
* *WHEN* exapump writes (or rewrites) the config from any subcommand that mutates profiles (`add`, `init`, `edit`, `remove`)
* *THEN* stderr MUST warn that the config file permissions can expose credentials
* *AND* stderr SHOULD suggest `chmod 600`
* *AND* exapump MUST NOT change the file mode automatically
* *AND* on non-unix platforms exapump MUST NOT fail because of permission handling
