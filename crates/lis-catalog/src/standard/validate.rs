//! The Standard Catalog file rules that need no network (`standard/README.md`,
//! rules 1–5 and the record rules). LOINC verification against the pinned
//! release (rule 6) needs the release files and runs outside this crate.

use std::collections::HashSet;

use lis_core::Decimal;

use crate::content::{component_count, unit};
use crate::error::{CatalogError, invalid};
use crate::model::{CODE_MAX, NAME_MAX, code, text};
use crate::standard::format::{
    Analyte, Component, Cutoff, Method, Orderable, Source, SpecimenType, StandardCatalog,
};

/// The only `format` value of version 1.
const FORMAT: &str = "vibe-tox/standard-catalog";
const FORMAT_VERSION: u32 = 1;
/// Longest LOINC display name accepted.
const LOINC_NAME_MAX: usize = 400;

/// Checks every rule; the first violation names its field path.
pub(super) fn validate(c: &StandardCatalog) -> Result<(), CatalogError> {
    header(c)?;
    let sources = unique("sources", c.sources.iter().map(|s| s.id.as_str()))?;
    for (index, source) in c.sources.iter().enumerate() {
        source_record(&format!("sources[{index}]"), source)?;
    }
    let classes = unique(
        "drugClasses",
        c.drug_classes.iter().map(|d| d.code.as_str()),
    )?;
    for (index, class) in c.drug_classes.iter().enumerate() {
        text(&format!("drugClasses[{index}].name"), &class.name, NAME_MAX)?;
    }
    let specimen_types = unique(
        "specimenTypes",
        c.specimen_types.iter().map(|s| s.code.as_str()),
    )?;
    for (index, specimen_type) in c.specimen_types.iter().enumerate() {
        specimen_type_record(&format!("specimenTypes[{index}]"), specimen_type, &sources)?;
    }
    unique("analytes", c.analytes.iter().map(|a| a.code.as_str()))?;
    for (index, analyte) in c.analytes.iter().enumerate() {
        analyte_record(&format!("analytes[{index}]"), analyte, &classes, &sources)?;
    }
    unique("methods", c.methods.iter().map(|m| m.code.as_str()))?;
    for (index, method) in c.methods.iter().enumerate() {
        method_record(
            &format!("methods[{index}]"),
            c,
            method,
            &specimen_types,
            &sources,
        )?;
    }
    unique("orderables", c.orderables.iter().map(|o| o.code.as_str()))?;
    for (index, orderable) in c.orderables.iter().enumerate() {
        orderable_record(&format!("orderables[{index}]"), c, orderable, &sources)?;
    }
    Ok(())
}

fn header(c: &StandardCatalog) -> Result<(), CatalogError> {
    if c.format != FORMAT || c.format_version != FORMAT_VERSION {
        return Err(invalid(
            "formatVersion",
            "unsupported",
            "This build reads format version 1 only.",
        ));
    }
    // The version is an audit detail, so it must be a machine token.
    let version_ok = !c.catalog_version.is_empty()
        && c.catalog_version.len() <= CODE_MAX
        && c.catalog_version
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-'));
    if !version_ok {
        return Err(invalid(
            "catalogVersion",
            "invalid_version",
            "Use lowercase letters, digits, '.' and '-'.",
        ));
    }
    required("disclaimer", &c.disclaimer)?;
    text("loinc.version", &c.loinc.version, CODE_MAX)?;
    required("loinc.notice", &c.loinc.notice)
}

/// Long verbatim text: present and not blank.
fn required(field: &str, value: &str) -> Result<(), CatalogError> {
    if value.trim().is_empty() {
        Err(invalid(field, "required", "Enter a value."))
    } else {
        Ok(())
    }
}

/// Every code has the catalog's code syntax and appears once.
fn unique<'a>(
    kind: &str,
    codes: impl Iterator<Item = &'a str>,
) -> Result<HashSet<&'a str>, CatalogError> {
    let mut seen = HashSet::new();
    for (index, value) in codes.enumerate() {
        let field = format!("{kind}[{index}]");
        code(&field, value)?;
        if !seen.insert(value) {
            return Err(invalid(&field, "duplicate", "This code is listed twice."));
        }
    }
    Ok(seen)
}

