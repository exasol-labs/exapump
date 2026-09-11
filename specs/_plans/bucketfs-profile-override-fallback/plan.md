# Plan: bucketfs-profile-override-fallback

## Summary

`exapump bucketfs <ls|cp|rm>` treats `~/.exapump/config.toml` as a fallback value source instead of a precondition. A caller that names a BucketFS host on the command line runs on a machine with no config file.

## Design

### Context

`src/commands/bucketfs.rs::run` resolves a base profile before it applies any `--bfs-*` override. Without `--profile`, it calls `config::find_default_profile`, which fails with `No profiles found in config` when the config file is absent. The `--bfs-*` flags never get a chance to stand on their own.

Two defects follow from that order:

1. On a machine with no `~/.exapump/config.toml`, `exapump bucketfs ls --bfs-host h --bfs-port p --bfs-write-password pw` exits non-zero. `lakehouse-engine-rs` `deploy/scripts/install.sh` documents this exact invocation for its `--host`/`--dsn` connectivity mode.
2. On a machine that has an unrelated default profile, that profile silently supplies `bucket`, `tls`, `validate_certificate`, and `port`. The same command then behaves differently on two machines.

- **Goals**: make an override-only BucketFS invocation succeed without a config file, and make it resolve to the same connection on every machine.
- **Non-Goals**: no change to database connection resolution (`--dsn`, `EXAPUMP_DSN`, `src/connection.rs`), no change to `find_default_profile` itself, no change to how a malformed config file fails, no change to the Basic-auth username that `BucketFsClient` sends.

### Decision

Replace the inline profile lookup in `run` with one pure function that picks the base connection. The config is read once, as today, and consulted only when the overrides leave a gap.

#### Architecture

```
exapump bucketfs <ls|cp|rm>
        │
        ▼
  base_connection(&Config, &BfsConnectionOverrides)     src/commands/bucketfs.rs
        │
        ├─ --profile <name>                                    ──▶ Profile::resolve_bfs_connection()
        ├─ host + BucketFS credential                          ──▶ BfsConnection::with_defaults(host)
        ├─ default profile resolves (with or without host)     ──▶ Profile::resolve_bfs_connection()
        ├─ host, config holds zero profiles                    ──▶ BfsConnection::with_defaults(host)
        └─ no default profile and no host                      ──▶ error naming both remedies
        │
        ▼
  resolve_connection(&base, &overrides)                 src/commands/bucketfs.rs
        │
        ▼
  BucketFsClient::new(conn)
```

Branch order is the precedence rule. An explicit `--profile` always wins. A host plus a BucketFS credential makes the config irrelevant. Branch 3 covers two cases at once: a run that passes no `--bfs-*` flag at all and a run that passes `--bfs-host` without a credential. A run with no flag and a resolvable default profile therefore keeps today's behavior, and a host-only run keeps today's inheritance, because a multi-node Exasol cluster shares one BucketFS password across nodes.

Branch 4 fires only when `find_default_profile` fails because the config holds zero profiles. `find_default_profile` also fails for two other reasons: two or more profiles with none marked `default = true`, and more than one profile marked `default = true`. Both MUST propagate, so the diagnostics that the recorded `config/profiles` scenarios require still reach stderr. With `--bfs-host` present they propagate unchanged. Without `--bfs-host`, branch 5 adds the both-remedies guidance and keeps the original error as the `anyhow` cause, so the named diagnostic still prints.

`BfsConnection::with_defaults(host)` takes the one field that has no default and fills the rest: port `2581`, bucket `default`, TLS on, certificate validation on, no credentials.

#### Patterns

| Pattern | Where | Why |
|---------|-------|-----|
| Single owner for the BucketFS defaults | `BfsConnection::with_defaults` in `src/config.rs` | Port `2581`, bucket `default`, and TLS on form one decision. `Profile::resolve_bfs_connection` and `base_connection` both read it from one place instead of restating the literals. |
| Pure resolution function | `base_connection` in `src/commands/bucketfs.rs` | The function performs no I/O and reads no ambient state, so a unit test covers every branch. |
| Config as fallback, not precondition | `base_connection` | The config supplies values the caller omitted. It never gates the command. |

### Consequences

