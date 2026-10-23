use super::*;
use crate::m2_lab::{Lab, Role};

#[tokio::test]
async fn material_and_deviation_reads_use_the_actual_granted_quality_authority() {
    let lab = Lab::start(lis_analytical::RevisionIndicator::None).await;
    let location_id = setup_storage(&lab).await;
    let sample = &lab.samples[0];
    lab.sign_in(Role::Accessioner).await;
    let excursion = lab
        .engine()
        .record_excursion(lis_specimens::RecordExcursionInput {
            location_id,
            observed_from: crate::fixture::start_time(),
            observed_to: None,
            description: "Synthetic verified storage interruption".into(),
        })
        .await
        .unwrap();
    lab.engine()
        .get_container_history(GetContainerHistoryInput {
            container_id: sample.container_id,
            offset: 0,
            snapshot: None,
        })
        .await
        .unwrap();
    let actor: String = lab.t().conn().query_row("SELECT principal_id FROM audit_events WHERE action='specimen.record_excursion' ORDER BY seq DESC LIMIT 1", [], |r|r.get(0)).unwrap();
    assert_access(
        lab.t(),
        "specimen.view_material_history",
        &actor,
        "accession",
        Some(&sample.container_id.to_string()),
    );
    lab.sign_in(Role::Quality).await;
    let deviation = lab
        .engine()
        .open_deviation(lis_analytical::OpenDeviationInput {
            scope: lis_analytical::DeviationTarget::Run(lab.run),
            analyte_id: None,
            description: CANARY.into(),
            reason: "Synthetic quality investigation".into(),
        })
        .await
        .unwrap();
    let quality: String = lab.t().conn().query_row("SELECT principal_id FROM audit_events WHERE action='analytical.open_deviation' ORDER BY seq DESC LIMIT 1", [], |r|r.get(0)).unwrap();
    assert_quality_material_access(&lab, &quality).await;
    assert_quality_incident_access(
        &lab,
        &quality,
        excursion.excursion_id,
        deviation.deviation_id,
    )
    .await;
    assert!(!audit_text(lab.t()).contains(CANARY));
    lab.sign_in(Role::Accessioner).await;
    assert_eq!(
        code_of(
            lab.engine()
                .get_deviation(GetDeviationInput {
                    deviation_id: deviation.deviation_id
                })
                .await
        ),
        ErrorCode::Forbidden
    );
}

/// Material history names who recorded each event by the account's current display name.
#[tokio::test]
async fn material_history_names_who_recorded_each_event() {
    let lab = Lab::start(lis_analytical::RevisionIndicator::None).await;
    let container_id = lab.samples[0].container_id;
    lab.sign_in(Role::Quality).await;
    lab.engine()
        .place_hold(lis_specimens::PlaceContainerHoldInput {
            container_id,
            kind: lis_specimens::ContainerHoldKind::Quality,
            reason: "Synthetic leaking container".into(),
        })
        .await
        .unwrap();
    let history = lab
        .engine()
        .get_container_history(GetContainerHistoryInput {
            container_id,
            offset: 0,
            snapshot: None,
        })
        .await
        .unwrap();
    let hold = history.holds.first().expect("the placed hold");
    let name: String = lab
        .t()
        .conn()
        .query_row(
            "SELECT display_name FROM auth_users WHERE id = ?1",
            [&hold.actor],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(hold.actor_display.as_deref(), Some(name.as_str()));
}

async fn setup_storage(lab: &Lab) -> lis_specimens::LocationId {
    lab.sign_in(Role::Admin).await;
    let location = lab
        .engine()
        .configure_location(lis_specimens::ConfigureStorageLocationInput {
            location_id: None,
            expected_revision: None,
            code: "syn-history".into(),
            name: "Synthetic storage".into(),
            storage_condition: None,
            active: true,
        })
        .await
        .unwrap();
    let sample = &lab.samples[0];
    assert_eq!(
        code_of(
            lab.engine()
                .get_container_history(GetContainerHistoryInput {
                    container_id: sample.container_id,
                    offset: 0,
                    snapshot: None,
                })
                .await
        ),
        ErrorCode::Forbidden
    );
    location.location_id
}

async fn assert_quality_material_access(lab: &Lab, quality: &str) {
    let sample = &lab.samples[0];
    let found = lab
        .engine()
        .find_container(lis_engine::FindContainerInput {
            label_code: sample.label_code.clone(),
        })
        .await
        .unwrap();
    lab.engine()
        .get_accession(lis_engine::GetAccessionInput {
            accession_id: found.accession_id,
        })
        .await
        .unwrap();
    lab.engine()
        .get_container_history(GetContainerHistoryInput {
            container_id: sample.container_id,
            offset: 0,
            snapshot: None,
        })
        .await
        .unwrap();
    assert_access(
        lab.t(),
        "specimen.find_container",
        quality,
        "manage_quality",
        Some(&sample.container_id.to_string()),
    );
    assert_access(
        lab.t(),
        "specimen.view_accession",
        quality,
        "manage_quality",
        Some(&found.accession_id.to_string()),
    );
    assert_access(
        lab.t(),
        "specimen.view_material_history",
        quality,
        "manage_quality",
        Some(&sample.container_id.to_string()),
    );
}

async fn assert_quality_incident_access(
    lab: &Lab,
    quality: &str,
    excursion_id: lis_specimens::ExcursionId,
    deviation_id: lis_analytical::DeviationId,
) {
    lab.engine()
        .list_excursions(ListExcursionsInput {
            offset: 0,
            snapshot: None,
        })
        .await
        .unwrap();
    lab.engine()
        .get_excursion(GetExcursionInput { excursion_id })
        .await
        .unwrap();
    lab.engine()
        .get_deviation(GetDeviationInput { deviation_id })
        .await
        .unwrap();
    assert_access(
        lab.t(),
        "specimen.list_excursions",
        quality,
        "manage_quality",
        None,
    );
    assert_access(
        lab.t(),
        "specimen.view_excursion",
        quality,
        "manage_quality",
        Some(&excursion_id.to_string()),
    );
    assert_access(
        lab.t(),
        "analytical.get_deviation",
        quality,
        "manage_quality",
        Some(&deviation_id.to_string()),
    );
}
