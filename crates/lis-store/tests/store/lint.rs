//! Schema lint rules.

use lis_core::{Migration, ModuleSchema};
use lis_store::{SchemaProblem, SchemaViolation, lint_schema};

use crate::DEMO;

#[test]
fn built_in_and_demo_schemas_pass() {
    assert_eq!(lint_schema(&[]).unwrap(), []);
    assert_eq!(lint_schema(&[DEMO]).unwrap(), []);
}

#[test]
fn lint_reports_each_violation() {
    const BAD: ModuleSchema = ModuleSchema {
        module: "bad",
        table_prefix: "bad_",
        migrations: &[Migration {
            version: 1,
            name: "bad_tables",
            sql: "CREATE TABLE bad_loose (id INTEGER PRIMARY KEY, label TEXT);
                  CREATE TABLE bad_real (id INTEGER PRIMARY KEY, value REAL) STRICT;
                  CREATE TABLE bad_any (id INTEGER PRIMARY KEY, value ANY) STRICT;
                  CREATE TABLE unprefixed (id INTEGER PRIMARY KEY) STRICT;
                  CREATE TABLE demo_stolen (id INTEGER PRIMARY KEY) STRICT;
                  CREATE VIRTUAL TABLE bad_search USING fts5(body);",
        }],
    };
    const LATE: ModuleSchema = ModuleSchema {
        module: "late",
        table_prefix: "late_",
        migrations: &[Migration {
            version: 1,
            name: "late_alter",
            sql: "ALTER TABLE demo_items ADD COLUMN weight REAL;",
        }],
    };
    let violation = |table: &str, problem| SchemaViolation {
        module: if table == "demo_items" { "demo" } else { "bad" },
        table: table.to_owned(),
        problem,
    };
    let forbidden = |column: &str, declared: &str| SchemaProblem::ForbiddenColumnType {
        column: column.to_owned(),
        declared: declared.to_owned(),
    };
    assert_eq!(
        lint_schema(&[DEMO, BAD, LATE]).unwrap(),
        [
            violation("demo_items", forbidden("weight", "REAL")),
            violation("bad_any", forbidden("value", "ANY")),
            violation("bad_loose", SchemaProblem::NotStrict),
            violation("bad_real", forbidden("value", "REAL")),
            violation("bad_search", SchemaProblem::NotStrict),
            violation("demo_stolen", SchemaProblem::MissingPrefix),
            violation("unprefixed", SchemaProblem::MissingPrefix),
        ]
    );
}
