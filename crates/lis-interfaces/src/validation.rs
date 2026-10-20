use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use crate::InterfaceError;

pub(crate) fn validate_identity(code: &str, name: &str) -> Result<(), InterfaceError> {
    let valid_code = !code.is_empty()
        && code.len() <= 64
        && code
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    if !valid_code {
        return Err(InterfaceError::InvalidInstrumentCode);
    }
    if !valid_text(name, 128) {
        return Err(InterfaceError::InvalidInstrumentName);
    }
    Ok(())
}

pub(crate) fn valid_text(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
}

pub(crate) fn is_lab_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_lab_ipv4(address),
        IpAddr::V6(address) => is_lab_ipv6(address),
    }
}

pub(crate) fn valid_peer_allowlist(peers: &[IpAddr]) -> bool {
    !peers.is_empty()
        && peers.iter().all(|peer| is_lab_address(*peer))
        && !peers
            .iter()
            .enumerate()
            .any(|(index, peer)| peers[..index].contains(peer))
}

fn is_lab_ipv4(address: Ipv4Addr) -> bool {
    address.is_loopback() || address.is_private() || address.is_link_local()
}

fn is_lab_ipv6(address: Ipv6Addr) -> bool {
    let first = address.octets()[0];
    address.is_loopback() || address.is_unicast_link_local() || first & 0xfe == 0xfc
}
