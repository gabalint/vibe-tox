//! Checks on how an HL7 Orders connection reads and accepts orders. Pure.

use lis_core::required_text;

use super::error::{PartnerError, invalid};
use super::model::{
    IntakeSettings, MovedTo, MovedToChoice, PartnerConnectionSettings, PartnerMessageType,
    PartnerProtocol,
};
use super::settings::valid_folder_at;
use super::sftp_settings::valid_remote_folder;

/// Seconds between two readings of the input folder, at least and at most.
const CHECK_EVERY: std::ops::RangeInclusive<u16> = 10..=3600;
/// MSH-4 is an HD: namespace, universal ID and its type.
const ACCEPT_FROM_MAX: usize = 180;

/// The intake settings, trimmed: present for an incoming type, absent for an outgoing one.
pub(crate) fn valid_intake(
    message_type: PartnerMessageType,
    protocol: PartnerProtocol,
    intake: Option<IntakeSettings>,
) -> Result<Option<IntakeSettings>, PartnerError> {
    match (message_type.incoming(), intake) {
        (false, None) => Ok(None),
        (false, Some(_)) => Err(invalid(
            "settings.intake",
            "not_for_outgoing",
            "Only an incoming connection reads orders.",
        )),
        (true, None) => Err(invalid(
            "settings.intake.acceptFrom",
            "required",
            "Enter the sending facility (MSH-4) to accept.",
        )),
        (true, Some(intake)) => valid_reading(protocol, intake).map(Some),
    }
}

fn valid_reading(
    protocol: PartnerProtocol,
    intake: IntakeSettings,
) -> Result<IntakeSettings, PartnerError> {
    if !CHECK_EVERY.contains(&intake.check_every_seconds) {
        return Err(invalid(
            "settings.intake.checkEverySeconds",
            "out_of_range",
            "Use 10 to 3600 seconds.",
        ));
    }
    let accept_from = required_text(
        "settings.intake.acceptFrom",
        &intake.accept_from,
        ACCEPT_FROM_MAX,
    )
    .map_err(PartnerError::Invalid)?;
    let remote = |field: &str, folder: Option<&String>| {
        valid_remote_folder(field, folder.map_or("", String::as_str)).map(Some)
    };
    // MLLP – LIS connects sends results; its settings check refuses it for orders.
    let (processed, failed) = match protocol {
        PartnerProtocol::Folder | PartnerProtocol::MllpConnect => (None, None),
        PartnerProtocol::Sftp => (
            remote(
                "settings.intake.remoteProcessedFolder",
                intake.remote_processed_folder.as_ref(),
            )?,
            remote(
                "settings.intake.remoteFailedFolder",
                intake.remote_failed_folder.as_ref(),
            )?,
        ),
    };
    Ok(IntakeSettings {
        accept_from,
        remote_processed_folder: processed,
        remote_failed_folder: failed,
        ..intake
    })
}

/// Where an incoming connection's read files move to: over SFTP its server folders, over a
/// folder the newly chosen local folders, else `current` (the connection's local ones, if
/// it already read from a folder). `None` for an outgoing connection.
pub(crate) fn moved_to(
    settings: &PartnerConnectionSettings,
    chosen: MovedToChoice,
    current: Option<&MovedTo>,
) -> Result<Option<MovedTo>, PartnerError> {
    let Some(intake) = &settings.intake else {
        return Ok(None);
    };
    if settings.sftp.is_some() {
        return Ok(Some(MovedTo {
            processed: intake.remote_processed_folder.clone().unwrap_or_default(),
            failed: intake.remote_failed_folder.clone().unwrap_or_default(),
        }));
    }
    let local = |field: &str, label: &str, folder: Option<String>| {
        let folder = folder
            .ok_or_else(|| invalid(field, "required", &format!("Choose the {label} folder.")))?;
        valid_folder_at(field, &folder).map(|()| folder)
    };
    Ok(Some(MovedTo {
        processed: local(
            "processedFolder",
            "Processed",
            chosen
                .processed
                .or_else(|| current.map(|moved_to| moved_to.processed.clone())),
        )?,
        failed: local(
            "failedFolder",
            "Failed",
            chosen
                .failed
                .or_else(|| current.map(|moved_to| moved_to.failed.clone())),
        )?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::partner::model::UnknownPatient;

    fn intake() -> IntakeSettings {
        IntakeSettings {
            check_every_seconds: 30,
            accept_from: " CLINIC-A ".to_owned(),
            unknown_patient: UnknownPatient::Hold,
            remote_processed_folder: Some("/outbound/processed".to_owned()),
            remote_failed_folder: Some("/outbound/failed".to_owned()),
        }
    }

    #[test]
    fn orders_need_intake_settings_and_only_orders_have_them() {
        let orders = PartnerMessageType::Hl7Orders;
        let valid = valid_intake(orders, PartnerProtocol::Sftp, Some(intake()))
            .unwrap()
            .unwrap();
        assert_eq!(valid.accept_from, "CLINIC-A");
        // A folder connection's Processed and Failed folders come from the dialog.
        let folder = valid_intake(orders, PartnerProtocol::Folder, Some(intake()))
            .unwrap()
            .unwrap();
        assert_eq!(folder.remote_processed_folder, None);
        assert!(valid_intake(orders, PartnerProtocol::Folder, None).is_err());
        assert!(
            valid_intake(
                PartnerMessageType::Hl7Oru,
                PartnerProtocol::Folder,
                Some(intake())
            )
            .is_err()
        );
        for broken in [
            IntakeSettings {
                check_every_seconds: 5,
                ..intake()
            },
            IntakeSettings {
                accept_from: " ".to_owned(),
                ..intake()
            },
            IntakeSettings {
                remote_failed_folder: None,
                ..intake()
            },
            IntakeSettings {
                remote_processed_folder: Some("processed".to_owned()),
                ..intake()
            },
        ] {
            assert!(valid_intake(orders, PartnerProtocol::Sftp, Some(broken)).is_err());
        }
    }
}
