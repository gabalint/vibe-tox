//! Every command refuses a principal without its capability, even one holding
//! every other capability, and writes nothing (NFR-08, REQ-OPS-001).

use lis_core::{Capability, ErrorCode, Principal, UtcTimestamp};
use lis_specimens::{
    AssessExcursionInput, ConfigureLabelPrinterInput, ConfigureStorageLocationInput,
    ContainerHoldKind, ContainerId, CreateAliquotInput, DisposeContainerInput, ExcursionId,
    ExcursionOutcome, HoldId, LabelJobKind, LocationId, MarkContainerDepletedInput,
    MoveContainerInput, PlaceContainerHoldInput, PrintLabelsInput, PrinterId, PrinterTransport,
    RaiseNumberFloorsInput, ReceiptDecision, RecordExcursionInput, RecordReceiptDecisionInput,
    ReleaseContainerHoldInput, SpecimenError, SpecimenId, VolumeKind,
};
use lis_store::CommandContext;

use crate::{Fixture, accession_input, counts, fixture, run, run_as};

const ALL: [Capability; 14] = [
    Capability::Accession,
    Capability::EnterOrders,
    Capability::PerformAnalysis,
    Capability::ReviewResults,
    Capability::ReleaseReports,
    Capability::ManageQuality,
    Capability::Billing,
    Capability::ViewAudit,
    Capability::Export,
    Capability::BackupRestore,
    Capability::ManageUsers,
    Capability::ManageConfiguration,
    Capability::ApproveConfiguration,
    Capability::InstrumentIntake,
];

