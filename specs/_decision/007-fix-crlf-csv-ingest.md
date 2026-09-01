# Decisions: fix-crlf-csv-ingest

## ADR: Detect the row separator per file instead of importing everything as LF

**ID:** detect-row-separator-per-file
**Plan:** fix-crlf-csv-ingest
**Status:** Accepted

### Context

`CsvImportOptions::row_separator` defaults to `LF` and exapump never set it, so
every CSV was imported as if its rows ended with `\n`. Against a CRLF file —
what a Windows editor writes, and what Python's `csv.writer` writes by default —
Exasol keeps the `\r` as the last byte of the last column of every row. Nothing
fails: the row count is right, the import reports success, and the only symptom
is that `LENGTH('Critical')` returns 9 instead of 8. The corruption reaches the
warehouse silently and every later query is wrong.

Exasol accepts exactly one `ROW SEPARATOR` per IMPORT, and the wrong choice is
silent in both directions: `CRLF` against an LF file makes the server read the
whole file as one record, which the header skip then discards, so the import
reports `Imported 0 rows` and leaves an empty table.

### Decision

Scan the file before the import and name the separator the file actually uses.
The scan is quote-aware, so only a `\n` outside a quoted field counts as a
record boundary, and a `\r` counts as part of the terminator only when it is the
byte immediately before such a `\n`.

### Options Considered

| Option | Verdict |
|--------|---------|
| Scan the file and pass the matching `ROW SEPARATOR` | ✓ Chosen — the server's own parser stays in charge, and no byte of user data is rewritten |
| Strip every `\r` from the stream before sending it | ✗ Rejected — destroys a `\r` that is genuinely part of a quoted value |
| Add a `--row-separator` flag and leave the default at LF | ✗ Rejected — the failure is silent, so a user who does not already know about it will never reach for the flag |
| Rewrite the file to LF in a temporary copy | ✗ Rejected — doubles the disk cost of every upload to fix what one option already expresses |

### Consequences

An upload reads the file twice: once to scan, once to stream. The scan is a
sequential 64 KiB-buffered pass with no allocation per row, against a file the
import is about to read anyway. CR-only line endings (classic Mac OS) are not
detected; `ImportRowSeparator::CR` exists, but a lone `\r` is indistinguishable
from data in an LF file, and claiming otherwise would risk breaking imports that
work today.

## ADR: Refuse a file with mixed line endings

**ID:** refuse-mixed-line-endings
**Plan:** fix-crlf-csv-ingest
**Status:** Accepted

### Context

A file where some rows end with `\r\n` and others with `\n` has no correct
`ROW SEPARATOR`. Under `LF` the CRLF rows arrive with a stray `\r`; under `CRLF`
the LF rows are glued to their neighbours and Exasol rejects the file with
`ETL-2101 … Expected: [3], found [5] columns`, which names neither the file's
line endings nor the row at fault.

### Decision

Detect the mix during the scan and refuse the upload, naming the file, the count
of rows in each style, and a command that normalises the file. The refusal
happens before the connection is opened, so no table is created and no row is
loaded.

### Options Considered

| Option | Verdict |
|--------|---------|
| Refuse, naming both counts and the fix | ✓ Chosen — the only outcome that neither corrupts data nor half-loads a table |
| Follow the majority style | ✗ Rejected — silently corrupts or drops the minority rows, which is the bug being fixed |
| Normalise the file in place or in a copy | ✗ Rejected — exapump does not own the user's input file, and a silent rewrite hides a data-quality problem the user should see |

### Consequences

A mixed file that previously "succeeded" with a stray `\r` on some rows now
fails. That is a deliberate behaviour change: a loud, actionable failure
replaces silent corruption.

## ADR: Gate the reserved-word hint on the word appearing in the statement

**ID:** gate-reserved-word-hint-on-the-statement
**Plan:** fix-crlf-csv-ingest
**Status:** Accepted

### Context

`CREATE TABLE t (STATE VARCHAR(2))` fails with
`syntax error, unexpected STATE_`. The message never says `STATE` is reserved,
nor that quoting it is the fix. The same message shape carries grammar-internal
tokens — `SELEC 1` answers `unexpected UNSIGNED_INTEGER_` — so the trailing
underscore alone cannot tell a keyword collision from an ordinary parse failure.

### Decision

Offer the hint only when the unexpected token, minus the grammar's trailing
underscore, is a plain alphabetic word **and** occurs as a whole word in the
statement that failed. Everything else keeps the existing generic hint.

### Options Considered

| Option | Verdict |
|--------|---------|
| Match the token against the statement | ✓ Chosen — no list to maintain, and the two-part test excludes parser tokens |
| Ship Exasol's reserved-word list | ✗ Rejected — a second copy of a list that Exasol owns and revises per version |
| Treat every `unexpected <TOKEN>_` as a reserved word | ✗ Rejected — `UNSIGNED_INTEGER_` and friends would produce nonsense advice |

### Consequences

A statement that genuinely misuses a keyword — `SELECT 1 GROUP 2` — also draws
the hint. The wording is conditional ("to use it as a column or table name"), so
it stays true in that case. `exapump upload` cannot hit this at all: the DDL it
generates quotes every column name.

## ADR: Enrich the `--format` rejection rather than change the flag

**ID:** enrich-the-format-rejection
**Plan:** fix-crlf-csv-ingest
**Status:** Accepted

### Context

`-f` reads as "file", so `exapump sql -f query.sql` is a common first attempt.
clap answers `invalid value 'query.sql' for '--format <FORMAT>'`, which names
the flag as the problem and leaves the user no closer to running the file.

### Decision

Wrap clap's enum parser for `sql --format`. When the rejected value looks like a
file name, replace the message with one that says `-f/--format` picks the output
format and shows `exapump sql - < <value>`. Every other rejected value keeps
clap's own message, including its did-you-mean suggestion.

### Options Considered

| Option | Verdict |
|--------|---------|
| Wrap the value parser and rewrite only the file-shaped rejection | ✓ Chosen — the flag, its short form and its help output are untouched |
| Give `-f` to a new `--file` option | ✗ Rejected — breaks every existing `-f json` invocation |
| Add the guidance to every `--format` rejection | ✗ Rejected — noise on an ordinary typo such as `--format jsonl` |

### Consequences

`possible_values` is implemented on the wrapper, so `--help` still lists `csv`
and `json` and still shows `[default: csv]`. `export --format` is left alone:
its sibling `--output` already carries the file, so the same confusion has no
foothold there.
