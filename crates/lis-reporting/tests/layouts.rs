//! The report layout choice (REQ-RPT-007): an administrator drafts a layout,
//! a quality manager approves or rejects it, and only an approval changes the
//! layout of later candidates. The author approves only as the laboratory's
//! separation-of-duties policy allows (ADR 0015). Real SQLite and commands,
//! synthetic data.

// Marks helpers outside `#[test]` functions as test code for clippy's test allowances.
#![cfg(test)]

use lis_auth::{Attempt, CreateFirstAdministratorInput, CreateUserInput};
use lis_core::{Capability, CorrelationId, ErrorCode, Principal, SessionId, UserId, WorkstationId};
use lis_reporting::layouts::{self, ReportLayout};
use lis_reporting::{
    ApproveReportLayoutInput, DraftReportLayoutInput, LayoutVersionId, LayoutVersionState,
    RejectReportLayoutInput, ReportLayoutsView, ReportingError,
};
use lis_store::StoreError;
use lis_store::testing::TestDatabase;
use secrecy::SecretString;

const PASSWORD: &str = "synthetic passphrase 1";

struct Lab {
    test: TestDatabase,
    /// The administrator, who drafts layouts.
    admin: UserId,
    /// A quality manager, who decides them.
    quality: UserId,
}

fn user(user_id: UserId, capabilities: &[Capability]) -> Principal {
    Principal::user(
        user_id,
        SessionId::generate(),
        WorkstationId::generate(),
        capabilities.iter().copied(),
    )
}

impl Lab {
    fn new() -> Self {
        let test = TestDatabase::open(&[
            lis_auth::SCHEMA,
            lis_identity::SCHEMA,
            lis_catalog::SCHEMA,
            lis_interfaces::SCHEMA,
            lis_orders::SCHEMA,
            lis_specimens::SCHEMA,
            lis_analytical::SCHEMA,
            lis_results::SCHEMA,
            lis_reporting::SCHEMA,
        ])
        .unwrap();
        let admin = test
            .db
            .write_blocking(
                Principal::system("sign-in-screen", []),
                CorrelationId::generate(),
                |ctx| {
                    lis_auth::create_first_administrator(
                        ctx,
                        CreateFirstAdministratorInput {
                            username: "admin1".to_owned(),
                            display_name: "Synthetic Administrator".to_owned(),
                            password: SecretString::from(PASSWORD),
                            one_person: false,
                            load_demo_data: None,
                        },
                    )
                },
            )
            .unwrap();
        let created = test
            .db
            .write_blocking(
                user(admin, &[Capability::ManageUsers]),
                CorrelationId::generate(),
                |ctx| {
                    lis_auth::create_user(
                        ctx,
                        CreateUserInput {
                            username: "quality1".to_owned(),
                            display_name: "Synthetic Quality".to_owned(),
                            roles: Vec::new(),
                            temporary_password: SecretString::from("synthetic temporary 1"),
                            password: SecretString::from(PASSWORD).into(),
                        },
                    )
                },
            )
            .unwrap();
        let Attempt::Accepted(quality) = created else {
            panic!("the quality manager was not created");
        };
        Self {
            test,
            admin,
            quality: quality.user_id,
        }
    }

    fn draft(
        &self,
        by: Principal,
        layout: ReportLayout,
    ) -> Result<ReportLayoutsView, ReportingError> {
        self.test
            .db
            .write_blocking(by, CorrelationId::generate(), move |ctx| {
                lis_reporting::draft_report_layout(ctx, DraftReportLayoutInput { layout })
            })
    }

    fn admin(&self) -> Principal {
        user(self.admin, &[Capability::ManageConfiguration])
    }

    fn quality(&self) -> Principal {
        user(self.quality, &[Capability::ManageQuality])
    }

