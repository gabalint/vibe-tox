//! Protocol selection and validated transport configuration.

use super::{AuConnectionSettings, DataBits, Parity, StopBits};
use crate::InterfaceError;
use crate::validation::{is_lab_address, valid_peer_allowlist, valid_text};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use ts_rs::TS;

/// How results reach the LIS from an analyzer.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum InstrumentTransport {
    /// ASTM LIS1-A framing over a TCP connection.
    AstmTcp,
    /// ASTM LIS1-A framing over a serial port.
    AstmSerial,
    /// Olympus AU protocol over a transparent TCP serial bridge.
    AuTcp,
    /// Olympus AU protocol over a local serial port.
    AuSerial,
    /// Result files exported by the instrument software, loaded by hand or
    /// picked up from a watched folder (result-files brief F1).
    ResultFile,
}

impl InstrumentTransport {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::AstmTcp => "astm_tcp",
            Self::AstmSerial => "astm_serial",
            Self::AuTcp => "au_tcp",
            Self::AuSerial => "au_serial",
            Self::ResultFile => "result_file",
        }
    }

    pub(crate) fn parse(value: &str) -> rusqlite::Result<Self> {
        match value {
            "astm_tcp" => Ok(Self::AstmTcp),
            "astm_serial" => Ok(Self::AstmSerial),
            "au_tcp" => Ok(Self::AuTcp),
            "au_serial" => Ok(Self::AuSerial),
            "result_file" => Ok(Self::ResultFile),
            _ => Err(rusqlite::Error::InvalidQuery),
        }
    }

    /// Whether this transport receives Olympus AU frames.
    #[must_use]
    pub const fn is_au(self) -> bool {
        matches!(self, Self::AuTcp | Self::AuSerial)
    }

    pub(crate) const fn is_tcp(self) -> bool {
        matches!(self, Self::AstmTcp | Self::AuTcp)
    }

    pub(crate) const fn is_serial(self) -> bool {
        matches!(self, Self::AstmSerial | Self::AuSerial)
    }
}

/// Validated connection settings for an analyzer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
#[serde(deny_unknown_fields)]
#[ts(rename = "InstrumentConnectionSettings")]
pub enum ConnectionSettings {
    /// A lab-LAN or loopback TCP listener.
    AstmTcp {
        /// Exact local address on which the listener binds.
        bind_address: IpAddr,
        /// Nonzero TCP port.
        port: u16,
        /// Nonempty set of analyzer source addresses allowed to use this listener.
        allowed_peers: Vec<IpAddr>,
    },
    /// A local serial device.
    AstmSerial {
        /// OS serial device path or port name.
        port_path: String,
        /// Bits per second.
        baud_rate: u32,
        /// Data bits per character.
        data_bits: DataBits,
        /// Parity mode.
        parity: Parity,
        /// Stop-bit count.
        stop_bits: StopBits,
    },
    /// An AU400/AU640 sending unchanged protocol bytes through a TCP bridge.
    AuTcp {
        /// Local laboratory address on which the LIS listens.
        bind_address: IpAddr,
        /// Nonzero listener port.
        port: u16,
        /// Allowed bridge addresses; source IP is not cryptographic identity.
        allowed_peers: Vec<IpAddr>,
        /// Settings matched to the analyzer's online parameter sheet.
        au: AuConnectionSettings,
    },
    /// A directly connected AU400/AU640 serial line.
    AuSerial {
        /// OS serial device path or port name.
        port_path: String,
        /// 2400, 4800 or 9600 bits per second.
        baud_rate: u32,
        /// Seven or eight data bits.
        data_bits: DataBits,
        /// Configured parity.
        parity: Parity,
        /// Configured stop bits.
        stop_bits: StopBits,
        /// Settings matched to the analyzer's online parameter sheet.
        au: AuConnectionSettings,
    },
    /// Result files from the instrument software. The optional watched folder
    /// is set separately, from a system dialog (`Instrument::result_folder`).
    ResultFile {},
}

impl ConnectionSettings {
    pub(crate) fn validate(&self) -> Result<(), InterfaceError> {
        match self {
            Self::AstmTcp {
                bind_address,
                port,
                allowed_peers,
            }
            | Self::AuTcp {
                bind_address,
                port,
                allowed_peers,
                ..
            } => {
                if !is_lab_address(*bind_address) {
                    return Err(InterfaceError::InvalidTcpBindAddress);
                }
                if *port == 0 {
                    return Err(InterfaceError::InvalidTcpPort);
                }
                if !valid_peer_allowlist(allowed_peers) {
                    return Err(InterfaceError::InvalidTcpAllowedPeers);
                }
            }
            Self::AstmSerial {
                port_path,
                baud_rate,
                ..
            }
            | Self::AuSerial {
                port_path,
                baud_rate,
                ..
            } => {
                if !valid_text(port_path, 1024) {
                    return Err(InterfaceError::InvalidSerialPort);
                }
                if !(1..=4_000_000).contains(baud_rate) {
                    return Err(InterfaceError::InvalidBaudRate);
                }
            }
            Self::ResultFile {} => {}
        }
        if let Some(au) = self.au_settings() {
            au.validate()?;
        }
        if let Self::AuSerial {
            baud_rate,
            data_bits,
            ..
        } = self
            && (!matches!(baud_rate, 2400 | 4800 | 9600)
                || !matches!(data_bits, DataBits::Seven | DataBits::Eight))
        {
            return Err(InterfaceError::InvalidAuConfiguration);
        }
        Ok(())
    }

    /// Transport selected by these settings.
    #[must_use]
    pub const fn transport(&self) -> InstrumentTransport {
        match self {
            Self::AstmTcp { .. } => InstrumentTransport::AstmTcp,
            Self::AstmSerial { .. } => InstrumentTransport::AstmSerial,
            Self::AuTcp { .. } => InstrumentTransport::AuTcp,
            Self::AuSerial { .. } => InstrumentTransport::AuSerial,
            Self::ResultFile {} => InstrumentTransport::ResultFile,
        }
    }

    /// The configured AU wire profile, absent for other protocols.
    #[must_use]
    pub fn au_profile(&self) -> Option<&lis_au::AuConfig> {
        self.au_settings().map(|settings| &settings.protocol)
    }

    pub(crate) fn au_settings(&self) -> Option<&AuConnectionSettings> {
        match self {
            Self::AuTcp { au, .. } | Self::AuSerial { au, .. } => Some(au),
            Self::AstmTcp { .. } | Self::AstmSerial { .. } | Self::ResultFile {} => None,
        }
    }
}
