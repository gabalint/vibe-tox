//! Interface contracts and acceptance tests (ticket #4555 step 5a; REQ-INT-001): helpers
//! the connection scenarios share. A connection writes and reads patient data only once a
//! second person approved its contract and its synthetic acceptance tests passed for it as
//! it is now. Synthetic data only.

use std::fs;

use lis_core::ErrorCode;
use lis_engine::{Engine, PartnerConnectionInput, UpdatePractitioner};
use lis_interfaces::partner::{
    AcceptanceState, ApprovePartnerContract, ApprovePartnerMappings, ContractEdge,
    DraftPartnerContract, DraftPartnerMapping, InterfaceContract, MappingKind, MappingValues,
    PartnerConnectionId, PartnerContract,
};

use crate::m2_lab::Role;
use crate::m2_steps::gate_codes;
use crate::m3_fixture::Staff;
use crate::partner_orders::{Orders, SENDER, oml, revision};

/// Synthetic terms, as a partner's IT and the laboratory might agree them.
pub(crate) fn terms() -> InterfaceContract {
    InterfaceContract {
        profile: "HL7 v2.5.1, synthetic profile".to_owned(),
        acknowledgements: "AA accepted; AE error in content; AR not accepted".to_owned(),
        corrections: "Corrections replace the earlier result; withdrawals remove it".to_owned(),
        edge: ContractEdge::Lis,
        engine: None,
        owners: "Partner interface analyst; laboratory manager".to_owned(),
        deviations: String::new(),
    }
}

/// The signed-in person drafts the connection's contract.
pub(crate) async fn draft_contract(engine: &Engine, connection_id: PartnerConnectionId) {
    engine
        .draft_partner_contract(DraftPartnerContract {
            connection_id,
            contract: terms(),
        })
        .await
        .unwrap();
}

/// The signed-in person, not the drafter, approves the draft and runs the acceptance tests.
pub(crate) async fn approve_and_test(
    engine: &Engine,
    connection_id: PartnerConnectionId,
) -> PartnerContract {
    let revision = engine
        .get_partner_connection(PartnerConnectionInput { connection_id })
        .await
        .unwrap()
        .connection
        .revision;
    engine
        .approve_partner_contract(ApprovePartnerContract {
            connection_id,
            expected_revision: revision,
            reason: "Synthetic: terms confirmed with the partner's IT".to_owned(),
            self_approval: false,
        })
        .await
        .unwrap();
    run_tests(engine, connection_id).await
}

/// Runs the acceptance tests of the connection as it is now.
pub(crate) async fn run_tests(
    engine: &Engine,
    connection_id: PartnerConnectionId,
) -> PartnerContract {
    engine
        .run_partner_acceptance_tests(PartnerConnectionInput { connection_id })
        .await
        .unwrap()
        .contract
}

#[tokio::test]
async fn an_order_connection_reads_nothing_until_its_contract_is_approved_and_tested() {
    let orders = Orders::checked().await;
    let id = orders.connection.connection.id;
    let engine = orders.bench.lab().engine();
    assert_eq!(
        orders.connection.contract.acceptance,
        AcceptanceState::NotRun
    );
    assert!(orders.connection.contract.current.is_none());
    fs::write(
        orders.input.path().join("clinic_0001.hl7"),
        oml(SENDER, "MSG-1"),
    )
    .unwrap();
    orders.read().await;
    assert!(orders.states().is_empty());

    // No tests before an approved contract; the drafter may not approve it themselves.
    let input = PartnerConnectionInput { connection_id: id };
    let refused = engine
        .run_partner_acceptance_tests(input)
        .await
        .unwrap_err();
    assert_eq!(refused.code(), ErrorCode::Conflict);
    orders.bench.sign_in(Staff::Desk(Role::Admin)).await;
    draft_contract(engine, id).await;
    let refused = engine
        .approve_partner_contract(ApprovePartnerContract {
            connection_id: id,
            expected_revision: revision(&orders.bench, id).await,
            reason: "Synthetic".to_owned(),
            self_approval: false,
        })
        .await
        .unwrap_err();
    assert_eq!(gate_codes(&refused), ["second_person_required"]);
    orders.bench.sign_in(Staff::Racer).await;
    let contract = approve_and_test(engine, id).await;
    assert_passed(&contract);
    assert_eq!(
        contract.current.as_ref().map(|current| current.version),
        Some(1)
    );
    let codes: Vec<&str> = contract
        .last_run
        .as_ref()
        .unwrap()
        .cases
        .iter()
        .map(|case| case.code.as_str())
        .collect();
    assert_eq!(
        codes,
        [
            "mapped_order",
            "replay",
            "changed_content",
            "unknown_test",
            "other_sender",
            "unknown_patient"
        ]
    );
    // Nothing the tests made is kept; the waiting file is read now.
    assert!(orders.states().is_empty());
    orders.read().await;
    assert_eq!(orders.states().len(), 1);
    out_of_date_after_a_mapping_change(&orders).await;
}

