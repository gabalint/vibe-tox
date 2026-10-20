//! Audited collection timing corrections and quality review (REQ-SPC-002, REQ-BIL-002).

use lis_core::{Capability, CommandError, Principal};
use lis_specimens::{
    CollectionHistory, CorrectCollectionTimeInput, ResolveCollectionCorrectionInput,
};

use super::specimens::GetAccessionInput;
use crate::engine::Engine;
use crate::pipeline::access;
use crate::registry::{Access, CommandSpec, spec};

pub(crate) const GET_COLLECTION_HISTORY: CommandSpec = spec(
    "get_collection_history",
    "specimen.collection_history",
    Access::EitherCapability(Capability::Accession, Capability::ManageQuality),
);
pub(crate) const CORRECT_COLLECTION_TIME: CommandSpec = spec(
    "correct_collection_time",
    "specimen.correct_collection_time",
    Access::Capability(Capability::Accession),
);
pub(crate) const RESOLVE_COLLECTION_CORRECTION: CommandSpec = spec(
    "resolve_collection_correction",
    "specimen.resolve_collection_correction",
    Access::Capability(Capability::ManageQuality),
);

impl Engine {
    /// Reads retained original collection facts and their attributable corrections.
    pub async fn get_collection_history(
        &self,
        input: GetAccessionInput,
    ) -> Result<CollectionHistory, CommandError> {
        self.run(&GET_COLLECTION_HISTORY, |call| async move {
            let principal = call.principal.clone();
            self.audited_read(
                &call,
                move |conn| {
                    let history = lis_specimens::get_collection_history(conn, input.accession_id)?;
                    Ok::<_, lis_specimens::SpecimenError>(allowed_actions(history, &principal))
                },
                |_| access(GET_COLLECTION_HISTORY.action).record("accession", input.accession_id),
            )
            .await
        })
        .await
    }

    /// Corrects timing with evidence; dependent clinical work requires quality review.
    pub async fn correct_collection_time(
        &self,
        input: CorrectCollectionTimeInput,
    ) -> Result<CollectionHistory, CommandError> {
        self.run(&CORRECT_COLLECTION_TIME, |call| async move {
            self.write(&call, move |ctx| {
                let history = lis_specimens::correct_collection_time(ctx, input)?;
                ctx.audit(
                    access(CORRECT_COLLECTION_TIME.action)
                        .record("accession", history.accession_id),
                )?;
                Ok::<_, lis_specimens::SpecimenError>(allowed_actions(history, ctx.principal()))
            })
            .await
        })
        .await
    }

    /// Records the laboratory quality assessment of a collection correction.
    pub async fn resolve_collection_correction(
        &self,
        input: ResolveCollectionCorrectionInput,
    ) -> Result<CollectionHistory, CommandError> {
        self.run(&RESOLVE_COLLECTION_CORRECTION, |call| async move {
            self.write(&call, move |ctx| {
                let history = lis_specimens::resolve_collection_correction(ctx, input)?;
                ctx.audit(
                    access(RESOLVE_COLLECTION_CORRECTION.action)
                        .record("accession", history.accession_id),
                )?;
                Ok::<_, lis_specimens::SpecimenError>(allowed_actions(history, ctx.principal()))
            })
            .await
        })
        .await
    }
}

fn allowed_actions(mut history: CollectionHistory, principal: &Principal) -> CollectionHistory {
    if principal.require(Capability::Accession).is_err() {
        history.allowed_actions.clear();
    }
    if principal.require(Capability::ManageQuality).is_err() {
        for correction in &mut history.corrections {
            correction.allowed_actions.clear();
        }
    }
    history
}
