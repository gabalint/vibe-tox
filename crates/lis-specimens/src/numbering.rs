//! Accession and container numbering: the one home of the formats and of number floors
//! (design brief §2.3, risk R2; M4 design brief §3.8). These are defaults until the
//! laboratory confirms its formats. Neither number carries PHI, and both fit the
//! 14-character Code 128 field of the `specimen` v1 label.
//!
//! Numbers are allocated as the larger of the highest issued number and the highest
//! floor of the same prefix, plus one, inside the command's `BEGIN IMMEDIATE`
//! transaction, so the single writer makes them unique; the `UNIQUE` constraints back
//! this up. A floor only goes up (REQ-OPS-002): after a restore it skips the numbers the
//! lost interval may have put on physical labels.

use lis_core::UtcTimestamp;
use rusqlite::{Connection, params};

use crate::error::{SpecimenError, invalid};

/// Accession numbers: `A`, the two-digit UTC year, then a sequence that restarts each year.
const ACCESSION_PREFIX: &str = "A";
const ACCESSION_DIGITS: usize = 6;
/// Container label codes: `C`, then a sequence that never restarts.
const CONTAINER_PREFIX: &str = "C";
const CONTAINER_DIGITS: usize = 8;

/// A numbering sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sequence {
    /// Accession numbers, such as `A26000001`.
    Accession,
    /// Container label codes, such as `C00000001`.
    Container,
}

impl Sequence {
    /// The stored `kind` and audit code.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Accession => "accession",
            Self::Container => "container",
        }
    }

    fn digits(self) -> usize {
        match self {
            Self::Accession => ACCESSION_DIGITS,
            Self::Container => CONTAINER_DIGITS,
        }
    }

    /// The prefix of the numbers issued at `now`: `A26`, or `C`.
    fn prefix(self, now: UtcTimestamp) -> String {
        match self {
            // The canonical timestamp text starts with the four-digit year.
            Self::Accession => {
                let year = now.to_string().get(2..4).unwrap_or("00").to_owned();
                format!("{ACCESSION_PREFIX}{year}")
            }
            Self::Container => CONTAINER_PREFIX.to_owned(),
        }
    }

    /// The prefix of a number of this sequence: the letter, and the year for accessions.
    fn prefix_len(self) -> usize {
        match self {
            Self::Accession => ACCESSION_PREFIX.len() + 2,
            Self::Container => CONTAINER_PREFIX.len(),
        }
    }

    fn issued_sql(self) -> &'static str {
        match self {
            Self::Accession => {
                "SELECT max(accession_number) FROM spc_accessions \
                 WHERE accession_number LIKE ?1 || '%'"
            }
            Self::Container => {
                "SELECT max(label_code) FROM spc_containers WHERE label_code LIKE ?1 || '%'"
            }
        }
    }
}

/// Where a sequence stands for one prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Position {
    /// The highest number issued with the prefix.
    pub(crate) highest_issued: Option<String>,
    /// The highest floor with the prefix.
    pub(crate) floor: Option<String>,
    /// The number the next allocation issues.
    pub(crate) next: String,
}

/// The next accession number for the year of `now`, such as `A26000001`.
pub(crate) fn next_accession_number(
    conn: &Connection,
    now: UtcTimestamp,
) -> Result<String, SpecimenError> {
    Ok(position(conn, Sequence::Accession, now)?.next)
}

/// The next container label code, such as `C00000001`.
pub(crate) fn next_container_code(conn: &Connection) -> Result<String, SpecimenError> {
    Ok(position_of(conn, Sequence::Container, CONTAINER_PREFIX)?.next)
}

/// Where `sequence` stands for the numbers issued at `now`.
pub(crate) fn position(
    conn: &Connection,
    sequence: Sequence,
    now: UtcTimestamp,
) -> Result<Position, SpecimenError> {
    position_of(conn, sequence, &sequence.prefix(now))
}

fn position_of(
    conn: &Connection,
    sequence: Sequence,
    prefix: &str,
) -> Result<Position, SpecimenError> {
    let highest_issued = highest(conn, sequence.issued_sql(), [prefix])?;
    let floor = highest(
        conn,
        "SELECT max(floor_number) FROM spc_number_floors \
         WHERE kind = ?2 AND floor_number LIKE ?1 || '%'",
        (prefix, sequence.as_str()),
    )?;
    // Same prefix and length, so text order is number order.
    let last = highest_issued.clone().max(floor.clone());
    let next = next(
        prefix,
        last.as_deref(),
        sequence.digits(),
        sequence.as_str(),
    )?;
    Ok(Position {
        highest_issued,
        floor,
        next,
    })
}

