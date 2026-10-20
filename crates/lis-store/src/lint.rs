//! Schema lint for every module's tests (coding standards, "SQL and migrations").

use std::collections::BTreeSet;

use lis_core::ModuleSchema;
use rusqlite::{Connection, OptionalExtension};

use crate::database::built_in_schemas;
use crate::error::StoreError;
use crate::migrate::BOOTSTRAP_SQL;

/// What is wrong with a table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaProblem {
    /// The table is not declared `STRICT`.
    NotStrict,
    /// A column is declared `REAL` or `ANY`; quantities are lexical TEXT.
    ForbiddenColumnType {
        /// The column.
        column: String,
        /// Its declared type.
        declared: String,
    },
    /// The table name does not start with the creating module's prefix.
    MissingPrefix,
}

/// A table created by `module`'s migrations that breaks a schema rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaViolation {
    /// The module whose migrations created the table.
    pub module: &'static str,
    /// The table.
    pub table: String,
    /// The rule it breaks.
    pub problem: SchemaProblem,
}

/// Applies the built-in schemas and then `modules` to an in-memory database
/// and checks every table each module creates: it must be `STRICT`, have no
/// `REAL` or `ANY` column, and carry that module's prefix. Virtual tables
/// count as tables. Strictness and columns are checked after every module
/// has run, so a later `ALTER TABLE … ADD COLUMN x REAL` is caught too; it is
/// reported against the table's creating module.
///
/// Call it from each module's tests with the modules it depends on, in
/// crate-map order, followed by the module itself.
pub fn lint_schema(modules: &[ModuleSchema]) -> Result<Vec<SchemaViolation>, StoreError> {
    let conn = Connection::open_in_memory()?;
    let mut created = Vec::new();
    for schema in built_in_schemas().iter().chain(modules) {
        let before = tables(&conn)?;
        if schema.module == crate::SCHEMA.module {
            conn.execute_batch(BOOTSTRAP_SQL)?;
        }
        for migration in schema.migrations {
            conn.execute_batch(migration.sql)?;
        }
        for table in tables(&conn)?.difference(&before) {
            created.push((*schema, table.clone()));
        }
    }
    let mut violations = Vec::new();
    for (schema, table) in &created {
        check_table(&conn, schema, table, &mut violations)?;
    }
    Ok(violations)
}

fn tables(conn: &Connection) -> Result<BTreeSet<String>, StoreError> {
    let mut statement = conn.prepare(
        "SELECT name FROM pragma_table_list WHERE schema = 'main' \
         AND type IN ('table', 'virtual') AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\'",
    )?;
    let names = statement.query_map([], |row| row.get(0))?;
    Ok(names.collect::<rusqlite::Result<_>>()?)
}

fn check_table(
    conn: &Connection,
    schema: &ModuleSchema,
    table: &str,
    violations: &mut Vec<SchemaViolation>,
) -> Result<(), StoreError> {
    let mut report = |problem| {
        violations.push(SchemaViolation {
            module: schema.module,
            table: table.to_owned(),
            problem,
        });
    };
    if !table.starts_with(schema.table_prefix) {
        report(SchemaProblem::MissingPrefix);
    }
    let strict: Option<bool> = conn
        .query_row(
            "SELECT strict FROM pragma_table_list WHERE schema = 'main' AND name = ?1",
            [table],
            |row| row.get(0),
        )
        .optional()?;
    let Some(strict) = strict else {
        // A later migration dropped the table.
        return Ok(());
    };
    if !strict {
        report(SchemaProblem::NotStrict);
    }
    let mut statement = conn.prepare("SELECT name, type FROM pragma_table_xinfo(?1)")?;
    let columns = statement.query_map([table], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for column in columns {
        let (column, declared) = column?;
        if matches!(
            declared.trim().to_ascii_uppercase().as_str(),
            "REAL" | "ANY"
        ) {
            report(SchemaProblem::ForbiddenColumnType { column, declared });
        }
    }
    Ok(())
}
