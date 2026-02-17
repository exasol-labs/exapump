# Feature: Help and Version Output

The CLI provides standard help and version information so users can discover available commands and verify the installed version.

## Background

exapump is invoked as a single binary from the command line. All subcommands and flags are discoverable via `--help`.

## Scenarios

### Scenario: Display top-level help

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump --help`
* *THEN* the output MUST include the program description
* *AND* the output MUST list the `upload` subcommand
* *AND* the output MUST show the `--help` and `--version` flags

### Scenario: Display version

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump --version`
* *THEN* the output MUST include the program name and version number

### Scenario: No arguments shows help

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump` with no arguments
* *THEN* the output MUST display help text
* *AND* the exit code SHOULD be non-zero