| Decision | Alternatives Considered | Rationale |
|----------|------------------------|-----------|
| Treat a host plus any BucketFS credential as self-sufficient | Treat `--bfs-host` alone as self-sufficient. Treat host plus write password only. | Host alone drops the credential inheritance that a multi-node cluster relies on. Host plus write password only leaves `--bfs-read-password` asymmetric for no reason. |
| Fall back to the defaults when a host is given and the config holds zero profiles | Report the `find_default_profile` error. Fall back on any `find_default_profile` error. | This branch fires only when the config holds zero profiles, so it replaces one error with a working connection and leaves every other profile-resolution error intact. It also covers an override-only read of a public bucket. |
| Let `--bfs-write-password` supply the read credential | Leave the read credential unset | The repro command in issue #46 is `ls`, which reads. `Profile::resolve_bfs_connection` already applies this fallback to the profile fields. |
| Read the config once, as today | Skip the config read when the overrides are self-sufficient | A malformed config file already fails every other exapump command. One consistent failure mode beats one more branch. |
| Add a `Default` derive to `BfsConnectionOverrides` | Build all eight fields in every unit test | The struct holds eight `Option` fields. A derive keeps each unit test to the fields it exercises. |

## Features

| Feature | Status | Spec |
|---------|--------|------|
| cli/bucketfs-command-structure | CHANGED | `specs/_plans/bucketfs-profile-override-fallback/cli/bucketfs-command-structure/spec.md` |
| config/profiles | CHANGED | `specs/_plans/bucketfs-profile-override-fallback/config/profiles/spec.md` |

## Impact

`exapump bucketfs <ls|cp|rm>` works with `--bfs-*` flags on a machine that has no `~/.exapump/config.toml`. This fixes exapump issue #46 and unblocks `lakehouse-engine-rs` `deploy/scripts/install.sh`.

Two behavior changes affect existing callers:

1. **Breaking.** When the caller passes `--bfs-host` together with `--bfs-write-password` or `--bfs-read-password`, and passes no `--profile`, exapump no longer reads any profile. Values that a default profile previously supplied (`bfs_port`, `bfs_bucket`, `bfs_tls`, `bfs_validate_certificate`) now come from the BucketFS defaults. A caller that relied on that inheritance MUST add `--profile <name>` or the matching `--bfs-*` flag. Against a BucketFS with a self-signed certificate, that means adding `--bfs-validate-certificate false`.
2. `--bfs-write-password` now also serves as the read credential when neither `--bfs-read-password` nor a profile read password is set. A read that previously ran anonymously now authenticates.

Passing `--profile <name>` keeps today's behavior in every case.

The change makes the override-only path the documented way to run `bucketfs` without a config file. A password passed as `--bfs-write-password` is visible in the process list, as it is today. This plan adds no new credential path and no new credential store.

## Dependencies

No new crate, API, or upstream change. The production change touches `src/cli.rs`, `src/config.rs`, and `src/commands/bucketfs.rs` only.

Tasks 3.3 to 3.6 and task 3.8 need a running Exasol container with BucketFS reachable on `localhost:2581`. Start it before those tasks:

```sh
docker run -d --name exasol-test --privileged --shm-size=2g -p 8563:8563 -p 2581:2581 exasol/docker-db:2025.2.0
exapump wait --dsn 'exasol://sys:exasol@localhost:8563?tls=true&validateservercertificate=0'
```

Tasks 1.1 to 2.5, 3.1, 3.2, 3.7, 4.1, and 4.2 need no container.

## Implementation Tasks

