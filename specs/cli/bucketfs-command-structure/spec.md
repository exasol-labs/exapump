# Feature: BucketFS Command Structure

The `bucketfs` subcommand group provides file management operations against Exasol's BucketFS distributed file system. It supports listing, uploading, downloading, and deleting files via BucketFS's HTTP/HTTPS REST API.

## Background

BucketFS is Exasol's built-in distributed file system for storing JARs, UDF scripts, and other artifacts. It exposes an HTTP/HTTPS REST API on a configurable port (default: 2581 HTTPS). Authentication uses Basic auth with dedicated read/write passwords. The `bucketfs` subcommand group is available as `exapump bucketfs <subcommand>`. Connection parameters come from the profile system — BucketFS reuses `host` (overridable via `bfs_host`), defaults bucket to `"default"`, port to `2581`, and inherits `tls`/`validate_certificate` (overridable via `bfs_tls`/`bfs_validate_certificate`). Per-command flags can override any profile value.

## Scenarios

### Scenario: BucketFS help shows subcommands

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump bucketfs --help`
* *THEN* the output MUST show a `ls` subcommand
* *AND* the output MUST show a `cp` subcommand
* *AND* the output MUST show a `rm` subcommand

### Scenario: BucketFS help shows connection options

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump bucketfs --help`
* *THEN* the output MUST show a `--profile` option
* *AND* the output MUST show a `--bfs-host` option
* *AND* the output MUST show a `--bfs-port` option
* *AND* the output MUST show a `--bfs-bucket` option

### Scenario: BucketFS ls help

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump bucketfs ls --help`
* *THEN* the output MUST show a `[PATH]` positional argument
* *AND* the output MUST show a `--recursive` flag

### Scenario: BucketFS cp help

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump bucketfs cp --help`
* *THEN* the output MUST show `<SOURCE>` and `<DESTINATION>` positional arguments
* *AND* the output MUST show a `--recursive` flag

### Scenario: BucketFS rm help

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump bucketfs rm --help`
* *THEN* the output MUST show a `<PATH>` positional argument
* *AND* the output MUST show a `--recursive` flag

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
