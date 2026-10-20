//! The exit package's `schema/schema.sql` and its generated data dictionary (M4 design
//! brief §5), read from the exported snapshot: every table and view with its owning
//! module, columns, keys, DDL (which holds the CHECK constraints), indexes and triggers,
//! and the columns holding artifact digests marked.

use std::fmt::Write as _;

use lis_core::ModuleSchema;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::backup::BackupError;

/// A column holding artifact digests: each value names `artifacts/<value>` in the package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactReference {
    /// The table.
    pub table: String,
    /// The column.
    pub column: String,
}

/// The `vibe-tox-dictionary-1` document.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Dictionary {
    format: &'static str,
    tables: Vec<TableEntry>,
    #[serde(skip)]
    references: Vec<ArtifactReference>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TableEntry {
    name: String,
    kind: String,
    module: Option<&'static str>,
    columns: Vec<ColumnEntry>,
    foreign_keys: Vec<ForeignKey>,
    ddl: String,
    indexes: Vec<String>,
    triggers: Vec<TriggerEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ColumnEntry {
    name: String,
    #[serde(rename = "type")]
    type_name: String,
    not_null: bool,
    default: Option<String>,
    /// Position in the primary key, 0 when not part of it.
    primary_key: i64,
    generated: bool,
    artifact: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ForeignKey {
    from: String,
    table: String,
    to: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TriggerEntry {
    name: String,
    ddl: String,
}

/// Every DDL statement of `sqlite_schema`, in creation order.
pub(super) fn schema_sql(conn: &Connection) -> rusqlite::Result<String> {
    let mut statement =
        conn.prepare("SELECT sql FROM sqlite_schema WHERE sql IS NOT NULL ORDER BY rowid")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    let mut sql = String::new();
    for row in rows {
        sql.push_str(&row?);
        sql.push_str(";\n\n");
    }
    Ok(sql)
}

/// Builds the dictionary of every table and view of `conn`.
pub(super) fn build(
    conn: &Connection,
    modules: &[ModuleSchema],
    artifact_columns: &[(&str, &str)],
) -> Result<Dictionary, BackupError> {
    let mut statement = conn.prepare(
        "SELECT name, type, sql FROM sqlite_schema WHERE type IN ('table', 'view') \
         AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let objects = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<rusqlite::Result<Vec<(String, String, String)>>>()?;
    let mut tables = Vec::with_capacity(objects.len());
    for (name, kind, ddl) in objects {
        tables.push(TableEntry {
            columns: columns(conn, &name, artifact_columns)?,
            foreign_keys: foreign_keys(conn, &name)?,
            indexes: dependents(conn, "index", &name)?
                .into_iter()
                .map(|(_, ddl)| ddl)
                .collect(),
            triggers: dependents(conn, "trigger", &name)?
                .into_iter()
                .map(|(name, ddl)| TriggerEntry { name, ddl })
                .collect(),
            module: owner(&name, modules),
            name,
            kind,
            ddl,
        });
    }
    let references = artifact_columns
        .iter()
        .map(|&(table, column)| ArtifactReference {
            table: table.to_owned(),
            column: column.to_owned(),
        })
        .collect();
    Ok(Dictionary {
        format: "vibe-tox-dictionary-1",
        tables,
        references,
    })
}

fn columns(
    conn: &Connection,
    table: &str,
    artifact_columns: &[(&str, &str)],
) -> rusqlite::Result<Vec<ColumnEntry>> {
    let mut statement = conn.prepare(
        "SELECT name, type, \"notnull\", dflt_value, pk, hidden \
         FROM pragma_table_xinfo(?1) ORDER BY cid",
    )?;
    statement
        .query_map([table], |row| {
            let name: String = row.get(0)?;
            let hidden: i64 = row.get(5)?;
            Ok(ColumnEntry {
                artifact: artifact_columns.contains(&(table, name.as_str())),
                name,
                type_name: row.get(1)?,
                not_null: row.get(2)?,
                default: row.get(3)?,
                primary_key: row.get(4)?,
                generated: hidden >= 2,
            })
        })?
        .collect()
}

fn foreign_keys(conn: &Connection, table: &str) -> rusqlite::Result<Vec<ForeignKey>> {
    let mut statement = conn.prepare(
        "SELECT \"from\", \"table\", \"to\" FROM pragma_foreign_key_list(?1) ORDER BY id, seq",
    )?;
    statement
        .query_map([table], |row| {
            Ok(ForeignKey {
                from: row.get(0)?,
                table: row.get(1)?,
                to: row.get(2)?,
            })
        })?
        .collect()
}

/// The `(name, sql)` of the indexes or triggers of `table` that have DDL.
fn dependents(
    conn: &Connection,
    kind: &str,
    table: &str,
) -> rusqlite::Result<Vec<(String, String)>> {
    let mut statement = conn.prepare(
        "SELECT name, sql FROM sqlite_schema WHERE type = ?1 AND tbl_name = ?2 \
         AND sql IS NOT NULL ORDER BY name",
    )?;
    statement
        .query_map([kind, table], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect()
}

/// The module whose table prefix the name has; where several modules share a prefix, the
/// one whose migrations create it.
fn owner(name: &str, modules: &[ModuleSchema]) -> Option<&'static str> {
    let candidates: Vec<&ModuleSchema> = modules
        .iter()
        .filter(|module| name.starts_with(module.table_prefix))
        .collect();
    candidates
        .iter()
        .find(|module| module.migrations.iter().any(|m| creates(m.sql, name)))
        .or(candidates.first())
        .map(|module| module.module)
}

fn creates(sql: &str, name: &str) -> bool {
    ["TABLE ", "VIEW "].iter().any(|kind| {
        let needle = format!("CREATE {kind}{name}");
        sql.match_indices(&needle).any(|(at, _)| {
            sql[at + needle.len()..]
                .chars()
                .next()
                .is_some_and(|next| next.is_whitespace() || next == '(')
        })
    })
}

impl Dictionary {
    /// The JSON document, pretty-printed.
    pub(super) fn to_json(&self) -> Result<Vec<u8>, BackupError> {
        let mut bytes =
            serde_json::to_vec_pretty(self).map_err(|_| crate::format::FormatError::Corrupt)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// The columns holding artifact digests.
    pub(super) fn references(&self) -> Vec<ArtifactReference> {
        self.references.clone()
    }

    /// The same content as Markdown, for reading.
    pub(super) fn to_markdown(&self) -> String {
        let mut text = String::from(
            "# Data dictionary\n\nGenerated from the exported database. A column marked as an \
             artifact holds the SHA-256 of a file in `artifacts/`. The DDL of each table holds \
             its CHECK constraints.\n",
        );
        for table in &self.tables {
            let module = table.module.unwrap_or("unknown");
            let _ = write!(
                text,
                "\n## `{}`\n\n{}, module `{module}`.\n\n\
                 | Column | Type | Not null | Default | Primary key | Artifact |\n\
                 | --- | --- | --- | --- | --- | --- |\n",
                table.name, table.kind
            );
            for column in &table.columns {
                let _ = writeln!(
                    text,
                    "| `{}` | {} | {} | {} | {} | {} |",
                    column.name,
                    column.type_name,
                    yes(column.not_null),
                    column.default.as_deref().unwrap_or(""),
                    column.primary_key,
                    yes(column.artifact),
                );
            }
            for key in &table.foreign_keys {
                let to = key.to.as_deref().unwrap_or("primary key");
                let _ = writeln!(
                    text,
                    "\nForeign key: `{}` → `{}` (`{to}`).",
                    key.from, key.table
                );
            }
            let _ = writeln!(text, "\n```sql\n{}\n```", table.ddl);
            for trigger in &table.triggers {
                let _ = writeln!(text, "\nTrigger `{}`.", trigger.name);
            }
        }
        text
    }
}

fn yes(value: bool) -> &'static str {
    if value { "yes" } else { "" }
}
