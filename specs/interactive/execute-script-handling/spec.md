# Feature: Execute Script Handling

The REPL correctly classifies `EXECUTE SCRIPT` statements as a distinct statement type and dispatches to the appropriate execution path at runtime. Whether the script returns a result set is determined by inspecting `result_set.row_count()` after execution, since the script body is server-side and cannot be known statically.

## Background

`EXECUTE SCRIPT` falls into neither the `Query` nor the `Ddl`/`Dml` arms. It is dispatched via `conn.execute` and the response is inspected at runtime: if `row_count()` is `Some`, the script returned only a row count (display `OK`); if `None`, the script returned a result set (render rows via the active output format). Leading comments before `EXECUTE` are ignored for classification purposes but the original statement text is sent to Exasol verbatim.

## Scenarios

### Scenario: Statement prefixed with a block-comment hint is classified as a query

* *GIVEN* the REPL is running with a valid connection
* *WHEN* the user enters `/*snapshot execution*/ SELECT 1;`
* *THEN* the REPL MUST classify the statement as a query
* *AND* the REPL MUST send the original statement text including the `/*snapshot execution*/` hint to Exasol
* *AND* the REPL MUST display the result in a formatted table
* *AND* the REPL MUST show `1 row` below the result

### Scenario: Statement prefixed with a line-comment is classified by the keyword after the comment

* *GIVEN* the REPL is running with a valid connection
* *WHEN* the user enters `-- a comment\nSELECT 1;` across two lines
* *THEN* the REPL MUST classify the statement as a query
* *AND* the REPL MUST send the original multi-line text including the leading comment to Exasol
* *AND* the REPL MUST display the result in a formatted table

### Scenario: EXECUTE SCRIPT with RETURNS TABLE displays a result set

* *GIVEN* the REPL is running with a valid connection
* *AND* a script `S.HELLO` exists with `RETURNS TABLE`
* *WHEN* the user enters `EXECUTE SCRIPT "S"."HELLO"();`
* *THEN* the REPL MUST execute the script via `conn.execute` (not `execute_update`)
* *AND* the REPL MUST display the result rows in the active output format
* *AND* the REPL MUST show the row count below the result

### Scenario: EXECUTE SCRIPT without a result set displays OK

* *GIVEN* the REPL is running with a valid connection
* *AND* a script `S.NOOP` exists with no `RETURNS TABLE`
* *WHEN* the user enters `EXECUTE SCRIPT "S"."NOOP"();`
* *THEN* the REPL MUST display `OK`
* *AND* the REPL MUST NOT print a row-count line
