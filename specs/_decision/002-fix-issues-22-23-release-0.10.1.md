# Decisions: fix-issues-22-23-release-0.10.1

## ADR: Auto-detect CREATE … SCRIPT bodies in the statement splitter via a ScriptBody state

**ID:** auto-detect-script-bodies-scriptbody-state
**Plan:** fix-issues-22-23-release-0.10.1
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

## ADR: Lone `/` line is the only script-body terminator; FUNCTION … END is out of scope

**ID:** lone-slash-line-terminates-script-body
**Plan:** fix-issues-22-23-release-0.10.1
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
