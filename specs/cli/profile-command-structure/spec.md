# Feature: Profile Command Structure

The `profile` subcommand manages connection profiles stored in `~/.exapump/config.toml`. It provides subcommands to list, show, add, and remove profiles.

## Background

The `profile` subcommand is available as `exapump profile`. It does not require a database connection. Profile data is stored in TOML format at `~/.exapump/config.toml`.

## Scenarios

### Scenario: Profile list with no config file

* *GIVEN* no config file exists at `~/.exapump/config.toml`
* *WHEN* the user runs `exapump profile list`
* *THEN* the output MUST indicate no profiles are configured
* *AND* the output SHOULD suggest running `exapump profile add <name>`

### Scenario: Profile list with profiles

* *GIVEN* a config file exists with profiles `local` and `production`
* *AND* `production` has `default = true`
* *WHEN* the user runs `exapump profile list`
* *THEN* the output MUST list `local` and `production`
* *AND* `production` MUST be annotated with `(default)`

### Scenario: Profile list single profile shows default

* *GIVEN* a config file exists with exactly one profile named `mydb`
* *AND* `mydb` does not have `default = true`
* *WHEN* the user runs `exapump profile list`
* *THEN* the output MUST list `mydb`
* *AND* `mydb` MUST be annotated with `(default)`

### Scenario: Profile add default with Docker presets

* *GIVEN* no config file exists
* *WHEN* the user runs `exapump profile add default`
* *THEN* a `[default]` section MUST be written with Docker preset values
* *AND* the profile MUST have `default = true` (auto-defaulted as first profile)
* *AND* stdout MUST include "(set as default)"

### Scenario: Profile add refuses to overwrite existing

* *GIVEN* a config file exists with a profile
* *WHEN* the user runs `exapump profile add <name>` where `<name>` already exists
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate the profile already exists
* *AND* stderr SHOULD suggest using `exapump profile remove <name>` first

### Scenario: Profile add with --default flag

* *GIVEN* a config file exists with a profile `local`
* *WHEN* the user runs `exapump profile add production --host prod.example.com --user admin --password s3cret --default`
* *THEN* the `[production]` section MUST be written with `default = true`
* *AND* stdout MUST confirm the profile was added

### Scenario: Profile add --default removes other defaults

* *GIVEN* a config file exists with profile `local` having `default = true`
* *WHEN* the user runs `exapump profile add production --host prod.example.com --user admin --password s3cret --default`
* *THEN* the `[production]` section MUST have `default = true`
* *AND* the `[local]` section MUST have its `default` field removed or set to `false`

### Scenario: Profile help shows subcommands

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump profile --help`
* *THEN* the output MUST show the `list`, `show`, `add`, and `remove` subcommands

### Scenario: Profile show displays details

* *GIVEN* a config file exists with a profile containing `host = "localhost"`, `port = 8563`, `user = "sys"`, `password = "exasol"`
* *WHEN* the user runs `exapump profile show <name>`
* *THEN* the output MUST show the profile name, host, port, user, TLS setting, and certificate validation setting
* *AND* the password MUST be masked (e.g., `****`)
* *AND* if `certificate_fingerprint` is set, the output MUST show it (in full, since it is a public identifier)

### Scenario: Profile show for missing profile

* *GIVEN* a config file exists but does not contain a profile named `staging`
* *WHEN* the user runs `exapump profile show staging`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate the profile was not found

### Scenario: Profile add with explicit flags

* *GIVEN* no config file exists
* *WHEN* the user runs `exapump profile add production --host exasol.example.com --user admin --password secret`
* *THEN* a `[production]` section MUST be written with the provided values
* *AND* the profile MUST have `default = true` (auto-defaulted as first profile)
* *AND* stdout MUST include "(set as default)"

### Scenario: Profile add with partial flags uses defaults

* *GIVEN* no config file exists
* *WHEN* the user runs `exapump profile add mydb --host myhost --user myuser --password mypass`
* *THEN* the profile MUST be created with `port = 8563`, `tls = true`, and `validate_certificate = true` as defaults
* *AND* the explicitly provided `host`, `user`, and `password` MUST be used

### Scenario: Profile remove deletes a profile

* *GIVEN* a config file exists with profiles `local` and `production`
* *WHEN* the user runs `exapump profile remove production`
* *THEN* the `[production]` section MUST be removed from the config file
* *AND* the `[local]` section MUST remain unchanged
* *AND* stdout MUST confirm the profile was removed

### Scenario: Profile remove for missing profile

* *GIVEN* a config file exists but does not contain a profile named `staging`
* *WHEN* the user runs `exapump profile remove staging`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate the profile was not found

### Scenario: Profile add missing required fields

* *GIVEN* no config file exists
* *WHEN* the user runs `exapump profile add production --host myhost` without `--user` or `--password`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate which fields are missing

### Scenario: Profile add rejects invalid name

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump profile add -bad-name --host localhost --user sys --password exasol`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate the profile name is invalid

### Scenario: Profile name is required for add

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump profile add` without a profile name
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate a profile name is required

### Scenario: Profile add accepts --certificate-fingerprint

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump profile add staging --host exa.example.com --user u --password p --certificate-fingerprint deadbeefcafebabe`
* *THEN* the `[staging]` section MUST be written with `certificate_fingerprint = "deadbeefcafebabe"`
* *AND* stdout MUST confirm the profile was added
