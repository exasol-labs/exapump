# Feature: BucketFS Connection Resolution Errors

When `bucketfs/connection-resolution`'s base-connection order finds no usable base, `exapump bucketfs <ls|cp|rm>` fails and names a remedy. A profile-resolution failure other than a config with zero profiles always propagates, even when `--bfs-host` is present.

## Background

## Scenarios

### Scenario: Missing host and missing profile name both remedies

* *GIVEN* no config file exists at the path exapump resolves for the config
* *AND* the user provides no `--bfs-host` flag and no `--profile` flag
* *WHEN* the user runs `exapump bucketfs ls`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST name `--bfs-host` as an alternative to a profile
* *AND* stderr MUST name `exapump profile add`

### Scenario: Ambiguous default profile still fails a host-only run

* *GIVEN* a config file holding two profiles that both set `default = true`
* *AND* the user provides `--bfs-host` and no BucketFS credential flag
* *AND* the user provides no `--profile` flag
* *WHEN* the user runs `exapump bucketfs ls`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST list the conflicting profile names

### Scenario: Host-only run still fails when no default profile is set

* *GIVEN* a config file holding two profiles and no `default = true`
* *AND* the user provides `--bfs-host` and no BucketFS credential flag
* *AND* the user provides no `--profile` flag
* *WHEN* the user runs `exapump bucketfs ls`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST suggest adding `default = true` to one profile
