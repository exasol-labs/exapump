# Feature: Transport Selection

The `--transport` flag selects the wire transport exposed by `exarrow-rs` 0.12. The flag is shared across the `upload`, `export`, `sql`, and `interactive` subcommands via `ConnectionArgs`. It accepts `native` or `websocket` and defaults to `native`. When the resolved transport is `native`, exapump MUST NOT inject any transport parameter into the DSN (native is the upstream default). When the resolved transport is `websocket`, exapump MUST append `transport=websocket` to the resolved DSN using the same `?` / `&` separator rule used by `--certificate-fingerprint`.

## Background

Transport selection is applied after DSN resolution. The `append_transport` step runs after fingerprint injection, so both parameters may coexist in the final DSN. The `native` transport value is a no-op by design — it produces no change to the DSN string.

## Scenarios

### Scenario: ConnectionArgs exposes --transport flag in upload help

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump upload --help`
* *THEN* the output MUST show a `--transport` option
* *AND* the option description MUST indicate it selects the wire transport
* *AND* the option MUST accept the values `native` and `websocket`
* *AND* the option MUST default to `native`

### Scenario: ConnectionArgs exposes --transport flag in sql help

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump sql --help`
* *THEN* the output MUST show a `--transport` option
* *AND* the option MUST accept the values `native` and `websocket`
* *AND* the option MUST default to `native`

### Scenario: ConnectionArgs exposes --transport flag in export help

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump export --help`
* *THEN* the output MUST show a `--transport` option
* *AND* the option MUST accept the values `native` and `websocket`
* *AND* the option MUST default to `native`

### Scenario: ConnectionArgs exposes --transport flag in interactive help

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump interactive --help`
* *THEN* the output MUST show a `--transport` option
* *AND* the option MUST accept the values `native` and `websocket`
* *AND* the option MUST default to `native`

### Scenario: Default transport is native and adds no DSN parameter

* *GIVEN* the user provides `--dsn exasol://user:pwd@host:8563`
* *AND* no `--transport` flag is provided
* *WHEN* exapump resolves the connection string
* *THEN* the resolved DSN MUST NOT contain a `transport` query parameter
* *AND* the resolved DSN MUST equal the input DSN with no extra parameters added by transport selection

### Scenario: Explicit --transport native adds no DSN parameter

* *GIVEN* the user provides `--dsn exasol://user:pwd@host:8563`
* *AND* the user provides `--transport native`
* *WHEN* exapump resolves the connection string
* *THEN* the resolved DSN MUST NOT contain a `transport` query parameter

### Scenario: --transport websocket injects transport parameter into a bare DSN

* *GIVEN* the user provides `--dsn exasol://user:pwd@host:8563`
* *AND* the user provides `--transport websocket`
* *WHEN* exapump resolves the connection string
* *THEN* the resolved DSN MUST contain `transport=websocket` as a query parameter
* *AND* the parameter MUST be introduced with a `?` separator since the DSN has no existing query string

### Scenario: --transport websocket appends to a DSN that already has query parameters

* *GIVEN* the user provides `--dsn exasol://user:pwd@host:8563?tls=true&validateservercertificate=0`
* *AND* the user provides `--transport websocket`
* *WHEN* exapump resolves the connection string
* *THEN* the resolved DSN MUST contain `transport=websocket` as a query parameter
* *AND* the parameter MUST be appended with an `&` separator
* *AND* the existing `tls` and `validateservercertificate` parameters MUST be preserved

### Scenario: --transport websocket coexists with --certificate-fingerprint

* *GIVEN* the user provides `--dsn exasol://user:pwd@host:8563` with `--certificate-fingerprint aabbcc112233` and `--transport websocket`
* *WHEN* exapump resolves the connection string
* *THEN* the resolved DSN MUST contain `transport=websocket`
* *AND* the resolved DSN MUST contain `certificate_fingerprint=aabbcc112233`
* *AND* both parameters MUST appear with valid `?` / `&` separators

### Scenario: --transport websocket applies when DSN comes from a profile

* *GIVEN* a config profile named `prod` exists and produces a DSN with no `transport` parameter
* *AND* the user provides `--profile prod`
* *AND* the user provides `--transport websocket`
* *WHEN* exapump resolves the connection string
* *THEN* the resolved DSN MUST contain `transport=websocket`

### Scenario: Invalid --transport value is rejected by the CLI

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump sql 'SELECT 1' --transport http`
* *THEN* argument parsing MUST fail with a non-zero exit code
* *AND* stderr MUST mention the accepted values `native` and `websocket`
