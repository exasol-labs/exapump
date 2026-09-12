# Feature: BucketFS Profile Metadata

`exapump profile add` and `exapump profile show` manage the BucketFS fields alongside the database fields on a profile (see `bucketfs/profile-integration` for how those fields resolve a connection). A `bfss://` URI in `bucketfs cp` can also force TLS independently of profile or flag state.

## Background

## Scenarios

### Scenario: Profile add includes BucketFS fields

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump profile add myprofile --host h --user u --password p --bfs-write-password w`
* *THEN* the profile MUST be saved with both database and BucketFS fields

### Scenario: Profile show displays BucketFS fields

* *GIVEN* a profile exists with BucketFS fields
* *WHEN* the user runs `exapump profile show <name>`
* *THEN* the output MUST include the BucketFS fields that are set
* *AND* the `bfs_write_password` and `bfs_read_password` MUST be masked (shown as `***`)

### Scenario: Docker preset excludes BucketFS

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump profile add docker`
* *THEN* the docker preset MUST NOT include BucketFS fields
* *AND* the profile MUST still be valid for database operations

### Scenario: bfss:// URI activates TLS when --bfs-tls is not set

* *GIVEN* a profile with `bfs_tls = false`
* *AND* the `--bfs-tls` flag is NOT provided on the command line
* *WHEN* a `bfss://` URI is present as source or destination in `bucketfs cp`
* *THEN* the resolved BucketFS connection MUST use TLS regardless of the profile `bfs_tls` value

### Scenario: Explicit --bfs-tls false suppresses bfss:// TLS inference

* *GIVEN* a `bfss://` URI is provided as source or destination in `bucketfs cp`
* *WHEN* the user explicitly passes `--bfs-tls false`
* *THEN* the resolved BucketFS connection MUST NOT use TLS
* *AND* the explicit CLI flag MUST take precedence over URI-based TLS inference
