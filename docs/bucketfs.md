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

BucketFS commands resolve their connection from your profile, with optional CLI overrides. A profile is not required: `--bfs-host` together with `--bfs-write-password` or `--bfs-read-password` fully specifies a connection, and the command then reads no profile at all. This works on a machine that has no `~/.exapump/config.toml`.

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

Resolution runs in two steps. exapump first picks a base connection, then applies the `--bfs-*` flags on top of it.

The config file is a fallback value source, not a precondition. The first rule below that matches picks the base:

1. `--profile <name>` selects that profile. A name that is not in the config is an error.
2. `--bfs-host` together with `--bfs-write-password` or `--bfs-read-password` selects the BucketFS defaults. exapump reads no profile in this case.
3. The default profile is the base when one resolves, whether or not `--bfs-host` is given.
4. `--bfs-host` alone selects the BucketFS defaults when the config holds no profiles.
5. `--bfs-host` alone with a config that holds profiles but resolves no default reports the profile error, so an ambiguous or missing `default = true` stays visible.
6. Without `--bfs-host` and without a resolvable profile, the command fails and names both remedies: pass `--bfs-host`, or create a profile with `exapump profile add`.

The BucketFS defaults are port `2581`, bucket `default`, TLS on, and certificate validation on. Against a BucketFS with a self-signed certificate, add `--bfs-validate-certificate false`.

A `--bfs-*` flag always outranks the base. A field that neither the flags nor the base supplies takes its default: a profile base falls back to `host`, `tls`, and `validate_certificate` from the profile's database fields, then to the BucketFS defaults.

## Authentication

BucketFS uses HTTP Basic authentication with role-based usernames:

- **Read operations** (`ls`, `cp` download): authenticated as user `r` with the read password
- **Write operations** (`cp` upload, `rm`): authenticated as user `w` with the write password

If no read password is set, the write password serves as the read credential. This applies to the profile fields and to the flags: `--bfs-write-password` supplies the read credential when neither `--bfs-read-password` nor a profile read password is set. A read against a public bucket with no password set at all is still sent without authentication.
