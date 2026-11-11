//! An HL7 Results delivery's bytes: made at its first attempt from the revision and the
//! connection's approved code mappings, kept in the artifact store with the mapping
//! version, and read back for every retry, so a retry writes the same bytes. Without a
//! mapping, or without a patient identifier for PID-3 (#4676), the delivery is held and
//! nothing is recorded; every pass looks again.

use std::collections::HashMap;

use lis_core::{Capability, CorrelationId, Principal, UtcTimestamp};
use lis_identity::SubjectId;
use lis_interfaces::partner::hl7::Oru;
use lis_interfaces::partner::{
    DueDelivery, LisRecordId, MessageSource, PartnerConnectionId, PartnerDeliveryId,
};
use lis_reporting::{ReportId, ReportRevisionId, ReportingError};
use lis_store::Database;
use rusqlite::Connection;

use super::hl7_message::{self, Header, Missing};
use super::{COMPONENT, stored};
use crate::commands::partner::ReportRoutingError;

/// What a delivery can write now.
pub(crate) enum Payload {
    Ready(Vec<u8>),
    /// Its kept bytes are missing or damaged.
    Lost,
    /// A mapping or the patient identifier is missing, or the message could not be made;
    /// it waits.
    Held,
}

/// The message an HL7 delivery or log row is about.
pub(crate) struct Target {
    pub(crate) delivery_id: PartnerDeliveryId,
    pub(crate) connection_id: PartnerConnectionId,
    pub(crate) created_at: UtcTimestamp,
    pub(crate) partner: String,
    pub(crate) source: MessageSource,
    pub(crate) source_id: Option<String>,
    /// Sent over MLLP, which answers each message.
    pub(crate) acknowledged: bool,
}

impl Target {
    fn of(delivery: &DueDelivery) -> Self {
        Self {
            delivery_id: delivery.delivery_id,
            connection_id: delivery.connection_id,
            created_at: delivery.created_at,
            partner: delivery.partner.clone(),
            source: delivery.source,
            source_id: delivery.source_id.clone(),
            acknowledged: delivery.destination.mllp.is_some(),
        }
    }
}

/// The ORU and the mapping version it used, or what it misses.
pub(crate) type Made = Result<(Oru, i64), Vec<Missing>>;

/// Reads what the message needs, in one read, and makes it. When the released revision
/// names no patient identifier, PID-3 carries the patient's active identifiers now: the
/// released content is unchanged, and the message can go once one is added (#4676).
pub(crate) fn make(conn: &Connection, target: &Target) -> Result<Made, ReportRoutingError> {
    let revision_id = revision_of(conn, target)?;
    let revision = lis_reporting::revision_outbound(conn, revision_id)?;
    let mappings = lis_interfaces::partner::approved_mappings(conn, target.connection_id)?;
    let loinc = catalog_loinc(conn)?;
    let header = Header {
        control_id: target.delivery_id.to_string(),
        created_at: target.created_at,
        partner: &target.partner,
        withdrawal: target.source == MessageSource::ReportWithdrawal,
        acknowledged: target.acknowledged,
    };
    let version = mappings.as_ref().map_or(0, |mappings| mappings.version);
    let mut made = hl7_message::oru(&revision, mappings.as_ref(), &loinc, &header);
    if let Ok(oru) = &mut made
        && oru.patient.identifiers.is_empty()
    {
        oru.patient.identifiers = current_identifiers(conn, revision.snapshot.subject.subject_id)?;
    }
    Ok(made.map(|oru| (oru, version)))
}

/// The patient's active identifiers: value, assigning authority and type.
fn current_identifiers(
    conn: &Connection,
    subject_id: SubjectId,
) -> Result<Vec<(String, String, String)>, ReportRoutingError> {
    let subject = lis_identity::subject_summary(conn, subject_id).map_err(ReportingError::from)?;
    Ok(subject
        .identifiers
        .into_iter()
        .map(|id| {
            (
                id.value.expose().clone(),
                id.assigning_authority,
                id.identifier_type,
            )
        })
        .collect())
}

/// The revision a message carries: its own, or the withdrawn report's latest.
fn revision_of(conn: &Connection, target: &Target) -> Result<ReportRevisionId, ReportRoutingError> {
    let inconsistent = || ReportingError::Inconsistent("HL7 message source");
    let id = target.source_id.as_deref().ok_or_else(inconsistent)?;
    match target.source {
        MessageSource::ReportRevision => Ok(id.parse().map_err(|_| inconsistent())?),
        MessageSource::ReportWithdrawal => {
            let report_id: ReportId = id.parse().map_err(|_| inconsistent())?;
            Ok(lis_reporting::latest_revision_id(conn, report_id)?)
        }
        MessageSource::BillingHandoff | MessageSource::Test | MessageSource::Order => {
            Err(inconsistent().into())
        }
    }
}

/// The catalog's LOINC code of every analyte that has one.
fn catalog_loinc(conn: &Connection) -> Result<HashMap<LisRecordId, String>, ReportRoutingError> {
    Ok(lis_catalog::list_analytes(conn)
        .map_err(ReportingError::from)?
        .into_iter()
        .filter_map(|analyte| Some((analyte.id.cast(), analyte.loinc_code?)))
        .collect())
}

/// The bytes to write for `delivery`.
pub(crate) fn payload(db: &Database, delivery: &DueDelivery) -> Payload {
    if let Some(digest) = &delivery.payload_sha256 {
        return stored(db, digest).map_or(Payload::Lost, Payload::Ready);
    }
    let delivery_id = delivery.delivery_id;
    let target = Target::of(delivery);
    let (oru, version) = match db.read_blocking(move |conn| make(conn, &target)) {
        Ok(Ok(made)) => made,
        Ok(Err(_missing)) => return Payload::Held,
        Err(error) => {
            tracing::warn!(%delivery_id, error_code = error_code(&error), "HL7 message not made");
            return Payload::Held;
        }
    };
    // PID-3 is required: never sent empty; Logs say why it waits.
    if oru.patient.identifiers.is_empty() {
        return Payload::Held;
    }
    let bytes = oru.encode();
    if let Err(code) = keep(db, delivery_id, &bytes, version) {
        tracing::warn!(%delivery_id, error_code = code, "HL7 message not kept; it waits");
        return Payload::Held;
    }
    Payload::Ready(bytes)
}

/// Stores the bytes and records them on the delivery, before any write.
fn keep(
    db: &Database,
    delivery_id: PartnerDeliveryId,
    bytes: &[u8],
    version: i64,
) -> Result<(), &'static str> {
    let digest = db
        .artifacts()
        .put(bytes)
        .map_err(|_| "artifact_not_stored")?
        .digest
        .to_string();
    db.write_blocking(
        Principal::system(COMPONENT, [Capability::Export]),
        CorrelationId::generate(),
        move |ctx| {
            lis_interfaces::partner::record_delivery_payload(ctx, delivery_id, &digest, version)
        },
    )
    .map_err(|error| error.code().as_str())
}

/// The PHI-free code of an error, for logs.
pub(crate) fn error_code(error: &ReportRoutingError) -> &'static str {
    match error {
        ReportRoutingError::Reporting(error) => error.code().as_str(),
        ReportRoutingError::Partner(error) => error.code().as_str(),
    }
}
