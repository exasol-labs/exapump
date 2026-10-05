# Decisions: bucketfs-profile-override-fallback

## ADR: A host plus any BucketFS credential makes the overrides self-sufficient

**ID:** bucketfs-host-plus-credential-self-sufficient
**Plan:** bucketfs-profile-override-fallback
**Status:** Accepted

### Context

`exapump bucketfs` fails without a config file even when `--bfs-*` overrides specify the whole connection (issue #46).

### Decision

With no `--profile`, exapump skips the config when `--bfs-host` and at least one of `--bfs-write-password` or `--bfs-read-password` are given.

### Options Considered

| Option | Verdict |
|--------|---------|
| Host plus any BucketFS credential | ✓ Chosen — fixes the reported case and breaks nothing |
| Host plus write password only | ✗ Rejected — `ls` and download need only a read credential |
| Host alone | ✗ Rejected — breaks callers that inherit a shared cluster password from the default profile |

### Consequences

A caller that relies on a default profile for the port, bucket, TLS, or certificate validation must pass `--profile` or the matching `--bfs-*` flag.