fn source_record(field: &str, source: &Source) -> Result<(), CatalogError> {
    text(&format!("{field}.title"), &source.title, NAME_MAX * 2)?;
    text(&format!("{field}.publisher"), &source.publisher, NAME_MAX)?;
    if !source.url.starts_with("https://") && !source.url.starts_with("http://") {
        return Err(invalid(
            &format!("{field}.url"),
            "invalid_url",
            "Use an http or https address.",
        ));
    }
    if let Some(published) = &source.published {
        date(&format!("{field}.published"), published)?;
    }
    date(&format!("{field}.checked"), &source.checked)?;
    if source.note.chars().any(char::is_control) {
        return Err(invalid(
            &format!("{field}.note"),
            "control_character",
            "Remove control characters.",
        ));
    }
    Ok(())
}

/// A calendar date written `YYYY-MM-DD`.
fn date(field: &str, value: &str) -> Result<(), CatalogError> {
    let bytes = value.as_bytes();
    let shape_ok = bytes.len() == 10
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            4 | 7 => *byte == b'-',
            _ => byte.is_ascii_digit(),
        });
    if shape_ok {
        Ok(())
    } else {
        Err(invalid(
            field,
            "invalid_date",
            "Write the date as YYYY-MM-DD.",
        ))
    }
}

/// A record cites at least one source, and every cited source exists.
fn cited(field: &str, cited: &[String], sources: &HashSet<&str>) -> Result<(), CatalogError> {
    if cited.is_empty() {
        return Err(invalid(field, "required", "Cite at least one source."));
    }
    for (index, source) in cited.iter().enumerate() {
        resolves(&format!("{field}[{index}]"), source, sources)?;
    }
    Ok(())
}

/// README rule 3: the reference resolves to a record in the file.
fn resolves(field: &str, reference: &str, known: &HashSet<&str>) -> Result<(), CatalogError> {
    if known.contains(reference) {
        Ok(())
    } else {
        Err(unresolved(field))
    }
}

fn unresolved(field: &str) -> CatalogError {
    invalid(field, "unknown", "This reference does not resolve.")
}

fn specimen_type_record(
    field: &str,
    specimen_type: &SpecimenType,
    sources: &HashSet<&str>,
) -> Result<(), CatalogError> {
    text(&format!("{field}.name"), &specimen_type.name, NAME_MAX)?;
    cited(&format!("{field}.sources"), &specimen_type.sources, sources)
}

fn analyte_record(
    field: &str,
    analyte: &Analyte,
    classes: &HashSet<&str>,
    sources: &HashSet<&str>,
) -> Result<(), CatalogError> {
    text(&format!("{field}.name"), &analyte.name, NAME_MAX)?;
    if let Some(class) = &analyte.drug_class {
        resolves(&format!("{field}.drugClass"), class, classes)?;
    }
    if let Some(loinc) = &analyte.loinc {
        code(&format!("{field}.loinc.code"), &loinc.code)?;
        text(
            &format!("{field}.loinc.displayName"),
            &loinc.display_name,
            LOINC_NAME_MAX,
        )?;
    }
    cited(&format!("{field}.sources"), &analyte.sources, sources)
}

fn method_record(
    field: &str,
    c: &StandardCatalog,
    method: &Method,
    specimen_types: &HashSet<&str>,
    sources: &HashSet<&str>,
) -> Result<(), CatalogError> {
    text(&format!("{field}.name"), &method.name, NAME_MAX)?;
    let specimen_type = method.specimen_type.as_str();
    resolves(
        &format!("{field}.specimenType"),
        specimen_type,
        specimen_types,
    )?;
    if method.analytes.is_empty() {
        return Err(invalid(
            &format!("{field}.analytes"),
            "required",
            "Add at least one analyte.",
        ));
    }
    let prefix = format!("{specimen_type}-");
    for (index, entry) in method.analytes.iter().enumerate() {
        let field = format!("{field}.analytes[{index}]");
        if c.analyte(&entry.analyte).is_none() {
            return Err(unresolved(&format!("{field}.analyte")));
        }
        // README rule 4: an analyte belongs to one specimen type.
        if !entry.analyte.starts_with(&prefix) {
            return Err(invalid(
                &format!("{field}.analyte"),
                "specimen_type_mismatch",
                "The analyte code must start with the method's specimen type code.",
            ));
        }
        if method.analytes[..index]
            .iter()
            .any(|a| a.analyte == entry.analyte)
        {
            return Err(invalid(
                &field,
                "duplicate",
                "This analyte is listed twice.",
            ));
        }
        unit(&format!("{field}.unit"), entry.unit.as_deref())?;
        if let Some(cutoff) = &entry.cutoff {
            cutoff_record(&format!("{field}.cutoff"), cutoff, sources)?;
        }
    }
    cited(&format!("{field}.sources"), &method.sources, sources)
}

