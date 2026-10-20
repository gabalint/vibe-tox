//! Source-backed collection timing correction and clinical review (REQ-SPC-002).

use super::{clinical, require_user};
use crate::collection_history::{
    CollectionCorrectionId, CollectionHistory, get_collection_history,
};
use crate::error::{ensure_revision, invalid};
use crate::input::{self, TEXT_MAX};
use crate::repo::collection_corrections as repo;
use crate::{AccessionId, CollectionTimeInput, SpecimenError};
use lis_audit::AuditDetail;
use lis_core::Capability;
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

/// Corrects collection timing without modifying the original evidence.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct CorrectCollectionTimeInput {
    pub accession_id: AccessionId,
    #[ts(type = "number")]
    pub expected_revision: i64,
    pub time: CollectionTimeInput,
    pub source_document_ref: String,
    pub reason: String,
}

/// Completes the clinical timing review; report-impact assessment remains separate.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ResolveCollectionCorrectionInput {
    pub correction_id: CollectionCorrectionId,
    pub reason: String,
}

/// Records a new collection version and opens a clinical timing review.
pub fn correct_collection_time(
    ctx: &mut CommandContext<'_>,
    input: CorrectCollectionTimeInput,
) -> Result<CollectionHistory, SpecimenError> {
    require_user(ctx, Capability::Accession)?;
    let source = input::required_text("sourceDocumentRef", &input.source_document_ref, TEXT_MAX)?;
    let reason = input::reason(Some(&input.reason))?;
    let time = input::collection_time(&input.time, ctx.now())?;
    let history = get_collection_history(ctx.tx(), input.accession_id)?;
    ensure_revision(history.revision, input.expected_revision)?;
    let current = &history.current;
    if (
        time.precision,
        time.collected_at,
        time.offset_minutes,
        &time.date,
    ) == (
        current.precision,
        current.collected_at,
        current.utc_offset_minutes,
        &current.collected_date,
    ) {
        return Err(invalid(
            "time",
            "unchanged",
            "The collection time is unchanged.",
        ));
    }
    let id = CollectionCorrectionId::generate();
    repo::insert(
        ctx,
        id,
        history.collection_id,
        history.revision + 1,
        &time,
        (&source, &reason),
    )?;
    ctx.audit(
        clinical("specimen.correct_collection_time")
            .record("collection_correction", id)
            .old_revision(history.revision)
            .new_revision(history.revision + 1)
            .detail("accession", AuditDetail::id(input.accession_id))
            .reason(reason),
    )?;
    get_collection_history(ctx.tx(), input.accession_id)
}

/// Records quality review of corrected timing; original and correction are retained.
pub fn resolve_collection_correction(
    ctx: &mut CommandContext<'_>,
    input: ResolveCollectionCorrectionInput,
) -> Result<CollectionHistory, SpecimenError> {
    require_user(ctx, Capability::ManageQuality)?;
    let reason = input::reason(Some(&input.reason))?;
    let accession = repo::accession_for_correction(ctx.tx(), input.correction_id)?;
    repo::resolve(ctx, input.correction_id, &reason)?;
    ctx.audit(
        clinical("specimen.resolve_collection_correction")
            .record("accession", accession)
            .detail("correction", AuditDetail::id(input.correction_id))
            .reason(reason),
    )?;
    get_collection_history(ctx.tx(), accession)
}
