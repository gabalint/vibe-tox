//! The changes ("X") that race a report release in
//! `docs/planning/m3-design-brief.md` §3.6, with the outcomes its table
//! expects in each commit order. Every expectation is copied from that
//! table, never derived from the implementation. Synthetic data only.

use lis_analytical::{QcOutcome, QcScope};
use lis_auth::{RoleCode, SetUserRolesInput};
use lis_core::{CommandError, ErrorCode, Phi};
use lis_engine::{
    AssessQcInput, DecideCancellationInput, Decision, DeviationTarget, Engine, FindContainerInput,
    GetAccessionInput, GetOrderInput, GetRunQcInput, OpenDeviationInput,
    PlaceClarificationHoldInput, PlaceContainerHoldInput, RecordReceiptDecisionInput,
    RecordResultDispositionInput, RecordValidityAssessmentInput, RegisterSubject,
    RequestCancellationInput, ResultOutcome, ResultRevisionId,
};
use lis_identity::DemographicsInput;
use lis_orders::{ClarificationDeficiency, OrderComponentId};
use lis_specimens::{ContainerHoldKind, ReceiptDecision};

use crate::fixture::{NEW_PASSWORD, secret};
use crate::m2_lab::Role;
use crate::m2_steps::line;
use crate::m3_fixture::{Bench, HOLD, Staff, Variant};

/// One row of the §3.6 table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Change {
    /// `assess_qc(run_analyte, rejected, supersedes accepted)` by Q.
    QcRejection,
    /// `open_deviation(run, analyte)` by Q.
    Deviation,
    /// `register_subject` of an acknowledged name and birth-date candidate.
    IdentityHold,
    /// `request_cancellation` + `decide_cancellation(accept)` on the reported item.
    Cancellation,
    /// `record_result_disposition` on a reported line, by T.
    ResultRevision,
    /// `set_user_roles(R, without releaser)` by A.
    ReleaserAuthority,
    /// `place_hold(quality)` on the specimen's container ("also covered").
    ContainerHold,
    /// `place_clarification_hold` on the order ("also covered").
    ClarificationHold,
    /// Validity reassessment with action `hold` ("also covered").
    ValidityHold,
}

/// How the release must fail when the change commits first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// `conflict` (`candidate_stale`); the gate reasons include this code.
    Stale(&'static str),
    /// `gate_blocked` with this gate reason; the content is unchanged.
    Blocked(&'static str),
    /// Refused with this gate reason; the table does not name the error code.
    Gate(&'static str),
    /// `forbidden`, with a security `denied` event.
    Forbidden,
}

/// A change ready to commit, with what the impact key names.
pub(crate) enum Prepared {
    Qc(AssessQcInput, ResultRevisionId),
    Deviation(OpenDeviationInput, ResultRevisionId),
    Subject(RegisterSubject),
    Cancellation(DecideCancellationInput, OrderComponentId),
    Disposition(RecordResultDispositionInput, ResultRevisionId),
    Roles(SetUserRolesInput),
    ContainerHold(PlaceContainerHoldInput),
    ClarificationHold(PlaceClarificationHoldInput),
    Validity(RecordValidityAssessmentInput, ResultRevisionId),
}

impl Change {
    pub(crate) fn variant(self) -> Variant {
        if self == Self::ValidityHold {
            Variant::Validity
        } else {
            Variant::Panel
        }
    }

    /// Who commits the change in the sequential tests.
    pub(crate) fn actor(self) -> Staff {
        Staff::Desk(match self {
            Self::QcRejection | Self::Deviation | Self::ContainerHold => Role::Quality,
            Self::IdentityHold | Self::Cancellation | Self::ClarificationHold => Role::Accessioner,
            Self::ResultRevision | Self::ValidityHold => Role::Reviewer,
            // Clinical roles are changed by the quality manager (REQ-IDN-004).
            Self::ReleaserAuthority => Role::Quality,
        })
    }

    /// The committing command, whose audit event orders the race.
    pub(crate) fn command(self) -> &'static str {
        match self {
            Self::QcRejection => "assess_qc",
            Self::Deviation => "open_deviation",
            Self::IdentityHold => "register_subject",
            Self::Cancellation => "decide_cancellation",
            Self::ResultRevision => "record_result_disposition",
            Self::ReleaserAuthority => "set_user_roles",
            Self::ContainerHold => "place_hold",
            Self::ClarificationHold => "place_clarification_hold",
            Self::ValidityHold => "record_validity_assessment",
        }
    }

    /// The audit stream of the committing command's event.
    pub(crate) fn stream(self) -> &'static str {
        if self == Self::ReleaserAuthority {
            "security"
        } else {
            "clinical"
        }
    }

    /// "X first: release fails with".
    pub(crate) fn refusal(self) -> Refusal {
        match self {
            Self::QcRejection => Refusal::Stale("review_stale:qc_rejected"),
            Self::Deviation => Refusal::Stale("review_stale:deviation_open"),
            Self::IdentityHold => Refusal::Blocked("identity_hold_open"),
            Self::Cancellation => Refusal::Stale("component_cancelled"),
            Self::ResultRevision => Refusal::Stale("review_stale:revision_superseded"),
            Self::ReleaserAuthority => Refusal::Forbidden,
            Self::ContainerHold => Refusal::Gate("container_quality_hold"),
            Self::ClarificationHold => Refusal::Gate("clarification_hold_open"),
            Self::ValidityHold => Refusal::Gate("validity_hold"),
        }
    }

    /// Reads and earlier steps through the desk; `releaser` is who loses
    /// the releaser role in the authority row.
    pub(crate) async fn prepare(self, bench: &Bench, releaser: Staff) -> Prepared {
        let sample = bench.sample().clone();
        match self {
            Self::QcRejection => Prepared::Qc(qc_rejection(bench).await, mor(bench).await),
            Self::Deviation => {
                let lab = bench.desk().await;
                let input = OpenDeviationInput {
                    scope: DeviationTarget::Run(lab.run),
                    analyte_id: Some(lab.analytes.mor),
                    description: "Synthetic internal standard drift".to_owned(),
                    reason: "Synthetic bench observation".to_owned(),
                };
                Prepared::Deviation(input, mor(bench).await)
            }
            Self::IdentityHold => Prepared::Subject(namesake(bench).await),
            Self::Cancellation => cancellation(bench).await,
            Self::ResultRevision => disposition(bench).await,
            Self::ReleaserAuthority => Prepared::Roles(without_releaser(bench, releaser).await),
            Self::ContainerHold => Prepared::ContainerHold(PlaceContainerHoldInput {
                container_id: sample.container_id,
                kind: ContainerHoldKind::Quality,
                reason: "Synthetic leaking container".to_owned(),
            }),
            Self::ClarificationHold => {
                let lab = bench.desk().await;
                lab.sign_in(Role::Accessioner).await;
                let order_id = sample.order_id;
                let order = lab.engine().get_order(GetOrderInput { order_id });
                Prepared::ClarificationHold(PlaceClarificationHoldInput {
                    order_id,
                    expected_revision: order.await.unwrap().revision,
                    deficiency: ClarificationDeficiency::OtherAuthority,
                    note: Some("Synthetic unclear requisition".to_owned()),
                })
            }
            Self::ValidityHold => validity_hold(bench).await,
        }
    }
}

