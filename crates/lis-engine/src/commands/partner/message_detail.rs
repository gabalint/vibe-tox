//! One outgoing message's details for the Logs panel (ticket #4678): its control ID, the HL7
//! message as sent and its attempts. The message is the bytes kept when it was made, read
//! back and verified against their recorded digest, never made again: a message made now
//! from the same revision could differ from the one sent (mappings, catalog codes, the
//! connection's partner). It holds patient data, so every read records an `access` event
//! naming the delivery before anything is returned.

use std::sync::Arc;

use lis_audit::AuditDetail;
use lis_core::{Capability, CommandError};
use lis_interfaces::partner::{GetPartnerMessage, PartnerMessageDetail};
use lis_store::{ArtifactDigest, StoreError};

use super::GET_PARTNER_MESSAGE;
use crate::engine::Engine;
use crate::pipeline::{Failure, access};

impl Engine {
    /// `get_partner_message`: one delivery's control ID, HL7 segments as sent and attempts.
    pub async fn get_partner_message(
        &self,
        input: GetPartnerMessage,
    ) -> Result<PartnerMessageDetail, CommandError> {
        self.run(&GET_PARTNER_MESSAGE, |call| async move {
            let delivery_id = input.delivery_id;
            let mut detail = self
                .read(&call, move |conn| {
                    lis_interfaces::partner::partner_message_detail(conn, delivery_id)
                })
                .await?;
            if let Some(digest) = detail.kept_sha256.clone() {
                match self.kept_message(&digest).await? {
                    Some(bytes) => detail.show(&bytes),
                    None => tracing::warn!(%delivery_id, "kept partner message unavailable"),
                }
            }
            let event = access(GET_PARTNER_MESSAGE.action)
                .record("partner_delivery", delivery_id)
                .detail("content", AuditDetail::code(detail.content.as_str()));
            // Rechecked in the audit transaction, as every audited read does.
            self.write(&call, move |ctx| {
                ctx.require(Capability::ManageConfiguration)?;
                ctx.audit(event).map(drop)
            })
            .await?;
            Ok(detail)
        })
        .await
    }

    /// The kept bytes, verified against `digest`; `None` when they are missing or damaged.
    async fn kept_message(&self, digest: &str) -> Result<Option<Vec<u8>>, Failure> {
        let Ok(parsed) = digest.parse::<ArtifactDigest>() else {
            return Ok(None);
        };
        let db = Arc::clone(&self.db);
        match tokio::task::spawn_blocking(move || db.artifacts().read(&parsed))
            .await
            .map_err(|_| Failure::Internal("partner message read task stopped".to_owned()))?
        {
            Ok(bytes) => Ok(Some(bytes)),
            Err(StoreError::ArtifactNotFound | StoreError::ArtifactCorrupt) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}
