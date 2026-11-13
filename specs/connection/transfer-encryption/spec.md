# Feature: Data Transfer Encryption

`upload` and `export` move rows through `exarrow-rs`'s HTTP transport tunnel, a second connection to the database next to the login connection. The tunnel MUST use TLS whenever the resolved connection uses TLS, so the rows travel the same way as the login and the statements. exapump MUST set the tunnel's TLS option from the connection's own parameters on every import and export it runs, because `exarrow-rs` leaves that option off unless the caller sets it.

## Background

The tunnel opens to the same host and port as the login connection. With TLS, exapump generates a certificate for the tunnel, and the `IMPORT` or `EXPORT` statement names the tunnel as `https://` with that certificate's `PUBLIC KEY`. Without TLS, the statement names `http://`.

## Scenarios

### Scenario: Upload uses TLS for the data transfer when the connection uses TLS

* *GIVEN* the resolved DSN sets `tls=true`
* *AND* the user uploads a CSV or Parquet file
* *WHEN* exapump runs the import
* *THEN* the transfer tunnel MUST use TLS
* *AND* the rows MUST travel inside the tunnel's TLS session

### Scenario: Export uses TLS for the data transfer when the connection uses TLS

* *GIVEN* the resolved DSN sets `tls=true`
* *AND* the user exports a table or query to CSV or Parquet, with or without file splitting
* *WHEN* exapump runs the export
* *THEN* every transfer tunnel the export opens MUST use TLS
* *AND* the rows MUST travel inside the tunnel's TLS session
* *AND* the output file or files MUST hold every exported row
