# Decision Log: bucketfs-profile-override-fallback

## Interview

No live interview ran. The orchestrator started this plan in headless mode and passed exapump issue #46 as the full and final requirement set. Every decision below is a headless assumption, recorded per the planning skill's headless rule.

**Q:** What is the defect?
**A:** From issue #46. `exapump bucketfs <ls|cp|rm>` always resolves a base profile before it applies `--bfs-*` overrides. Without `--profile`, `src/commands/bucketfs.rs::run` calls `config::find_default_profile`, which fails with `No profiles found in config` on a machine with no `~/.exapump/config.toml`, even when the overrides alone fully specify the connection.

**Q:** Who is affected?
**A:** From issue #46. Any tool that shells out to `exapump bucketfs` with pure `--bfs-*` overrides on a fresh machine. `lakehouse-engine-rs` `deploy/scripts/install.sh` documents its `--host`/`--dsn` connectivity mode as usable with `--bfs-host` plus `--bfs-write-password`, and never passes `--profile` in that mode.

**Q:** Is there a second defect?
**A:** From issue #46. Yes. When an unrelated default profile exists, `find_default_profile` picks it as the base and merges the overrides on top. Fields the caller did not override, such as `tls` and `validate_certificate`, silently inherit from that unrelated profile.

**Q:** What fix does the issue suggest?
**A:** From issue #46. Skip `find_default_profile` when `--profile` is absent and the overrides fully specify the connection (host plus write password at minimum). Build from overrides plus hardcoded defaults: port `2581`, bucket `default`, TLS on, certificate validation on. Fall back to `find_default_profile` only when the overrides are incomplete.

**Q:** What is the scope of this run?
**A:** Plan only. Produce spec deltas, `plan.md`, and `decision-log.md`. Write no implementation code.

## Design Decisions

### [1] A host plus any BucketFS credential makes the overrides self-sufficient

- **Decision:** `base_connection` skips the config when `--profile` is absent, `--bfs-host` is present, and at least one of `--bfs-write-password` or `--bfs-read-password` is present.
- **Alternatives:**
  - *Host plus write password only*, as issue #46 words it. Rejected. It makes `--bfs-read-password` behave differently from `--bfs-write-password` for no stated reason. `ls` and `cp` download need a read credential, not a write one.
  - *Host alone*. Rejected. Today a caller can pass `--bfs-host node2` with no `--profile` and inherit the default profile's BucketFS password. A multi-node Exasol cluster shares one BucketFS password across its nodes, so that inheritance is a real use. Dropping it would break working commands.
- **Rationale:** The host is the only BucketFS field with no default. A credential is the only other field a caller cannot get from a default. Requiring exactly those two fields keeps the rule narrow enough to break nothing and wide enough to fix the reported case.
- **Promotes to ADR:** yes

### [2] A host-only run falls back to the defaults only when the config holds zero profiles

- **Decision:** When `--profile` is absent, `--bfs-host` is present, no credential flag is present, and `find_default_profile` fails because the config holds zero profiles, `base_connection` returns the defaults base instead of the error. Every other `find_default_profile` failure propagates.
- **Alternatives:**
  - *Report the `find_default_profile` error, as today.* Rejected. It leaves `exapump bucketfs ls --bfs-host h` against a public bucket broken on a fresh machine, which is the same defect class issue #46 reports.
  - *Fall back on any `find_default_profile` failure.* Rejected after round 1 of plan review. `find_default_profile` fails for three distinct reasons: the config holds zero profiles, the config holds two or more profiles and none is marked `default = true`, and the config holds two or more profiles marked `default = true`. Only the first is a missing config. The other two are misconfigurations that the recorded scenarios `config/profiles/Multiple default profiles is an error` and `config/profiles/Multiple profiles without default is an error` require exapump to report by name.
- **Rationale:** A config with zero profiles is the fresh-machine case issue #46 reports, and no working invocation exists there, so replacing the error with the defaults base breaks nothing. The two misconfiguration cases are different: a silent fallback there would connect to the wrong bucket and hide the conflict the user has to fix. The spec `config/profiles` already allows an anonymous read of a public bucket, so a connection with a host and no credential is valid.
- **Promotes to ADR:** no

### [3] The BucketFS defaults get one owner in `src/config.rs`

- **Decision:** Add `BfsConnection::with_defaults(host: String)` and `DEFAULT_BFS_BUCKET` to `src/config.rs`. Rewrite `Profile::resolve_bfs_connection` to read its defaults from that constructor.
- **Alternatives:** Restate `2581`, `"default"`, and `true` inside `src/commands/bucketfs.rs`. Rejected. The same default values would then live in two modules, and a change to one would silently diverge from the other.
- **Rationale:** Port `2581`, bucket `default`, TLS on, and certificate validation on are one decision. Per `/speq:design-philosophy`, one module owns it. The constructor takes the host as a required argument, so the type states the invariant that a host has no default.
- **Promotes to ADR:** no

### [4] `--bfs-write-password` supplies the read credential as a last resort

- **Decision:** `resolve_connection` resolves the read credential as `--bfs-read-password`, then the base read password, then `--bfs-write-password`.
- **Alternatives:**
  - *Leave the read credential unset.* Rejected. The repro command in issue #46 is `ls`, which reads. With a defaults base and only `--bfs-write-password`, the request would carry no credential and fail on a non-public bucket. The reported fix would not fix the reported command.
  - *Put `--bfs-write-password` ahead of the base read password.* Rejected. A profile with an explicit `bfs_read_password` would then lose it whenever the caller passed `--bfs-write-password`, which changes behavior for callers this plan does not target.
