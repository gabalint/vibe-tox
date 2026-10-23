//! Patient identity, clients and practitioners (REQ-IDN-001, REQ-OPS-001).
//!
//! Subject reads return names, birth dates and identifiers, so each is an
//! audited read: the `access` event names the subject, or counts the results
//! of a search, and never copies a search term. Client and practitioner lists
//! are laboratory configuration without patient data; any signed-in user may
//! read them, because both order entry and configuration need them. The
//! laboratory profile is configuration too: any signed-in user reads it, and
//! `manage_configuration` sets it.

pub(crate) mod directories;
pub(crate) mod intake;
mod profile;

use lis_audit::AuditDetail;
use lis_core::{Capability, CommandError};
use lis_identity::{
    AddClientLocation, AddSubjectIdentifier, AssertDemographics, ClientId, ClientLocationId,
    ClientView, ConfirmDistinctSubjects, CreateClient, DeactivateSubjectIdentifier,
    IdentifierAdded, IdentityError, IdentityHoldResolved, IdentityHoldView, LaboratoryProfileView,
    PractitionerId, PractitionerView, RegisterPractitioner, RegisterSubject, SetLaboratoryProfile,
    SubjectCandidates, SubjectId, SubjectMatchQuery, SubjectRegistered, SubjectRevision,
    SubjectSearch, SubjectSummary, SubjectView,
};
use serde::Deserialize;
use ts_rs::TS;

use crate::engine::Engine;
use crate::pipeline::{access, domain_error};
use crate::registry::{Access, CommandSpec, spec};

const ENTER_ORDERS: Access = Access::Capability(Capability::EnterOrders);
const READ_IDENTITY: Access =
    Access::EitherCapability(Capability::EnterOrders, Capability::ManageQuality);
const MANAGE_QUALITY: Access = Access::Capability(Capability::ManageQuality);
const MANAGE_CONFIGURATION: Access = Access::Capability(Capability::ManageConfiguration);
/// The oldest open holds the review list shows when no subject is named.
const ALL_HOLDS_LIMIT: u32 = 500;

pub(crate) const SEARCH_SUBJECTS: CommandSpec =
    spec("search_subjects", "identity.search_subjects", READ_IDENTITY);
pub(crate) const MATCH_SUBJECT_CANDIDATES: CommandSpec = spec(
    "match_subject_candidates",
    "identity.match_subject_candidates",
    READ_IDENTITY,
);
pub(crate) const REGISTER_SUBJECT: CommandSpec = spec(
    "register_subject",
    "identity.register_subject",
    ENTER_ORDERS,
);
pub(crate) const GET_SUBJECT: CommandSpec =
    spec("get_subject", "identity.get_subject", READ_IDENTITY);
pub(crate) const ASSERT_DEMOGRAPHICS: CommandSpec = spec(
    "assert_demographics",
    "identity.assert_demographics",
    MANAGE_QUALITY,
);
pub(crate) const ADD_SUBJECT_IDENTIFIER: CommandSpec = spec(
    "add_subject_identifier",
    "identity.add_identifier",
    ENTER_ORDERS,
);
pub(crate) const DEACTIVATE_SUBJECT_IDENTIFIER: CommandSpec = spec(
    "deactivate_subject_identifier",
    "identity.deactivate_identifier",
    MANAGE_QUALITY,
);
pub(crate) const LIST_IDENTITY_HOLDS: CommandSpec =
    spec("list_identity_holds", "identity.list_holds", MANAGE_QUALITY);
pub(crate) const RESOLVE_IDENTITY_HOLD: CommandSpec = spec(
    "resolve_identity_hold",
    "identity.confirm_distinct",
    MANAGE_QUALITY,
);
pub(crate) const LIST_CLIENTS: CommandSpec =
    spec("list_clients", "identity.list_clients", Access::Session);
pub(crate) const CREATE_CLIENT: CommandSpec = spec(
    "create_client",
    "identity.create_client",
    MANAGE_CONFIGURATION,
);
pub(crate) const ADD_CLIENT_LOCATION: CommandSpec = spec(
    "add_client_location",
    "identity.add_client_location",
    MANAGE_CONFIGURATION,
);
pub(crate) const LIST_PRACTITIONERS: CommandSpec = spec(
    "list_practitioners",
    "identity.list_practitioners",
    Access::Session,
);
pub(crate) const REGISTER_PRACTITIONER: CommandSpec = spec(
    "register_practitioner",
    "identity.register_practitioner",
    MANAGE_CONFIGURATION,
);
pub(crate) const LIST_LABORATORY_PROFILES: CommandSpec = spec(
    "list_laboratory_profiles",
    "identity.list_laboratory_profiles",
    Access::Session,
);
pub(crate) const GET_LABORATORY_PROFILE: CommandSpec = spec(
    "get_laboratory_profile",
    "identity.get_laboratory_profile",
    Access::Session,
);
pub(crate) const SET_LABORATORY_PROFILE: CommandSpec = spec(
    "set_laboratory_profile",
    "identity.set_laboratory_profile",
    MANAGE_CONFIGURATION,
);