- [ ] 1.1 Add `DEFAULT_BFS_BUCKET` and `BfsConnection::with_defaults(host: String) -> BfsConnection` to `src/config.rs`, with a doc comment stating that the host is the only BucketFS field without a default. Unit-test the returned port, bucket, TLS flag, certificate flag, and absent credentials.
- [ ] 1.2 Rewrite `Profile::resolve_bfs_connection` to take its defaults from `BfsConnection::with_defaults` instead of restating `2581`, `"default"`, and `true`. Keep every existing `resolve_bfs_connection_*` unit test green without editing it.
- [ ] 2.1 Add `#[derive(Default)]` to `BfsConnectionOverrides` in `src/cli.rs`.
- [ ] 2.2 Add `base_connection(config: &Config, overrides: &BfsConnectionOverrides) -> anyhow::Result<BfsConnection>` to `src/commands/bucketfs.rs`, implementing the five-branch precedence rule from the Architecture diagram. Write one unit test per branch first. [expert]
- [ ] 2.3 Extend `resolve_connection` so the read credential resolves as `--bfs-read-password`, then the base read password, then `--bfs-write-password`. Write the two ordering unit tests first. [expert]
- [ ] 2.4 Replace the inline profile match in `run` with a call to `base_connection`, dropping the `Profile` clone.
- [ ] 2.5 Attach the BucketFS guidance to the profile-resolution error in `base_connection` when no `--bfs-host` is given. The message names `--bfs-host` and `exapump profile add`, and keeps the original `find_default_profile` error as the cause. When `--bfs-host` is given and the config holds one or more profiles, the `find_default_profile` error propagates unchanged.
- [ ] 3.1 Add `bucketfs_without_host_or_profile_names_both_remedies` to `tests/cli_test.rs`.
- [ ] 3.2 Add `bucketfs_host_override_without_config_skips_profile_error` to `tests/cli_test.rs`, using `--bfs-host 127.0.0.1 --bfs-port 1` so the run fails at connect time, not at profile resolution.
- [ ] 3.3 Add `overrides_only_lists_bucket_without_config` to `tests/bucketfs_test.rs`, pointing `EXAPUMP_CONFIG` at a path that does not exist.
- [ ] 3.4 Add `overrides_only_ignores_default_profile_bucket` to `tests/bucketfs_test.rs`, with a single profile that sets `bfs_bucket = "wrongbucket"` and `bfs_port = 9999`. The run passes neither `--bfs-bucket` nor `--bfs-port` and still reaches bucket `default` on port `2581`.
- [ ] 3.5 Add `host_override_inherits_default_profile_password` to `tests/bucketfs_test.rs`.
- [ ] 3.6 Add `named_profile_stays_base_with_overrides` to `tests/bucketfs_test.rs`, using the profile's `bfs_validate_certificate = false` as the discriminator against the defaults base.
- [ ] 3.7 Add `bucketfs_host_override_reports_ambiguous_default_profile` to `tests/cli_test.rs`, pointing `EXAPUMP_CONFIG` at a config with two profiles that both set `default = true`. The run passes `--bfs-host` and no credential flag, MUST exit non-zero, and stderr MUST list both profile names.
- [ ] 3.8 Add `default_profile_lists_bucket_without_profile_flag` to `tests/bucketfs_test.rs`, with a single profile that sets `bfs_write_password` and the working bucket parameters. The run passes no `--profile` flag and no `--bfs-*` flag and still lists the bucket.
- [ ] 4.1 Update `docs/bucketfs.md`. Rewrite the "Parameter Resolution" section to state the base-selection branches from the Architecture diagram, and note in "Connection Options" that `--bfs-host` plus a password flag needs no profile.
- [ ] 4.2 Add a `CHANGELOG.md` entry under a new version heading, referencing `#46` and both behavior changes from the Impact section.

## Parallelization

| Group | Tasks | Depends on | Knowledge |
|-------|-------|------------|-----------|
| A: BucketFS connection base selection | 1.1-1.2, 2.1-2.5, 3.1-3.8, 4.1-4.2 | None | spec deltas `cli/bucketfs-command-structure`, `config/profiles`; `src/config.rs`, `src/cli.rs`, `src/commands/bucketfs.rs`, `tests/bucketfs_test.rs`, `tests/cli_test.rs`, `docs/bucketfs.md`, `CHANGELOG.md` |

One group. Both spec deltas are implemented by the same two source files, so splitting them would give two groups with overlapping `Knowledge` entries. Task 2.2 carries `[expert]`, so the group routes to `implementer-expert-agent`.

## Dead Code Removal

| Type | Location | Reason |
|------|----------|--------|
| Code block | `src/commands/bucketfs.rs`, the `match &overrides.profile` block in `run` | Replaced by `base_connection` |
| Expression | `src/commands/bucketfs.rs`, the `.clone()` on the resolved `Profile` in `run` | `base_connection` calls `resolve_bfs_connection` on a borrow |
| Literals | `src/config.rs`, the `2581`, `"default"`, and `true` defaults inside `Profile::resolve_bfs_connection` | Replaced by `BfsConnection::with_defaults` |

## References

- exapump issue #46, "bucketfs: `--bfs-host`/`--bfs-write-password` overrides still require a pre-existing profile": https://github.com/exasol-labs/exapump/issues/46

## Verification

### Scenario Coverage

