# Architecture Decision Records

<!-- ADRs are numbered sequentially starting from ADR-001. Never renumber. -->
<!-- recorder-agent appends new ADRs from plan decision logs. -->

---

## ADR-001: Preserve original SQL byte-for-byte to Exasol; strip comments only for classification

**Date:** 2026-05-21
**Plan:** `fix-sql-classification-and-hint-preservation`
**Status:** Accepted

### Context

SQL statements containing leading or inline comments (e.g. `/*snapshot execution*/`, `-- hint`) were being silently rewritten before reaching Exasol. The `strip_comments` pre-pass in `sql.rs::run` removed comments to enable statement-type classification, but this also dropped Exasol optimizer hints that users intentionally include in their SQL. The same pre-pass prevented the comment-aware splitter from being needed, but meant the split result was already mutated.

### Decision

Remove the `strip_comments(&sql_input)` pre-pass in `sql.rs::run`. Make `split_statements` comment-aware so it can find top-level semicolons without mutating the input. Call `strip_comments` inside `StatementType::from_sql` only, keeping comment stripping as a local, side-effect-free transformation used solely for keyword extraction.

### Options Considered

| Option | Verdict |
|--------|---------|
| Strip comments inside `StatementType::from_sql` only; pass original SQL to Exasol | ✓ Chosen — preserves hints, fixes the root cause |
| Keep `strip_comments` pre-pass in `sql.rs::run` | ✗ Rejected — silently drops `/*snapshot execution*/` and similar Exasol hints, defeating user intent |
| Detect hints heuristically and preserve only those | ✗ Rejected — fragile; a hint catalog cannot anticipate every Exasol-side directive |

### Consequences

The principle "what the user wrote is what Exasol sees" is enforced by the architecture. `split_statements` now requires a comment-aware four-state scanner to correctly identify top-level semicolons without rewriting input. All downstream consumers of `split_statements` receive the original statement text.

---

## ADR-002: Add `StatementType::Execute` variant and dispatch at runtime on `result_set.row_count()`

**Date:** 2026-05-21
**Plan:** `fix-sql-classification-and-hint-preservation`
**Status:** Accepted

### Context

`EXECUTE SCRIPT` statements were falling into the `Ddl` arm of the statement dispatcher, which calls `execute_update()`. When the script is defined with `RETURNS TABLE`, exarrow-rs's `execute_update` requires the server to return a row count — which fails because the script returns a result set instead. The statement type cannot be determined statically: whether a script returns rows depends on its server-side definition.

### Decision

Introduce a new `Execute` variant in `StatementType` mapped from the `EXECUTE` keyword. In both `sql.rs::run` and `interactive.rs::execute_statement`, call `conn.execute(stmt)` for the `Execute` arm and branch on `result_set.row_count().is_some()`: if `Some`, print `OK`; otherwise `fetch_all` and render via the same code path as the `Query` arm.

### Options Considered

| Option | Verdict |
|--------|---------|
| Add `StatementType::Execute` variant; runtime-branch on `row_count()` | ✓ Chosen — minimal-blast-radius change; existing Dml/Ddl paths stay intact |
| Always use `conn.execute` for every statement and branch on `row_count()` globally | ✗ Rejected — Dml/Ddl rely on the existing row-count contract for status-line formatting; a wholesale switch risks regressing those messages |
| Inspect SQL for `RETURNS TABLE` at the call site to determine dispatch | ✗ Rejected — script body is server-side; cannot know what it returns without executing |

### Consequences

`EXECUTE SCRIPT` is modelled as its own polymorphic variant, reflecting that it is genuinely the ambiguous case in Exasol SQL. The existing `Query`, `Dml`, and `Ddl` paths are unchanged. Both runners (`sql.rs` and `interactive.rs`) share the same branching logic.

---

## ADR-003: Auto-detect CREATE … SCRIPT bodies in the statement splitter via a ScriptBody state

**Date:** 2026-06-11
**Plan:** `fix-issues-22-23-release-0.10.1`
**Status:** Accepted

### Context

The `split_statements` scanner in `sql.rs` is a five-state character scanner (Normal, SingleQuote, DoubleQuote, LineComment, BlockComment). It had no concept of Exasol script bodies, so every top-level `;` inside a `CREATE … SCRIPT … AS` body wrongly split the statement, causing the script body to arrive at Exasol as multiple broken fragments. Users reported this as issue #23.

### Decision

Extend `split_statements` with a sixth `ScanState::ScriptBody` state. A word-boundary keyword tracker in the Normal state follows the token progression `CREATE → SCRIPT → AS` (case-insensitive, tolerant of `OR REPLACE`, language keywords such as `PYTHON3`, and `ADAPTER/SET/SCALAR`). When `AS` completes while `CREATE … SCRIPT` is in effect, the scanner enters `ScriptBody`. In `ScriptBody`, all characters accumulate and the exit pattern is a line containing only `/` (newline, optional whitespace, `/`, then newline or end-of-input). On exit the accumulated text is flushed as one statement with the lone `/` line removed, and the scanner returns to Normal.

### Options Considered

