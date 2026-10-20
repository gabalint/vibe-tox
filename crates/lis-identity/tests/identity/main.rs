//! Integration tests on real SQLite files with the production pragmas.
//! All names, dates and identifiers are synthetic.

// Marks helpers outside `#[test]` functions as test code for clippy's test allowances.
#![cfg(test)]

mod authorization;
mod billing;
mod holds;
mod laboratory;
mod organizations;
mod search;
mod subjects;

use lis_core::{Capability, CorrelationId, ErrorCode, Phi, Principal};
use lis_identity::{
    DemographicsInput, IdentifierInput, IdentityError, RegisterSubject, SCHEMA, SubjectId,
    SubjectRegistered, register_subject,
};
use lis_store::testing::{TestDatabase, principal};
use lis_store::{CommandContext, StoreError, lint_schema};
use rusqlite::Connection;

/// Every table this module and the audit trail write.
const TABLES: [&str; 10] = [
    "idn_subjects",
    "idn_identity_assertions",
    "idn_subject_identifiers",
    "idn_identity_holds",
    "idn_identity_hold_candidates",
    "idn_client_organizations",
    "idn_client_locations",
    "idn_practitioners",
    "idn_laboratory_profiles",
    "idn_patient_billing_profiles",
];

pub(crate) fn open() -> TestDatabase {
    TestDatabase::open(&[SCHEMA]).unwrap()
}

/// Runs `command` as a user holding `capabilities`.
pub(crate) fn run<T, F>(
    t: &TestDatabase,
    capabilities: &[Capability],
    command: F,
) -> Result<T, IdentityError>
where
    T: Send + 'static,
    F: FnOnce(&mut CommandContext<'_>) -> Result<T, IdentityError> + Send + 'static,
{
    run_as(t, principal(capabilities), command)
}

pub(crate) fn run_as<T, F>(
    t: &TestDatabase,
    principal: Principal,
    command: F,
) -> Result<T, IdentityError>
where
    T: Send + 'static,
    F: FnOnce(&mut CommandContext<'_>) -> Result<T, IdentityError> + Send + 'static,
{
    t.db.write_blocking(principal, CorrelationId::generate(), command)
}

pub(crate) fn read<T, F>(t: &TestDatabase, query: F) -> T
where
    T: Send + 'static,
    F: FnOnce(&Connection) -> Result<T, IdentityError> + Send + 'static,
{
    t.db.read_blocking(query).unwrap()
}

/// Row counts of every module table plus the audit trail.
pub(crate) fn counts(t: &TestDatabase) -> Vec<i64> {
    read(t, |conn| {
        TABLES
            .iter()
            .chain(&["audit_events"])
            .map(|table| {
                conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .map_err(IdentityError::from)
            })
            .collect()
    })
}

/// Audit actions in the clinical stream, in order.
pub(crate) fn audit_actions(t: &TestDatabase) -> Vec<String> {
    read(t, |conn| {
        let mut statement =
            conn.prepare("SELECT action FROM audit_events WHERE stream = 'clinical' ORDER BY seq")?;
        let actions = statement.query_map([], |row| row.get(0))?;
        Ok(actions.collect::<rusqlite::Result<_>>()?)
    })
}

pub(crate) fn assert_code<T: std::fmt::Debug>(result: Result<T, IdentityError>, code: ErrorCode) {
    let error = result.unwrap_err();
    assert_eq!(error.code(), code, "{error:?}");
}

pub(crate) fn demographics(family: &str, given: &str, birth_date: &str) -> DemographicsInput {
    DemographicsInput {
        family_name: Phi::new(family.to_owned()),
        given_names: Some(Phi::new(given.to_owned())),
        birth_date: Phi::new(birth_date.to_owned()),
        administrative_sex: Some(Phi::new("U".to_owned())),
    }
}

pub(crate) fn identifier(authority: &str, kind: &str, value: &str) -> IdentifierInput {
    IdentifierInput {
        assigning_authority: authority.to_owned(),
        identifier_type: kind.to_owned(),
        value: Phi::new(value.to_owned()),
    }
}

/// Registers a subject as a user holding `enter_orders`.
pub(crate) fn register(
    t: &TestDatabase,
    demographics: DemographicsInput,
    identifiers: Vec<IdentifierInput>,
    acknowledged_candidates: Vec<SubjectId>,
) -> Result<SubjectRegistered, IdentityError> {
    run(t, &[Capability::EnterOrders], move |ctx| {
        register_subject(
            ctx,
            RegisterSubject {
                demographics,
                source: "requisition".to_owned(),
                identifiers,
                acknowledged_candidates,
            },
        )
    })
}

#[test]
fn schema_passes_the_lint() {
    assert_eq!(lint_schema(&[SCHEMA]).unwrap(), Vec::new());
}

#[test]
fn store_errors_keep_their_code() {
    let denied = IdentityError::Store(StoreError::Denied(lis_core::Denied {
        capability: Capability::EnterOrders,
    }));
    assert_eq!(denied.code(), ErrorCode::Forbidden);
}

#[test]
fn command_inputs_refuse_unknown_fields() {
    let valid = serde_json::json!({
        "demographics": {"familyName": "Testperson", "birthDate": "1990-01-01"},
        "source": "requisition",
    });
    assert!(serde_json::from_value::<RegisterSubject>(valid.clone()).is_ok());
    let mut extra = valid;
    extra["isAdmin"] = serde_json::json!(true);
    assert!(serde_json::from_value::<RegisterSubject>(extra).is_err());
}
