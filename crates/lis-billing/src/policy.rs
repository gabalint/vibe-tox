//! The fixed M3 billing policy, the one home of these rules (brief §0, §5,
//! L14; Q-21 keeps other rules for later): the date of service is the
//! collection's local date, and a derivation needs the laboratory's billing
//! NPI and TIN.

use jiff::Timestamp;
use jiff::tz::{Offset, TimeZone};
use lis_identity::LaboratoryProfileView;
use lis_specimens::{CollectionPrecision, SubjectSpecimen};

use crate::error::BillingError;

/// The date-of-service rule recorded on every derivation.
pub(crate) const DATE_OF_SERVICE_RULE: &str = "collection_local_date_v1";

/// `collection_local_date_v1`: the date of the collection instant at the
/// source's own offset, or the recorded date when only a date is known;
/// `None` when the collection time is unknown.
pub(crate) fn date_of_service(specimen: &SubjectSpecimen) -> Result<Option<String>, BillingError> {
    match specimen.precision {
        CollectionPrecision::Datetime => {
            let (Some(at), Some(minutes)) = (specimen.collected_at, specimen.utc_offset_minutes)
            else {
                return Err(BillingError::Inconsistent("collection time"));
            };
            let instant: Timestamp = at
                .to_string()
                .parse()
                .map_err(|_| BillingError::Inconsistent("collection time"))?;
            let offset = Offset::from_seconds(minutes.saturating_mul(60))
                .map_err(|_| BillingError::Inconsistent("collection offset"))?;
            let local = instant.to_zoned(TimeZone::fixed(offset));
            Ok(Some(local.date().to_string()))
        }
        CollectionPrecision::Date => specimen
            .collected_date
            .clone()
            .map(Some)
            .ok_or(BillingError::Inconsistent("collection date")),
        CollectionPrecision::Unknown => Ok(None),
    }
}

/// The laboratory can bill only with both its billing NPI and TIN on the
/// current profile.
pub(crate) fn billing_identity_complete(profile: Option<&LaboratoryProfileView>) -> bool {
    profile.is_some_and(|profile| profile.billing_npi.is_some() && profile.tax_id.is_some())
}

#[cfg(test)]
mod tests {
    use lis_core::UtcTimestamp;
    use lis_specimens::{AccessionId, SpecimenAcceptance, SpecimenId};

    use super::*;

    fn specimen(precision: CollectionPrecision) -> SubjectSpecimen {
        SubjectSpecimen {
            specimen_id: SpecimenId::generate(),
            specimen_type_code: "SYN-URINE".to_owned(),
            acceptance: SpecimenAcceptance::Accepted,
            received_at: UtcTimestamp::parse("2026-10-12T12:00:00.000000000Z").unwrap(),
            accession_id: AccessionId::generate(),
            accession_number: "A26000001".to_owned(),
            order_id: lis_orders::OrderId::generate(),
            precision,
            collected_at: None,
            utc_offset_minutes: None,
            collected_date: None,
        }
    }

    #[test]
    fn the_date_of_service_is_the_collection_date_at_the_source_offset() {
        let mut late_evening = specimen(CollectionPrecision::Datetime);
        // 2026-10-13T02:30Z at UTC-04:00 is 22:30 on 2026-10-12.
        late_evening.collected_at =
            Some(UtcTimestamp::parse("2026-10-13T02:30:00.000000000Z").unwrap());
        late_evening.utc_offset_minutes = Some(-240);
        assert_eq!(
            date_of_service(&late_evening).unwrap().as_deref(),
            Some("2026-10-12")
        );

        let mut date_only = specimen(CollectionPrecision::Date);
        date_only.collected_date = Some("2026-10-11".to_owned());
        assert_eq!(
            date_of_service(&date_only).unwrap().as_deref(),
            Some("2026-10-11")
        );

        assert_eq!(
            date_of_service(&specimen(CollectionPrecision::Unknown)).unwrap(),
            None
        );
    }
}
