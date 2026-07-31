//! Small, explicit helpers for authoring MSI database tables.
//!
//! Table-specific row layouts stay beside their MSI concepts; this module only
//! centralizes error context and repetitive database calls.

use std::io::{Read, Seek, Write};

use anyhow::Context;
use msi::{Column, Insert, Package, Value};

pub(super) fn create_table<W>(
    package: &mut Package<W>,
    table: &str,
    columns: Vec<Column>,
) -> anyhow::Result<()>
where
    W: Read + Write + Seek,
{
    package
        .create_table(table, columns)
        .with_context(|| format!("failed to create MSI table {table}"))
}

pub(super) fn insert<W>(
    package: &mut Package<W>,
    table: &str,
    rows: Vec<Vec<Value>>,
) -> anyhow::Result<()>
where
    W: Read + Write + Seek,
{
    package
        .insert_rows(Insert::into(table).rows(rows))
        .with_context(|| format!("failed to insert MSI table {table} rows"))
}
