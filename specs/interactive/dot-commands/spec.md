# Feature: Dot-Commands

The interactive REPL supports dot-commands (prefixed with `.`) for controlling REPL behavior. Dot-commands are not sent to Exasol but are interpreted locally by the REPL.

## Background

Dot-commands start with a `.` as the first non-whitespace character on a line. They are processed immediately (not accumulated for multi-line input). Unknown dot-commands produce an error message.

## Scenarios

### Scenario: Switch output format to CSV

* *GIVEN* the REPL is running
* *WHEN* the user types `.format csv`
* *THEN* the REPL MUST switch the output format to CSV
* *AND* subsequent SELECT results MUST be displayed in CSV format
* *AND* the REPL MUST confirm the change (e.g. `Output format: csv`)

### Scenario: Switch output format to JSON

* *GIVEN* the REPL is running
* *WHEN* the user types `.format json`
* *THEN* the REPL MUST switch the output format to JSON
* *AND* subsequent SELECT results MUST be displayed in JSON format

### Scenario: Switch output format to table

* *GIVEN* the REPL is running with CSV format active
* *WHEN* the user types `.format table`
* *THEN* the REPL MUST switch back to table output format
* *AND* subsequent SELECT results MUST be displayed as formatted tables

### Scenario: Format without argument shows current format

* *GIVEN* the REPL is running with table format (default)
* *WHEN* the user types `.format`
* *THEN* the REPL MUST display the current output format (e.g. `Output format: table`)

### Scenario: Invalid format argument

* *GIVEN* the REPL is running
* *WHEN* the user types `.format xml`
* *THEN* the REPL MUST display an error message listing valid formats (table, csv, json)

### Scenario: Help dot-command

* *GIVEN* the REPL is running
* *WHEN* the user types `.help`
* *THEN* the REPL MUST display a list of available dot-commands with descriptions

### Scenario: Exit dot-command

* *GIVEN* the REPL is running
* *WHEN* the user types `.exit`
* *THEN* the REPL MUST print `Bye!`
* *AND* the REPL MUST exit with code 0

### Scenario: Unknown dot-command

* *GIVEN* the REPL is running
* *WHEN* the user types `.unknown`
* *THEN* the REPL MUST display an error message like `Unknown command: .unknown`
* *AND* the REPL MUST suggest typing `.help` for available commands
* *AND* the REPL MUST NOT exit
