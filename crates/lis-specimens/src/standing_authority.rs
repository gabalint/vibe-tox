//! Resolve standing authorization from current source-backed collection evidence.

use lis_core::GateReason;
use rusqlite::Connection;

use crate::repo::accessions;
use crate::{CollectionPrecision, CollectionView, SpecimenError, SpecimenId};

pub(crate) fn append_gate(
    conn: &Connection,
    specimen_id: SpecimenId,
    gates: &mut Vec<GateReason>,
) -> Result<(), SpecimenError> {
    let accession_id = crate::specimen_accession(conn, specimen_id)?;
    let accession =
        accessions::accession(conn, accession_id)?.ok_or(SpecimenError::NotFound("accession"))?;
    let collection = accessions::collection(conn, accession_id)?;
    let date = collection_date(&collection)?;
    gates.extend(lis_orders::standing_collection_gate(
        conn,
        accession.order_id,
        date.as_deref(),
    )?);
    Ok(())
}

fn collection_date(collection: &CollectionView) -> Result<Option<String>, SpecimenError> {
    match collection.precision {
        CollectionPrecision::Unknown => Ok(None),
        CollectionPrecision::Date => Ok(collection.collected_date.clone()),
        CollectionPrecision::Datetime => {
            let (Some(at), Some(offset)) = (collection.collected_at, collection.utc_offset_minutes)
            else {
                return Ok(None);
            };
            let invalid = || {
                crate::error::invalid(
                    "collectionTime",
                    "invalid_evidence",
                    "The recorded collection time or original offset cannot be read. Record a source-backed correction.",
                )
            };
            let instant: jiff::Timestamp = at.to_string().parse().map_err(|_| invalid())?;
            let offset = offset
                .checked_mul(60)
                .and_then(|seconds| jiff::tz::Offset::from_seconds(seconds).ok())
                .ok_or_else(invalid)?;
            Ok(Some(
                instant
                    .to_zoned(jiff::tz::TimeZone::fixed(offset))
                    .date()
                    .to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collection(precision: CollectionPrecision) -> CollectionView {
        CollectionView {
            collection_id: crate::CollectionId::generate(),
            precision,
            collected_at: None,
            utc_offset_minutes: None,
            collected_date: None,
            collector: None,
            observations: Vec::new(),
        }
    }

    #[test]
    fn authorization_date_uses_source_offset_and_keeps_unknown_and_date_only_precision() {
        let mut evidence = collection(CollectionPrecision::Datetime);
        evidence.collected_at = Some("2026-10-15T02:00:00.000000000Z".parse().unwrap());
        evidence.utc_offset_minutes = Some(-240);
        assert_eq!(
            collection_date(&evidence).unwrap().as_deref(),
            Some("2026-10-14")
        );
        evidence.utc_offset_minutes = Some(600);
        assert_eq!(
            collection_date(&evidence).unwrap().as_deref(),
            Some("2026-10-15")
        );
        evidence.utc_offset_minutes = None;
        assert_eq!(collection_date(&evidence).unwrap(), None);
        let mut evidence = collection(CollectionPrecision::Date);
        evidence.collected_date = Some("2026-10-13".into());
        assert_eq!(
            collection_date(&evidence).unwrap().as_deref(),
            Some("2026-10-13")
        );
        assert_eq!(
            collection_date(&collection(CollectionPrecision::Unknown)).unwrap(),
            None
        );
    }
}
