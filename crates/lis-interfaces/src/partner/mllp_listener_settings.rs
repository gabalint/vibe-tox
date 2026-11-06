//! Checks on an MLLP listener's settings, and the destination a destination check covers
//! (ticket #4555 step 4b; REQ-INT-006). The LIS listens only on an address of this computer
//! on the lab network, never on an internet address or on every address, and lets only the
//! allowed partner addresses in. Pure.

use std::net::{IpAddr, SocketAddr};

use super::error::{PartnerError, invalid};
use super::mllp_settings::lab_ip;
use super::model::{AllowedPartner, MllpListener, PartnerMessageType, PartnerProtocol};
use crate::validation::valid_text;

/// Partner addresses one listener allows, at most.
const PARTNERS_MAX: usize = 20;
/// A partner note's length, at most.
const NOTE_MAX: usize = 80;
/// A partner's connection may be idle 10 seconds to an hour.
const IDLE_SECONDS: std::ops::RangeInclusive<u16> = 10..=3600;

/// The listener, checked: present for MLLP – LIS listens, absent otherwise. The LIS listens
/// over MLLP for HL7 orders only.
pub(crate) fn valid_listener(
    message_type: PartnerMessageType,
    protocol: PartnerProtocol,
    listener: Option<MllpListener>,
) -> Result<Option<MllpListener>, PartnerError> {
    match (protocol, listener) {
        (PartnerProtocol::MllpListen, _) if message_type != PartnerMessageType::Hl7Orders => {
            Err(invalid(
                "settings.protocol",
                "not_for_type",
                "MLLP – LIS listens receives HL7 Orders only.",
            ))
        }
        (PartnerProtocol::MllpListen, Some(listener)) => valid_mllp_listener(listener).map(Some),
        (PartnerProtocol::MllpListen, None) => Err(invalid(
            "settings.mllpListener.address",
            "required",
            "Enter the address to listen on.",
        )),
        (_, None) => Ok(None),
        (_, Some(_)) => Err(invalid(
            "settings.mllpListener",
            "not_for_protocol",
            "Only an MLLP listener has a listen address.",
        )),
    }
}

/// The listener with its addresses written as the LIS reads them, or the first field that
/// is wrong; Test listener checks them as saving does.
pub fn valid_mllp_listener(listener: MllpListener) -> Result<MllpListener, PartnerError> {
    let address = lab_ip(
        "settings.mllpListener.address",
        &listener.address,
        "Enter this computer's IP address on the lab network to listen on, such as 192.168.1.20.",
        "Use an address on the lab network. The LIS never listens on an internet address; \
         outside partners use SFTP, or MLLP through the lab's VPN.",
    )?;
    if listener.port == 0 {
        return Err(invalid(
            "settings.mllpListener.port",
            "out_of_range",
            "Use a port from 1 to 65535.",
        ));
    }
    if !IDLE_SECONDS.contains(&listener.idle_timeout_seconds) {
        return Err(invalid(
            "settings.mllpListener.idleTimeoutSeconds",
            "out_of_range",
            "Use 10 to 3600 seconds.",
        ));
    }
    Ok(MllpListener {
        address: address.to_string(),
        allowed_partners: valid_partners(listener.allowed_partners)?,
        ..listener
    })
}

/// One to 20 distinct partner addresses on the lab network, each with a short note.
fn valid_partners(partners: Vec<AllowedPartner>) -> Result<Vec<AllowedPartner>, PartnerError> {
    let field = "settings.mllpListener.allowedPartners";
    if partners.is_empty() || partners.len() > PARTNERS_MAX {
        return Err(invalid(
            field,
            "out_of_range",
            "Enter 1 to 20 partner addresses allowed to connect.",
        ));
    }
    let mut valid: Vec<AllowedPartner> = Vec::with_capacity(partners.len());
    for (index, partner) in partners.into_iter().enumerate() {
        let address = lab_ip(
            &format!("{field}.{index}.address"),
            &partner.address,
            "Enter each partner's IP address, such as 192.168.1.50 (Hospital B interface engine).",
            "Allow only partner addresses on the lab network; a partner outside it connects \
             through the lab's VPN.",
        )?
        .to_string();
        if valid.iter().any(|kept| kept.address == address) {
            return Err(invalid(
                field,
                "duplicate",
                "Enter each partner address once.",
            ));
        }
        let note = partner.note.trim().to_owned();
        if !note.is_empty() && !valid_text(&note, NOTE_MAX) {
            return Err(invalid(
                &format!("{field}.{index}.note"),
                "invalid_note",
                "Keep each note to one line of up to 80 characters.",
            ));
        }
        valid.push(AllowedPartner { address, note });
    }
    Ok(valid)
}

impl MllpListener {
    /// The address and port to listen on; `None` before the settings are checked.
    #[must_use]
    pub fn socket(&self) -> Option<SocketAddr> {
        let address: IpAddr = self.address.parse().ok()?;
        Some(SocketAddr::new(address, self.port))
    }

    /// Whether `peer` is one of the partner addresses allowed to connect.
    #[must_use]
    pub fn allows(&self, peer: IpAddr) -> bool {
        self.allowed_partners
            .iter()
            .any(|partner| partner.address.parse::<IpAddr>() == Ok(peer))
    }
}

/// What a destination check covers for a listener: where it listens and who may connect.
pub(crate) fn listener_destination(listener: &MllpListener) -> String {
    let partners: Vec<&str> = listener
        .allowed_partners
        .iter()
        .map(|partner| partner.address.as_str())
        .collect();
    let socket = listener.socket().map_or_else(
        || format!("{}:{}", listener.address, listener.port),
        |socket| socket.to_string(),
    );
    format!("mllp-listen://{socket} from {}", partners.join(", "))
}

#[cfg(test)]
#[path = "mllp_listener_settings_tests.rs"]
mod tests;
