# SQL Interaction

## sql

Execute SQL statements against Exasol and print results.

```bash
exapump sql 'SELECT * FROM my_schema.events LIMIT 10'
```

If the SQL argument is omitted (or `-` is given), exapump reads from stdin:

```bash
echo 'SELECT 1' | exapump sql
cat query.sql | exapump sql
```

### Options

| Flag | Default | Description |
|------|---------|-------------|
| `--format` / `-f` | `csv` | Output format: `csv` or `json` |

### Examples

```bash
# CSV output (default)
exapump sql 'SELECT * FROM t'

# JSON output
exapump sql -f json 'SELECT * FROM t'

# Read SQL from a file
exapump sql < migration.sql
```

---

## interactive

Start an interactive SQL session (REPL) connected to Exasol.

```bash
exapump interactive
```

Inside the session, type SQL statements and see results rendered as a table. The interactive command uses the same connection resolution as all other commands (profile, `--dsn`, or `EXAPUMP_DSN`).
