//! The acceptance tests of an HL7 Orders connection (ticket #4555 step 5a; REQ-INT-001).
//! Synthetic order messages go through the same receiving rules as real ones, with the
//! connection's settings and approved code mappings, inside one transaction that is rolled
//! back: nothing they make is kept, and none of it is audited. A synthetic patient and
//! ordering provider are registered there first, as the person who runs the tests. Each case expects one outcome: an order
//! made, a replay ignored, changed content held, an unknown test or another sender refused,
//! and an unknown patient waiting or refused as the connection says; over MLLP, each with
//! the acknowledgement it gets (AA, AE or AR).

use lis_core::{Actor, Capability, CorrelationId, Phi, Principal};
use lis_identity::{DemographicsInput, IdentifierInput, RegisterPractitioner, RegisterSubject};
use lis_interfaces::partner::{
    AcceptanceCase, AckCode, IntakeSource, MappingKind, ReceivedOutcome, UnknownPatient,
};
use lis_store::{CommandContext, Database, StoreError};
use sha2::{Digest, Sha256};

use super::IntakeError;
use super::decide::{ReadFile, receive};

/// The synthetic ordering provider's NPI: a valid check digit, no real person.
const NPI: &str = "1234567893";
/// The synthetic patient's identifier, under the connection's sending facility.
const PATIENT: &str = "ACCEPTANCE-00001";

/// How the rolled-back transaction ends: with the cases, or with what stopped them.
enum DryRun {
    Done(Vec<AcceptanceCase>),
    Failed(IntakeError),
}

impl From<StoreError> for DryRun {
    fn from(error: StoreError) -> Self {
        Self::Failed(IntakeError::Store(error))
    }
}

/// Runs the cases as `caller`, who asked for them, and rolls everything back. Blocking.
/// Registering the synthetic provider needs a signed-in person; the cases hold only the
/// capabilities the receiving rules and the fixtures need.
pub(crate) fn order_cases(
    db: &Database,
    caller: &Actor,
    source: IntakeSource,
) -> Result<Vec<AcceptanceCase>, IntakeError> {
    let &Actor::User {
        user_id,
        session_id,
        workstation_id,
    } = caller
    else {
        return Err(lis_identity::IdentityError::UserRequired.into());
    };
    let principal = Principal::user(
        user_id,
        session_id,
        workstation_id,
        [Capability::EnterOrders, Capability::ManageConfiguration],
    );
    let ran = db.write_blocking(principal, CorrelationId::generate(), move |ctx| {
        let cases = fixtures(ctx, &source).map_err(DryRun::Failed)?;
        Err::<(), DryRun>(DryRun::Done(cases))
    });
    match ran {
        Err(DryRun::Done(cases)) => Ok(cases),
        Err(DryRun::Failed(error)) => Err(error),
        Ok(()) => Ok(Vec::new()),
    }
}

fn fixtures(
    ctx: &mut CommandContext<'_>,
    source: &IntakeSource,
) -> Result<Vec<AcceptanceCase>, IntakeError> {
    let Some(code) = mapped_code(ctx, source)? else {
        return Ok(vec![case("mapped_order", Some("no_test_mapping"))]);
    };
    register_fixtures(ctx, source)?;
    let sender = source.intake.accept_from.as_str();
    let listens = source.destination.mllp_listener.is_some();
    let mut run = Run {
        ctx,
        source,
        listens,
    };
    let order = oml(sender, "ACCEPTANCE-1", "ACCEPTANCE-ORD-1", &code, PATIENT);
    let unknown_patient = match source.intake.unknown_patient {
        UnknownPatient::Hold => Expect::Waiting,
        UnknownPatient::Reject => Expect::Rejected("patient_unknown"),
    };
    Ok(vec![
        run.check("mapped_order", &order, Expect::Order, AckCode::Accepted)?,
        run.check("replay", &order, Expect::Duplicate, AckCode::Accepted)?,
        run.check(
            "changed_content",
            &oml(sender, "ACCEPTANCE-1", "ACCEPTANCE-ORD-2", &code, PATIENT),
            Expect::Held("changed_content"),
            AckCode::Accepted,
        )?,
        run.check(
            "unknown_test",
            &oml(
                sender,
                "ACCEPTANCE-2",
                "ACCEPTANCE-ORD-3",
                "ACCEPTANCE-UNMAPPED",
                PATIENT,
            ),
            Expect::Rejected("unknown_test"),
            AckCode::Error,
        )?,
        run.check(
            "other_sender",
            &oml(
                "ACCEPTANCE-OTHER",
                "ACCEPTANCE-3",
                "ACCEPTANCE-ORD-4",
                &code,
                PATIENT,
            ),
            Expect::Rejected("not_accepted_sender"),
            AckCode::Rejected,
        )?,
        run.check(
            "unknown_patient",
            &oml(
                sender,
                "ACCEPTANCE-4",
                "ACCEPTANCE-ORD-5",
                &code,
                "ACCEPTANCE-99999",
            ),
            unknown_patient,
            AckCode::Accepted,
        )?,
    ])
}

