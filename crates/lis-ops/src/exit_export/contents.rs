//! What an exit package records besides the copies (M4 design brief §5): facts read
//! from the snapshot copy (ledger, heads, seals as anchor lines, schema and data
//! dictionary), the migration sources, the README taken from the formats document and
//! the independent verifier.

use std::collections::BTreeSet;
use std::path::Path;

use lis_audit::Stream;
use lis_core::{InstallationId, ModuleSchema};
use rusqlite::{Connection, OpenFlags};

use super::ExitExportJob;
use super::dictionary::{self, ArtifactReference};
use crate::backup::{BackupError, HeadEntry, SchemaEntry};

const FORMATS_DOC: &str =
    include_str!("../../../../docs/architecture/backup-and-export-formats.md");
/// `tools/verify_export.py`: a byte-identical copy of `scripts/verify_exit_export.py`.
pub(super) const VERIFIER: &str = include_str!("../../../../scripts/verify_exit_export.py");
const EXIT_HEADING: &str = "\n## Exit export";
const ANCHORS_HEADING: &str = "\n## Audit anchors";

/// What the package records about the snapshot, read from the snapshot copy.
pub(super) struct Facts {
    pub(super) schema: Vec<SchemaEntry>,
    pub(super) heads: Vec<HeadEntry>,
    pub(super) seal_count: usize,
    pub(super) anchor_lines: String,
    pub(super) schema_sql: String,
    pub(super) dictionary_json: Vec<u8>,
    pub(super) dictionary_markdown: String,
    pub(super) references: Vec<ArtifactReference>,
    pub(super) required_artifacts: BTreeSet<String>,
}

pub(super) fn read_facts(
    database: &Path,
    installation: InstallationId,
    job: &ExitExportJob<'_>,
) -> Result<Facts, BackupError> {
    let conn = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut anchor_lines = String::new();
    let mut seal_count = 0;
    for stream in Stream::ALL {
        for seal in lis_audit::seals(&conn, stream, 0)? {
            anchor_lines.push_str(&lis_audit::to_line(&seal, installation)?);
            anchor_lines.push('\n');
            seal_count += 1;
        }
    }
    let modules = all_schemas(job.modules);
    let dictionary = dictionary::build(&conn, &modules, job.artifact_columns)?;
    let references = dictionary.references();
    let required_artifacts = referenced_artifacts(&conn, &references)?;
    Ok(Facts {
        schema: lis_store::applied_migrations(&conn)?
            .into_iter()
            .map(SchemaEntry::from)
            .collect(),
        heads: lis_audit::heads(&conn)?
            .iter()
            .map(|head| HeadEntry {
                stream: head.stream.as_str().to_owned(),
                seq: head.seq,
                hash: head.hash.clone(),
            })
            .collect(),
        seal_count,
        anchor_lines,
        schema_sql: dictionary::schema_sql(&conn)?,
        dictionary_json: dictionary.to_json()?,
        dictionary_markdown: dictionary.to_markdown(),
        references,
        required_artifacts,
    })
}

/// Every distinct digest named by every declared artifact-reference column in the
/// snapshot. The column declaration is supplied by the engine's module owners.
fn referenced_artifacts(
    conn: &Connection,
    references: &[ArtifactReference],
) -> Result<BTreeSet<String>, BackupError> {
    let mut digests = BTreeSet::new();
    for reference in references {
        let table = quoted_identifier(&reference.table)?;
        let column = quoted_identifier(&reference.column)?;
        let sql = format!("SELECT DISTINCT {column} FROM {table} WHERE {column} IS NOT NULL");
        let mut statement = conn.prepare(&sql)?;
        for digest in statement.query_map([], |row| row.get::<_, String>(0))? {
            digests.insert(digest?);
        }
    }
    Ok(digests)
}

fn quoted_identifier(name: &str) -> Result<String, BackupError> {
    let valid = !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_');
    if !valid {
        return Err(crate::format::FormatError::Corrupt.into());
    }
    Ok(format!("\"{name}\""))
}

/// The built-in audit and store schemas, then `modules`, each once.
fn all_schemas(modules: &[ModuleSchema]) -> Vec<ModuleSchema> {
    let mut all = vec![lis_audit::SCHEMA, lis_store::SCHEMA];
    for module in modules {
        if all.iter().all(|known| known.module != module.module) {
            all.push(*module);
        }
    }
    all
}

/// `schema/migrations/<module>/<NNNN>_<name>.sql` for every migration of this build.
pub(super) fn migration_sources(modules: &[ModuleSchema]) -> Vec<(String, &'static str)> {
    all_schemas(modules)
        .iter()
        .flat_map(|module| {
            module.migrations.iter().map(move |migration| {
                (
                    format!(
                        "schema/migrations/{}/{:04}_{}.sql",
                        module.module, migration.version, migration.name
                    ),
                    migration.sql,
                )
            })
        })
        .collect()
}

/// The package's `README.txt`: the exit export section of the formats document, then
/// its audit anchors section, which defines the `sha256-v1` chain and anchor checks.
pub(super) fn readme(verifier_sha256: &str) -> String {
    let sections = [EXIT_HEADING, ANCHORS_HEADING]
        .iter()
        .map(|heading| section(heading))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{sections}\n\nEmbedded verifier SHA-256: {verifier_sha256}\n\
         Compare this digest with a trusted copy before running tools/verify_export.py.\n"
    )
}

/// The formats document's section starting at the level-2 `heading`, up to the next one.
fn section(heading: &str) -> &'static str {
    let Some(start) = FORMATS_DOC.find(heading) else {
        return "";
    };
    let section = &FORMATS_DOC[start + 1..];
    let end = section[1..]
        .find("\n## ")
        .map_or(section.len(), |end| end + 2);
    &section[..end]
}
