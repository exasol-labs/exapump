# Configuration

## Connection Profiles

exapump stores connection profiles in `~/.exapump/config.toml`. Each TOML section is a named profile:

```toml
[local]
host = "localhost"
port = 8563
user = "sys"
password = "exasol"
tls = true
validate_certificate = false

[production]
default = true
host = "exasol-prod.example.com"
port = 8563
user = "admin"
password = "s3cret"
schema = "my_schema"
tls = true
validate_certificate = true
bfs_write_password = "bucketpw"
bfs_read_password = "bucketpw"
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
| `default` | No | — | Mark this profile as the default (see below) |
| `bfs_host` | No | same as `host` | BucketFS hostname |
| `bfs_port` | No | `2581` | BucketFS port |
| `bfs_bucket` | No | `default` | BucketFS bucket name |
| `bfs_write_password` | No | — | BucketFS write password |
| `bfs_read_password` | No | falls back to `bfs_write_password` | BucketFS read password |
| `bfs_tls` | No | same as `tls` | Enable TLS for BucketFS |
| `bfs_validate_certificate` | No | same as `validate_certificate` | Validate BucketFS TLS certificate |

## Default Profile

If your config has a single profile, it is automatically used as the default — no extra configuration needed.

With multiple profiles, mark exactly one as the default:

```toml
[production]
host = "exasol-prod.example.com"
user = "admin"
password = "s3cret"
default = true
```

Setting `default = true` on more than one profile is an error.

## Resolution Priority

When resolving a connection, exapump checks (highest to lowest):

1. `--dsn` CLI flag
2. `EXAPUMP_DSN` environment variable (shell or `.env` file)
3. `--profile <name>` flag (named profile from config file)
4. Single profile in config (auto-default)
5. Profile with `default = true`

If none are available, the CLI exits with an error.

### Selecting a Profile

Use `--profile` (or `-p`) on any command to target a specific profile:

```bash
exapump sql -p production 'SELECT 1'
exapump upload data.csv --table t --profile staging
exapump bucketfs ls --profile production
```

## Profile Management

```bash
# Add a profile with Docker presets
exapump profile add local

# Add a custom profile and mark it as the default
exapump profile add production \
  --host exasol-prod.example.com \
  --user admin \
  --password s3cret \
  --schema my_schema \
  --default

# List all profiles ((default) marks the active default)
exapump profile list

# Show profile details (password masked)
exapump profile show production

# Remove a profile
exapump profile remove production
```

## Docker Presets

Running `exapump profile add <name>` without connection flags creates a profile pre-configured for the standard Exasol Docker container:

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
