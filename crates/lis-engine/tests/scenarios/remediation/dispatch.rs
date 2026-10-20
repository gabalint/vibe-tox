use super::*;
use lis_core::CommandError;
use lis_engine::Engine;

/// Registration in the table-driven registry authorization suite.
pub(crate) async fn remediation_call(
    engine: &Engine,
    name: &str,
) -> Option<Result<(), CommandError>> {
    if let Some(result) = directory_call(engine, name).await {
        return Some(result);
    }
    if let Some(result) = analytical_call(engine, name).await {
        return Some(result);
    }
    Some(match name {
        "get_container_history" => engine
            .get_container_history(GetContainerHistoryInput {
                container_id: lis_specimens::ContainerId::generate(),
                offset: 0,
                snapshot: None,
            })
            .await
            .map(drop),
        "list_excursions" => engine
            .list_excursions(ListExcursionsInput {
                offset: 0,
                snapshot: None,
            })
            .await
            .map(drop),
        "get_excursion" => engine
            .get_excursion(GetExcursionInput {
                excursion_id: lis_specimens::ExcursionId::generate(),
            })
            .await
            .map(drop),
        "get_deviation" => engine
            .get_deviation(GetDeviationInput {
                deviation_id: lis_analytical::DeviationId::generate(),
            })
            .await
            .map(drop),
        "get_delivery_obligation" => engine
            .get_delivery_obligation(GetDeliveryObligationInput {
                obligation_id: lis_reporting::ObligationId::generate(),
            })
            .await
            .map(drop),
        _ => return None,
    })
}

async fn directory_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    Some(match name {
        "record_intake_deferral" => engine.record_intake_deferral(intake()).await.map(drop),
        "list_intake_deferrals" => engine.list_intake_deferrals(intake_list()).await.map(drop),
        "resolve_intake_deferral" => engine
            .resolve_intake_deferral(ResolveIntakeDeferralInput {
                intake_id: lis_identity::IntakeDeferralId::generate(),
                expected_revision: 1,
                disposition: IntakeDisposition::Declined,
                subject_id: None,
                source_reference: "Synthetic source".into(),
                reason: "Source declined".into(),
            })
            .await
            .map(drop),
        "update_client" => engine
            .update_client(correction(lis_identity::ClientId::generate()))
            .await
            .map(drop),
        "directory_history" => engine
            .directory_history(DirectoryHistoryInput {
                record: DirectoryRecord::Client(lis_identity::ClientId::generate()),
            })
            .await
            .map(drop),
        "update_client_location" => engine
            .update_client_location(lis_identity::UpdateClientLocation {
                location_id: lis_identity::ClientLocationId::generate(),
                expected_revision: 1,
                code: "SYN-LOC".into(),
                name: "Synthetic location".into(),
                active: true,
                reason: "Verified correction".into(),
            })
            .await
            .map(drop),
        "update_practitioner" => engine
            .update_practitioner(lis_identity::UpdatePractitioner {
                practitioner_id: lis_identity::PractitionerId::generate(),
                expected_revision: 1,
                npi: "1234567893".into(),
                family_name: "Synthetic".into(),
                given_names: None,
                active: true,
                reason: "Verified correction".into(),
            })
            .await
            .map(drop),
        "list_laboratory_profiles" => engine.list_laboratory_profiles().await.map(drop),
        _ => return None,
    })
}

async fn analytical_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    Some(match name {
        "get_prep_batch" => engine
            .get_prep_batch(lis_engine::GetPrepBatchInput {
                prep_batch_id: lis_analytical::PrepBatchId::generate(),
            })
            .await
            .map(drop),
        "list_prep_batches" => engine
            .list_prep_batches(lis_analytical::ListPrepBatchesInput {
                method_version_id: lis_catalog::MethodVersionId::generate(),
                after: None,
                limit: 50,
            })
            .await
            .map(drop),
        "record_run_performing_site" => engine
            .record_run_performing_site(lis_analytical::RecordRunPerformingSiteInput {
                run_id: lis_analytical::RunId::generate(),
                expected_revision: 1,
                laboratory_profile_id: lis_identity::LaboratoryProfileId::generate(),
                source_reference: "Synthetic source".into(),
                reason: "Verified site".into(),
            })
            .await
            .map(drop),
        "resolve_receipt_chronology" => engine
            .resolve_receipt_chronology(lis_specimens::ResolveReceiptChronologyInput {
                specimen_id: lis_specimens::SpecimenId::generate(),
                expected_revision: 1,
                decision_ids: vec![],
                reason: "Source reviewed".into(),
            })
            .await
            .map(drop),
        "get_receipt_import_attempt" => engine
            .get_receipt_import_attempt(lis_engine::GetReceiptImportAttemptInput {
                receipt_id: lis_interfaces::ReceiptId::generate(),
            })
            .await
            .map(drop),
        _ => return None,
    })
}
