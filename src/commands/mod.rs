pub mod export;
pub mod sql;
pub mod upload;

/// Splits "schema.table" into (Some("schema"), "table") or (None, "table").
pub fn parse_table_name(table: &str) -> (Option<&str>, &str) {
    if let Some((schema, name)) = table.split_once('.') {
        (Some(schema), name)
    } else {
        (None, table)
    }
}
