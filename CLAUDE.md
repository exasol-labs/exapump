# CLAUDE.md

## Testing

Integration and manual tests must always run against a local Exasol Docker database.

- **Docker image:** `exasol/docker-db:2025.2.0` on port `8563`
- **Start command:**
  ```sh
  docker run -d --name exasol-test --privileged --shm-size=2g -p 8563:8563 exasol/docker-db:2025.2.0
  ```
- **DSN must use:** `tls=true&validateservercertificate=0`
- Tests must **fail** (not skip) if Exasol is unavailable

## Licenses

When adding a new dependency, check its license. If the license is not already in the allowed lists, add it to **both** `deny.toml` (`[licenses].allow`) and `about.toml` (`accepted`). These two files must stay in sync.
