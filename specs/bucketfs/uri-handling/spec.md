# Feature: BucketFS URI Handling

Parsing and normalisation of BucketFS path arguments that may arrive as plain in-bucket paths or as scheme-qualified URIs.

## Background

BucketFS paths may be given as plain in-bucket paths, `bfs://<bucket>/<path>` URIs (unsecured), or `bfss://<bucket>/<path>` URIs (TLS-secured). When a URI form is used, the scheme and bucket prefix MUST be stripped before composing the HTTP URL. A `bfss://` URI additionally infers that TLS should be used for the connection, unless the caller has explicitly overridden that setting via `--bfs-tls`.

## Scenarios

### Scenario: Upload accepts a bfs:// URI destination

* *GIVEN* a local file exists at `<source>` and valid BucketFS write parameters are provided for bucket `default`
* *WHEN* the user runs `exapump bucketfs cp <source> bfs://default/<path>`
* *THEN* the file MUST be uploaded to BucketFS at `<path>` within the bucket
* *AND* the HTTP request URL MUST NOT contain the `bfs://` prefix or repeat the bucket segment
* *AND* the upload MUST NOT fail with an HTTP 400 response and the exit code MUST be 0

### Scenario: Download accepts a bfs:// URI source

* *GIVEN* a file exists in BucketFS at `<path>` within bucket `default` and valid BucketFS read parameters are provided
* *WHEN* the user runs `exapump bucketfs cp bfs://default/<path> <local_destination>`
* *THEN* the file MUST be downloaded to the local destination path
* *AND* the HTTP request URL MUST NOT contain the `bfs://` prefix or repeat the bucket segment
* *AND* the downloaded content MUST match the original file content and the exit code MUST be 0

### Scenario: Plain destination path without bfs:// prefix still works

* *GIVEN* a local file exists at `<source>` and valid BucketFS write parameters are provided
* *WHEN* the user runs `exapump bucketfs cp <source> <plain_path>` where `<plain_path>` has no `bfs://` or `bfss://` prefix
* *THEN* the file MUST be uploaded to BucketFS at `<plain_path>` unchanged
* *AND* the exit code MUST be 0

### Scenario: Upload accepts a bfss:// URI destination

* *GIVEN* a local file exists at `<source>` and valid BucketFS write parameters are provided for bucket `default`
* *AND* the `--bfs-tls` flag is NOT provided
* *WHEN* the user runs `exapump bucketfs cp <source> bfss://default/<path>`
* *THEN* the file MUST be uploaded to BucketFS at `<path>` within the bucket using HTTPS
* *AND* the HTTP request URL MUST NOT contain the `bfss://` prefix or repeat the bucket segment
* *AND* the upload MUST NOT fail with an HTTP 400 response and the exit code MUST be 0

### Scenario: Download accepts a bfss:// URI source

* *GIVEN* a file exists in BucketFS at `<path>` within bucket `default` and valid BucketFS read parameters are provided
* *AND* the `--bfs-tls` flag is NOT provided
* *WHEN* the user runs `exapump bucketfs cp bfss://default/<path> <local_destination>`
* *THEN* the file MUST be downloaded to the local destination path using HTTPS
* *AND* the HTTP request URL MUST NOT contain the `bfss://` prefix or repeat the bucket segment
* *AND* the downloaded content MUST match the original file content and the exit code MUST be 0

### Scenario: bfss:// URI infers TLS even when profile has bfs_tls = false

* *GIVEN* a profile with `bfs_tls = false`
* *AND* the `--bfs-tls` flag is NOT provided
* *WHEN* the user runs `exapump bucketfs cp bfss://default/<path> <local_destination>`
* *THEN* the BucketFS connection MUST use TLS (HTTPS), overriding the profile setting

### Scenario: Explicit --bfs-tls false overrides bfss:// URI inference

* *GIVEN* a `bfss://` URI is provided as source or destination
* *WHEN* the user passes `--bfs-tls false` explicitly
* *THEN* the BucketFS connection MUST NOT use TLS
* *AND* `--bfs-tls false` MUST take precedence over URI inference
