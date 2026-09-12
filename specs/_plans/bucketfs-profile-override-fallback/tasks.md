# Tasks: bucketfs-profile-override-fallback

## PR Lifecycle
- [x] resolved
- [x] implemented
- [x] version-bumped
- [ ] tested-green
- [ ] recorded
- [ ] pr-ready

## Phase 2: Implementation (Group A)

- [x] 2.1 Add `DEFAULT_BFS_BUCKET` and `BfsConnection::with_defaults(host: String) -> BfsConnection` to `src/config.rs`, with a doc comment stating that the host is the only BucketFS field without a default. Also add `BfsConnection::effective_read_password(&self) -> Option<&str>` applying the write-to-read fallback once over the merged connection (Design Depth advisory: single owner for the fallback). Unit-test the returned port, bucket, TLS flag, certificate flag, absent credentials, and the fallback method.
- [x] 2.2 Rewrite `Profile::resolve_bfs_connection` to take its defaults from `BfsConnection::with_defaults` instead of restating `2581`, `"default"`, and `true`. Keep every existing `resolve_bfs_connection_*` unit test green without editing it.
- [x] 2.3 Add `#[derive(Default)]` to `BfsConnectionOverrides` in `src/cli.rs`.
- [x] 2.4 [expert] Add `base_connection(config: &Config, overrides: &BfsConnectionOverrides) -> anyhow::Result<BfsConnection>` to `src/commands/bucketfs.rs`, implementing the **six-branch** precedence rule (round-2 review COMPLETENESS_GAP: add branch `host, config holds one or more profiles, no default resolves ──▶ propagate the find_default_profile error`, between today's branches 4 and 5). Write one unit test per branch first, including `self_sufficient_overrides_ignore_profile_validate_certificate` (round-2 review COMPLETENESS_GAP on the `cli/bucketfs-command-structure` scenario).
- [x] 2.5 [expert] Extend the read-credential resolution so it resolves as `--bfs-read-password`, then the base read password, then `--bfs-write-password`, calling `BfsConnection::effective_read_password` from 2.1 at the single read site instead of restating the fallback in `resolve_connection` (round-2 review INFORMATION_LEAKAGE fix). Write the two ordering unit tests first.
- [x] 2.6 Replace the inline profile match in `run` with a call to `base_connection`, dropping the `Profile` clone.
- [x] 2.7 Attach the BucketFS guidance to the profile-resolution error in `base_connection` when no `--bfs-host` is given. The message names `--bfs-host` and `exapump profile add`, and keeps the original `find_default_profile` error as the cause. When `--bfs-host` is given and the config holds one or more profiles, the `find_default_profile` error propagates unchanged (including the new sixth branch from 2.4).
- [x] 2.8 Apply the round-2 plan-review advisory fixes (`specs/_plans/bucketfs-profile-override-fallback/review/round-2.md`) to `plan.md` and the two spec deltas, since these are documentation-only and do not gate on the code above:
  - plan.md § Design > Context Goals bullet + § Impact: narrow the "same connection on every machine" claim per the SCOPE_REDUCTION finding.
  - plan.md § Design > Architecture: add the sixth branch line; § Implementation Tasks: "five-branch" → "six-branch".
  - plan.md § Verification > Scenario Coverage: add rows for `multiple_defaults_error`, `no_default_among_multiple_error`, the new `self_sufficient_overrides_ignore_profile_validate_certificate` unit test, and the new task-3.9 test below; rename the "Read auth falls back to anonymous" row per the TRACEABILITY_GAP fix.
  - plan.md § Design > Patterns: add a row naming `BfsConnection::effective_read_password` as the single owner of the write-to-read fallback.
  - plan.md prose fixes: Summary sentence 2, Design > Context final sentence of paragraph 1, Architecture paragraphs at lines 46 and 48 (PROSE_UNCLEAR finding — split/replace exactly as the finding specifies).
  - decision-log.md entry [4]: record that the merged connection loses the explicit-vs-derived read-password distinction, per the INFORMATION_LEAKAGE finding.
  - `cli/bucketfs-command-structure` spec delta § Background bullets 4-5: resolve the AMBIGUOUS_REQUIREMENT conflict exactly as the finding specifies. § Scenarios: extend "Self-sufficient overrides ignore the default profile" with `bfs_validate_certificate = false` and the new THEN clause; add the new `DELTA:NEW` scenario "Host-only run still fails when no default profile is set".
  - `config/profiles` spec delta: add the `DELTA:CHANGED` block for "Read auth falls back to write_password" per the REQUIREMENT_CONFLICT finding (out-of-scope username note).
- [x] 3.1 Add `bucketfs_without_host_or_profile_names_both_remedies` to `tests/cli_test.rs`, pointing `EXAPUMP_CONFIG` at a path that does not exist (round-2 review UNSTATED_ASSUMPTION fix).
- [x] 3.2 Add `bucketfs_host_override_without_config_skips_profile_error` to `tests/cli_test.rs`, using `--bfs-host 127.0.0.1 --bfs-port 1`, pointing `EXAPUMP_CONFIG` at a path that does not exist (round-2 review UNSTATED_ASSUMPTION fix), so the run fails at connect time, not at profile resolution.
- [x] 3.3 Add `overrides_only_lists_bucket_without_config` to `tests/bucketfs_test.rs`, pointing `EXAPUMP_CONFIG` at a path that does not exist, passing `--bfs-validate-certificate false` (round-2 review UNSTATED_ASSUMPTION fix: the container's certificate is self-signed).
- [x] 3.4 Add `overrides_only_ignores_default_profile_bucket` to `tests/bucketfs_test.rs`, with a single profile that sets `bfs_bucket = "wrongbucket"` and `bfs_port = 9999`, passing `--bfs-validate-certificate false` (round-2 review UNSTATED_ASSUMPTION fix). The run passes neither `--bfs-bucket` nor `--bfs-port` and still reaches bucket `default` on port `2581`.
- [x] 3.5 Add `host_override_inherits_default_profile_password` to `tests/bucketfs_test.rs`.
- [x] 3.6 Add `named_profile_stays_base_with_overrides` to `tests/bucketfs_test.rs`, using the profile's `bfs_validate_certificate = false` as the discriminator against the defaults base.
- [x] 3.7 Add `bucketfs_host_override_reports_ambiguous_default_profile` to `tests/cli_test.rs`, pointing `EXAPUMP_CONFIG` at a config with two profiles that both set `default = true`. The run passes `--bfs-host` and no credential flag, MUST exit non-zero, and stderr MUST list both profile names.
- [x] 3.8 Add `default_profile_lists_bucket_without_profile_flag` to `tests/bucketfs_test.rs`, with a single profile that sets `bfs_write_password` and the working bucket parameters. The run passes no `--profile` flag and no `--bfs-*` flag and still lists the bucket.
- [x] 3.9 Add `bucketfs_host_override_reports_missing_default_profile` to `tests/cli_test.rs` (round-2 review COMPLETENESS_GAP fix): config with two profiles and no `default = true`, `--bfs-host` present, no credential flag, no `--profile`. MUST exit non-zero, stderr MUST suggest adding `default = true` to one profile.
- [x] 4.1 Update `docs/bucketfs.md`. Rewrite the "Parameter Resolution" section to state the base-selection branches from the (now six-branch) Architecture diagram, and note in "Connection Options" that `--bfs-host` plus a password flag needs no profile.
- [x] 4.2 Add a `CHANGELOG.md` entry under a new version heading, referencing `#46` and both behavior changes from the Impact section.

## Phase 3: Verification

- [x] 3.10 Run `cargo build`, `cargo test` (sandbox disabled, needs the Exasol container reachable on `localhost:2581`), `cargo clippy`, `cargo fmt --check`.
- [x] 3.11 Scenario coverage audit against plan.md § Verification > Scenario Coverage (as amended by 2.8).
- [x] 3.12 Manual verification per plan.md § Verification > Manual Testing.

## Phase 4: Review Fixes

- [x] 4.3 [expert] Change the `read_password` field initialiser in `Profile::resolve_bfs_connection` (src/config.rs) to `read_password: self.bfs_read_password.clone(),`, so `BfsConnection::effective_read_password` is the single owner of the write-to-read fallback ([INFORMATION_LEAKAGE]). Write `write_password_flag_replaces_the_profile_derived_read_credential` in src/commands/bucketfs.rs first and show it failing. Then change `resolve_bfs_connection_read_password_falls_back_to_write_password` (src/config.rs) to assert `conn.effective_read_password()` and rename it to `resolve_bfs_connection_read_credential_falls_back_to_write_password`. Then replace the `**Known consequence:**` bullet in decision-log.md entry [4] with the corrected record.
- [x] 4.4 Rewrite the five BucketFS credential error messages in src/commands/bucketfs.rs (lines 90, 109, 144, 168, 199, 214) so each names the flag alongside the profile key ([CONTEXTLESS_ERROR]). Add no new test.
- [x] 4.5 Delete the test `client_reads_with_the_write_password_when_no_read_password_is_set` from src/commands/bucketfs.rs and drop `BucketFsClient` from the `tests` module import if it becomes unused ([IMPLEMENTATION_COUPLED_TEST]).
- [x] 4.6 Delete the test `bfs_connection_defaults_use_port_2581_and_bucket_default` from src/config.rs ([DUPLICATE_TEST]).
