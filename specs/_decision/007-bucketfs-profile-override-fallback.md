# Decisions: bucketfs-profile-override-fallback

## ADR: A host plus any BucketFS credential makes the overrides self-sufficient

**ID:** bucketfs-host-plus-credential-self-sufficient
**Plan:** bucketfs-profile-override-fallback
**Status:** Accepted

### Context

`exapump bucketfs <ls|cp|rm>` resolved a base profile before it applied any `--bfs-*` override. Without `--profile`, it called `config::find_default_profile`, which failed with `No profiles found in config` on a machine with no `~/.exapump/config.toml`, even when the overrides alone fully specified the connection. `lakehouse-engine-rs` `deploy/scripts/install.sh` documents the invocation `--bfs-host` plus `--bfs-write-password` with no `--profile`, and exapump issue #46 reports it broken.

### Decision

`base_connection` skips the config when `--profile` is absent, `--bfs-host` is present, and at least one of `--bfs-write-password` or `--bfs-read-password` is present. The host is the only BucketFS field with no default, and a credential is the only other field a caller cannot get from a default.

### Options Considered

| Option | Verdict |
|--------|---------|
| Host plus any BucketFS credential (`--bfs-write-password` or `--bfs-read-password`) is self-sufficient | ✓ Chosen — keeps the rule narrow enough to break nothing and wide enough to fix the reported case |
| Host plus write password only, as issue #46 words it | ✗ Rejected — makes `--bfs-read-password` behave differently from `--bfs-write-password` for no stated reason; `ls` and `cp` download need a read credential, not a write one |
| Host alone | ✗ Rejected — a multi-node Exasol cluster shares one BucketFS password across its nodes, and callers rely on inheriting it via `--bfs-host node2` with no `--profile`; dropping that inheritance would break working commands |

### Consequences

An override-only `exapump bucketfs` invocation succeeds on a machine with no config file, when the caller supplies a host and a read or write BucketFS credential. A host-only invocation with no credential still inherits BucketFS settings from a resolvable default profile, so a multi-node cluster's shared password keeps working. A caller that relied on a default profile supplying `bfs_port`, `bfs_bucket`, `bfs_tls`, or `bfs_validate_certificate` alongside `--bfs-host` plus a credential must now pass `--profile <name>` or the matching `--bfs-*` flag explicitly.