/// What a case expects the message to become.
#[derive(Clone, Copy)]
enum Expect {
    Order,
    Duplicate,
    Waiting,
    Held(&'static str),
    Rejected(&'static str),
}

impl Expect {
    fn met_by(self, outcome: &ReceivedOutcome) -> bool {
        match (self, outcome) {
            (Self::Order, ReceivedOutcome::OrderCreated { .. })
            | (Self::Duplicate, ReceivedOutcome::Duplicate { .. })
            | (Self::Waiting, ReceivedOutcome::WaitingPatient { .. }) => true,
            (Self::Held(code), ReceivedOutcome::Held { reason })
            | (Self::Rejected(code), ReceivedOutcome::Rejected { reason }) => code == *reason,
            _ => false,
        }
    }
}

/// The cases' shared transaction. Over MLLP each case also checks the acknowledgement.
struct Run<'c, 'a, 's> {
    ctx: &'c mut CommandContext<'a>,
    source: &'s IntakeSource,
    listens: bool,
}

impl Run<'_, '_, '_> {
    fn check(
        &mut self,
        code: &str,
        message: &str,
        expect: Expect,
        ack: AckCode,
    ) -> Result<AcceptanceCase, IntakeError> {
        let bytes = message.as_bytes().to_vec();
        let file = ReadFile {
            name: format!("{code}.hl7"),
            sha256: hex::encode(Sha256::digest(&bytes)),
            bytes,
            peer: self.listens.then(|| "acceptance".to_owned()),
        };
        let (id, outcome) = receive(self.ctx, self.source, &file)?;
        if !expect.met_by(&outcome) {
            let became = outcome.reason().unwrap_or("unexpected_outcome");
            return Ok(case(code, Some(became)));
        }
        if self.listens {
            let answered = lis_interfaces::partner::received_ack(self.ctx.tx(), id)?;
            if answered.map(|(code, _)| code) != Some(ack) {
                return Ok(case(code, Some("ack_mismatch")));
            }
        }
        Ok(case(code, None))
    }
}

fn case(code: &str, problem: Option<&str>) -> AcceptanceCase {
    AcceptanceCase {
        code: code.to_owned(),
        passed: problem.is_none(),
        problem: problem.map(str::to_owned),
    }
}

/// The partner code of the connection's first approved test mapping.
fn mapped_code(
    ctx: &CommandContext<'_>,
    source: &IntakeSource,
) -> Result<Option<String>, IntakeError> {
    let mappings = lis_interfaces::partner::approved_mappings(ctx.tx(), source.connection_id)?;
    Ok(mappings.and_then(|mappings| {
        mappings
            .mappings
            .into_iter()
            .find(|mapping| mapping.kind == MappingKind::Test)
            .map(|mapping| mapping.values.partner_code)
    }))
}

/// The synthetic provider, unless the NPI is registered already, and the synthetic patient.
fn register_fixtures(
    ctx: &mut CommandContext<'_>,
    source: &IntakeSource,
) -> Result<(), IntakeError> {
    let registered = lis_identity::register_practitioner(
        ctx,
        RegisterPractitioner {
            npi: NPI.to_owned(),
            family_name: "Acceptance".to_owned(),
            given_names: Some("Synthetic".to_owned()),
        },
    );
    match registered {
        Ok(_) | Err(lis_identity::IdentityError::Duplicate(_)) => {}
        Err(error) => return Err(error.into()),
    }
    let demographics = DemographicsInput {
        family_name: Phi::new("Acceptance".to_owned()),
        given_names: Some(Phi::new("Synthetic".to_owned())),
        birth_date: Phi::new("1900-01-01".to_owned()),
        administrative_sex: None,
    };
    let candidates = lis_identity::find_subject_matches(
        ctx.tx(),
        &lis_identity::SubjectMatchQuery {
            demographics: Some(demographics.clone()),
            identifiers: Vec::new(),
        },
    )?
    .candidates;
    lis_identity::register_subject(
        ctx,
        RegisterSubject {
            demographics,
            source: "Synthetic acceptance test".to_owned(),
            identifiers: vec![IdentifierInput {
                assigning_authority: source.intake.accept_from.clone(),
                identifier_type: "MR".to_owned(),
                value: Phi::new(PATIENT.to_owned()),
            }],
            acknowledged_candidates: candidates,
        },
    )?;
    Ok(())
}

/// A synthetic OML^O21 from `sender` for `patient`, ordering `code`.
fn oml(sender: &str, control_id: &str, placer: &str, code: &str, patient: &str) -> String {
    [
        format!("MSH|^~\\&|ACCEPTANCE|{sender}|OPENBGA TOX|LAB|20260101120000||OML^O21|{control_id}|T|2.5.1"),
        format!("PID|1||{patient}^^^{sender}^MR||Acceptance^Synthetic||19000101"),
        format!("ORC|NW|{placer}^ACCEPTANCE||||||||||{NPI}^Acceptance"),
        format!("OBR|1|{placer}^ACCEPTANCE||{code}^Synthetic^L"),
    ]
    .join("\r")
}