impl Prepared {
    /// Commits the change as the signed-in user; returns the impact key
    /// the table expects on a revision released before it (none for the
    /// authority row: revocation is prospective).
    pub(crate) async fn commit(self, e: &Engine) -> Result<Option<String>, CommandError> {
        let stale = |rev: ResultRevisionId, cause: &str, by: String| {
            Some(format!("review_stale:{rev}:{cause}:{by}"))
        };
        Ok(match self {
            Self::Qc(input, rev) => {
                let assessment = e.assess_qc(input).await?.assessment_id;
                stale(rev, "qc_rejected", assessment.to_string())
            }
            Self::Deviation(input, rev) => {
                let deviation = e.open_deviation(input).await?.deviation_id;
                stale(rev, "deviation_open", deviation.to_string())
            }
            Self::Subject(input) => {
                let hold = e.register_subject(input).await?.identity_hold_id;
                Some(format!("identity_hold:{}", hold.unwrap()))
            }
            Self::Cancellation(input, component) => {
                e.decide_cancellation(input).await?;
                Some(format!("component_cancelled:{component}"))
            }
            Self::Disposition(input, rev) => {
                let new = e.record_result_disposition(input).await?.revision_id;
                stale(rev, "revision_superseded", new.to_string())
            }
            Self::Roles(input) => {
                e.set_user_roles(input).await?;
                None
            }
            Self::ContainerHold(input) => {
                let hold = e.place_hold(input).await?.hold_id;
                Some(format!("container_hold:{hold}"))
            }
            Self::ClarificationHold(input) => {
                let order = e.place_clarification_hold(input).await?;
                Some(format!(
                    "clarification_hold:{}",
                    order.holds.last().unwrap().hold_id
                ))
            }
            Self::Validity(input, rev) => {
                let assessment = e.record_validity_assessment(input).await?.assessment_id;
                stale(rev, "validity_assessment_changed", assessment.to_string())
            }
        })
    }
}

/// The reported `SYN-MOR` revision.
async fn mor(bench: &Bench) -> ResultRevisionId {
    bench.revision("SYN-MOR").await
}

/// Rejects `SYN-MOR` QC on the run, superseding the acceptance.
async fn qc_rejection(bench: &Bench) -> AssessQcInput {
    let lab = bench.desk().await;
    lab.sign_in(Role::Quality).await;
    let qc = lab.engine().get_run_qc(GetRunQcInput { run_id: lab.run });
    let qc = qc.await.unwrap();
    let entry = qc
        .analytes
        .iter()
        .find(|a| a.analyte_id == lab.analytes.mor)
        .unwrap();
    AssessQcInput {
        scope: QcScope::RunAnalyte,
        run_id: Some(lab.run),
        analyte_id: Some(lab.analytes.mor),
        prep_batch_id: None,
        outcome: QcOutcome::Rejected,
        evidence_observation_ids: entry
            .observations
            .iter()
            .filter(|o| !o.superseded)
            .map(|o| o.observation_id)
            .collect(),
        supersedes_assessment_id: Some(entry.assessments.current.as_ref().unwrap().assessment_id),
        reason: Some("Synthetic control shift".to_owned()),
    }
}

