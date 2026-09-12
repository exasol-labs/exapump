# Feature: BucketFS Profile Integration

Connection profiles optionally include BucketFS connection parameters alongside the Exasol database fields (see `config/profiles`). BucketFS reuses the profile's `host`, `tls`, and `validate_certificate` fields by default. Only BucketFS-specific overrides and credentials need to be added explicitly.

A BucketFS connection does not require a profile. When command-line overrides supply a host and a BucketFS credential, exapump builds the connection from those overrides and the BucketFS defaults. The BucketFS defaults are port `2581`, bucket `default`, TLS on, and certificate validation on. `default` is Exasol's standard bucket name.

## Background

## Scenarios

### Scenario: BucketFS host falls back to profile host

* *GIVEN* a profile with `host = "myhost"` and no `bfs_host` field
* *WHEN* a BucketFS command resolves connection parameters from the profile
* *THEN* the BucketFS host MUST be `myhost`

### Scenario: BucketFS host override

* *GIVEN* a profile with `host = "dbhost"` and `bfs_host = "bfshost"`
* *WHEN* a BucketFS command resolves connection parameters from the profile
* *THEN* the BucketFS host MUST be `bfshost`

### Scenario: BucketFS bucket defaults to default

* *GIVEN* a profile with no `bfs_bucket` field
* *AND* no `--bfs-bucket` flag is provided
* *WHEN* a BucketFS command resolves connection parameters
* *THEN* the bucket MUST default to `default`

### Scenario: BucketFS bucket override

* *GIVEN* a profile with `bfs_bucket = "custom"`
* *WHEN* a BucketFS command resolves connection parameters from the profile
* *THEN* the bucket MUST be `custom`

### Scenario: BucketFS TLS falls back to profile TLS

* *GIVEN* a profile with `tls = true` and no `bfs_tls` field
* *WHEN* a BucketFS command resolves connection parameters from the profile
* *THEN* the BucketFS connection MUST use TLS

### Scenario: BucketFS TLS override

* *GIVEN* a profile with `tls = true` and `bfs_tls = false`
* *WHEN* a BucketFS command resolves connection parameters from the profile
* *THEN* the BucketFS connection MUST NOT use TLS

### Scenario: BucketFS validate_certificate falls back to profile

* *GIVEN* a profile with `validate_certificate = false` and no `bfs_validate_certificate` field
* *WHEN* a BucketFS command resolves connection parameters from the profile
* *THEN* TLS certificate validation for BucketFS MUST be skipped

### Scenario: BucketFS validate_certificate override

* *GIVEN* a profile with `validate_certificate = false` and `bfs_validate_certificate = true`
* *WHEN* a BucketFS command resolves connection parameters from the profile
* *THEN* TLS certificate validation for BucketFS MUST be enabled

### Scenario: Profile with BucketFS builds connection URL

* *GIVEN* a profile with `host = "myhost"`, `tls = true`, `bfs_write_password = "write"`
* *AND* no `bfs_host`, `bfs_port`, or `bfs_bucket` overrides
* *WHEN* a BucketFS command resolves connection parameters from the profile
* *THEN* it MUST connect to `https://myhost:2581/default/`

### Scenario: BucketFS port defaults in profile

* *GIVEN* a profile with `host` set but no `bfs_port`
* *WHEN* the BucketFS connection is resolved
* *THEN* the port MUST default to `2581`
