# Feature: SQL Error Handling

SQL errors are formatted with contextual information to help users diagnose issues. Connection failures, syntax errors, and execution errors each have tailored formatting. Pattern-matched hints provide actionable guidance for common Exasol error messages.

## Background

Errors are printed to stderr with an `Error in statement N:` prefix. Syntax errors include a pointer (`^`) under the error position. Execution errors include the failing SQL indented. Hints are best-effort pattern matches on the Exasol error message.

## Scenarios

### Scenario: Connection failure shows formatted error

* *GIVEN* an invalid or unreachable DSN
* *WHEN* the user runs `exapump sql 'SELECT 1' --dsn exasol://bad:bad@nohost:9999`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST show the error with a clear `Error:` prefix
* *AND* stderr SHOULD include a hint about checking the connection string

### Scenario: SQL syntax error shows contextual pointer

* *GIVEN* a valid connection to Exasol
* *AND* the SQL contains a syntax error
* *WHEN* the command receives a `SyntaxError` with a position from exarrow-rs
* *THEN* stderr MUST print the failing SQL statement
* *AND* stderr MUST show a pointer (`^`) under the error position
* *AND* stderr MUST print the Exasol error message
* *AND* the error MUST be prefixed with `Error in statement N:`

### Scenario: SQL execution error shows formatted message

* *GIVEN* a valid connection to Exasol
* *WHEN* the command receives an `ExecutionFailed` error from exarrow-rs
* *THEN* stderr MUST print the failing SQL statement indented
* *AND* stderr MUST print the Exasol error message
* *AND* stderr SHOULD include a hint based on the error pattern
* *AND* the error MUST be prefixed with `Error in statement N:`

### Scenario: Hint for object not found

* *GIVEN* the Exasol error message contains "object" and "not found"
* *WHEN* the error is formatted
* *THEN* stderr MUST include a hint like "Check that the table exists and the schema is correct."

### Scenario: Hint for insufficient privileges

* *GIVEN* the Exasol error message contains "insufficient privileges" or "not allowed"
* *WHEN* the error is formatted
* *THEN* stderr MUST include a hint like "The user may not have the required permissions."