| Option | Verdict |
|--------|---------|
| Extend `split_statements` with `ScriptBody` state; auto-detect script bodies | ✓ Chosen — transparent to users; works on existing scripts with no changes |
| Add a `--no-split` CLI flag so users can opt out of splitting | ✗ Rejected — requires users to modify every invocation; adds CLI surface |
| Require a manually specified delimiter | ✗ Rejected — breaks existing workflows; users already use the exaplus `/` convention |

### Consequences

`CREATE … SCRIPT … AS` blocks using the exaplus `/` terminator are handled transparently by the splitter. No user-facing CLI change is required. The existing quote and comment states are unaffected. `FUNCTION … END` blocks remain out of scope and are not handled by this change.

---

## ADR-004: Lone `/` line is the only script-body terminator; FUNCTION … END is out of scope

**Date:** 2026-06-11
**Plan:** `fix-issues-22-23-release-0.10.1`
**Status:** Accepted

### Context

Once a `ScriptBody` state was introduced, the terminator pattern needed to be defined. Exasol scripts can also contain `FUNCTION … END` and other block constructs. The defect (#23) was specifically about `CREATE … SCRIPT` bodies using the exaplus `/`-on-a-line-by-itself convention.

### Decision

A line containing only `/` (after trim), followed by newline or EOF, terminates a script body. `FUNCTION … END` blocks are explicitly not handled by this fix.

### Options Considered

| Option | Verdict |
|--------|---------|
| Use lone `/` line as the sole terminator | ✓ Chosen — matches exaplus convention; scope is kept tight to the reported defect |
| Also match an `END` keyword to balance `FUNCTION … END` blocks | ✗ Rejected — broader than the bug; `END` appears in many non-block contexts and balancing is error-prone |
| Allow a configurable delimiter | ✗ Rejected — adds configuration surface; the `/` convention is standard for Exasol tooling |

### Consequences

The splitter correctly handles the dominant `CREATE … SCRIPT` use-case reported in issue #23. `FUNCTION … END` blocks remain a potential future extension but are not addressed here, keeping the change minimal and the correctness scope well-defined.

---

## ADR-005: Replace `scripts/wait-for-exasol.sh` with a first-class `exapump wait` subcommand

**Date:** 2026-06-12
**Plan:** `add-wait-command`
**Status:** Accepted

### Context

CI workflows for exapump and sibling repos started an Exasol Docker container and blocked via `scripts/wait-for-exasol.sh` before running integration tests. The script depended on bash, `/dev/tcp`, a separately-built `health_check` example binary, and direct `docker` invocations. It was fragile (shell-specific, required building an extra binary), duplicated across repos, and lived outside the spec-governed CLI surface.

### Decision

Replace `scripts/wait-for-exasol.sh` with a built-in `exapump wait` subcommand that reuses the shared `ConnectionArgs`/transport logic and gains full test coverage under the spec-governed CLI surface.

### Options Considered

| Option | Verdict |
|--------|---------|
| Replace with `exapump wait` subcommand | ✓ Chosen — reuses DSN/transport logic, gains test coverage, removes cross-repo shell dependency |
| Keep the bash script | ✗ Rejected — duplicates DSN logic, requires building a separate `health_check` binary, and is untested |
| Ship a thin wrapper script that calls exapump | ✗ Rejected — still requires shell and adds indirection without eliminating the duplication |

### Consequences

Readiness-polling is a first-class CLI capability governed by specs and covered by integration tests. The `health_check` example binary invocation is no longer needed in CI. Sibling repos can adopt `exapump wait` without maintaining their own polling scripts.

---

## ADR-006: Derive polled host/port from the resolved DSN rather than separate flags

**Date:** 2026-06-12
**Plan:** `add-wait-command`
**Status:** Accepted

### Context

The old `wait-for-exasol.sh` script used separate `EXASOL_HOST`/`EXASOL_PORT` environment variables to determine what to poll. The new `exapump wait` needed a connection-info strategy consistent with the rest of the CLI surface.

### Decision

Compute the TCP host and port to poll from the DSN resolved via `ConnectionArgs` (the same flattened struct all other subcommands use), rather than exposing independent `--host`/`--port` flags or separate environment variables.

### Options Considered

| Option | Verdict |
|--------|---------|
| Derive host/port from the resolved DSN via `ConnectionArgs` | ✓ Chosen — single source of truth; `wait` polls exactly what other subcommands connect to |
| Independent `--host`/`--port` flags | ✗ Rejected — duplicates connection configuration; users could inadvertently poll a different endpoint than the one being used |
| Separate env vars (`EXASOL_HOST`/`EXASOL_PORT`) mirroring the old script | ✗ Rejected — introduces a second connection-info mechanism inconsistent with `EXAPUMP_DSN` and profile precedence |

### Consequences

`exapump wait` inherits the full DSN resolution precedence (`--dsn` flag, `EXAPUMP_DSN` env var, profile) automatically. There is no risk of polling a different endpoint than other subcommands would connect to. A missing or unparseable DSN triggers a fast exit with code 1 (configuration error).
