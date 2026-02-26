# Feature: Profile Command Structure

The `profile` subcommand manages connection profiles stored in `~/.exapump/config.toml`. It provides subcommands to list, show, add, and remove profiles.

## Background

The `profile` subcommand is available as `exapump profile`. It does not require a database connection. Profile data is stored in TOML format at `~/.exapump/config.toml`.

## Scenarios

### Scenario: Profile help shows subcommands

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump profile --help`
* *THEN* the output MUST show the `list`, `show`, `add`, and `remove` subcommands

### Scenario: Profile list with no config file

* *GIVEN* no config file exists at `~/.exapump/config.toml`
* *WHEN* the user runs `exapump profile list`
* *THEN* the output MUST indicate no profiles are configured
* *AND* the output SHOULD suggest running `exapump profile add default`

### Scenario: Profile list with profiles

* *GIVEN* a config file exists with profiles `[default]` and `[production]`
* *WHEN* the user runs `exapump profile list`
* *THEN* the output MUST list `default` and `production`
* *AND* the `default` profile MUST be marked as the auto-selected profile

### Scenario: Profile show displays details

* *GIVEN* a config file exists with a `[default]` profile containing `host = "localhost"`, `port = 8563`, `user = "sys"`, `password = "exasol"`
* *WHEN* the user runs `exapump profile show default`
* *THEN* the output MUST show the profile name, host, port, user, TLS setting, and certificate validation setting
* *AND* the password MUST be masked (e.g., `****`)

### Scenario: Profile show for missing profile

* *GIVEN* a config file exists but does not contain a profile named `staging`
* *WHEN* the user runs `exapump profile show staging`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate the profile was not found

### Scenario: Profile add with explicit flags

* *GIVEN* no config file exists
* *WHEN* the user runs `exapump profile add production --host exasol-prod.example.com --port 8563 --user admin --password s3cret --no-validate-certificate`
* *THEN* the config file MUST be created at `~/.exapump/config.toml`
* *AND* a `[production]` section MUST be written with the provided values
* *AND* stdout MUST confirm the profile was added

### Scenario: Profile add default with Docker presets

* *GIVEN* no config file exists
* *WHEN* the user runs `exapump profile add default`
* *THEN* the config file MUST be created at `~/.exapump/config.toml`
* *AND* a `[default]` section MUST be written with `host = "localhost"`, `port = 8563`, `user = "sys"`, `password = "exasol"`, `tls = true`, `validate_certificate = false`
* *AND* stdout MUST confirm the profile was added with the preset values

### Scenario: Profile add with partial flags uses defaults

* *GIVEN* no config file exists
* *WHEN* the user runs `exapump profile add mydb --host myhost --user myuser --password mypass`
* *THEN* the profile MUST be created with `port = 8563`, `tls = true`, and `validate_certificate = true` as defaults
* *AND* the explicitly provided `host`, `user`, and `password` MUST be used

### Scenario: Profile add refuses to overwrite existing

* *GIVEN* a config file exists with a `[default]` profile
* *WHEN* the user runs `exapump profile add default --host newhost --user newuser --password newpass`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate the profile already exists
* *AND* stderr SHOULD suggest using `exapump profile remove default` first

### Scenario: Profile remove deletes a profile

* *GIVEN* a config file exists with profiles `[default]` and `[production]`
* *WHEN* the user runs `exapump profile remove production`
* *THEN* the `[production]` section MUST be removed from the config file
* *AND* the `[default]` section MUST remain unchanged
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
