//! Interfaces (ticket #4555, step 2): a released report as an HL7 ORU^R01 written to an
//! HL7 Results connection's folder, held until a second person approves the partner's code
//! mappings, never sent with a guessed code, and until the patient has an identifier for
//! PID-3 (#4676); and its correction sent with status C (REQ-INT-004, REQ-RPT-013,
//! REQ-RPT-012). Synthetic data only.

use std::fs;

use lis_core::{ErrorCode, Phi};
use lis_engine::{AddSubjectIdentifier, GetSubjectInput, PartnerConnectionInput};
use lis_identity::IdentifierInput;
use lis_interfaces::partner::{
    ApprovePartnerMappings, DraftPartnerMapping, MappingKind, MappingValues,
    PartnerConnectionSettings, PartnerMappings, PartnerMessageType, PartnerProtocol, Schedule,
    WaitReason,
};
use lis_reporting::ReportRevisionId;

use super::partner_pdfs::{AMENDMENT, Pdfs};
use crate::m2_lab::Role;
use crate::m2_steps::gate_codes;
use crate::m3_fixture::Staff;
use crate::partner_contract::{assert_passed, run_tests};

fn settings() -> PartnerConnectionSettings {
    PartnerConnectionSettings {
        name: "HL7 Results - Clinic A".to_owned(),
        partner: "CLINIC-A".to_owned(),
        message_type: PartnerMessageType::Hl7Oru,
        protocol: PartnerProtocol::Folder,
        rules: Vec::new(),
        schedule: Schedule::Immediately,
        file_name_pattern: "{report}_r{revision}.hl7".to_owned(),
        sftp: None,
        intake: None,
        mllp: None,
        mllp_listener: None,
    }
}

async fn mappings(hl7: &Pdfs, staff: Staff) -> PartnerMappings {
    hl7.bench.sign_in(staff).await;
    hl7.bench
        .lab()
        .engine()
        .list_partner_mappings(PartnerConnectionInput {
            connection_id: hl7.connection.connection.id,
        })
        .await
        .unwrap()
}

/// The administrator drafts the partner's code for every held test and analyte.
pub(super) async fn draft_held(hl7: &Pdfs) {
    let held = mappings(hl7, Staff::Desk(Role::Admin)).await;
    assert!(!held.rows.is_empty());
    assert!(
        held.rows
            .iter()
            .all(|row| row.held && row.approved.is_none())
    );
    assert!(held.rows.iter().any(|row| row.kind == MappingKind::Test));
    for row in held.rows {
        hl7.bench
            .lab()
            .engine()
            .draft_partner_mapping(DraftPartnerMapping {
                connection_id: hl7.connection.connection.id,
                kind: row.kind,
                lis_id: row.lis_id,
                mapping: Some(MappingValues {
                    partner_code: format!("CA-{}", row.lis_code),
                    loinc: None,
                    ucum_unit: (row.kind == MappingKind::Analyte).then(|| "ng/mL".to_owned()),
                    sequence_role: None,
                }),
            })
            .await
            .unwrap();
    }
}

/// The accessioner adds the patient's synthetic MRN, which the fixture registered without;
/// the patient's new record revision.
pub(super) async fn add_identifier(hl7: &Pdfs) -> i64 {
    hl7.bench.sign_in(Staff::Desk(Role::Accessioner)).await;
    let engine = hl7.bench.lab().engine();
    let subject_id = hl7.bench.lab().parties.subject;
    let subject = engine
        .get_subject(GetSubjectInput { subject_id })
        .await
        .unwrap();
    engine
        .add_subject_identifier(AddSubjectIdentifier {
            subject_id,
            expected_revision: subject.revision,
            identifier: IdentifierInput {
                assigning_authority: "CLINIC-A".to_owned(),
                identifier_type: "MR".to_owned(),
                value: Phi::new("MRN-00000".to_owned()),
            },
        })
        .await
        .unwrap()
        .subject_revision
}

/// #4676: the patient gets an identifier after release and the impact review it opens
/// records no impact, so a message held for PID-3 can go; the caller delivers next.
pub(super) async fn identify(hl7: &Pdfs, revision_id: ReportRevisionId) {
    let subject_revision = add_identifier(hl7).await;
    hl7.deliver().await;
    assert_eq!(hl7.rows().await[0].waiting, Some(WaitReason::ImpactReview));
    hl7.bench
        .assess_no_impact(
            revision_id,
            vec![format!("identity_changed:{subject_revision}")],
        )
        .await;
}

