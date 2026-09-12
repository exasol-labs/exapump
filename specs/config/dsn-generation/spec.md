# Feature: Config DSN Generation

A resolved profile (see `config/profiles`) becomes a database DSN. Fields with a driver-level default (`port`, `tls`, `validate_certificate`) are omitted from the generated DSN when unset in the profile, so the exarrow-rs driver's own default applies.

## Background

## Scenarios

### Scenario: Port defaults to 8563

* *GIVEN* a profile that omits the `port` field
* *WHEN* the profile is resolved
* *THEN* the port MUST default to `8563`

### Scenario: TLS defaults to true

* *GIVEN* a profile that omits the `tls` field
* *WHEN* the profile is resolved to a DSN
* *THEN* the generated DSN MUST NOT contain a `tls` query parameter
* *AND* the effective connection MUST still use TLS (the exarrow-rs driver defaults to `tls=true`)

### Scenario: Validate certificate defaults to true

* *GIVEN* a profile that omits the `validate_certificate` field
* *WHEN* the profile is resolved to a DSN
* *THEN* the generated DSN MUST NOT contain a `validateservercertificate` query parameter
* *AND* the effective connection MUST still validate the server certificate (the exarrow-rs driver defaults to `validateservercertificate=true`)

### Scenario: Profile omitting TLS and validate_certificate yields bare DSN

* *GIVEN* a profile with `host = "myhost"`, `port = 8563`, `user = "u"`, `password = "p"` and no `tls` or `validate_certificate` fields
* *WHEN* the profile is resolved to a DSN
* *THEN* the DSN MUST equal `exasol://u:p@myhost:8563`
* *AND* the DSN MUST NOT contain a `?` query string

### Scenario: Profile with only tls set emits only tls parameter

* *GIVEN* a profile with `tls = false` and no `validate_certificate` field
* *WHEN* the profile is resolved to a DSN
* *THEN* the DSN MUST contain `tls=false`
* *AND* the DSN MUST NOT contain a `validateservercertificate` parameter

### Scenario: Profile with only validate_certificate set emits only that parameter

* *GIVEN* a profile with `validate_certificate = false` and no `tls` field
* *WHEN* the profile is resolved to a DSN
* *THEN* the DSN MUST contain `validateservercertificate=0`
* *AND* the DSN MUST NOT contain a `tls` parameter

### Scenario: Profile builds a DSN

* *GIVEN* a profile with `host = "localhost"`, `port = 8563`, `user = "sys"`, `password = "exasol"`, `tls = true`, `validate_certificate = false`
* *WHEN* the profile is resolved for connection
* *THEN* it MUST produce a DSN string in the format `exasol://sys:exasol@localhost:8563?tls=true&validateservercertificate=0`

### Scenario: Profile with schema

* *GIVEN* a profile with `schema = "my_schema"` in addition to required fields
* *WHEN* the profile is resolved
* *THEN* the DSN MUST include the schema as `exasol://user:pwd@host:port/my_schema?tls=true&validateservercertificate=1`

### Scenario: Profile DSN maps all parameters

* *GIVEN* a profile with all fields set
* *WHEN* the profile is resolved to a DSN
* *THEN* `host` MUST map to the DSN host component
* *AND* `port` MUST map to the DSN port component
* *AND* `user` MUST map to the DSN username component
* *AND* `password` MUST map to the DSN password component
* *AND* `schema` MUST map to the DSN path component (e.g., `/my_schema`)
* *AND* `tls`, when set, MUST map to the `tls` query parameter
* *AND* `validate_certificate`, when set, MUST map to the `validateservercertificate` query parameter (`1` for true, `0` for false)
* *AND* unset optional fields MUST be omitted from the DSN query string
