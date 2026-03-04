# Feature: BucketFS Operations

Core file operations against BucketFS via its HTTP/HTTPS REST API. The API uses Basic auth with `w:<write_password>` for write operations and `r:<read_password>` for read operations. Files are addressed as `https://<host>:<port>/<bucket>/<path>`.

## Background

BucketFS exposes files through HTTP GET/PUT/DELETE. Listing returns newline-separated file paths. Upload uses HTTP PUT with the file body. Download uses HTTP GET. Delete uses HTTP DELETE. The `exapump bucketfs` commands map CLI arguments to these HTTP calls.

## Scenarios

### Scenario: List bucket root

* *GIVEN* a BucketFS service is running with a bucket containing files
* *AND* valid BucketFS connection parameters are provided
* *WHEN* the user runs `exapump bucketfs ls`
* *THEN* stdout MUST list the file names in the bucket root, one per line
* *AND* the exit code MUST be 0

### Scenario: List subdirectory

* *GIVEN* a BucketFS bucket contains files under a path prefix
* *AND* valid BucketFS connection parameters are provided
* *WHEN* the user runs `exapump bucketfs ls <path>`
* *THEN* stdout MUST list the files under the given path prefix, one per line

### Scenario: List recursive

* *GIVEN* a BucketFS bucket contains files in nested paths
* *AND* valid BucketFS connection parameters are provided
* *WHEN* the user runs `exapump bucketfs ls --recursive`
* *THEN* stdout MUST list all files recursively, showing their full paths

### Scenario: List empty bucket

* *GIVEN* a BucketFS bucket exists but contains no files
* *AND* valid BucketFS connection parameters are provided
* *WHEN* the user runs `exapump bucketfs ls`
* *THEN* stdout MUST be empty
* *AND* the exit code MUST be 0

### Scenario: Upload single file

* *GIVEN* a local file exists at `<source>`
* *AND* valid BucketFS connection parameters with write password are provided
* *WHEN* the user runs `exapump bucketfs cp <source> <destination>`
* *THEN* the file MUST be uploaded to BucketFS at the destination path
* *AND* stderr MUST indicate the upload was successful
* *AND* the exit code MUST be 0

### Scenario: Upload preserves filename

* *GIVEN* a local file `driver.jar` exists
* *AND* the destination is a path ending with `/` (e.g., `drivers/`)
* *WHEN* the user runs `exapump bucketfs cp driver.jar drivers/`
* *THEN* the file MUST be uploaded as `drivers/driver.jar` in BucketFS

### Scenario: Upload overwrites existing file

* *GIVEN* a file already exists at the destination path in BucketFS
* *AND* a local file with new content exists
* *WHEN* the user runs `exapump bucketfs cp <source> <destination>`
* *THEN* the file in BucketFS MUST be overwritten with the new content

### Scenario: Upload source file not found

* *GIVEN* the source file does not exist on the local filesystem
* *WHEN* the user runs `exapump bucketfs cp nonexistent.jar /bucket/path/`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate the source file was not found

### Scenario: Download single file

* *GIVEN* a file exists in BucketFS at `<source_path>`
* *AND* valid BucketFS connection parameters are provided
* *WHEN* the user runs `exapump bucketfs cp <bfs_path> <local_destination>`
* *THEN* the file MUST be downloaded to the local destination path
* *AND* stderr MUST indicate the download was successful
* *AND* the exit code MUST be 0

### Scenario: Download file not found

* *GIVEN* the specified path does not exist in BucketFS
* *WHEN* the user runs `exapump bucketfs cp <bfs_path> <local_destination>`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate the file was not found in BucketFS

### Scenario: Delete single file

* *GIVEN* a file exists in BucketFS at `<path>`
* *AND* valid BucketFS connection parameters with write password are provided
* *WHEN* the user runs `exapump bucketfs rm <path>`
* *THEN* the file MUST be deleted from BucketFS
* *AND* stderr MUST indicate the deletion was successful
* *AND* the exit code MUST be 0

### Scenario: Delete file not found

* *GIVEN* the specified path does not exist in BucketFS
* *WHEN* the user runs `exapump bucketfs rm <path>`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate the file was not found

### Scenario: Authentication failure

* *GIVEN* BucketFS connection parameters are provided with an incorrect password
* *WHEN* the user runs any BucketFS operation requiring authentication
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate an authentication failure

### Scenario: BucketFS service unreachable

* *GIVEN* BucketFS connection parameters point to a host/port that is not reachable
* *WHEN* the user runs any BucketFS command
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate the connection failed

### Scenario: Copy detects upload direction

* *GIVEN* the `cp` subcommand receives a source and destination
* *WHEN* the source is a local file path that exists on the filesystem
* *THEN* the operation MUST be treated as an upload (local → BucketFS)

### Scenario: Copy detects download direction

* *GIVEN* the `cp` subcommand receives a source and destination
* *WHEN* the source is not an existing local file path
* *THEN* the operation MUST be treated as a download (BucketFS → local)
