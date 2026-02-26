# Configuration

## Connection Profiles

exapump stores connection profiles in `~/.exapump/config.toml`. Each TOML section is a named profile:

```toml
[default]
host = "localhost"
port = 8563
user = "sys"
password = "exasol"
tls = true
validate_certificate = false

[production]
host = "exasol-prod.example.com"
port = 8563
user = "admin"
password = "s3cret"
schema = "my_schema"
tls = true
validate_certificate = true
```

### Profile Fields

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `host` | Yes | — | Exasol hostname or IP |
| `port` | No | `8563` | Exasol port |
| `user` | Yes | — | Database user |
| `password` | Yes | — | Database password |
| `schema` | No | — | Default schema |
| `tls` | No | `true` | Enable TLS |
| `validate_certificate` | No | `true` | Validate server TLS certificate |

## Resolution Priority

When resolving a connection, exapump checks (highest to lowest):

1. `--dsn` CLI flag
2. `EXAPUMP_DSN` environment variable (shell or `.env` file)
3. `--profile <name>` flag (named profile from config file)
4. `[default]` profile from config file

If none are available, the CLI exits with an error suggesting `exapump profile add default`.

## Profile Management

```bash
# Add the default profile (Docker presets)
exapump profile add default

# Add a custom profile
exapump profile add production \
  --host exasol-prod.example.com \
  --user admin \
  --password s3cret \
  --schema my_schema

# List all profiles (* marks auto-selected)
exapump profile list

# Show profile details (password masked)
exapump profile show production

# Remove a profile
exapump profile remove production
```

## Docker Presets

Running `exapump profile add default` without flags creates a profile pre-configured for the standard Exasol Docker container:

- `host = "localhost"`
- `port = 8563`
- `user = "sys"`
- `password = "exasol"`
- `tls = true`
- `validate_certificate = false`

## Profile Name Rules

Profile names must start with a letter or digit, followed by letters, digits, underscores, or hyphens. Valid: `default`, `my-docker`, `prod_eu`, `DB1`. Invalid: `-leading`, `has space`, empty string.

## Environment Variables

| Variable | Description |
|----------|-------------|
| `EXAPUMP_DSN` | Connection string (overrides config profiles) |
| `EXAPUMP_CONFIG` | Custom config file path (default: `~/.exapump/config.toml`) |

A `.env` file in the working directory is auto-loaded at startup.

## DSN Format

```
exasol://user:password@host:port/schema?tls=true&validateservercertificate=0
```

- `schema` — optional path component
- `tls` — `true` or `false`
- `validateservercertificate` — `1` (validate) or `0` (skip)
