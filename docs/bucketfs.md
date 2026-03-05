# BucketFS

[BucketFS](https://docs.exasol.com/db/latest/administration/on-premise/bucketfs/bucketfs.htm) is Exasol's built-in file system for storing UDF libraries, models, and other artifacts accessible from within the database.

exapump provides three subcommands to interact with BucketFS: `ls`, `cp`, and `rm`.

## Subcommands

### ls — List files

```bash
# List top-level entries in the default bucket
exapump bucketfs ls

# List a specific directory
exapump bucketfs ls my/path

# List recursively
exapump bucketfs ls -r
exapump bucketfs ls my/path --recursive
```

### cp — Copy files to/from BucketFS

Direction is auto-detected: if the source is a local file, exapump uploads; otherwise it downloads.

```bash
# Upload a local file
exapump bucketfs cp model.pkl models/model.pkl

# Upload, keeping the original filename
exapump bucketfs cp model.pkl models/

# Download a file from BucketFS
exapump bucketfs cp models/model.pkl ./local-copy.pkl
```

### rm — Delete a file

```bash
exapump bucketfs rm models/model.pkl
```

## Connection Options

BucketFS commands resolve their connection from your profile, with optional CLI overrides:

| Flag | Description |
|------|-------------|
| `--profile` | Connection profile name |
| `--bfs-host` | BucketFS host override |
| `--bfs-port` | BucketFS port override |
| `--bfs-bucket` | BucketFS bucket override |
| `--bfs-write-password` | BucketFS write password override |
| `--bfs-read-password` | BucketFS read password override |
| `--bfs-tls` | BucketFS TLS override |
| `--bfs-validate-certificate` | BucketFS certificate validation override |

## Profile Fields

BucketFS settings can be stored in your profile (`~/.exapump/config.toml`):

| Field | Default | Description |
|-------|---------|-------------|
| `bfs_host` | same as `host` | BucketFS hostname (falls back to the profile's `host`) |
| `bfs_port` | `2581` | BucketFS port |
| `bfs_bucket` | `default` | Bucket name |
| `bfs_write_password` | — | Password for write operations |
| `bfs_read_password` | falls back to `bfs_write_password` | Password for read operations |
| `bfs_tls` | same as `tls` | Enable TLS (falls back to the profile's `tls`) |
| `bfs_validate_certificate` | same as `validate_certificate` | Validate TLS certificate (falls back to the profile's `validate_certificate`) |

Example profile with BucketFS fields:

```toml
[production]
host = "exasol-prod.example.com"
user = "admin"
password = "s3cret"
default = true
bfs_write_password = "bucketpw"
bfs_read_password = "bucketpw"
```

## Parameter Resolution

Parameters are resolved in this order (highest to lowest priority):

1. CLI flags (e.g. `--bfs-host`)
2. Profile fields (e.g. `bfs_host`)
3. Smart defaults (e.g. `bfs_host` falls back to `host`, `bfs_port` defaults to `2581`)

## Authentication

BucketFS uses HTTP Basic authentication with role-based usernames:

- **Read operations** (`ls`, `cp` download): authenticated as user `r` with the read password
- **Write operations** (`cp` upload, `rm`): authenticated as user `w` with the write password

If `bfs_read_password` is not set, it falls back to `bfs_write_password`.