/// The drafter may not approve under the default policy; a second administrator does.
pub(super) async fn approve(hl7: &Pdfs) {
    let current = mappings(hl7, Staff::Desk(Role::Admin)).await;
    assert!(current.rows.iter().all(|row| row.draft.is_some()));
    let revision = hl7
        .bench
        .lab()
        .engine()
        .get_partner_connection(PartnerConnectionInput {
            connection_id: hl7.connection.connection.id,
        })
        .await
        .unwrap()
        .connection
        .revision;
    let input = || ApprovePartnerMappings {
        connection_id: hl7.connection.connection.id,
        expected_revision: revision,
        reason: "Synthetic: codes confirmed with the clinic's IT".to_owned(),
        self_approval: false,
    };
    let refused = hl7
        .bench
        .lab()
        .engine()
        .approve_partner_mappings(input())
        .await
        .unwrap_err();
    assert_eq!(refused.code(), ErrorCode::GateBlocked);
    assert_eq!(gate_codes(&refused), ["second_person_required"]);
    hl7.bench.sign_in(Staff::Racer).await;
    let approved = hl7
        .bench
        .lab()
        .engine()
        .approve_partner_mappings(input())
        .await
        .unwrap();
    assert_eq!(approved.version, Some(1));
    // A new mapping version needs the acceptance tests again (REQ-INT-001).
    let id = hl7.connection.connection.id;
    assert_passed(&run_tests(hl7.bench.lab().engine(), id).await);
}

#[tokio::test]
async fn an_hl7_result_waits_for_approved_codes_and_its_correction_follows() {
    let hl7 = Pdfs::start_with(settings()).await;
    let released = hl7.release(None).await;
    hl7.deliver().await;
    // REQ-INT-004, REQ-INT-001: no approved code, so the acceptance tests failed and
    // nothing is written.
    assert!(hl7.files().is_empty());
    assert_eq!(hl7.rows().await[0].waiting, Some(WaitReason::NotAccepted));
    let contract = &hl7.connection.contract;
    let problems: Vec<_> = contract
        .last_run
        .as_ref()
        .unwrap()
        .cases
        .iter()
        .filter_map(|case| case.problem.as_deref())
        .collect();
    assert_eq!(problems, ["no_approved_mappings", "no_approved_mappings"]);

    draft_held(&hl7).await;
    approve(&hl7).await;
    hl7.deliver().await;
    // #4676: the patient has no identifier, so PID-3 would be empty: nothing is written,
    // and the row says why; the report stays released.
    assert!(hl7.files().is_empty());
    assert_eq!(
        hl7.rows().await[0].waiting,
        Some(WaitReason::NoPatientIdentifier)
    );
    // The patient's record changes after release, so its impact review decides (L13);
    // with no impact, the message goes with the identifier now recorded.
    identify(&hl7, released.revision_id).await;
    hl7.deliver().await;
    let number = hl7.report().await.report_number;
    let name = format!("{number}_r1.hl7");
    assert_eq!(hl7.files(), std::slice::from_ref(&name));
    let text = fs::read_to_string(hl7.folder.path().join(&name)).unwrap();
    let segments: Vec<&str> = text.split('\r').collect();
    assert!(segments[0].starts_with("MSH|^~\\&|VIBETOX LIS|"));
    assert!(segments[0].contains("|CLINIC-A|"));
    assert!(segments[0].contains("|ORU^R01^ORU_R01|"));
    let pid = segments.iter().find(|s| s.starts_with("PID|")).unwrap();
    assert!(pid.starts_with("PID|1||MRN-00000^^^CLINIC-A^MR||"), "{pid}");
    let obr = segments.iter().find(|s| s.starts_with("OBR|")).unwrap();
    assert!(obr.ends_with("|F"), "{obr}");
    let obx: Vec<&&str> = segments.iter().filter(|s| s.starts_with("OBX|")).collect();
    assert!(!obx.is_empty());
    assert!(obx.iter().all(|s| s.contains("|CA-SYN-")));
    assert!(obx.iter().any(|s| s.contains("ng/mL^^UCUM")));
    // A second pass writes nothing more.
    hl7.deliver().await;
    assert_eq!(hl7.files().len(), 1);

    hl7.correct_result().await;
    hl7.approve_correction().await;
    hl7.release(Some(AMENDMENT)).await;
    hl7.deliver().await;
    let corrected = fs::read_to_string(hl7.folder.path().join(format!("{number}_r2.hl7"))).unwrap();
    let obr = corrected
        .split('\r')
        .find(|s| s.starts_with("OBR|"))
        .unwrap();
    assert!(obr.ends_with("|C"), "{obr}");
}
