//! An AU final block has no sequence number. Retain evidence that an identity
//! was multiblock so a reconnect cannot turn a trailing E into a whole sample.

use lis_au::{BlockId, ResultBlock, ResultKind, SampleHeader};
use lis_core::InstrumentId;
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};

use crate::{InterfaceError, Receipt};

pub(crate) fn requires_au_restart(
    conn: &Connection,
    instrument: InstrumentId,
    kind: ResultKind,
    header: &SampleHeader,
) -> Result<bool, InterfaceError> {
    Ok(conn
        .prepare_cached(
            "SELECT EXISTS(SELECT 1 FROM ifc_au_multiblock_guards
         WHERE instrument_id = ? AND identity_sha256 = ?)",
        )?
        .query_row(params![instrument, identity_digest(kind, header)], |row| {
            row.get(0)
        })?)
}

pub(crate) fn guard_au_multiblock(
    conn: &Connection,
    receipt: &Receipt,
    ordinal: u32,
    block: &ResultBlock,
) -> Result<(), InterfaceError> {
    if block.block == BlockId::Final {
        if ordinal == 0
            && requires_au_restart(conn, receipt.instrument_id, block.kind, &block.header)?
        {
            return Err(InterfaceError::FrameOutOfSequence);
        }
        return Ok(());
    }
    conn.prepare_cached(
        "INSERT INTO ifc_au_multiblock_guards (instrument_id, identity_sha256, first_receipt_id)
         VALUES (?, ?, ?) ON CONFLICT (instrument_id, identity_sha256) DO NOTHING",
    )?
    .execute(params![
        receipt.instrument_id,
        identity_digest(block.kind, &block.header),
        receipt.id
    ])?;
    Ok(())
}

fn identity_digest(kind: ResultKind, header: &SampleHeader) -> String {
    let code = match kind {
        ResultKind::Normal => b"D ",
        ResultKind::Rerun => b"DH",
        ResultKind::Stat => b"d ",
        ResultKind::StatRerun => b"dH",
        ResultKind::ReagentBlank => b"DR",
        ResultKind::Calibration => b"DA",
        ResultKind::QualityControl => b"DQ",
    };
    let mut digest = Sha256::new();
    digest.update(b"au-multiblock-identity-v1");
    digest.update(code);
    // Length boundaries distinguish field settings; demographics do not change
    // sample identity or permit a previously interrupted tail to be imported.
    for field in [
        header
            .unit_no
            .as_ref()
            .map_or(&[][..], |v| v.expose().as_slice()),
        header.rack.expose(),
        header.cup.expose(),
        header.sample_type.expose(),
        header.sample_no.expose(),
        header.sample_id.expose(),
        header.reserved.expose(),
    ] {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field);
    }
    hex::encode(digest.finalize())
}