/// README rule 5: a cutoff is a non-negative decimal with a source and a label.
fn cutoff_record(
    field: &str,
    cutoff: &Cutoff,
    sources: &HashSet<&str>,
) -> Result<(), CatalogError> {
    if cutoff.value.to_decimal() < Decimal::ZERO {
        return Err(invalid(
            &format!("{field}.value"),
            "negative",
            "A cutoff cannot be negative.",
        ));
    }
    resolves(&format!("{field}.source"), &cutoff.source, sources)?;
    text(&format!("{field}.label"), &cutoff.label, NAME_MAX)?;
    Ok(())
}

fn orderable_record(
    field: &str,
    c: &StandardCatalog,
    orderable: &Orderable,
    sources: &HashSet<&str>,
) -> Result<(), CatalogError> {
    text(&format!("{field}.name"), &orderable.name, NAME_MAX)?;
    component_count(
        &format!("{field}.components"),
        orderable.kind,
        orderable.components.len(),
    )?;
    for (index, component) in orderable.components.iter().enumerate() {
        let field = format!("{field}.components[{index}]");
        if orderable.components[..index]
            .iter()
            .any(|other| other.method == component.method)
        {
            return Err(invalid(&field, "duplicate", "This method is listed twice."));
        }
        component_record(&field, c, component)?;
    }
    cited(&format!("{field}.sources"), &orderable.sources, sources)
}

