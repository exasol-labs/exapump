# Feature: Help and Version

The help and version output reflects all available subcommands.

## Background

Top-level help and no-argument output must list all subcommands including `wait`.

## Scenarios

### Scenario: Display top-level help

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump --help`
* *THEN* the output MUST list `upload`, `sql`, `export`, `interactive`, `profile`, `bucketfs`, and `wait` as available subcommands

### Scenario: No arguments shows help

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump` with no arguments
* *THEN* the output MUST list `upload`, `sql`, `export`, `interactive`, `profile`, `bucketfs`, and `wait` as available subcommands
