# AGENTS.md

Spec-driven development with mission in: @specs/mission.md

## Testing

- Integration and E2E tests run against a local Exasol Docker database. Start the container yourself, do not ask the user.
- Tests must fail, not skip, when Exasol is unavailable.
- Connection strings must set `validateservercertificate=0`, because the Docker image uses a self-signed certificate.

Project specifics:

- Image: `exasol/docker-db:2025.2.0` on port `8563`.
- Start command: `docker run -d --name exasol-test --privileged --shm-size=2g -p 8563:8563 -p 2581:2581 exasol/docker-db:2025.2.0`
- The DSN uses `tls=true&validateservercertificate=0`.
- Any Bash command that opens a TCP connection to `localhost` or `127.0.0.1` (`cargo test`, `curl`, `exapump sql --dsn 'exasol://…@localhost:…'`) must run with `dangerouslyDisableSandbox: true`. The macOS sandbox blocks localhost TCP from subprocesses even though the allowlist names it. Commands that only touch the filesystem or the cargo build cache stay sandboxed.

## Code quality

- `cargo fmt --all` and `cargo clippy --all-targets` must pass with zero warnings before committing.

## Licenses

A new dependency's license must appear in both `deny.toml` (`[licenses].allow`) and `about.toml` (`accepted`). Keep the two files in sync.