/// Input of `get_subject`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GetSubjectInput {
    /// The subject to open.
    pub subject_id: SubjectId,
}

/// Input of `list_identity_holds`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListIdentityHoldsInput {
    /// Only holds on this subject or listing it as a candidate; every open hold when absent.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub subject_id: Option<SubjectId>,
}

domain_error!(IdentityError, |error: &IdentityError| match error {
    IdentityError::Invalid(field) => vec![field.clone()],
    _ => Vec::new(),
});

impl Engine {
    /// `search_subjects` (audited): patient directory browsing or search. The event
    /// records which criteria were used and how many subjects were returned.
    pub async fn search_subjects(
        &self,
        input: SubjectSearch,
    ) -> Result<Vec<SubjectSummary>, CommandError> {
        self.run(&SEARCH_SUBJECTS, |call| async move {
            let by_query = input
                .query
                .as_ref()
                .is_some_and(|query| !query.expose().trim().is_empty());
            let by_identifier = input.identifier.is_some();
            let by_name = input.family_name.is_some();
            let by_birth_date = input.birth_date.is_some();
            self.audited_read(
                &call,
                move |conn| lis_identity::search_subjects(conn, &input),
                move |found| {
                    access(SEARCH_SUBJECTS.action)
                        .detail("by_query", AuditDetail::boolean(by_query))
                        .detail("by_identifier", AuditDetail::boolean(by_identifier))
                        .detail("by_name", AuditDetail::boolean(by_name))
                        .detail("by_birth_date", AuditDetail::boolean(by_birth_date))
                        .detail("returned", AuditDetail::count(found.len()))
                },
            )
            .await
        })
        .await
    }

    /// `match_subject_candidates` (audited): identifier matches and name and
    /// birth date candidates to review before registration.
    pub async fn match_subject_candidates(
        &self,
        input: SubjectMatchQuery,
    ) -> Result<SubjectCandidates, CommandError> {
        self.run(&MATCH_SUBJECT_CANDIDATES, |call| async move {
            self.audited_read(
                &call,
                move |conn| lis_identity::match_subjects(conn, &input),
                |found| {
                    access(MATCH_SUBJECT_CANDIDATES.action)
                        .detail(
                            "identifier_matches",
                            AuditDetail::count(found.identifier_matches.len()),
                        )
                        .detail("candidates", AuditDetail::count(found.candidates.len()))
                },
            )
            .await
        })
        .await
    }

    /// `register_subject`.
    pub async fn register_subject(
        &self,
        input: RegisterSubject,
    ) -> Result<SubjectRegistered, CommandError> {
        self.run(&REGISTER_SUBJECT, |call| async move {
            self.write(&call, move |ctx| lis_identity::register_subject(ctx, input))
                .await
        })
        .await
    }

    /// `get_subject` (audited): every assertion, identifier and open hold.
    pub async fn get_subject(&self, input: GetSubjectInput) -> Result<SubjectView, CommandError> {
        self.run(&GET_SUBJECT, |call| async move {
            let subject_id = input.subject_id;
            self.audited_read(
                &call,
                move |conn| lis_identity::get_subject(conn, subject_id),
                move |_| access(GET_SUBJECT.action).record("subject", subject_id),
            )
            .await
        })
        .await
    }

    /// `assert_demographics`: a corrected demographic assertion with a reason.
    pub async fn assert_demographics(
        &self,
        input: AssertDemographics,
    ) -> Result<SubjectRevision, CommandError> {
        self.run(&ASSERT_DEMOGRAPHICS, |call| async move {
            self.write(&call, move |ctx| {
                lis_identity::assert_demographics(ctx, input)
            })
            .await
        })
        .await
    }

    /// `add_subject_identifier`.
    pub async fn add_subject_identifier(
        &self,
        input: AddSubjectIdentifier,
    ) -> Result<IdentifierAdded, CommandError> {
        self.run(&ADD_SUBJECT_IDENTIFIER, |call| async move {
            self.write(&call, move |ctx| {
                lis_identity::add_subject_identifier(ctx, input)
            })
            .await
        })
        .await
    }

