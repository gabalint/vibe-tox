//! Typed UUIDv7 identifiers.
//!
//! `Id<K>` is the one mechanism for entity identifiers. The marker type `K`
//! names the entity, so a `UserId` can never be passed where a `SessionId` is
//! expected. A crate that needs a new identifier declares a marker and an alias:
//!
//! ```
//! use lis_core::Id;
//!
//! /// Marker for specimen identifiers (or use the entity struct itself).
//! pub enum Specimen {}
//! /// Identifier of a specimen.
//! pub type SpecimenId = Id<Specimen>;
//!
//! let id = SpecimenId::generate();
//! let parsed: SpecimenId = id.to_string().parse().unwrap();
//! assert_eq!(id, parsed);
//! ```
//!
//! Every `Id<K>` is a version 7 UUID. It is written as the lowercase
//! hyphenated string in Display, serde, TypeScript (`string`) and SQLite
//! (`TEXT`).

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::str::FromStr;

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

use crate::error::ParseError;

/// Marker types for the identifiers the shared kernel needs.
pub mod kinds {
    /// Marker for [`UserId`](crate::UserId).
    pub enum User {}
    /// Marker for [`SessionId`](crate::SessionId).
    pub enum Session {}
    /// Marker for [`WorkstationId`](crate::WorkstationId).
    pub enum Workstation {}
    /// Marker for [`InstrumentId`](crate::InstrumentId).
    pub enum Instrument {}
    /// Marker for [`CorrelationId`](crate::CorrelationId).
    pub enum Correlation {}
    /// Marker for [`InstallationId`](crate::InstallationId).
    pub enum Installation {}
}

/// A local application user account.
pub type UserId = Id<kinds::User>;
/// One signed-in session of a user.
pub type SessionId = Id<kinds::Session>;
/// The computer (and, later, LAN workstation) a session runs on.
pub type WorkstationId = Id<kinds::Workstation>;
/// A configured analyzer or other instrument.
pub type InstrumentId = Id<kinds::Instrument>;
/// Links one command's audit events, diagnostic log line and returned error.
pub type CorrelationId = Id<kinds::Correlation>;
/// One installation of the application: one laboratory database and its lineage. Audit
/// anchors, backup sets and exit exports name it.
pub type InstallationId = Id<kinds::Installation>;

/// A UUIDv7 identifier of the entity named by the marker `K`.
pub struct Id<K> {
    uuid: Uuid,
    kind: PhantomData<fn() -> K>,
}

impl<K> Id<K> {
    /// Generates a new time-ordered identifier from the system clock and OS randomness.
    #[must_use]
    pub fn generate() -> Self {
        Self::from_uuid(Uuid::now_v7())
    }

    fn from_uuid(uuid: Uuid) -> Self {
        Self {
            uuid,
            kind: PhantomData,
        }
    }

    /// Accepts only the canonical lowercase hyphenated form, so stored and
    /// compared text always matches.
    fn parse(text: &str) -> Result<Self, ParseError> {
        let uuid = Uuid::try_parse(text).map_err(|_| ParseError::Id)?;
        if uuid.get_version_num() != 7 || uuid.hyphenated().to_string() != text {
            return Err(ParseError::Id);
        }
        Ok(Self::from_uuid(uuid))
    }
}

impl<K> Clone for Id<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K> Copy for Id<K> {}

impl<K> PartialEq for Id<K> {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid
    }
}

impl<K> Eq for Id<K> {}

impl<K> Hash for Id<K> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.uuid.hash(state);
    }
}

impl<K> PartialOrd for Id<K> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<K> Ord for Id<K> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.uuid.cmp(&other.uuid)
    }
}

impl<K> fmt::Debug for Id<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = std::any::type_name::<K>()
            .rsplit("::")
            .next()
            .unwrap_or("?");
        write!(f, "Id<{kind}>({})", self.uuid)
    }
}

impl<K> fmt::Display for Id<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.uuid.hyphenated().fmt(f)
    }
}

impl<K> FromStr for Id<K> {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

impl<K> Serialize for Id<K> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de, K> Deserialize<'de> for Id<K> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

impl<K: 'static> ts_rs::TS for Id<K> {
    type WithoutGenerics = Self;
    type OptionInnerType = Self;

    fn name(_: &ts_rs::Config) -> String {
        "string".to_owned()
    }

    fn inline(cfg: &ts_rs::Config) -> String {
        Self::name(cfg)
    }
}

impl<K> ToSql for Id<K> {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.to_string()))
    }
}

impl<K> FromSql for Id<K> {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        Self::parse(value.as_str()?).map_err(FromSqlError::other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_and_parse_round_trip() {
        let id = UserId::generate();
        let text = id.to_string();
        assert_eq!(text.len(), 36);
        assert_eq!(text, text.to_lowercase());
        assert_eq!(text.parse::<UserId>().unwrap(), id);
    }

    #[test]
    fn parse_rejects_non_v7_and_malformed_text() {
        // Version 4 and the nil UUID are valid UUIDs but not UUIDv7.
        assert_eq!(
            "4f1d2a3b-5c6d-4e7f-8a9b-0c1d2e3f4a5b".parse::<UserId>(),
            Err(ParseError::Id)
        );
        assert_eq!(
            "00000000-0000-0000-0000-000000000000".parse::<UserId>(),
            Err(ParseError::Id)
        );
        assert_eq!("".parse::<UserId>(), Err(ParseError::Id));
        // Other spellings of a valid UUIDv7 are not the canonical form.
        let id = UserId::generate().to_string();
        for text in [
            id.to_uppercase(),
            format!("{{{id}}}"),
            format!("urn:uuid:{id}"),
            id.replace('-', ""),
        ] {
            assert_eq!(text.parse::<UserId>(), Err(ParseError::Id), "{text}");
        }
        assert_eq!("not-an-id".parse::<UserId>(), Err(ParseError::Id));
    }

    #[test]
    fn serde_uses_the_string_form() {
        let id = SessionId::generate();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{id}\""));
        assert_eq!(serde_json::from_str::<SessionId>(&json).unwrap(), id);
        assert!(serde_json::from_str::<SessionId>("42").is_err());
    }

    #[test]
    fn sqlite_stores_text() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let id = InstrumentId::generate();
        let (kind, back): (String, InstrumentId) = conn
            .query_row("SELECT typeof(?1), ?1", [id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!(kind, "text");
        assert_eq!(back, id);
    }

    #[test]
    fn typescript_type_is_string() {
        use ts_rs::TS;
        assert_eq!(CorrelationId::inline(&ts_rs::Config::default()), "string");
    }
}
