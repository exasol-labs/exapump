# CLAUDE.md

## Testing

Integration and manual tests must always run against a local Exasol Docker database.

- **Docker image:** `exasol/docker-db:2025.2.0` on port `8563`
- **Start command:**
  ```sh
  docker run -d --name exasol-test --privileged --shm-size=2g -p 8563:8563 -p 2581:2581 exasol/docker-db:2025.2.0
  ```
- **DSN must use:** `tls=true&validateservercertificate=0`
- Tests must **fail** (not skip) if Exasol is unavailable

### Sandbox and localhost

Any Bash command that opens a TCP connection to `localhost` / `127.0.0.1` (e.g. `cargo test`, `nc -zv localhost …`, `exapump sql --dsn 'exasol://…@localhost:…'`, `curl http://localhost:…`) must be run with `dangerouslyDisableSandbox: true`. The global sandbox allowlist includes `localhost`/`127.0.0.1` but in practice the macOS sandbox blocks localhost TCP from subprocesses with "Operation not permitted" — the allowlist does not cover this path. Commands that only touch the filesystem or cargo's build cache must stay sandboxed as usual.

## Licenses

When adding a new dependency, check its license. If the license is not already in the allowed lists, add it to **both** `deny.toml` (`[licenses].allow`) and `about.toml` (`accepted`). These two files must stay in sync.