- **Rationale:** The chosen order only adds a value where the resolved read password is currently `None`, so no existing resolution changes except an anonymous read that now authenticates. `Profile::resolve_bfs_connection` already applies the same write-to-read fallback to the profile fields, so the flags now match the fields.
- **Promotes to ADR:** no

### [5] The failure with no host and no profile names both remedies

- **Decision:** `base_connection` attaches BucketFS guidance to the `find_default_profile` error. The message names `--bfs-host` and `exapump profile add`, and keeps the original error as the cause.
- **Alternatives:** Leave the bare `No profiles found in config` message. Rejected. A user who hits this path has no way to learn that the override-only path exists, which is the discoverability half of issue #46.
- **Rationale:** `find_default_profile` is shared with the database commands, so its own message must stay generic. The BucketFS-specific hint belongs at the BucketFS call site. `anyhow` prints both the hint and the cause, so a caller that greps for `No profiles found in config` still matches.
- **Promotes to ADR:** no

### [6] The config file is read once, as today

- **Decision:** `run` keeps its single `config::load_config()` call and passes the loaded `Config` to `base_connection`.
- **Alternatives:** Skip the read entirely when the overrides are self-sufficient, so a malformed `~/.exapump/config.toml` cannot break an override-only run. Rejected for this plan.
- **Rationale:** A malformed config file already fails every other exapump command. Making `bucketfs` the one exception would need its own spec scenario and its own test, and issue #46 does not report it. Keeping the read outside `base_connection` also keeps that function pure, which is what makes every branch unit-testable.
- **Promotes to ADR:** no

### [7] Both spec deltas ship as one implementation group

- **Decision:** One parallelization group covers both deltas, carrying the `[expert]` tag from task 2.2.
- **Alternatives:** Split the code work from the documentation work to keep `docs/bucketfs.md` and `CHANGELOG.md` off the expert model. Rejected. That split slices by layer, not by knowledge, and the documentation restates the same precedence rule the code implements.
- **Rationale:** `src/config.rs` and `src/commands/bucketfs.rs` implement both deltas. Two groups would carry overlapping `Knowledge` entries, which the planning skill names as a consolidation signal.
- **Promotes to ADR:** no

### [8] Out-of-scope observation: the read Basic-auth username

- **Decision:** Do not change the Basic-auth username that `BucketFsClient` sends on reads.
- **Alternatives:** Send `w` instead of `r` when the read credential came from a write password. Not attempted.
- **Rationale:** `BucketFsClient::list_bucket` and `BucketFsClient::download` hardcode the username `r`. The recorded scenario `config/profiles/Read auth falls back to write_password` states that the request MUST use `w:wp`. Spec and code disagree today. Resolving that needs a check against a BucketFS instance whose read and write passwords differ, which issue #46 does not cover. This plan's new scenarios assert the resolved read password value and say nothing about the username, so they add no conflict.
- **Promotes to ADR:** no

## Review Findings

### [plan-review] Branch 4 swallowed two recorded misconfiguration errors

- **Finding:** `plan-reviewer` round 1, `[REQUIREMENT_CONFLICT]` BLOCKER. Branch 4 of the Architecture diagram routed every `find_default_profile` failure to `BfsConnection::with_defaults(host)`. `find_default_profile` fails for three distinct reasons, and two of them are misconfigurations that the recorded scenarios `config/profiles/Multiple default profiles is an error` and `config/profiles/Multiple profiles without default is an error` require exapump to report by name. The plan deleted those diagnostics. A user with two `default = true` profiles running `exapump bucketfs ls --bfs-host node2` would have listed the wrong bucket with no warning.
- **Direction change:** Branch 4 now reads `host, config holds zero profiles`. The paragraph below the diagram names the three `find_default_profile` failures and states that the two misconfiguration failures propagate, unchanged when `--bfs-host` is present and wrapped with the both-remedies guidance when it is absent. Bullet 4 of the `cli/bucketfs-command-structure` § Background delta carries the same narrowing. A `DELTA:NEW` scenario "Ambiguous default profile still fails a host-only run" pins the behavior, task 3.7 adds `bucketfs_host_override_reports_ambiguous_default_profile` to `tests/cli_test.rs`, and § Verification > Scenario Coverage carries the row. A `DELTA:CHANGED` block in the `config/profiles` delta scopes both recorded scenarios' WHEN clause to "resolves connection parameters from the config". Task 2.5 states which errors get the guidance and which propagate unchanged. Design decision [2] and the matching Consequences row record the narrowed condition.
- **Promotes to ADR:** no

### [plan-review] The diagram had no branch for today's default invocation

- **Finding:** `plan-reviewer` round 1, `[COMPLETENESS_GAP]` BLOCKER. The five-branch diagram matched no case for "no `--bfs-host`, no `--profile`, and a default profile resolves", which is how `exapump bucketfs ls` runs today. Task 2.2 instructs the implementer to follow the diagram, so a literal reading routed that invocation to the branch-5 error. No test guarded it: every test in `tests/bucketfs_test.rs` passes `--profile bfs`.
- **Direction change:** Branch 3 now reads `default profile resolves (with or without host)` and branch 5 reads `no default profile and no host`. A sentence below the diagram states that a run with no flag and a resolvable default profile keeps today's behavior. Bullet 3 of the `cli/bucketfs-command-structure` § Background delta says the same. A `DELTA:NEW` scenario "Default profile stays the base when no override is given" pins it, task 3.8 adds `default_profile_lists_bucket_without_profile_flag` to `tests/bucketfs_test.rs`, and § Verification > Scenario Coverage carries the row.
- **Promotes to ADR:** no