fn highest(
    conn: &Connection,
    sql: &str,
    params: impl rusqlite::Params,
) -> Result<Option<String>, SpecimenError> {
    Ok(conn
        .prepare_cached(sql)?
        .query_row(params, |row| row.get(0))?)
}

/// `prefix` followed by the zero-padded successor of `last`'s sequence.
fn next(
    prefix: &str,
    last: Option<&str>,
    digits: usize,
    what: &'static str,
) -> Result<String, SpecimenError> {
    let sequence = match last.and_then(|text| text.strip_prefix(prefix)) {
        Some(digits_text) => digits_text
            .parse::<u64>()
            .map_err(|_| SpecimenError::NumberingExhausted(what))?,
        None => 0,
    };
    let next = sequence + 1;
    let text = format!("{prefix}{next:0digits$}");
    if text.len() > prefix.len() + digits {
        return Err(SpecimenError::NumberingExhausted(what));
    }
    Ok(text)
}

/// Checks a floor typed by a quality manager: the sequence's format, and room for a
/// number above it. Returns the floor and its prefix.
pub(crate) fn parse_floor(
    sequence: Sequence,
    field: &str,
    text: &str,
) -> Result<(String, String), SpecimenError> {
    let text = text.trim();
    let letter = match sequence {
        Sequence::Accession => ACCESSION_PREFIX,
        Sequence::Container => CONTAINER_PREFIX,
    };
    let well_formed = text.len() == sequence.prefix_len() + sequence.digits()
        && text.starts_with(letter)
        && text
            .bytes()
            .skip(letter.len())
            .all(|byte| byte.is_ascii_digit());
    let prefix = text.get(..sequence.prefix_len()).unwrap_or_default();
    if !well_formed || next(prefix, Some(text), sequence.digits(), "floor").is_err() {
        return Err(invalid(
            field,
            "format",
            "Enter a number in the laboratory's format, below the last number of its sequence.",
        ));
    }
    Ok((text.to_owned(), prefix.to_owned()))
}

/// The floor `skip` numbers above where `sequence` stands at `now` (after a restore).
pub(crate) fn floor_after(
    conn: &Connection,
    sequence: Sequence,
    now: UtcTimestamp,
    skip: u32,
) -> Result<String, SpecimenError> {
    let prefix = sequence.prefix(now);
    let next_number = position_of(conn, sequence, &prefix)?.next;
    let next_sequence: u64 = next_number
        .strip_prefix(prefix.as_str())
        .and_then(|digits| digits.parse().ok())
        .ok_or(SpecimenError::NumberingExhausted(sequence.as_str()))?;
    let floor_sequence = next_sequence - 1 + u64::from(skip);
    let digits = sequence.digits();
    let floor = format!("{prefix}{floor_sequence:0digits$}");
    next(&prefix, Some(&floor), digits, sequence.as_str())?;
    Ok(floor)
}

/// Where `prefix` of `sequence` stands, for a floor typed with that prefix.
pub(crate) fn position_for_prefix(
    conn: &Connection,
    sequence: Sequence,
    prefix: &str,
) -> Result<Position, SpecimenError> {
    position_of(conn, sequence, prefix)
}

/// Records a floor; the insert trigger refuses one that does not go up.
pub(crate) fn insert_floor(
    conn: &Connection,
    id: &str,
    sequence: Sequence,
    floor: &str,
    source: &str,
    (set_by, set_at): (&str, UtcTimestamp),
) -> Result<(), SpecimenError> {
    conn.prepare_cached(
        "INSERT INTO spc_number_floors (id, kind, floor_number, source, set_by, set_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?
    .execute(params![
        id,
        sequence.as_str(),
        floor,
        source,
        set_by,
        set_at
    ])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequences_start_at_one_and_are_zero_padded() {
        assert_eq!(next("A26", None, 6, "accession").unwrap(), "A26000001");
        assert_eq!(
            next("A26", Some("A26000041"), 6, "accession").unwrap(),
            "A26000042"
        );
        assert_eq!(
            next("C", Some("C00000009"), 8, "container").unwrap(),
            "C00000010"
        );
    }

    #[test]
    fn an_exhausted_sequence_is_refused_not_widened() {
        assert!(matches!(
            next("A26", Some("A26999999"), 6, "accession"),
            Err(SpecimenError::NumberingExhausted("accession"))
        ));
    }
}
