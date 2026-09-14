# Feature: BucketFS Command Structure

The `bucketfs` subcommand group provides file management operations against Exasol's BucketFS distributed file system. It supports listing, uploading, downloading, and deleting files via BucketFS's HTTP/HTTPS REST API.

## Background

BucketFS is Exasol's built-in distributed file system for storing JARs, UDF scripts, and other artifacts. It exposes an HTTP/HTTPS REST API on a configurable port (default: 2581 HTTPS). Authentication uses Basic auth with dedicated read/write passwords. The `bucketfs` subcommand group is available as `exapump bucketfs <subcommand>`.

Connection parameters come from three sources: per-command `--bfs-*` flags, a profile, and the BucketFS defaults. See `bucketfs/connection-resolution` and `bucketfs/connection-resolution-errors` for the resolution order and its failure modes.

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