/// A new subject with the journey subject's name and birth date, its
/// candidate acknowledged, which opens an identity hold.
async fn namesake(bench: &Bench) -> RegisterSubject {
    let subject = bench.desk().await.parties.subject;
    RegisterSubject {
        demographics: DemographicsInput {
            family_name: Phi::new("SYNTHETIC".to_owned()),
            given_names: Some(Phi::new("Patient Mu".to_owned())),
            birth_date: Phi::new("1980-01-01".to_owned()),
            administrative_sex: None,
        },
        source: "requisition".to_owned(),
        identifiers: Vec::new(),
        acknowledged_candidates: vec![subject],
    }
}

/// The client's cancellation request, entered by the accessioner.
async fn cancellation(bench: &Bench) -> Prepared {
    let lab = bench.desk().await;
    let component = line(&lab.results(bench.index).await, "SYN-MOR").component_id;
    lab.sign_in(Role::Accessioner).await;
    let requested = lab
        .engine()
        .request_cancellation(RequestCancellationInput {
            item_id: bench.sample().item_id,
            requested_by: "Synthetic Clinic front desk".to_owned(),
            reason: "Synthetic duplicate requisition".to_owned(),
        })
        .await
        .unwrap();
    let request = requested.cancellations.last().unwrap();
    let input = DecideCancellationInput {
        request_id: request.request_id,
        expected_revision: request.revision,
        decision: Decision::Accept,
        reason: "Synthetic confirmed with the client".to_owned(),
    };
    Prepared::Cancellation(input, component)
}

/// Declares the reported `SYN-MOR` result invalid: a new revision.
async fn disposition(bench: &Bench) -> Prepared {
    let results = bench.desk().await.results(bench.index).await;
    let mor = line(&results, "SYN-MOR");
    let input = RecordResultDispositionInput {
        result_id: Some(mor.result_id),
        expected_revision: mor.revision,
        component_id: None,
        analyte_id: None,
        specimen_id: None,
        outcome: ResultOutcome::Invalid,
        reason: "Synthetic integration error found".to_owned(),
    };
    Prepared::Disposition(input, mor.current.as_ref().unwrap().revision_id)
}

/// `releaser`'s current roles without the releaser role.
async fn without_releaser(bench: &Bench, releaser: Staff) -> SetUserRolesInput {
    let lab = bench.desk().await;
    lab.sign_in(Role::Quality).await;
    let user_id = bench.user_id(releaser);
    let users = lab.engine().list_users().await.unwrap();
    let user = users.into_iter().find(|u| u.user_id == user_id).unwrap();
    SetUserRolesInput {
        user_id,
        roles: user
            .roles
            .into_iter()
            .filter(|role| *role != RoleCode::Releaser)
            .collect(),
        expected_revision: user.revision,
        reason: "Synthetic release authority withdrawn".to_owned(),
        password: secret(NEW_PASSWORD).into(),
    }
}

/// Reassesses validity as `SYN-HOLD` on the same evidence.
async fn validity_hold(bench: &Bench) -> Prepared {
    let results = bench.desk().await.results(bench.index).await;
    let current = results.current_validity.clone().unwrap();
    let input = RecordValidityAssessmentInput {
        specimen_id: bench.sample().specimen_id,
        policy_version_id: bench.validity_policy.unwrap(),
        category_code: HOLD.to_owned(),
        evidence_revision_ids: current.evidence_revision_ids,
        supersedes_id: Some(current.assessment_id),
        reason: Some("Synthetic reassessment".to_owned()),
    };
    let rev = line(&results, "SYN-MOR")
        .current
        .as_ref()
        .unwrap()
        .revision_id;
    Prepared::Validity(input, rev)
}

/// The §3.6 "also covered" row `record_receipt_decision(held)` cannot race
/// a release: an accepted specimen takes no further receipt decision, so
/// the change itself is refused and the release has nothing to race.
#[tokio::test]
async fn a_reported_specimen_cannot_be_put_on_receipt_hold() {
    let bench = Bench::start(Variant::Panel).await;
    let lab = bench.desk().await;
    lab.sign_in(Role::Accessioner).await;
    let e = lab.engine();
    let label_code = bench.sample().label_code.clone();
    let accession_id = e
        .find_container(FindContainerInput { label_code })
        .await
        .unwrap()
        .accession_id;
    let accession = e.get_accession(GetAccessionInput { accession_id });
    let specimen = accession.await.unwrap().specimens.remove(0);
    let held = e
        .record_receipt_decision(RecordReceiptDecisionInput {
            specimen_id: specimen.specimen_id,
            expected_revision: specimen.revision,
            decision: ReceiptDecision::Held,
            discrepancies: Vec::new(),
            policy_reference: None,
            scope: None,
            reason: Some("Synthetic late discrepancy".to_owned()),
        })
        .await;
    assert_eq!(held.unwrap_err().code(), ErrorCode::Conflict);
}
