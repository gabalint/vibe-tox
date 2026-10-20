//! Accessioning, specimens, containers, holds, excursions, label printing
//! and their configuration (REQ-SPC-001–003, REQ-SPC-005, REQ-SPC-008,
//! REQ-OPS-001).
//!
//! An accession and a container looked up by label are clinical records of
//! a subject, so `get_accession` and `find_container` are audited reads.
//! Printers and locations are laboratory configuration without patient
//! data; any signed-in user may list them, because accessioning and
//! configuration both need them.

pub(crate) mod chronology;
pub(crate) mod material;

use lis_core::{Capability, CommandError};
use lis_specimens::{
    AccessionDetail, AccessionId, AssessExcursionInput, ConfigureLabelPrinterInput,
    ConfigureStorageLocationInput, ContainerHoldView, ContainerLookup, CreateAccessionInput,
    CreateAliquotInput, DisposeContainerInput, ExcursionView, LabelJobView, LabelPrinterView,
    MarkContainerDepletedInput, MoveContainerInput, PlaceContainerHoldInput, PrintLabelsInput,
    RecordExcursionInput, RecordReceiptDecisionInput, ReleaseContainerHoldInput, SpecimenError,
    StorageLocationView, actions,
};
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use crate::engine::Engine;
use crate::pipeline::{access, domain_error};
use crate::registry::{Access, CommandSpec, spec};

const ACCESSION: Access = Access::Capability(Capability::Accession);
const MATERIAL_READ: Access =
    Access::EitherCapability(Capability::Accession, Capability::ManageQuality);
const MANAGE_QUALITY: Access = Access::Capability(Capability::ManageQuality);
const MANAGE_CONFIGURATION: Access = Access::Capability(Capability::ManageConfiguration);

pub(crate) const CREATE_ACCESSION: CommandSpec =
    spec("create_accession", actions::CREATE_ACCESSION, ACCESSION);
pub(crate) const GET_ACCESSION: CommandSpec =
    spec("get_accession", actions::VIEW_ACCESSION, MATERIAL_READ);
pub(crate) const FIND_CONTAINER: CommandSpec =
    spec("find_container", actions::FIND_CONTAINER, MATERIAL_READ);
pub(crate) const RECORD_RECEIPT_DECISION: CommandSpec = spec(
    "record_receipt_decision",
    actions::RECORD_RECEIPT_DECISION,
    ACCESSION,
);
pub(crate) const CREATE_ALIQUOT: CommandSpec =
    spec("create_aliquot", actions::CREATE_ALIQUOT, ACCESSION);
pub(crate) const MARK_CONTAINER_DEPLETED: CommandSpec =
    spec("mark_container_depleted", actions::MARK_DEPLETED, ACCESSION);
pub(crate) const MOVE_CONTAINER: CommandSpec =
    spec("move_container", actions::MOVE_CONTAINER, ACCESSION);
pub(crate) const DISPOSE_CONTAINER: CommandSpec =
    spec("dispose_container", actions::DISPOSE_CONTAINER, ACCESSION);
pub(crate) const PRINT_LABELS: CommandSpec = spec("print_labels", actions::PRINT_LABEL, ACCESSION);
pub(crate) const LIST_PRINTERS: CommandSpec =
    spec("list_printers", "specimen.list_printers", Access::Session);
pub(crate) const LIST_LOCATIONS: CommandSpec =
    spec("list_locations", "specimen.list_locations", Access::Session);
pub(crate) const PLACE_HOLD: CommandSpec = spec("place_hold", actions::PLACE_HOLD, MANAGE_QUALITY);
pub(crate) const RELEASE_HOLD: CommandSpec =
    spec("release_hold", actions::RELEASE_HOLD, MANAGE_QUALITY);
pub(crate) const RECORD_EXCURSION: CommandSpec =
    spec("record_excursion", actions::RECORD_EXCURSION, ACCESSION);
