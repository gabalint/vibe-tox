//! Source-backed resolution of ambiguous legacy receipt chronology (DM-01).

use lis_core::CommandError;
use lis_specimens::{AccessionDetail, ResolveReceiptChronologyInput};

use super::{MANAGE_QUALITY, audit_accession_response};
use crate::Engine;
use crate::registry::{CommandSpec, spec};

pub(crate) const RESOLVE_RECEIPT_CHRONOLOGY: CommandSpec = spec(
    "resolve_receipt_chronology",
    "specimen.resolve_receipt_chronology",
    MANAGE_QUALITY,
);

impl Engine {
    /// Resolve legacy ordering from evidence; original events/timestamps remain intact.
    pub async fn resolve_receipt_chronology(
        &self,
        input: ResolveReceiptChronologyInput,
    ) -> Result<AccessionDetail, CommandError> {
        self.run(&RESOLVE_RECEIPT_CHRONOLOGY, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_specimens::resolve_receipt_chronology(ctx, input)?;
                audit_accession_response(ctx, RESOLVE_RECEIPT_CHRONOLOGY.action, detail)
            })
            .await
        })
        .await
    }
}
