//! The dependency closure of a Standard Catalog selection (REQ-OPS-014),
//! computed from the file and the laboratory's catalog; the UI's implied
//! selection is never trusted. A new test or panel implies its methods; a new
//! method implies its specimen type and its analytes, minus omitted ones.
//! Already present items are skipped and not descended into.

use crate::commands::standard::{LoadStandardCatalog, StandardOmittedAnalyte};
use crate::error::{CatalogError, invalid};
use crate::standard::format::{
    Analyte, Component, Method, MethodAnalyte, Orderable, SpecimenType, StandardCatalog,
};
use crate::standard::state::{Existing, StandardItemState, StandardItemStatus};

/// The items a load creates, in file order.
#[derive(Default)]
pub(super) struct Plan<'f> {
    pub(super) specimen_types: Vec<&'f SpecimenType>,
    pub(super) analytes: Vec<&'f Analyte>,
    pub(super) methods: Vec<PlannedMethod<'f>>,
    pub(super) orderables: Vec<PlannedOrderable<'f>>,
    /// Items of the closure already in the laboratory's catalog.
    pub(super) skipped: usize,
}

impl Plan<'_> {
    pub(super) fn is_empty(&self) -> bool {
        self.specimen_types.is_empty()
            && self.analytes.is_empty()
            && self.methods.is_empty()
            && self.orderables.is_empty()
    }
}

/// A method to create, with the analytes kept after omissions.
pub(super) struct PlannedMethod<'f> {
    pub(super) method: &'f Method,
    pub(super) analytes: Vec<&'f MethodAnalyte>,
}

/// A test or panel to create, with each component's kept analytes.
pub(super) struct PlannedOrderable<'f> {
    pub(super) orderable: &'f Orderable,
    pub(super) components: Vec<(&'f Component, Vec<&'f str>)>,
}

/// Sorts closure items into created, skipped and refused.
#[derive(Default)]
struct Admission {
    skipped: usize,
    refused: Vec<String>,
}

impl Admission {
    /// Whether the item is created.
    fn admit(&mut self, status: &StandardItemStatus, code: &str) -> bool {
        match status.state {
            StandardItemState::New => true,
            StandardItemState::AlreadyPresent => {
                self.skipped += 1;
                false
            }
            StandardItemState::CodeInUse | StandardItemState::Blocked => {
                self.refused.push(code.to_owned());
                false
            }
        }
    }
}

/// Plans the load of `input`'s selection, or refuses it naming every code
/// that is in use or blocked.
pub(super) fn plan<'f>(
    c: &'f StandardCatalog,
    existing: &Existing,
    input: &LoadStandardCatalog,
) -> Result<Plan<'f>, CatalogError> {
    let omitted = omissions(c, &input.omitted_analytes)?;
    let orderables = requested("orderableCodes", &input.orderable_codes, |code| {
        c.orderables.iter().any(|o| o.code == code)
    })?;
    let mut methods = requested("methodCodes", &input.method_codes, |code| {
        c.method(code).is_some()
    })?;
    let mut plan = Plan::default();
    let mut admission = Admission::default();
    for o in c
        .orderables
        .iter()
        .filter(|o| orderables.contains(&o.code.as_str()))
    {
        if admission.admit(&existing.orderable(o), &o.code) {
            methods.extend(o.method_codes());
            plan.orderables.push(planned_orderable(o, &omitted)?);
        }
    }
    for m in c
        .methods
        .iter()
        .filter(|m| methods.contains(&m.code.as_str()))
    {
        if admission.admit(&existing.method(m), &m.code) {
            plan.methods.push(planned_method(m, &omitted)?);
        }
    }
    let needs_analyte = |code: &str| {
        plan.methods
            .iter()
            .any(|m| m.analytes.iter().any(|a| a.analyte == code))
    };
    let analytes: Vec<&Analyte> = c
        .analytes
        .iter()
        .filter(|a| needs_analyte(&a.code))
        .collect();
    let needs_specimen_type =
        |code: &str| plan.methods.iter().any(|m| m.method.specimen_type == code);
    let specimen_types: Vec<&SpecimenType> = c
        .specimen_types
        .iter()
        .filter(|s| needs_specimen_type(&s.code))
        .collect();
    for a in analytes {
        if admission.admit(&existing.analyte(a), &a.code) {
            plan.analytes.push(a);
        }
    }
    for s in specimen_types {
        if admission.admit(&existing.specimen_type(s), &s.code) {
            plan.specimen_types.push(s);
        }
    }
    if !admission.refused.is_empty() {
        return Err(CatalogError::StandardItemsUnavailable {
            codes: admission.refused,
        });
    }
    plan.skipped = admission.skipped;
    Ok(plan)
}

/// The requested codes, each of which must be in the file.
fn requested<'a>(
    field: &str,
    codes: &'a [String],
    in_file: impl Fn(&str) -> bool,
) -> Result<Vec<&'a str>, CatalogError> {
    for (index, code) in codes.iter().enumerate() {
        if !in_file(code) {
            return Err(invalid(
                &format!("{field}[{index}]"),
                "unknown",
                "The standard catalog has no item with this code.",
            ));
        }
    }
    Ok(codes.iter().map(String::as_str).collect())
}

/// The omitted (method code, analyte code) pairs, each a method analyte of the file.
fn omissions<'a>(
    c: &StandardCatalog,
    omitted: &'a [StandardOmittedAnalyte],
) -> Result<Vec<(&'a str, &'a str)>, CatalogError> {
    let mut pairs = Vec::new();
    for (index, pair) in omitted.iter().enumerate() {
        let measured = c
            .method(&pair.method_code)
            .is_some_and(|m| m.analyte(&pair.analyte_code).is_some());
        if !measured {
            return Err(invalid(
                &format!("omittedAnalytes[{index}]"),
                "unknown",
                "The standard method does not measure this analyte.",
            ));
        }
        pairs.push((pair.method_code.as_str(), pair.analyte_code.as_str()));
    }
    Ok(pairs)
}

fn planned_method<'f>(
    m: &'f Method,
    omitted: &[(&str, &str)],
) -> Result<PlannedMethod<'f>, CatalogError> {
    let analytes: Vec<&MethodAnalyte> = m
        .analytes
        .iter()
        .filter(|a| !omitted.contains(&(m.code.as_str(), a.analyte.as_str())))
        .collect();
    if analytes.is_empty() {
        return Err(CatalogError::OmittedAllAnalytes {
            code: m.code.clone(),
        });
    }
    Ok(PlannedMethod {
        method: m,
        analytes,
    })
}

/// An omitted analyte also leaves every component that expected it.
fn planned_orderable<'f>(
    o: &'f Orderable,
    omitted: &[(&str, &str)],
) -> Result<PlannedOrderable<'f>, CatalogError> {
    let mut components = Vec::new();
    for component in &o.components {
        let analytes: Vec<&str> = component
            .analytes
            .iter()
            .map(String::as_str)
            .filter(|a| !omitted.contains(&(component.method.as_str(), *a)))
            .collect();
        if analytes.is_empty() {
            return Err(CatalogError::OmittedAllAnalytes {
                code: o.code.clone(),
            });
        }
        components.push((component, analytes));
    }
    Ok(PlannedOrderable {
        orderable: o,
        components,
    })
}