type Command = Box<dyn FnOnce(&mut CommandContext<'_>) -> Result<(), SpecimenError> + Send>;

fn commands(f: &Fixture) -> Vec<(Capability, Command)> {
    let detail = f.accession(1);
    let container_id = detail.specimens[0].containers[0].container_id;
    let mut commands = accession_commands(f, detail.specimens[0].specimen_id, container_id);
    commands.extend(container_commands(container_id));
    commands.extend(label_and_hold_commands(container_id));
    commands.extend(configuration_commands());
    commands.extend(floor_commands());
    commands
}

fn accession_commands(
    f: &Fixture,
    specimen_id: SpecimenId,
    container_id: ContainerId,
) -> Vec<(Capability, Command)> {
    let (order_id, items) = f.accepted_order(&[f.urine_test]);
    let accession = accession_input(order_id, items);
    vec![
        (
            Capability::Accession,
            Box::new(move |ctx| lis_specimens::create_accession(ctx, accession).map(drop)),
        ),
        (
            Capability::Accession,
            Box::new(move |ctx| {
                lis_specimens::record_receipt_decision(
                    ctx,
                    RecordReceiptDecisionInput {
                        specimen_id,
                        expected_revision: 1,
                        decision: ReceiptDecision::Accepted,
                        discrepancies: Vec::new(),
                        policy_reference: None,
                        scope: None,
                        reason: None,
                    },
                )
                .map(drop)
            }),
        ),
        (
            Capability::Accession,
            Box::new(move |ctx| {
                lis_specimens::create_aliquot(
                    ctx,
                    CreateAliquotInput {
                        parent_container_id: container_id,
                        expected_revision: 1,
                        volume_taken: None,
                        volume_unit: None,
                        volume_kind: VolumeKind::Unknown,
                    },
                )
                .map(drop)
            }),
        ),
    ]
}

fn container_commands(container_id: ContainerId) -> Vec<(Capability, Command)> {
    vec![
        (
            Capability::Accession,
            Box::new(move |ctx| {
                lis_specimens::mark_container_depleted(
                    ctx,
                    MarkContainerDepletedInput {
                        container_id,
                        expected_revision: 1,
                        reason: "Synthetic".to_owned(),
                    },
                )
                .map(drop)
            }),
        ),
        (
            Capability::Accession,
            Box::new(move |ctx| {
                lis_specimens::move_container(
                    ctx,
                    MoveContainerInput {
                        container_id,
                        expected_revision: 1,
                        location_id: LocationId::generate(),
                        reason: None,
                    },
                )
                .map(drop)
            }),
        ),
        (
            Capability::Accession,
            Box::new(move |ctx| {
                lis_specimens::dispose_container(
                    ctx,
                    DisposeContainerInput {
                        container_id,
                        expected_revision: 1,
                        reason: "Synthetic".to_owned(),
                    },
                )
                .map(drop)
            }),
        ),
    ]
}

fn label_and_hold_commands(container_id: ContainerId) -> Vec<(Capability, Command)> {
    vec![
        (
            Capability::Accession,
            Box::new(move |ctx| {
                lis_specimens::print_labels(
                    ctx,
                    PrintLabelsInput {
                        container_ids: vec![container_id],
                        printer_id: PrinterId::generate(),
                        kind: LabelJobKind::Print,
                        reason: None,
                    },
                )
                .map(drop)
            }),
        ),
        (
            Capability::Accession,
            Box::new(move |ctx| {
                lis_specimens::record_excursion(
                    ctx,
                    RecordExcursionInput {
                        location_id: LocationId::generate(),
                        observed_from: UtcTimestamp::parse("2026-01-01T00:00:00.000000000Z")
                            .unwrap(),
                        observed_to: None,
                        description: "Synthetic".to_owned(),
                    },
                )
                .map(drop)
            }),
        ),
        (
            Capability::ManageQuality,
            Box::new(move |ctx| {
                lis_specimens::place_hold(
                    ctx,
                    PlaceContainerHoldInput {
                        container_id,
                        kind: ContainerHoldKind::Quality,
                        reason: "Synthetic".to_owned(),
                    },
                )
                .map(drop)
            }),
        ),
        (
            Capability::ManageQuality,
            Box::new(|ctx| {
                lis_specimens::release_hold(
                    ctx,
                    ReleaseContainerHoldInput {
                        hold_id: HoldId::generate(),
                        expected_revision: 1,
                        reason: "Synthetic".to_owned(),
                    },
                )
                .map(drop)
            }),
        ),
        (
            Capability::ManageQuality,
            Box::new(|ctx| {
                lis_specimens::assess_excursion(
                    ctx,
                    AssessExcursionInput {
                        excursion_id: ExcursionId::generate(),
                        expected_revision: 1,
                        outcome: ExcursionOutcome::NoImpact,
                        container_ids: Vec::new(),
                        reason: "Synthetic".to_owned(),
                    },
                )
                .map(drop)
            }),
        ),
    ]
}

fn floor_commands() -> Vec<(Capability, Command)> {
    vec![(
        Capability::ManageQuality,
        Box::new(|ctx| {
            lis_specimens::raise_number_floors(
                ctx,
                RaiseNumberFloorsInput {
                    accession_floor: Some("A26000900".to_owned()),
                    container_floor: None,
                    reason: "Synthetic".to_owned(),
                },
            )
            .map(drop)
        }),
    )]
}

fn configuration_commands() -> Vec<(Capability, Command)> {
    vec![
        (
            Capability::ManageConfiguration,
            Box::new(|ctx| {
                lis_specimens::configure_location(
                    ctx,
                    ConfigureStorageLocationInput {
                        location_id: None,
                        expected_revision: None,
                        code: "fridge-9".to_owned(),
                        name: "Synthetic".to_owned(),
                        storage_condition: None,
                        active: true,
                    },
                )
                .map(drop)
            }),
        ),
        (
            Capability::ManageConfiguration,
            Box::new(|ctx| {
                lis_specimens::configure_printer(
                    ctx,
                    ConfigureLabelPrinterInput {
                        printer_id: None,
                        expected_revision: None,
                        code: "bench-9".to_owned(),
                        name: "Synthetic".to_owned(),
                        transport: PrinterTransport::Tcp,
                        ip_address: Some("192.0.2.9".to_owned()),
                        port: None,
                        queue: None,
                        active: true,
                    },
                    lis_labels::PrinterDestinations::LabNetwork,
                )
                .map(drop)
            }),
        ),
    ]
}

#[test]
fn every_command_refuses_a_principal_without_its_capability() {
    let f = fixture();
    let commands = commands(&f);
    assert_eq!(commands.len(), 14);
    for (required, command) in commands {
        let others: Vec<Capability> = ALL.into_iter().filter(|c| *c != required).collect();
        let before = counts(&f.t);
        let error = run(&f.t, &others, command).unwrap_err();
        assert_eq!(error.code(), ErrorCode::Forbidden, "{required:?}");
        assert_eq!(counts(&f.t), before);
    }
}

#[test]
fn every_human_command_refuses_a_system_actor_with_the_right_capability() {
    let f = fixture();
    let commands = commands(&f);
    assert_eq!(commands.len(), 14);
    for (required, command) in commands {
        let before = counts(&f.t);
        let principal = Principal::system("label-printer", [required]);
        let error = run_as(&f.t, principal, command).unwrap_err();
        assert_eq!(error.code(), ErrorCode::Forbidden, "{required:?}");
        assert_eq!(counts(&f.t), before);
    }
}