pub(crate) const ASSESS_EXCURSION: CommandSpec = spec(
    "assess_excursion",
    actions::ASSESS_EXCURSION,
    MANAGE_QUALITY,
);
pub(crate) const CONFIGURE_LOCATION: CommandSpec = spec(
    "configure_location",
    actions::CONFIGURE_LOCATION,
    MANAGE_CONFIGURATION,
);
pub(crate) const CONFIGURE_PRINTER: CommandSpec = spec(
    "configure_printer",
    actions::CONFIGURE_PRINTER,
    MANAGE_CONFIGURATION,
);

/// Input of `get_accession`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GetAccessionInput {
    /// The accession to open.
    pub accession_id: AccessionId,
}

/// Input of `find_container`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct FindContainerInput {
    /// The scanned or typed label code.
    pub label_code: String,
}

domain_error!(SpecimenError, SpecimenError::field_errors);

fn audit_accession_response(
    ctx: &mut CommandContext<'_>,
    action: &'static str,
    detail: AccessionDetail,
) -> Result<AccessionDetail, SpecimenError> {
    ctx.audit(access(action).record("accession", detail.accession_id))?;
    Ok(detail)
}

fn audit_hold_response(
    ctx: &mut CommandContext<'_>,
    action: &'static str,
    detail: ContainerHoldView,
) -> Result<ContainerHoldView, SpecimenError> {
    ctx.audit(access(action).record("container", detail.container_id))?;
    Ok(detail)
}

fn audit_excursion_response(
    ctx: &mut CommandContext<'_>,
    action: &'static str,
    detail: ExcursionView,
) -> Result<ExcursionView, SpecimenError> {
    ctx.audit(access(action).record("excursion", detail.excursion_id))?;
    Ok(detail)
}