| Scenario | Test Type | Test Location | Test Name |
|----------|-----------|---------------|-----------|
| cli/bucketfs-command-structure: BucketFS overrides work without a config file | Integration | `tests/bucketfs_test.rs` | `overrides_only_lists_bucket_without_config` |
| cli/bucketfs-command-structure: Self-sufficient overrides ignore the default profile | Integration | `tests/bucketfs_test.rs` | `overrides_only_ignores_default_profile_bucket` |
| cli/bucketfs-command-structure: Named profile stays the base when overrides are self-sufficient | Integration | `tests/bucketfs_test.rs` | `named_profile_stays_base_with_overrides` |
| cli/bucketfs-command-structure: Host override alone still inherits the default profile | Integration | `tests/bucketfs_test.rs` | `host_override_inherits_default_profile_password` |
| cli/bucketfs-command-structure: Host override alone works when no profile resolves | Integration | `tests/cli_test.rs` | `bucketfs_host_override_without_config_skips_profile_error` |
| cli/bucketfs-command-structure: Missing host and missing profile name both remedies | Integration | `tests/cli_test.rs` | `bucketfs_without_host_or_profile_names_both_remedies` |
| cli/bucketfs-command-structure: Ambiguous default profile still fails a host-only run | Integration | `tests/cli_test.rs` | `bucketfs_host_override_reports_ambiguous_default_profile` |
| cli/bucketfs-command-structure: Default profile stays the base when no override is given | Integration | `tests/bucketfs_test.rs` | `default_profile_lists_bucket_without_profile_flag` |
| config/profiles: BucketFS defaults apply when no profile supplies a base | Unit | `src/config.rs` | `bfs_connection_with_defaults_uses_standard_values` |
| config/profiles: Read credential falls back to the --bfs-write-password flag | Unit | `src/commands/bucketfs.rs` | `read_credential_falls_back_to_write_password_flag` |
| config/profiles: Configured read password outranks the write-password flag | Unit | `src/commands/bucketfs.rs` | `configured_read_password_outranks_write_password_flag` |
| config/profiles: Read auth falls back to anonymous on public bucket | Unit | `src/commands/bucketfs.rs` | `read_credential_stays_unset_without_any_password` |

Credential resolution and default selection are pure computation with no I/O, so they map to unit tests. Every scenario whose outcome a user observes on the command line maps to an integration test.

Task 2.2 adds one unit test per `base_connection` branch in `src/commands/bucketfs.rs`. Those tests back the integration tests above. They do not replace them.

### Manual Testing

Start the container first, then read the BucketFS write password from it:

```sh
docker run -d --name exasol-test --privileged --shm-size=2g -p 8563:8563 -p 2581:2581 exasol/docker-db:2025.2.0
exapump wait --dsn 'exasol://sys:exasol@localhost:8563?tls=true&validateservercertificate=0'
BFS_PW=$(docker exec exasol-test cat /exa/etc/EXAConf | grep WritePasswd | cut -d= -f2 | tr -d ' ' | base64 -d)
```

| Feature | Command | Expected Output |
|---------|---------|-----------------|
| cli/bucketfs-command-structure | `EXAPUMP_CONFIG=/tmp/no-such-config.toml exapump bucketfs ls --bfs-host localhost --bfs-port 2581 --bfs-write-password "$BFS_PW" --bfs-validate-certificate false` | Exit 0. Bucket entries on stdout. No `No profiles found in config` on stderr. |
| cli/bucketfs-command-structure | `EXAPUMP_CONFIG=/tmp/no-such-config.toml exapump bucketfs ls` | Exit 1. stderr names `--bfs-host` and `exapump profile add`, with `No profiles found in config` as the cause. |
| config/profiles | `EXAPUMP_CONFIG=/tmp/no-such-config.toml exapump bucketfs ls --bfs-host localhost --bfs-write-password "$BFS_PW" --bfs-validate-certificate false` | Exit 0. The run reaches port `2581` and bucket `default` without either flag. |
| config/profiles | `EXAPUMP_CONFIG=/tmp/no-such-config.toml exapump bucketfs cp /etc/hostname issue46.txt --bfs-host localhost --bfs-write-password "$BFS_PW" --bfs-validate-certificate false` | Exit 0. stderr prints `Uploaded /etc/hostname to issue46.txt`. |

### Checklist

Run `cargo test` with the sandbox disabled. The BucketFS tests open a TCP connection to `localhost:2581`.

| Step | Command | Expected |
|------|---------|----------|
| Build | `cargo build` | Exit 0 |
| Test | `cargo test` | 0 failures |
| Lint | `cargo clippy && cargo fmt --check` | 0 errors and 0 warnings |
| Format | `cargo fmt` | No changes |
