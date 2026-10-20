//! Seeds exact synthetic wire evidence at the listener/import boundary.

#![cfg(test)]

use lis_core::{Capability, CorrelationId, InstrumentId};
use lis_interfaces::ReceiptId;
use lis_results::ResultError;
use lis_store::testing::{TestDatabase, principal, test_start};
use rusqlite::params;

pub(crate) fn seed(
    database: &TestDatabase,
    instrument: InstrumentId,
    profile: &lis_au::AuConfig,
    bytes: &[u8],
) -> ReceiptId {
    let receipt = ReceiptId::generate();
    let artifact = database.db.artifacts().put(bytes).unwrap();
    let json = serde_json::to_string(profile).unwrap();
    let wire = bytes.to_vec();
    database.db.write_blocking(principal(&[Capability::InstrumentIntake]), CorrelationId::generate(), move |ctx| {
        ctx.tx().execute("INSERT INTO ifc_receipts (id, instrument_id, transport, peer, started_at,
            state, frame_count) VALUES (?1, ?2, 'au_tcp', '127.0.0.1:62003', ?3, 'receiving', 1)",
            params![receipt, instrument, test_start()])?;
        ctx.tx().execute("INSERT INTO ifc_receipt_au_profiles (receipt_id, profile_json, profile_version)
            VALUES (?1, ?2, 1)", params![receipt, json])?;
        ctx.tx().execute("INSERT INTO ifc_receipt_frames (receipt_id, ordinal, bytes) VALUES (?1, 0, ?2)", params![receipt, wire])?;
        ctx.tx().execute("UPDATE ifc_receipts SET state = 'complete', completed_at = ?2,
            message_sha256 = ?3, artifact_digest = ?3 WHERE id = ?1",
            params![receipt, test_start(), artifact.digest.to_string()])?;
        Ok::<_, ResultError>(())
    }).unwrap();
    receipt
}