    fn approve(
        &self,
        by: Principal,
        version_id: LayoutVersionId,
        expected_revision: i64,
        self_approval: bool,
    ) -> Result<ReportLayoutsView, ReportingError> {
        self.test
            .db
            .write_blocking(by, CorrelationId::generate(), move |ctx| {
                lis_reporting::approve_report_layout(
                    ctx,
                    ApproveReportLayoutInput {
                        version_id,
                        expected_revision,
                        self_approval,
                    },
                )
            })
    }

    fn reject(
        &self,
        version_id: LayoutVersionId,
        reason: &str,
    ) -> Result<ReportLayoutsView, ReportingError> {
        let reason = reason.to_owned();
        self.test
            .db
            .write_blocking(self.quality(), CorrelationId::generate(), move |ctx| {
                lis_reporting::reject_report_layout(
                    ctx,
                    RejectReportLayoutInput {
                        version_id,
                        expected_revision: 1,
                        reason,
                    },
                )
            })
    }

    fn in_force(&self) -> ReportLayout {
        self.test.db.read_blocking(layouts::in_force).unwrap()
    }

    fn view(&self, caller: Principal) -> ReportLayoutsView {
        self.test
            .db
            .read_blocking(move |conn| lis_reporting::report_layouts(conn, &caller))
            .unwrap()
    }

    /// The actions of the layout audit events, in order.
    fn events(&self) -> Vec<String> {
        self.test
            .db
            .read_blocking(|conn| {
                let mut statement = conn.prepare(
                    "SELECT action FROM audit_events WHERE action LIKE 'report.%layout' \
                     ORDER BY seq",
                )?;
                let actions = statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<rusqlite::Result<_>>()?;
                Ok::<_, StoreError>(actions)
            })
            .unwrap()
    }
}

fn draft_id(view: &ReportLayoutsView) -> LayoutVersionId {
    view.draft.as_ref().expect("a draft").version_id
}

// REQ-RPT-007: a draft affects no report; the default stays in force until a
// quality manager approves it, and the approval is audited.
#[test]
fn a_draft_takes_effect_only_when_approved() {
    let lab = Lab::new();
    assert_eq!(lab.in_force(), ReportLayout::DEFAULT);
    let drafted = lab.draft(lab.admin(), ReportLayout::Standard).unwrap();
    let draft = drafted.draft.clone().unwrap();
    assert_eq!(draft.state, LayoutVersionState::Draft);
    assert_eq!(draft.drafted_by.user_id, lab.admin);
    assert_eq!(drafted.in_force.layout, ReportLayout::Classic);
    assert_eq!(lab.in_force(), ReportLayout::DEFAULT);

    let approved = lab
        .approve(lab.quality(), draft.version_id, 1, false)
        .unwrap();
    assert!(approved.draft.is_none());
    let active = approved.in_force.version.unwrap();
    assert_eq!(active.state, LayoutVersionState::Active);
    assert_eq!(active.decided_by.unwrap().user_id, lab.quality);
    assert_eq!(lab.in_force(), ReportLayout::Standard);
    assert_eq!(
        lab.events(),
        ["report.draft_layout", "report.approve_layout"]
    );
}

// REQ-RPT-007: a later approval supersedes the active version.
#[test]
fn a_new_approval_supersedes_the_active_layout() {
    let lab = Lab::new();
    let first = lab.draft(lab.admin(), ReportLayout::Standard).unwrap();
    lab.approve(lab.quality(), draft_id(&first), 1, false)
        .unwrap();
    let second = lab.draft(lab.admin(), ReportLayout::Compact).unwrap();
    let view = lab
        .approve(lab.quality(), draft_id(&second), 1, false)
        .unwrap();
    let states: Vec<_> = view
        .versions
        .iter()
        .map(|v| (v.version_no, v.state))
        .collect();
    assert_eq!(
        states,
        [
            (2, LayoutVersionState::Active),
            (1, LayoutVersionState::Superseded)
        ]
    );
    assert_eq!(lab.in_force(), ReportLayout::Compact);
}

