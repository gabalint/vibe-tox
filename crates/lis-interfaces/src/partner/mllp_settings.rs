//! Checks on an MLLP connection's partner settings, and the destination a destination
//! check covers (REQ-INT-006). MLLP is not encrypted, so the partner's address must be on
//! the lab network. Pure.

use std::net::{IpAddr, SocketAddr};

use super::error::{PartnerError, invalid};
use super::model::{MllpPartner, PartnerMessageType, PartnerProtocol};
use crate::validation::is_lab_address;

/// The longest wait for an acknowledgement, in seconds.
const ACK_TIMEOUT_MAX: u16 = 300;

/// The partner's interface, checked: present for MLLP, absent for a folder or SFTP. The
/// LIS connects over MLLP to send HL7 results only.
pub(crate) fn valid_mllp(
    message_type: PartnerMessageType,
    protocol: PartnerProtocol,
    mllp: Option<MllpPartner>,
) -> Result<Option<MllpPartner>, PartnerError> {
    match (protocol, mllp) {
        (PartnerProtocol::MllpConnect, _) if message_type != PartnerMessageType::Hl7Oru => {
            Err(invalid(
                "settings.protocol",
                "not_for_type",
                "MLLP – LIS connects sends HL7 Results only; use a folder or SFTP for this message.",
            ))
        }
        (PartnerProtocol::MllpConnect, Some(partner)) => valid_mllp_partner(partner).map(Some),
        (PartnerProtocol::MllpConnect, None) => Err(invalid(
            "settings.mllp.address",
            "required",
            "Enter the partner's address.",
        )),
        (PartnerProtocol::Folder | PartnerProtocol::Sftp, None) => Ok(None),
        (PartnerProtocol::Folder | PartnerProtocol::Sftp, Some(_)) => Err(invalid(
            "settings.mllp",
            "not_for_protocol",
            "Only an MLLP connection has a partner address.",
        )),
    }
}

/// The partner settings with the address written as the LIS reads it, or the first field
/// that is wrong; a connection test checks them as saving does.
pub fn valid_mllp_partner(partner: MllpPartner) -> Result<MllpPartner, PartnerError> {
    let Ok(address) = partner.address.trim().parse::<IpAddr>() else {
        return Err(invalid(
            "settings.mllp.address",
            "invalid_address",
            "Enter the partner's IP address on the lab network, such as 10.20.0.15.",
        ));
    };
    if !is_lab_address(address) {
        return Err(invalid(
            "settings.mllp.address",
            "not_lab_network",
            "Use an address on the lab network. MLLP is not encrypted: reach a partner outside \
             it through the lab's VPN, or use SFTP.",
        ));
    }
    if partner.port == 0 {
        return Err(invalid(
            "settings.mllp.port",
            "out_of_range",
            "Use a port from 1 to 65535.",
        ));
    }
    if !(1..=ACK_TIMEOUT_MAX).contains(&partner.ack_timeout_seconds) {
        return Err(invalid(
            "settings.mllp.ackTimeoutSeconds",
            "out_of_range",
            "Use 1 to 300 seconds.",
        ));
    }
    Ok(MllpPartner {
        address: address.to_string(),
        ..partner
    })
}

impl MllpPartner {
    /// The partner's address and port; `None` before the settings are checked.
    #[must_use]
    pub fn socket(&self) -> Option<SocketAddr> {
        let address: IpAddr = self.address.parse().ok()?;
        Some(SocketAddr::new(address, self.port))
    }
}

/// What a destination check covers for MLLP: the partner's address and port.
pub(crate) fn mllp_destination(partner: &MllpPartner) -> String {
    match partner.socket() {
        Some(socket) => format!("mllp://{socket}"),
        None => format!("mllp://{}:{}", partner.address, partner.port),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::partner::model::MllpAcknowledgement;

    fn partner(address: &str) -> MllpPartner {
        MllpPartner {
            address: address.to_owned(),
            port: 6661,
            acknowledgement: MllpAcknowledgement::Application,
            ack_timeout_seconds: 30,
        }
    }

    fn field_of(error: PartnerError) -> String {
        error.field_errors()[0].field.clone()
    }

    #[test]
    fn a_partner_on_the_lab_network_is_kept_as_its_address() {
        let valid = valid_mllp_partner(partner(" 10.20.0.15 ")).unwrap();
        assert_eq!(valid.address, "10.20.0.15");
        assert_eq!(mllp_destination(&valid), "mllp://10.20.0.15:6661");
        let six = valid_mllp_partner(partner("fd00::15")).unwrap();
        assert_eq!(mllp_destination(&six), "mllp://[fd00::15]:6661");
    }

    #[test]
    fn an_internet_address_a_name_or_a_bad_number_is_refused() {
        for (broken, field) in [
            (partner("8.8.8.8"), "settings.mllp.address"),
            (partner("hl7.clinic-a.example"), "settings.mllp.address"),
            (partner("2001:db8::1"), "settings.mllp.address"),
            (
                MllpPartner {
                    port: 0,
                    ..partner("10.20.0.15")
                },
                "settings.mllp.port",
            ),
            (
                MllpPartner {
                    ack_timeout_seconds: 0,
                    ..partner("10.20.0.15")
                },
                "settings.mllp.ackTimeoutSeconds",
            ),
        ] {
            assert_eq!(field_of(valid_mllp_partner(broken).unwrap_err()), field);
        }
    }

    #[test]
    fn mllp_sends_hl7_results_only_and_only_mllp_has_a_partner() {
        let mllp = |message_type, protocol, settings| valid_mllp(message_type, protocol, settings);
        let some = || Some(partner("10.20.0.15"));
        assert!(
            mllp(
                PartnerMessageType::Hl7Oru,
                PartnerProtocol::MllpConnect,
                some()
            )
            .is_ok()
        );
        for message_type in [
            PartnerMessageType::ReportPdf,
            PartnerMessageType::BillingJson,
            PartnerMessageType::Hl7Orders,
        ] {
            let refused = mllp(message_type, PartnerProtocol::MllpConnect, some()).unwrap_err();
            assert_eq!(field_of(refused), "settings.protocol");
        }
        assert!(
            mllp(
                PartnerMessageType::Hl7Oru,
                PartnerProtocol::MllpConnect,
                None
            )
            .is_err()
        );
        assert!(mllp(PartnerMessageType::Hl7Oru, PartnerProtocol::Folder, some()).is_err());
        assert_eq!(
            mllp(PartnerMessageType::Hl7Oru, PartnerProtocol::Sftp, None).unwrap(),
            None
        );
    }
}