impl Engine {
    /// `create_accession`.
    pub async fn create_accession(
        &self,
        input: CreateAccessionInput,
    ) -> Result<AccessionDetail, CommandError> {
        self.run(&CREATE_ACCESSION, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_specimens::create_accession(ctx, input)?;
                audit_accession_response(ctx, CREATE_ACCESSION.action, detail)
            })
            .await
        })
        .await
    }

    /// `get_accession` (audited read).
    pub async fn get_accession(
        &self,
        input: GetAccessionInput,
    ) -> Result<AccessionDetail, CommandError> {
        self.run(&GET_ACCESSION, |call| async move {
            self.audited_read(
                &call,
                move |conn| lis_specimens::get_accession(conn, input.accession_id),
                |detail| access(GET_ACCESSION.action).record("accession", detail.accession_id),
            )
            .await
        })
        .await
    }

    /// `find_container` (audited read).
    pub async fn find_container(
        &self,
        input: FindContainerInput,
    ) -> Result<ContainerLookup, CommandError> {
        self.run(&FIND_CONTAINER, |call| async move {
            self.audited_read(
                &call,
                move |conn| lis_specimens::find_container(conn, &input.label_code),
                |found| {
                    access(FIND_CONTAINER.action).record("container", found.container.container_id)
                },
            )
            .await
        })
        .await
    }

    /// `record_receipt_decision`.
    pub async fn record_receipt_decision(
        &self,
        input: RecordReceiptDecisionInput,
    ) -> Result<AccessionDetail, CommandError> {
        self.run(&RECORD_RECEIPT_DECISION, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_specimens::record_receipt_decision(ctx, input)?;
                audit_accession_response(ctx, RECORD_RECEIPT_DECISION.action, detail)
            })
            .await
        })
        .await
    }

    /// `create_aliquot`.
    pub async fn create_aliquot(
        &self,
        input: CreateAliquotInput,
    ) -> Result<AccessionDetail, CommandError> {
        self.run(&CREATE_ALIQUOT, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_specimens::create_aliquot(ctx, input)?;
                audit_accession_response(ctx, CREATE_ALIQUOT.action, detail)
            })
            .await
        })
        .await
    }

    /// `mark_container_depleted`.
    pub async fn mark_container_depleted(
        &self,
        input: MarkContainerDepletedInput,
    ) -> Result<AccessionDetail, CommandError> {
        self.run(&MARK_CONTAINER_DEPLETED, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_specimens::mark_container_depleted(ctx, input)?;
                audit_accession_response(ctx, MARK_CONTAINER_DEPLETED.action, detail)
            })
            .await
        })
        .await
    }

    /// `move_container`.
    pub async fn move_container(
        &self,
        input: MoveContainerInput,
    ) -> Result<AccessionDetail, CommandError> {
        self.run(&MOVE_CONTAINER, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_specimens::move_container(ctx, input)?;
                audit_accession_response(ctx, MOVE_CONTAINER.action, detail)
            })
            .await
        })
        .await
    }

    /// `dispose_container`.
    pub async fn dispose_container(
        &self,
        input: DisposeContainerInput,
    ) -> Result<AccessionDetail, CommandError> {
        self.run(&DISPOSE_CONTAINER, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_specimens::dispose_container(ctx, input)?;
                audit_accession_response(ctx, DISPOSE_CONTAINER.action, detail)
            })
            .await
        })
        .await
    }

    /// `print_labels`: records the jobs; the label worker sends them after commit.
    pub async fn print_labels(
        &self,
        input: PrintLabelsInput,
    ) -> Result<Vec<LabelJobView>, CommandError> {
        self.run(&PRINT_LABELS, |call| async move {
            self.write(&call, move |ctx| lis_specimens::print_labels(ctx, input))
                .await
        })
        .await
    }

    /// `list_printers`.
    pub async fn list_printers(&self) -> Result<Vec<LabelPrinterView>, CommandError> {
        self.run(&LIST_PRINTERS, |call| async move {
            self.read(&call, lis_specimens::list_printers).await
        })
        .await
    }

    /// `list_locations`.
    pub async fn list_locations(&self) -> Result<Vec<StorageLocationView>, CommandError> {
        self.run(&LIST_LOCATIONS, |call| async move {
            self.read(&call, lis_specimens::list_locations).await
        })
        .await
    }

    /// `place_hold`.
    pub async fn place_hold(
        &self,
        input: PlaceContainerHoldInput,
    ) -> Result<ContainerHoldView, CommandError> {
        self.run(&PLACE_HOLD, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_specimens::place_hold(ctx, input)?;
                audit_hold_response(ctx, PLACE_HOLD.action, detail)
            })
            .await
        })
        .await
    }

    /// `release_hold`.
    pub async fn release_hold(
        &self,
        input: ReleaseContainerHoldInput,
    ) -> Result<ContainerHoldView, CommandError> {
        self.run(&RELEASE_HOLD, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_specimens::release_hold(ctx, input)?;
                audit_hold_response(ctx, RELEASE_HOLD.action, detail)
            })
            .await
        })
        .await
    }

    /// `record_excursion`.
    pub async fn record_excursion(
        &self,
        input: RecordExcursionInput,
    ) -> Result<ExcursionView, CommandError> {
        self.run(&RECORD_EXCURSION, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_specimens::record_excursion(ctx, input)?;
                audit_excursion_response(ctx, RECORD_EXCURSION.action, detail)
            })
            .await
        })
        .await
    }

    /// `assess_excursion`.
    pub async fn assess_excursion(
        &self,
        input: AssessExcursionInput,
    ) -> Result<ExcursionView, CommandError> {
        self.run(&ASSESS_EXCURSION, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_specimens::assess_excursion(ctx, input)?;
                audit_excursion_response(ctx, ASSESS_EXCURSION.action, detail)
            })
            .await
        })
        .await
    }

    /// `configure_location`.
    pub async fn configure_location(
        &self,
        input: ConfigureStorageLocationInput,
    ) -> Result<StorageLocationView, CommandError> {
        self.run(&CONFIGURE_LOCATION, |call| async move {
            self.write(&call, move |ctx| {
                lis_specimens::configure_location(ctx, input)
            })
            .await
        })
        .await
    }

    /// `configure_printer`: a TCP printer's address must be one the engine's
    /// printer destination policy admits.
    pub async fn configure_printer(
        &self,
        input: ConfigureLabelPrinterInput,
    ) -> Result<LabelPrinterView, CommandError> {
        let destinations = self.printer_destinations;
        self.run(&CONFIGURE_PRINTER, |call| async move {
            self.write(&call, move |ctx| {
                lis_specimens::configure_printer(ctx, input, destinations)
            })
            .await
        })
        .await
    }
}