fn component_record(
    field: &str,
    c: &StandardCatalog,
    component: &Component,
) -> Result<(), CatalogError> {
    let Some(method) = c.method(&component.method) else {
        return Err(unresolved(&format!("{field}.method")));
    };
    if component.analytes.is_empty() {
        return Err(invalid(
            &format!("{field}.analytes"),
            "required",
            "Add at least one analyte.",
        ));
    }
    for (index, analyte) in component.analytes.iter().enumerate() {
        let field = format!("{field}.analytes[{index}]");
        if component.analytes[..index].contains(analyte) {
            return Err(invalid(
                &field,
                "duplicate",
                "This analyte is listed twice.",
            ));
        }
        if method.analyte(analyte).is_none() {
            return Err(invalid(
                &field,
                "not_measured",
                "The method does not measure this analyte.",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::error::CatalogError;
    use crate::standard::StandardFile;

    /// The file this build ships parses and passes every offline rule.
    #[test]
    fn the_shipped_standard_catalog_is_valid() {
        let file = StandardFile::embedded().unwrap();
        let info = file.info();
        assert_eq!(info.file_digest.len(), 64);
        assert!(!info.disclaimer.is_empty());
        assert!(info.loinc_notice.contains("Regenstrief"));
    }

    /// REQ-OPS-014: clinical starter cutoffs transcribed for content 1.1.0.
    /// Source comparisons and assay qualifications are in standard/content-notes.md.
    /// This is an evidence-transcription pin, not clinical validation. Update only
    /// after checking the publication for the exact analyte, matrix and technique.
    const PINNED_CUTOFFS: [(&str, &str, &str, &str); 101] = [
        ("UR-IA-SCREEN", "UR-AMP-IA", "500", "DOM-143"),
        ("UR-IA-SCREEN", "UR-BAR-IA", "200", "DOM-143"),
        ("UR-IA-SCREEN", "UR-BZO-IA", "100", "DOM-143"),
        ("UR-IA-SCREEN", "UR-BUP-IA", "5", "DOM-142"),
        ("UR-IA-SCREEN", "UR-THC-IA", "50", "DOM-143"),
        ("UR-IA-SCREEN", "UR-COC-IA", "150", "DOM-143"),
        ("UR-IA-SCREEN", "UR-MTD-IA", "150", "DOM-142"),
        ("UR-IA-SCREEN", "UR-EDDP-IA", "100", "DOM-145"),
        ("UR-IA-SCREEN", "UR-FEN-IA", "1", "DOM-142"),
        ("UR-IA-SCREEN", "UR-OPI-IA", "300", "DOM-143"),
        ("UR-IA-SCREEN", "UR-OXY-IA", "100", "DOM-142"),
        ("UR-IA-SCREEN", "UR-6AM-IA", "10", "DOM-146"),
        ("UR-IA-SCREEN", "UR-PCP-IA", "25", "DOM-143"),
        ("UR-IA-SCREEN", "UR-TRA-IA", "100", "DOM-142"),
        ("UR-IA-SCREEN", "UR-MDMA-IA", "500", "DOM-142"),
        ("UR-IA-SCREEN", "UR-ETG-IA", "500", "DOM-142"),
        ("OF-IA-SCREEN", "OF-AMP-IA", "20", "DOM-170"),
        ("OF-IA-SCREEN", "OF-THC-IA", "4", "DOM-171"),
        ("OF-IA-SCREEN", "OF-COC-IA", "20", "DOM-172"),
        ("OF-IA-SCREEN", "OF-MTD-IA", "10", "DOM-170"),
        ("OF-IA-SCREEN", "OF-FEN-IA", "1", "DOM-170"),
        ("OF-IA-SCREEN", "OF-OPI-IA", "10", "DOM-170"),
        ("OF-IA-SCREEN", "OF-OXY-IA", "10", "DOM-170"),
        ("OF-IA-SCREEN", "OF-6AM-IA", "5", "DOM-170"),
        ("OF-IA-SCREEN", "OF-PCP-IA", "5", "DOM-170"),
        ("UR-LCMSMS-OPIATE", "UR-MOR", "25", "DOM-148"),
        ("UR-LCMSMS-OPIATE", "UR-COD", "25", "DOM-148"),
        ("UR-LCMSMS-OPIATE", "UR-6AM", "10", "DOM-147"),
        ("UR-LCMSMS-OPIATE", "UR-DHC", "25", "DOM-148"),
        ("UR-LCMSMS-OPIATE", "UR-HYC", "25", "DOM-148"),
        ("UR-LCMSMS-OPIATE", "UR-NHYC", "25", "DOM-148"),
        ("UR-LCMSMS-OPIATE", "UR-HYM", "25", "DOM-148"),
        ("UR-LCMSMS-OPIATE", "UR-OXYC", "25", "DOM-148"),
        ("UR-LCMSMS-OPIATE", "UR-NOXYC", "25", "DOM-148"),
        ("UR-LCMSMS-OPIATE", "UR-OXYM", "25", "DOM-148"),
        ("UR-LCMSMS-FENTANYL", "UR-FENT", "1", "DOM-150"),
        ("UR-LCMSMS-FENTANYL", "UR-NFENT", "2", "DOM-150"),
        ("UR-LCMSMS-METHADONE", "UR-MTD", "100", "DOM-152"),
        ("UR-LCMSMS-METHADONE", "UR-EDDP", "100", "DOM-152"),
        ("UR-LCMSMS-BUPRENORPHINE", "UR-BUP", "2", "DOM-153"),
        ("UR-LCMSMS-BUPRENORPHINE", "UR-NBUP", "2", "DOM-153"),
        ("UR-LCMSMS-OPIOID-DUAL", "UR-TRA", "25", "DOM-154"),
        ("UR-LCMSMS-OPIOID-DUAL", "UR-ODT", "25", "DOM-154"),
        ("UR-LCMSMS-OPIOID-DUAL", "UR-TAP", "50", "DOM-149"),
        ("UR-LCMSMS-OPIOID-DUAL", "UR-NTAP", "50", "DOM-149"),
        ("UR-LCMSMS-BENZO", "UR-ALP", "5", "DOM-155"),
        ("UR-LCMSMS-BENZO", "UR-AOHALP", "5", "DOM-155"),
        ("UR-LCMSMS-BENZO", "UR-CLO", "5", "DOM-155"),
        ("UR-LCMSMS-BENZO", "UR-7ACLO", "5", "DOM-155"),
        ("UR-LCMSMS-BENZO", "UR-DIAZ", "20", "DOM-155"),
        ("UR-LCMSMS-BENZO", "UR-NORD", "20", "DOM-155"),
        ("UR-LCMSMS-BENZO", "UR-OXAZ", "20", "DOM-155"),
        ("UR-LCMSMS-BENZO", "UR-TEMA", "20", "DOM-155"),
        ("UR-LCMSMS-BENZO", "UR-LORA", "20", "DOM-155"),
        ("UR-LCMSMS-AMPHET", "UR-AMP", "50", "DOM-157"),
        ("UR-LCMSMS-AMPHET", "UR-MAMP", "200", "DOM-157"),
        ("UR-LCMSMS-AMPHET", "UR-MDMA", "200", "DOM-157"),
        ("UR-LCMSMS-AMPHET", "UR-MDA", "200", "DOM-157"),
        ("UR-LCMSMS-AMPHET", "UR-MDEA", "200", "DOM-157"),
        ("UR-LCMSMS-COCAINE", "UR-BZE", "50", "DOM-159"),
        ("UR-LCMSMS-COCAINE", "UR-COC", "25", "DOM-160"),
        ("UR-LCMSMS-CANNABINOID", "UR-THCA", "15", "DOM-161"),
        ("UR-LCMSMS-BARBITURATE", "UR-BUTA", "50", "DOM-162"),
        ("UR-LCMSMS-BARBITURATE", "UR-PHENO", "50", "DOM-162"),
        ("UR-LCMSMS-BARBITURATE", "UR-PENTO", "50", "DOM-162"),
        ("UR-LCMSMS-BARBITURATE", "UR-SECO", "100", "DOM-149"),
        ("UR-LCMSMS-DISSOCIATIVE", "UR-PCP", "25", "DOM-149"),
        ("UR-LCMSMS-DISSOCIATIVE", "UR-KET", "50", "DOM-149"),
        ("UR-LCMSMS-ALCOHOL-MARKER", "UR-ETG", "250", "DOM-163"),
        ("UR-LCMSMS-ALCOHOL-MARKER", "UR-ETS", "100", "DOM-163"),
        ("UR-LCMSMS-MUSCLE-RELAXANT", "UR-CARI", "100", "DOM-165"),
        ("UR-LCMSMS-MUSCLE-RELAXANT", "UR-MEPRO", "100", "DOM-165"),
        ("UR-LCMSMS-GABAPENTINOID", "UR-GABA", "500", "DOM-149"),
        ("UR-LCMSMS-GABAPENTINOID", "UR-PREG", "500", "DOM-149"),
        ("OF-LCMSMS-OPIATE", "OF-MOR", "2.5", "DOM-168"),
        ("OF-LCMSMS-OPIATE", "OF-COD", "2.5", "DOM-168"),
        ("OF-LCMSMS-OPIATE", "OF-6AM", "1", "DOM-169"),
        ("OF-LCMSMS-OPIATE", "OF-HYC", "2.5", "DOM-168"),
        ("OF-LCMSMS-OPIATE", "OF-HYM", "2.5", "DOM-168"),
        ("OF-LCMSMS-OPIATE", "OF-OXYC", "2.5", "DOM-168"),
        ("OF-LCMSMS-OPIATE", "OF-OXYM", "2.5", "DOM-168"),
        ("OF-LCMSMS-FENTANYL", "OF-FENT", "0.1", "DOM-168"),
        ("OF-LCMSMS-METHADONE", "OF-MTD", "5", "DOM-168"),
        ("OF-LCMSMS-METHADONE", "OF-EDDP", "5", "DOM-168"),
        ("OF-LCMSMS-BUPRENORPHINE", "OF-BUP", "0.1", "DOM-168"),
        ("OF-LCMSMS-BUPRENORPHINE", "OF-NBUP", "0.5", "DOM-168"),
        ("OF-LCMSMS-OPIOID-DUAL", "OF-TRA", "5", "DOM-168"),
        ("OF-LCMSMS-OPIOID-DUAL", "OF-TAP", "5", "DOM-168"),
        ("OF-LCMSMS-BENZO", "OF-ALP", "0.5", "DOM-168"),
        ("OF-LCMSMS-BENZO", "OF-CLO", "0.5", "DOM-168"),
        ("OF-LCMSMS-BENZO", "OF-DIAZ", "0.5", "DOM-168"),
        ("OF-LCMSMS-BENZO", "OF-NORD", "0.5", "DOM-168"),
        ("OF-LCMSMS-BENZO", "OF-OXAZ", "0.5", "DOM-168"),
        ("OF-LCMSMS-BENZO", "OF-TEMA", "0.5", "DOM-168"),
        ("OF-LCMSMS-AMPHET", "OF-AMP", "10", "DOM-168"),
        ("OF-LCMSMS-AMPHET", "OF-MAMP", "10", "DOM-168"),
        ("OF-LCMSMS-AMPHET", "OF-MDMA", "10", "DOM-168"),
        ("OF-LCMSMS-COCAINE", "OF-COC", "5", "DOM-168"),
        ("OF-LCMSMS-COCAINE", "OF-BZE", "5", "DOM-168"),
        ("OF-LCMSMS-CANNABINOID", "OF-THC", "2.5", "DOM-168"),
        ("OF-LCMSMS-DISSOCIATIVE", "OF-PCP", "10", "DOM-168"),
    ];
    #[test]
    fn the_shipped_cutoffs_match_the_reviewed_pin() {
        let file = StandardFile::embedded().unwrap();
        let entries = file
            .catalog()
            .methods
            .iter()
            .flat_map(|m| m.analytes.iter().map(move |a| (m, a)));
        let filled: Vec<_> = entries
            .filter_map(|(m, a)| {
                let cutoff = a.cutoff.as_ref()?;
                Some((
                    m.code.as_str(),
                    a.analyte.as_str(),
                    cutoff.value.as_str(),
                    cutoff.source.as_str(),
                ))
            })
            .collect();
        assert_eq!(filled, PINNED_CUTOFFS);
    }

    fn field_of(json: &str) -> (String, String) {
        match StandardFile::parse(json) {
            Err(CatalogError::StandardFile(field)) => (field.field, field.code),
            other => panic!("{other:?}"),
        }
    }

    fn minimal(extra: &str) -> String {
        format!(
            r#"{{"format":"vibe-tox/standard-catalog","formatVersion":1,"catalogVersion":"9.9.9",
            "reviewStatus":"unreviewed","disclaimer":"Synthetic","loinc":{{"version":"0","notice":"N"}},
            "sources":[{{"id":"SRC","title":"T","publisher":"P","url":"https://example.com",
            "published":null,"checked":"2026-10-13","note":""}}],"drugClasses":[],
            "specimenTypes":[{{"code":"SY","name":"Synthetic","sources":["SRC"]}}],
            "analytes":[{{"code":"SY-A","name":"A","drugClass":null,"loinc":null,"sources":["SRC"]}}],
            {extra}}}"#
        )
    }

    #[test]
    fn file_rules_name_the_failing_field() {
        let method = |analyte: &str, cutoff: &str| {
            format!(
                r#""methods":[{{"code":"M","name":"M","specimenType":"SY","certainty":"definitive",
                "outputScale":"quantitative","thresholdRule":"at_or_above",
                "thresholdRounding":"before_rounding","analytes":[{{"analyte":"{analyte}",
                "unit":"ng/mL","cutoff":{cutoff}}}],"sources":["SRC"]}}],"orderables":[]"#
            )
        };
        assert!(StandardFile::parse(&minimal(&method("SY-A", "null"))).is_ok());
        let cases = [
            (method("XX-A", "null"), "methods[0].analytes[0].analyte"),
            (
                method("SY-A", r#"{"value":"-1","source":"SRC","label":"L"}"#),
                "methods[0].analytes[0].cutoff.value",
            ),
            (
                method("SY-A", r#"{"value":"1","source":"NONE","label":"L"}"#),
                "methods[0].analytes[0].cutoff.source",
            ),
        ];
        for (extra, field) in cases {
            assert_eq!(field_of(&minimal(&extra)).0, field);
        }
        let numeric = method("SY-A", r#"{"value":1,"source":"SRC","label":"L"}"#);
        assert_eq!(
            field_of(&minimal(&numeric)),
            ("file".into(), "syntax".into())
        );
        let unknown = format!("{},\"extra\":1", method("SY-A", "null"));
        assert_eq!(field_of(&minimal(&unknown)).1, "syntax");
    }
}
