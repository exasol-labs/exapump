# Feature: REPL Loop

The interactive REPL reads SQL input line by line, supports multi-line statements terminated by semicolons, executes them against Exasol, and displays results. The loop continues until the user exits.

## Background

The REPL uses rustyline for readline-style editing with persistent history. SQL statements are accumulated across lines until a semicolon is encountered at the end of a line. The connection established at startup is reused for all statements in the session.

## Scenarios

### Scenario: Single-line statement execution

* *GIVEN* the REPL is running with a valid connection
* *WHEN* the user types `SELECT 1;` and presses Enter
* *THEN* the REPL MUST execute the statement against Exasol
* *AND* the REPL MUST display the result
* *AND* the REPL MUST show the prompt again for the next input

### Scenario: Multi-line statement accumulation

* *GIVEN* the REPL is running with a valid connection
* *WHEN* the user types `SELECT` and presses Enter
* *AND* the prompt changes to a continuation prompt (e.g. `     > `)
* *AND* the user types `  1;` and presses Enter
* *THEN* the REPL MUST concatenate the lines into `SELECT\n  1`
* *AND* the REPL MUST execute the combined statement

### Scenario: DDL statement shows OK

* *GIVEN* the REPL is running with a valid connection
* *WHEN* the user executes `CREATE TABLE t(id INT);`
* *THEN* the REPL MUST display `OK`

### Scenario: DML statement shows row count

* *GIVEN* the REPL is running with a valid connection
* *WHEN* the user executes `INSERT INTO t VALUES (1),(2);`
* *THEN* the REPL MUST display `2 rows affected`

### Scenario: Query statement shows table

* *GIVEN* the REPL is running with a valid connection
* *AND* the output format is `table` (default)
* *WHEN* the user executes a SELECT statement
* *THEN* the REPL MUST display results in a formatted table
* *AND* the table MUST include column headers
* *AND* the table MUST show a row count below the result (e.g. `3 rows`)

### Scenario: Empty result set

* *GIVEN* the REPL is running with a valid connection
* *AND* the output format is `table`
* *WHEN* the user executes a SELECT that returns no rows
* *THEN* the REPL MUST display a table with column headers only
* *AND* the REPL MUST show `0 rows`

### Scenario: SQL error shows message and continues

* *GIVEN* the REPL is running with a valid connection
* *WHEN* the user executes an invalid SQL statement
* *THEN* the REPL MUST display the error message
* *AND* the REPL MUST NOT exit
* *AND* the REPL MUST show the prompt again for the next input

### Scenario: Exit with Ctrl-D

* *GIVEN* the REPL is running
* *WHEN* the user presses Ctrl-D (EOF)
* *THEN* the REPL MUST print `Bye!`
* *AND* the REPL MUST exit with code 0

### Scenario: Ctrl-C cancels pending input

* *GIVEN* the REPL is running
* *AND* the user has typed partial multi-line input
* *WHEN* the user presses Ctrl-C
* *THEN* the REPL MUST discard the accumulated input
* *AND* the REPL MUST show the primary prompt again

### Scenario: Ctrl-C on empty prompt exits

* *GIVEN* the REPL is running
* *AND* no input has been typed (prompt is empty)
* *WHEN* the user presses Ctrl-C
* *THEN* the REPL MUST print `Bye!`
* *AND* the REPL MUST exit with code 0

### Scenario: Persistent history across sessions

* *GIVEN* the REPL is running
* *WHEN* the user types a statement and executes it
* *AND* the user exits the REPL
* *AND* the user starts a new REPL session
* *THEN* the previous statement MUST be accessible via the up-arrow key
* *AND* history MUST be stored in `~/.exapump_history`

### Scenario: Multiple statements on one line

* *GIVEN* the REPL is running with a valid connection
* *WHEN* the user types `CREATE TABLE a(id INT); CREATE TABLE b(id INT);`
* *THEN* the REPL MUST split the input into two statements
* *AND* the REPL MUST execute both statements sequentially
* *AND* the REPL MUST display the result of each statement