    /// `deactivate_subject_identifier`.
    pub async fn deactivate_subject_identifier(
        &self,
        input: DeactivateSubjectIdentifier,
    ) -> Result<SubjectRevision, CommandError> {
        self.run(&DEACTIVATE_SUBJECT_IDENTIFIER, |call| async move {
            self.write(&call, move |ctx| {
                lis_identity::deactivate_subject_identifier(ctx, input)
            })
            .await
        })
        .await
    }

    /// `list_identity_holds`: open holds, by subject ID only.
    pub async fn list_identity_holds(
        &self,
        input: ListIdentityHoldsInput,
    ) -> Result<Vec<IdentityHoldView>, CommandError> {
        self.run(&LIST_IDENTITY_HOLDS, |call| async move {
            self.audited_read(
                &call,
                move |conn| match input.subject_id {
                    Some(subject_id) => lis_identity::list_open_identity_holds(conn, subject_id),
                    None => lis_identity::list_all_open_identity_holds(conn, ALL_HOLDS_LIMIT),
                },
                move |holds| {
                    let event = access(LIST_IDENTITY_HOLDS.action)
                        .detail("returned", AuditDetail::count(holds.len()));
                    match input.subject_id {
                        Some(subject_id) => event.record("subject", subject_id),
                        None => event,
                    }
                },
            )
            .await
        })
        .await
    }

    /// `resolve_identity_hold`: confirms the subjects are different people.
    pub async fn resolve_identity_hold(
        &self,
        input: ConfirmDistinctSubjects,
    ) -> Result<IdentityHoldResolved, CommandError> {
        self.run(&RESOLVE_IDENTITY_HOLD, |call| async move {
            self.write(&call, move |ctx| {
                lis_identity::confirm_distinct_subjects(ctx, input)
            })
            .await
        })
        .await
    }

    /// `list_clients`: client organizations with their locations.
    pub async fn list_clients(&self) -> Result<Vec<ClientView>, CommandError> {
        self.run(&LIST_CLIENTS, |call| async move {
            self.read(&call, lis_identity::list_clients).await
        })
        .await
    }

    /// `create_client`.
    pub async fn create_client(&self, input: CreateClient) -> Result<ClientId, CommandError> {
        self.run(&CREATE_CLIENT, |call| async move {
            self.write(&call, move |ctx| lis_identity::create_client(ctx, input))
                .await
        })
        .await
    }

    /// `add_client_location`.
    pub async fn add_client_location(
        &self,
        input: AddClientLocation,
    ) -> Result<ClientLocationId, CommandError> {
        self.run(&ADD_CLIENT_LOCATION, |call| async move {
            self.write(&call, move |ctx| {
                lis_identity::add_client_location(ctx, input)
            })
            .await
        })
        .await
    }

    /// `list_practitioners`.
    pub async fn list_practitioners(&self) -> Result<Vec<PractitionerView>, CommandError> {
        self.run(&LIST_PRACTITIONERS, |call| async move {
            self.read(&call, lis_identity::list_practitioners).await
        })
        .await
    }

    /// `register_practitioner`.
    pub async fn register_practitioner(
        &self,
        input: RegisterPractitioner,
    ) -> Result<PractitionerId, CommandError> {
        self.run(&REGISTER_PRACTITIONER, |call| async move {
            self.write(&call, move |ctx| {
                lis_identity::register_practitioner(ctx, input)
            })
            .await
        })
        .await
    }

    /// `get_laboratory_profile`: the current laboratory profile, or `null`
    /// before one is set. Laboratory configuration, readable by any
    /// signed-in user because every screen shows local times in its zone.
    pub async fn get_laboratory_profile(
        &self,
    ) -> Result<Option<LaboratoryProfileView>, CommandError> {
        self.run(&GET_LABORATORY_PROFILE, |call| async move {
            self.read(&call, profile::current).await
        })
        .await
    }

    /// Retained laboratory profile versions for an explicit performing-site attestation.
    pub async fn list_laboratory_profiles(
        &self,
    ) -> Result<Vec<LaboratoryProfileView>, CommandError> {
        self.run(&LIST_LABORATORY_PROFILES, |call| async move {
            self.read(&call, profile::list).await
        })
        .await
    }

    /// `set_laboratory_profile`: appends a new profile version.
    pub async fn set_laboratory_profile(
        &self,
        input: SetLaboratoryProfile,
    ) -> Result<LaboratoryProfileView, CommandError> {
        self.run(&SET_LABORATORY_PROFILE, |call| async move {
            self.write(&call, move |ctx| {
                lis_identity::set_laboratory_profile(ctx, input)
            })
            .await
        })
        .await
    }
}
