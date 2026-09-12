# Verification Report: bucketfs-profile-override-fallback

## Verdict

| Result | Details |
|--------|---------|
| **PASS** | `exapump bucketfs <ls|cp|rm>` resolves a connection from `--bfs-*` overrides alone, with no config file required. All 16 scenario-coverage tests and all 4 manual-testing commands pass against a live Exasol container. |
| Code review | 4 findings — 4 fixed |

| Check | Status |
|-------|--------|
| Build | ✓ |
| Tests | ✓ |
| Lint | ✓ |
| Format | ✓ |
| Scenario Coverage | ✓ |
| Manual Tests | ✓ |

## Test Evidence

### Coverage

Coverage-percentage tooling is not configured in this project (no `cargo tarpaulin`/`cargo llvm-cov` in `Cargo.toml` or CI). Coverage is evidenced instead by the Scenario Coverage table below, which maps every plan-listed scenario to a passing test.

### Test Results

| Type | Run | Passed | Ignored |
|------|-----|--------|---------|
| Unit | 356 | 356 | 0 |
| Integration | 194 | 194 | 1 |

Full command: `cargo test`, run against a live `exasol/docker-db:2025.2.0` container (`localhost:8563`/`localhost:2581`). Total: 550 passed, 0 failed, 1 ignored (pre-existing `wait_dumps_logs_when_container_crashes` in `tests/wait_test.rs`, documented manual-only, untouched by this plan).

### Manual Tests

| Test | Result |
|------|--------|
| `EXAPUMP_CONFIG=/tmp/no-such-config.toml exapump bucketfs ls --bfs-host localhost --bfs-port 2581 --bfs-write-password "$BFS_PW" --bfs-validate-certificate false` — expect exit 0, bucket entries, no `No profiles found in config` | ✓ |
| `EXAPUMP_CONFIG=/tmp/no-such-config.toml exapump bucketfs ls` — expect exit 1, stderr names `--bfs-host` and `exapump profile add`, `No profiles found in config` as cause | ✓ |
| `EXAPUMP_CONFIG=/tmp/no-such-config.toml exapump bucketfs ls --bfs-host localhost --bfs-write-password "$BFS_PW" --bfs-validate-certificate false` (no `--bfs-port`) — expect exit 0, reaches port 2581 / bucket default | ✓ |
| `EXAPUMP_CONFIG=/tmp/no-such-config.toml exapump bucketfs cp /etc/hostname issue46.txt --bfs-host localhost --bfs-write-password "$BFS_PW" --bfs-validate-certificate false` — expect exit 0, `Uploaded /etc/hostname to issue46.txt` | ✓ |

## Tool Evidence

### Linter

```
cargo clippy --all-targets -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.03s
(0 warnings, 0 errors)
```

### Formatter

```
cargo fmt --check
(no output — no changes needed)
```

## Scenario Coverage

| Domain | Feature | Scenario | Test Location | Test Name | Passes |
|--------|---------|----------|---------------|-----------|--------|
| cli | bucketfs-command-structure | BucketFS overrides work without a config file | `tests/bucketfs_test.rs` | `overrides_only_lists_bucket_without_config` | Pass |
| cli | bucketfs-command-structure | Self-sufficient overrides ignore the default profile | `tests/bucketfs_test.rs` | `overrides_only_ignores_default_profile_bucket` | Pass |
| cli | bucketfs-command-structure | Named profile stays the base when overrides are self-sufficient | `tests/bucketfs_test.rs` | `named_profile_stays_base_with_overrides` | Pass |
| cli | bucketfs-command-structure | Host override alone still inherits the default profile | `tests/bucketfs_test.rs` | `host_override_inherits_default_profile_password` | Pass |
| cli | bucketfs-command-structure | Host override alone works when no profile resolves | `tests/cli_test.rs` | `bucketfs_host_override_without_config_skips_profile_error` | Pass |
| cli | bucketfs-command-structure | Missing host and missing profile name both remedies | `tests/cli_test.rs` | `bucketfs_without_host_or_profile_names_both_remedies` | Pass |
| cli | bucketfs-command-structure | Ambiguous default profile still fails a host-only run | `tests/cli_test.rs` | `bucketfs_host_override_reports_ambiguous_default_profile` | Pass |
| cli | bucketfs-command-structure | Host-only run still fails when no default profile is set | `tests/cli_test.rs` | `bucketfs_host_override_reports_missing_default_profile` | Pass |
| cli | bucketfs-command-structure | Default profile stays the base when no override is given | `tests/bucketfs_test.rs` | `default_profile_lists_bucket_without_profile_flag` | Pass |
| cli | bucketfs-command-structure | Self-sufficient overrides ignore the default profile (certificate validation) | `src/commands/bucketfs.rs` | `self_sufficient_overrides_ignore_profile_validate_certificate` | Pass |
| config | profiles | BucketFS defaults apply when no profile supplies a base | `src/config.rs` | `bfs_connection_with_defaults_uses_standard_values` | Pass |
| config | profiles | Read credential falls back to the --bfs-write-password flag | `src/commands/bucketfs.rs` | `read_credential_falls_back_to_write_password_flag` | Pass |
| config | profiles | Configured read password outranks the write-password flag | `src/commands/bucketfs.rs` | `configured_read_password_outranks_write_password_flag` | Pass |
| config | profiles | Read credential stays unset when no password is given | `src/commands/bucketfs.rs` | `read_credential_stays_unset_without_any_password` | Pass |
| config | profiles | Multiple default profiles is an error | `tests/profile_test.rs` | `multiple_defaults_error` | Pass |
| config | profiles | Multiple profiles without default is an error | `tests/profile_test.rs` | `no_default_among_multiple_error` | Pass |

## Notes

Code review found 4 issues, all fixed before this report: a real correctness defect (`Profile::resolve_bfs_connection` applied the write-to-read credential fallback a second time, one step before `BfsConnection::effective_read_password`, so a rotated `--bfs-write-password` could be silently overridden by a stale profile-derived read password — fixed by making `effective_read_password` the sole owner), five credential error messages that named only a profile config key on a path that has no profile (now also name the `--bfs-*` flag), one test asserting a private struct field instead of observable behavior (deleted, already covered by two other tests), and one duplicate test (deleted). Full detail in `review-findings.md`.

Two behavior changes affect existing callers, both documented in `plan.md` § Impact and `CHANGELOG.md`: (1) `--bfs-host` plus a BucketFS credential with no `--profile` no longer inherits `bfs_port`/`bfs_bucket`/`bfs_tls`/`bfs_validate_certificate` from an unrelated default profile — a caller relying on that inheritance against a self-signed certificate needs `--profile <name>` or `--bfs-validate-certificate false`; (2) `--bfs-write-password` now also serves as the read credential when no read credential is otherwise set.

Round-2 plan review's 9 advisory findings (`review/round-2.md`) are all applied to `plan.md`, `decision-log.md`, and both spec deltas, verified independently during code review.

One pre-existing, out-of-scope issue observed and left untouched: `bfss_uri_tls_inference` in `tests/bucketfs_test.rs` leaks a test file into the bucket because its cleanup path does not match what the `bfss://` upload actually writes.