// REQ-RPT-007: a rejection needs a reason and leaves the active layout in force.
#[test]
fn a_rejection_needs_a_reason_and_changes_nothing_in_force() {
    let lab = Lab::new();
    let drafted = lab.draft(lab.admin(), ReportLayout::ResultsFirst).unwrap();
    let refused = lab.reject(draft_id(&drafted), "  ").unwrap_err();
    assert_eq!(refused.code(), ErrorCode::Validation);
    let rejected = lab
        .reject(draft_id(&drafted), "Synthetic: the cards hide the limits")
        .unwrap();
    let version = &rejected.versions[0];
    assert_eq!(version.state, LayoutVersionState::Rejected);
    assert_eq!(
        version.rejection_reason.as_deref(),
        Some("Synthetic: the cards hide the limits")
    );
    assert_eq!(lab.in_force(), ReportLayout::DEFAULT);
    assert_eq!(
        lab.events(),
        ["report.draft_layout", "report.reject_layout"]
    );
}

// One open draft at a time; drafting the active layout is refused.
#[test]
fn drafts_are_refused_while_one_is_open_or_for_the_active_layout() {
    let lab = Lab::new();
    let unchanged = lab.draft(lab.admin(), ReportLayout::Classic).unwrap_err();
    assert_eq!(unchanged.code(), ErrorCode::Validation);
    lab.draft(lab.admin(), ReportLayout::Standard).unwrap();
    let duplicate = lab.draft(lab.admin(), ReportLayout::Compact).unwrap_err();
    assert!(matches!(duplicate, ReportingError::Duplicate(_)));
    assert_eq!(lab.events(), ["report.draft_layout"]);
}

// Drafting needs `manage_configuration`; deciding needs `manage_quality`.
#[test]
fn each_step_needs_its_capability_and_writes_nothing_without_it() {
    let lab = Lab::new();
    let denied = lab
        .draft(lab.quality(), ReportLayout::Standard)
        .unwrap_err();
    assert!(matches!(
        denied,
        ReportingError::Store(StoreError::Denied(_))
    ));
    let drafted = lab.draft(lab.admin(), ReportLayout::Standard).unwrap();
    let denied = lab
        .approve(lab.admin(), draft_id(&drafted), 1, false)
        .unwrap_err();
    assert!(matches!(
        denied,
        ReportingError::Store(StoreError::Denied(_))
    ));
    assert_eq!(lab.in_force(), ReportLayout::DEFAULT);
    assert_eq!(lab.events(), ["report.draft_layout"]);
}

// ADR 0015: under the default policy the author may not approve their own draft.
#[test]
fn the_author_may_not_approve_under_the_default_policy() {
    let lab = Lab::new();
    let both = user(
        lab.admin,
        &[Capability::ManageConfiguration, Capability::ManageQuality],
    );
    let drafted = lab.draft(both.clone(), ReportLayout::Standard).unwrap();
    let refused = lab.approve(both, draft_id(&drafted), 1, true).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::GateBlocked);
    let codes: Vec<_> = refused.gate_reasons().into_iter().map(|g| g.code).collect();
    assert_eq!(codes, ["second_person_required"]);
    assert_eq!(lab.in_force(), ReportLayout::DEFAULT);
}

// A stale revision is a conflict; the view offers actions by capability.
#[test]
fn stale_revisions_conflict_and_actions_follow_capabilities() {
    let lab = Lab::new();
    assert_eq!(
        lab.view(lab.admin()).allowed_actions,
        ["draft_report_layout"]
    );
    assert!(lab.view(lab.quality()).allowed_actions.is_empty());
    let drafted = lab.draft(lab.admin(), ReportLayout::Standard).unwrap();
    assert!(drafted.allowed_actions.is_empty());
    assert_eq!(
        lab.view(lab.quality()).allowed_actions,
        ["approve_report_layout", "reject_report_layout"]
    );
    let stale = lab
        .approve(lab.quality(), draft_id(&drafted), 2, false)
        .unwrap_err();
    assert!(matches!(stale, ReportingError::RevisionConflict));
}
