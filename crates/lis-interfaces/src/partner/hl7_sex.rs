//! PID-8 of every HL7 message the LIS sends (ticket #4675): the administrative sex is
//! recorded as written on the requisition, and HL7 v2.5.1 expects a code from table 0001.
//! The record is never changed; only the sent field is coded.

/// HL7 v2.5.1 table 0001, administrative sex: each code and its name.
pub(crate) const TABLE_0001: [(&str, &str); 6] = [
    ("A", "ambiguous"),
    ("F", "female"),
    ("M", "male"),
    ("N", "not applicable"),
    ("O", "other"),
    ("U", "unknown"),
];

/// The table 0001 code for the sex as recorded: a code or a name, in any case, gives its
/// code; any other value gives `U` (unknown), never a guess; none recorded gives an empty
/// field.
#[must_use]
pub fn administrative_sex(recorded: Option<&str>) -> &'static str {
    let Some(recorded) = recorded.map(str::trim).filter(|value| !value.is_empty()) else {
        return "";
    };
    TABLE_0001
        .iter()
        .find(|(code, name)| {
            recorded.eq_ignore_ascii_case(code) || recorded.eq_ignore_ascii_case(name)
        })
        .map_or("U", |(code, _)| code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_names_give_their_code() {
        assert_eq!(administrative_sex(Some("Female")), "F");
        assert_eq!(administrative_sex(Some(" male ")), "M");
        assert_eq!(administrative_sex(Some("f")), "F");
        assert_eq!(administrative_sex(Some("Not applicable")), "N");
        assert_eq!(administrative_sex(Some("U")), "U");
    }

    #[test]
    fn an_unrecognised_value_is_unknown_and_none_is_empty() {
        assert_eq!(administrative_sex(Some("W")), "U");
        assert_eq!(administrative_sex(Some("Woman")), "U");
        assert_eq!(administrative_sex(Some("  ")), "");
        assert_eq!(administrative_sex(None), "");
    }
}
