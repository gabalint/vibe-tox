//! Serial line settings of an analyzer connection.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Number of data bits in a serial character.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename = "SerialDataBits")]
pub enum DataBits {
    /// Five data bits.
    Five,
    /// Six data bits.
    Six,
    /// Seven data bits.
    Seven,
    /// Eight data bits.
    Eight,
}

impl DataBits {
    pub(crate) const fn value(self) -> u8 {
        match self {
            Self::Five => 5,
            Self::Six => 6,
            Self::Seven => 7,
            Self::Eight => 8,
        }
    }

    pub(crate) fn parse(value: u8) -> rusqlite::Result<Self> {
        match value {
            5 => Ok(Self::Five),
            6 => Ok(Self::Six),
            7 => Ok(Self::Seven),
            8 => Ok(Self::Eight),
            _ => Err(rusqlite::Error::InvalidQuery),
        }
    }
}

/// Serial parity mode.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "SerialParity")]
pub enum Parity {
    /// No parity bit.
    None,
    /// Odd parity.
    Odd,
    /// Even parity.
    Even,
}

impl Parity {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Odd => "odd",
            Self::Even => "even",
        }
    }

    pub(crate) fn parse(value: &str) -> rusqlite::Result<Self> {
        match value {
            "none" => Ok(Self::None),
            "odd" => Ok(Self::Odd),
            "even" => Ok(Self::Even),
            _ => Err(rusqlite::Error::InvalidQuery),
        }
    }
}

/// Serial stop-bit count.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename = "SerialStopBits")]
pub enum StopBits {
    /// One stop bit.
    One,
    /// Two stop bits.
    Two,
}

impl StopBits {
    pub(crate) const fn value(self) -> u8 {
        match self {
            Self::One => 1,
            Self::Two => 2,
        }
    }

    pub(crate) fn parse(value: u8) -> rusqlite::Result<Self> {
        match value {
            1 => Ok(Self::One),
            2 => Ok(Self::Two),
            _ => Err(rusqlite::Error::InvalidQuery),
        }
    }
}
