//! Versioned ZPL label templates with named placeholders (REQ-SPC-001).

use std::ops::Range;

use crate::encode::{FieldError, FieldKind, encode_field};

/// Identifies one immutable label template version, for example `specimen` v1.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LabelTemplateRef {
    /// Stable template family name.
    pub id: String,
    /// Version within the family; an issued version never changes.
    pub version: u32,
}

/// Why a label could not be rendered. Names template fields, never values.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LabelError {
    /// No embedded template has this id and version.
    #[error("unknown label template {id} version {version}")]
    UnknownTemplate { id: String, version: u32 },
    /// The template's placeholders and declared fields disagree, or a
    /// placeholder sits outside a `^FH^FD … ^FS` field.
    #[error("label template {id} version {version} is inconsistent")]
    TemplateMismatch { id: &'static str, version: u32 },
    /// A field the template requires was not supplied.
    #[error("missing label field {field}")]
    MissingField { field: &'static str },
    /// A supplied field is not part of the template.
    #[error("unknown label field")]
    UnknownField,
    /// The same field was supplied twice.
    #[error("label field {field} supplied more than once")]
    DuplicateField { field: &'static str },
    /// A field value was refused by the encoder.
    #[error("label field {field} {reason}")]
    InvalidField {
        field: &'static str,
        reason: FieldError,
    },
}

struct FieldSpec {
    name: &'static str,
    kind: FieldKind,
    max_chars: usize,
}

struct Template {
    id: &'static str,
    version: u32,
    source: &'static str,
    fields: &'static [FieldSpec],
    /// Fixed ZPL the demo laboratory's labels add before `^XZ`, in space the layout
    /// leaves free; never a placeholder (proposed REQ-OPS-016).
    demo_marking: &'static str,
}

/// What an installation's labels carry besides their template (proposed REQ-OPS-016).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marking {
    /// A laboratory's label, byte for byte as the template renders it.
    Unmarked,
    /// The demo laboratory's label: a reversed "DEMO" block in the template's free space.
    Demo,
}

/// Every label template. Add a new version as a new file and entry; never
/// edit an issued one.
const TEMPLATES: &[Template] = &[Template {
    id: "specimen",
    version: 1,
    // 2 x 1 inch label at 203 dpi. Only the container identifier is machine
    // readable; patient data appears as text only (domain model, identifier rules).
    source: include_str!("../templates/specimen/v1.zpl"),
    fields: &[
        FieldSpec {
            name: "container_id",
            kind: FieldKind::Code128,
            max_chars: 14,
        },
        FieldSpec {
            name: "subject_name",
            kind: FieldKind::Text,
            max_chars: 28,
        },
        FieldSpec {
            name: "birth_date",
            kind: FieldKind::Text,
            max_chars: 10,
        },
        FieldSpec {
            name: "accession",
            kind: FieldKind::Text,
            max_chars: 16,
        },
        FieldSpec {
            name: "matrix",
            kind: FieldKind::Text,
            max_chars: 12,
        },
    ],
    // A black block with "DEMO" in white, right of the birth-date line, whose text is
    // fixed at 14 characters, so it never covers a field or the barcode.
    demo_marking: "^FO280,136^GB110,32,32^FS\n^FO300,140^A0N,28,28^FR^FDDEMO^FS\n",
}];

/// Renders a label: every value is checked and encoded, then substituted for
/// its `{{name}}` placeholder. Returns the exact bytes to send to the printer.
pub fn render(template: &LabelTemplateRef, fields: &[(&str, &str)]) -> Result<Vec<u8>, LabelError> {
    render_marked(template, fields, Marking::Unmarked)
}

/// Renders as [`render`] does, with the installation's [`Marking`]: an unmarked label is
/// byte-identical to [`render`]'s; a demo label adds the template's fixed demo marking
/// before its final `^XZ` (proposed REQ-OPS-016).
pub fn render_marked(
    template: &LabelTemplateRef,
    fields: &[(&str, &str)],
    marking: Marking,
) -> Result<Vec<u8>, LabelError> {
    let found = TEMPLATES
        .iter()
        .find(|candidate| candidate.id == template.id && candidate.version == template.version)
        .ok_or_else(|| LabelError::UnknownTemplate {
            id: template.id.clone(),
            version: template.version,
        })?;
    let placeholders = check(found)?;
    let values = encode_values(found, fields)?;
    let mut label = String::with_capacity(found.source.len());
    let mut copied = 0;
    for (range, name) in placeholders {
        label.push_str(&found.source[copied..range.start]);
        let index = found.fields.iter().position(|spec| spec.name == name);
        let value = index.and_then(|index| values.get(index));
        label.push_str(value.ok_or(LabelError::TemplateMismatch {
            id: found.id,
            version: found.version,
        })?);
        copied = range.end;
    }
    label.push_str(&found.source[copied..]);
    if marking == Marking::Demo {
        let end = label.rfind("^XZ").ok_or(LabelError::TemplateMismatch {
            id: found.id,
            version: found.version,
        })?;
        label.insert_str(end, found.demo_marking);
    }
    Ok(label.into_bytes())
}

/// Encodes the supplied values in the template's field order.
fn encode_values(template: &Template, fields: &[(&str, &str)]) -> Result<Vec<String>, LabelError> {
    if fields
        .iter()
        .any(|(name, _)| !template.fields.iter().any(|spec| spec.name == *name))
    {
        return Err(LabelError::UnknownField);
    }
    template
        .fields
        .iter()
        .map(|spec| {
            let mut supplied = fields.iter().filter(|(name, _)| *name == spec.name);
            let (_, value) = supplied
                .next()
                .ok_or(LabelError::MissingField { field: spec.name })?;
            if supplied.next().is_some() {
                return Err(LabelError::DuplicateField { field: spec.name });
            }
            encode_field(value, spec.kind, spec.max_chars).map_err(|reason| {
                LabelError::InvalidField {
                    field: spec.name,
                    reason,
                }
            })
        })
        .collect()
}

/// Checks that the placeholders are exactly the declared fields and that each
/// sits inside a `^FH^FD … ^FS` field, so encoding always applies. Returns the
/// placeholders in source order.
fn check(template: &Template) -> Result<Vec<(Range<usize>, &'static str)>, LabelError> {
    let mismatch = LabelError::TemplateMismatch {
        id: template.id,
        version: template.version,
    };
    let placeholders = placeholders(template.source).ok_or_else(|| mismatch.clone())?;
    let all_declared = placeholders.iter().all(|(range, name)| {
        template.fields.iter().any(|spec| spec.name == *name)
            && in_hex_field(template.source, range.start)
    });
    let all_used = template
        .fields
        .iter()
        .all(|spec| placeholders.iter().any(|(_, name)| *name == spec.name));
    if all_declared && all_used {
        Ok(placeholders)
    } else {
        Err(mismatch)
    }
}

/// Finds `{{name}}` placeholders; `None` if a `{{` is not closed.
fn placeholders(source: &'static str) -> Option<Vec<(Range<usize>, &'static str)>> {
    let mut found = Vec::new();
    let mut offset = 0;
    while let Some(start) = source[offset..]
        .find("{{")
        .map(|position| offset + position)
    {
        let end = source[start..]
            .find("}}")
            .map(|position| start + position + 2)?;
        found.push((start..end, &source[start + 2..end - 2]));
        offset = end;
    }
    Some(found)
}

/// True if `position` is inside a field opened with `^FH^FD` and not yet closed.
fn in_hex_field(source: &str, position: usize) -> bool {
    let before = &source[..position];
    before
        .rfind("^FD")
        .is_some_and(|field| before[..field].ends_with("^FH") && !before[field..].contains("^FS"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn specimen_v1() -> LabelTemplateRef {
        LabelTemplateRef {
            id: "specimen".to_owned(),
            version: 1,
        }
    }

    const SYNTHETIC: &[(&str, &str)] = &[
        ("container_id", "C-SYN-0001"),
        ("subject_name", "SYNTHETIC, Patient Alpha"),
        ("birth_date", "1980-01-01"),
        ("accession", "ACC-SYN-0001"),
        ("matrix", "Urine"),
    ];

    #[test]
    fn every_template_matches_its_declared_fields() {
        for template in TEMPLATES {
            assert!(
                check(template).is_ok(),
                "{} v{}",
                template.id,
                template.version
            );
        }
    }

    #[test]
    fn renders_exact_synthetic_label() {
        let label = render(&specimen_v1(), SYNTHETIC).expect("renders");
        let expected = "^XA\n^CI28\n^PW406\n^LL203\n^LH0,0\n\
            ^FO16,10^BY2^BCN,70,Y,N,N^FH^FDC-SYN-0001^FS\n\
            ^FO16,112^A0N,26,26^FH^FDSYNTHETIC, Patient Alpha^FS\n\
            ^FO16,142^A0N,22,22^FH^FDDOB 1980-01-01^FS\n\
            ^FO16,170^A0N,22,22^FH^FDACC ACC-SYN-0001 Urine^FS\n^XZ\n";
        assert_eq!(String::from_utf8(label).expect("ascii"), expected);
    }

    // Proposed REQ-OPS-016: a demo label carries the marking and nothing else changes; a
    // laboratory label is byte-identical to the template's.
    #[test]
    fn renders_exact_synthetic_demo_label() {
        let label = render_marked(&specimen_v1(), SYNTHETIC, Marking::Demo).expect("renders");
        let expected = "^XA\n^CI28\n^PW406\n^LL203\n^LH0,0\n\
            ^FO16,10^BY2^BCN,70,Y,N,N^FH^FDC-SYN-0001^FS\n\
            ^FO16,112^A0N,26,26^FH^FDSYNTHETIC, Patient Alpha^FS\n\
            ^FO16,142^A0N,22,22^FH^FDDOB 1980-01-01^FS\n\
            ^FO16,170^A0N,22,22^FH^FDACC ACC-SYN-0001 Urine^FS\n\
            ^FO280,136^GB110,32,32^FS\n^FO300,140^A0N,28,28^FR^FDDEMO^FS\n^XZ\n";
        assert_eq!(String::from_utf8(label).expect("ascii"), expected);
        assert_eq!(
            render_marked(&specimen_v1(), SYNTHETIC, Marking::Unmarked),
            render(&specimen_v1(), SYNTHETIC)
        );
    }

    #[test]
    fn the_demo_marking_has_no_placeholders() {
        for template in TEMPLATES {
            assert!(template.demo_marking.contains("^FDDEMO^FS"));
            assert!(!template.demo_marking.contains("{{"));
        }
    }

    #[test]
    fn only_the_container_id_is_in_the_barcode() {
        let label =
            String::from_utf8(render(&specimen_v1(), SYNTHETIC).expect("renders")).expect("ascii");
        let barcode_fields: Vec<&str> = label.lines().filter(|line| line.contains("^BC")).collect();
        assert_eq!(
            barcode_fields,
            ["^FO16,10^BY2^BCN,70,Y,N,N^FH^FDC-SYN-0001^FS"]
        );
    }

    #[test]
    fn injected_commands_are_escaped() {
        let mut fields = SYNTHETIC.to_vec();
        fields[1] = ("subject_name", "^XZ^XA~JR");
        let label =
            String::from_utf8(render(&specimen_v1(), &fields).expect("renders")).expect("ascii");
        assert!(label.contains("^FH^FD_5EXZ_5EXA_7EJR^FS"));
        assert_eq!(label.matches("^XZ").count(), 1);
    }

    #[test]
    fn unknown_template_is_refused() {
        let template = LabelTemplateRef {
            id: "specimen".to_owned(),
            version: 9,
        };
        assert_eq!(
            render(&template, SYNTHETIC),
            Err(LabelError::UnknownTemplate {
                id: "specimen".to_owned(),
                version: 9
            })
        );
    }

    #[test]
    fn missing_field_is_refused() {
        assert_eq!(
            render(&specimen_v1(), &SYNTHETIC[1..]),
            Err(LabelError::MissingField {
                field: "container_id"
            })
        );
    }

    #[test]
    fn unknown_field_is_refused() {
        let mut fields = SYNTHETIC.to_vec();
        fields.push(("mrn", "SYN-MRN-0001"));
        assert_eq!(
            render(&specimen_v1(), &fields),
            Err(LabelError::UnknownField)
        );
    }

    #[test]
    fn duplicate_field_is_refused() {
        let mut fields = SYNTHETIC.to_vec();
        fields.push(("matrix", "Oral fluid"));
        assert_eq!(
            render(&specimen_v1(), &fields),
            Err(LabelError::DuplicateField { field: "matrix" })
        );
    }

    #[test]
    fn invalid_value_names_the_field_not_the_value() {
        let mut fields = SYNTHETIC.to_vec();
        fields[1] = ("subject_name", "SYNTHETIC\nSECRET");
        let error = render(&specimen_v1(), &fields).unwrap_err();
        assert_eq!(
            error,
            LabelError::InvalidField {
                field: "subject_name",
                reason: FieldError::ControlCharacter
            }
        );
        assert!(!error.to_string().contains("SECRET"));
    }

    #[test]
    fn placeholder_outside_hex_field_is_a_mismatch() {
        let template = Template {
            id: "test",
            version: 1,
            source: "^XA^FO0,0^FD{{a}}^FS^XZ",
            fields: &[FieldSpec {
                name: "a",
                kind: FieldKind::Text,
                max_chars: 4,
            }],
            demo_marking: "",
        };
        assert!(check(&template).is_err());
    }

    #[test]
    fn undeclared_or_unused_placeholders_are_mismatches() {
        let undeclared = Template {
            id: "test",
            version: 1,
            source: "^XA^FH^FD{{a}}{{b}}^FS^XZ",
            fields: &[FieldSpec {
                name: "a",
                kind: FieldKind::Text,
                max_chars: 4,
            }],
            demo_marking: "",
        };
        let unused = Template {
            source: "^XA^XZ",
            ..undeclared
        };
        let unclosed = Template {
            source: "^XA^FH^FD{{a^FS^XZ",
            ..undeclared
        };
        assert!(check(&undeclared).is_err());
        assert!(check(&unused).is_err());
        assert!(check(&unclosed).is_err());
    }
}
