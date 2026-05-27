# Feature: Profile Command Structure

The `profile` subcommand manages connection profiles stored in `~/.exapump/config.toml`. It provides subcommands to list, show, add, remove, init (guided wizard), and edit (interactive editor) profiles.

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

### Scenario: Profile add prompts for password on TTY when --password omitted

* *GIVEN* exapump is installed
* *AND* the user runs `exapump profile add staging --host h --user u` without `--password` in an interactive terminal
* *WHEN* the command executes
* *THEN* exapump MUST prompt for the password via a hidden TTY input (no echo)
* *AND* the password value MUST NOT appear in the process command line, shell history, or stdout
* *AND* if the entered password is empty the command MUST exit with a non-zero code and indicate the password cannot be empty

### Scenario: Profile add refuses missing password in non-TTY context

* *GIVEN* exapump is installed
* *AND* the user runs `exapump profile add staging --host h --user u` without `--password` and without an interactive terminal (e.g. piped stdin, CI)
* *WHEN* the command executes
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate `--password` is required
* *AND* stderr MUST hint at the `profile init` wizard as the interactive alternative

### Scenario: Profile init runs the guided wizard

* *GIVEN* exapump is installed and stdin is an interactive terminal
* *WHEN* the user runs `exapump profile init` with no other arguments
* *THEN* exapump MUST prompt sequentially for: profile name, host, port (defaulting to 8563), user, password (hidden), password confirmation (hidden), schema (optional), TLS enable, server certificate validation, default-profile selection, and an optional BucketFS section
* *AND* on success exapump MUST write the new profile to the config and print a confirmation line
* *AND* the password MUST NOT be accepted as a command-line flag on the `init` subcommand

### Scenario: Profile init accepts pre-fill flags

* *GIVEN* exapump is installed and stdin is an interactive terminal
* *WHEN* the user runs `exapump profile init prod --host h --port 8563 --user u --schema s --default --no-bucketfs`
* *THEN* exapump MUST skip the prompts for the pre-filled fields
* *AND* exapump MUST still prompt for the password and password confirmation via hidden TTY input
* *AND* exapump MUST NOT prompt for BucketFS settings

### Scenario: Profile init refuses without a TTY

* *GIVEN* exapump is installed
* *AND* stdin is not an interactive terminal
* *WHEN* the user runs `exapump profile init`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate an interactive terminal is required
* *AND* stderr MUST suggest `profile add` with explicit flags for scripted setups

### Scenario: Profile init rejects an existing profile name

* *GIVEN* a config file contains a profile named `prod`
* *AND* stdin is an interactive terminal
* *WHEN* the user runs `exapump profile init prod`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate the profile already exists and suggest `profile remove` first

### Scenario: Profile edit updates an existing profile interactively

* *GIVEN* a config file contains a profile named `prod`
* *AND* stdin is an interactive terminal
* *WHEN* the user runs `exapump profile edit prod`
* *THEN* exapump MUST prompt for each field with the current value shown as the default (pressing Enter keeps it)
* *AND* exapump MUST gate the password rotation behind a "Change password?" confirmation prompt
* *AND* if the user declines the password change the saved password MUST remain unchanged
* *AND* if the user accepts the password change exapump MUST prompt for the new password and confirmation via hidden TTY input

### Scenario: Profile edit refuses without a TTY

* *GIVEN* exapump is installed
* *AND* stdin is not an interactive terminal
* *WHEN* the user runs `exapump profile edit prod`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate an interactive terminal is required

### Scenario: Profile edit on missing profile

* *GIVEN* a config file exists but does not contain a profile named `ghost`
* *WHEN* the user runs `exapump profile edit ghost`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate the profile was not found or the command requires an interactive terminal

### Scenario: Profile remove prompts for confirmation on TTY

* *GIVEN* a config file contains a profile named `prod`
* *AND* stdin is an interactive terminal
* *WHEN* the user runs `exapump profile remove prod` without `--yes`
* *THEN* exapump MUST prompt the user to confirm the removal
* *AND* if the user declines the profile MUST remain in the config file

### Scenario: Profile remove --yes skips confirmation

* *GIVEN* a config file contains a profile named `prod`
* *WHEN* the user runs `exapump profile remove prod --yes` (or `-y`)
* *THEN* exapump MUST remove the profile without prompting
* *AND* stdout MUST confirm the profile was removed

### Scenario: Profile remove refuses to delete without --yes in non-TTY context

* *GIVEN* a config file contains a profile named `prod`
* *AND* stdin is not an interactive terminal
* *WHEN* the user runs `exapump profile remove prod` without `--yes`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate `--yes` is required in non-interactive contexts
* *AND* the profile MUST remain in the config file