/// Where no practitioner has the tests' synthetic NPI, they register one as the person who
/// runs them, and keep nothing.
#[tokio::test]
async fn order_tests_pass_where_their_synthetic_provider_is_not_registered() {
    let orders = Orders::checked().await;
    let id = orders.connection.connection.id;
    let engine = orders.bench.lab().engine();
    orders.bench.sign_in(Staff::Desk(Role::Admin)).await;
    let practitioner = engine.list_practitioners().await.unwrap().remove(0);
    engine
        .update_practitioner(UpdatePractitioner {
            practitioner_id: practitioner.id,
            expected_revision: practitioner.revision,
            npi: "1000000004".to_owned(),
            family_name: practitioner.family_name,
            given_names: practitioner.given_names,
            active: true,
            reason: "Synthetic: NPI corrected".to_owned(),
        })
        .await
        .unwrap();
    draft_contract(engine, id).await;
    orders.bench.sign_in(Staff::Racer).await;
    assert_passed(&approve_and_test(engine, id).await);
    orders.bench.sign_in(Staff::Desk(Role::Admin)).await;
    let npis: Vec<String> = engine
        .list_practitioners()
        .await
        .unwrap()
        .iter()
        .map(|practitioner| practitioner.npi.as_str().to_owned())
        .collect();
    assert_eq!(npis, ["1000000004"]);
}

/// A new mapping version makes the tests out of date: nothing more is read until they pass
/// again.
async fn out_of_date_after_a_mapping_change(orders: &Orders) {
    let id = orders.connection.connection.id;
    let engine = orders.bench.lab().engine();
    orders.bench.sign_in(Staff::Desk(Role::Admin)).await;
    engine
        .draft_partner_mapping(DraftPartnerMapping {
            connection_id: id,
            kind: MappingKind::Test,
            lis_id: orders.bench.lab().orderables.panel.0.cast(),
            mapping: Some(MappingValues {
                partner_code: "SYN-PANEL".to_owned(),
                loinc: Some("12345-6".to_owned()),
                ucum_unit: None,
                sequence_role: Some("first_line".to_owned()),
            }),
        })
        .await
        .unwrap();
    orders.bench.sign_in(Staff::Racer).await;
    engine
        .approve_partner_mappings(ApprovePartnerMappings {
            connection_id: id,
            expected_revision: revision(&orders.bench, id).await,
            reason: "Synthetic: LOINC added".to_owned(),
            self_approval: false,
        })
        .await
        .unwrap();
    let detail = engine
        .get_partner_connection(PartnerConnectionInput { connection_id: id })
        .await
        .unwrap();
    assert_eq!(detail.contract.acceptance, AcceptanceState::OutOfDate);
    fs::write(
        orders.input.path().join("clinic_0002.hl7"),
        oml(SENDER, "MSG-2"),
    )
    .unwrap();
    orders.read().await;
    assert_eq!(orders.states().len(), 1);
    assert_passed(&run_tests(engine, id).await);
    orders.read().await;
    assert_eq!(orders.states().len(), 2);
}

/// The tests passed, every case of them.
pub(crate) fn assert_passed(contract: &PartnerContract) {
    let run = contract.last_run.as_ref().unwrap();
    assert_eq!(
        contract.acceptance,
        AcceptanceState::Passed,
        "{:?}",
        run.cases
    );
    assert!(run.cases.iter().all(|case| case.passed));
}
