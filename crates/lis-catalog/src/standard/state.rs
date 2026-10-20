//! Whether each Standard Catalog item can be loaded into this laboratory's
//! catalog (REQ-OPS-014). Codes are permanent, so an existing code is either
//! the same item (skipped) or a different one (refused). The preview tree and
//! the load command share these rules.

use std::collections::HashMap;

use lis_core::text_enum;
use rusqlite::Connection;
use serde::Serialize;
use ts_rs::TS;

use crate::error::CatalogError;
use crate::repo::reference;
use crate::standard::format::{Analyte, Method, Orderable, SpecimenType};
use crate::views::{AnalyteView, MethodView, OrderableView, SpecimenTypeView};

text_enum! {
    /// Whether a Standard Catalog item can be loaded.
    StandardItemState {
        /// The code is free; loading creates the item as a draft.
        New = "new",
        /// The same item is already in the laboratory's catalog; it is skipped.
        AlreadyPresent = "already_present",
        /// The code is used by a different item; it cannot be loaded.
        CodeInUse = "code_in_use",
        /// A test or panel whose method is already in the laboratory's
        /// catalog. In version 1 a test loads only with its methods.
        Blocked = "blocked",
    }
}

/// An item's state, with the reason when it cannot be loaded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StandardItemStatus {
    /// Whether the item can be loaded.
    pub state: StandardItemState,
    /// Why it cannot be loaded, for `code_in_use` and `blocked`.
    pub reason: Option<String>,
}

impl StandardItemStatus {
    fn new(state: StandardItemState) -> Self {
        Self {
            state,
            reason: None,
        }
    }

    fn refused(state: StandardItemState, reason: String) -> Self {
        Self {
            state,
            reason: Some(reason),
        }
    }
}

/// The laboratory's reference rows by code.
pub(crate) struct Existing {
    pub(crate) specimen_types: HashMap<String, SpecimenTypeView>,
    pub(crate) analytes: HashMap<String, AnalyteView>,
    pub(crate) methods: HashMap<String, MethodView>,
    orderables: HashMap<String, OrderableView>,
}

impl Existing {
    pub(crate) fn load(conn: &Connection) -> Result<Self, CatalogError> {
        Ok(Self {
            specimen_types: by_code(reference::specimen_types(conn)?, |v| &v.code),
            analytes: by_code(reference::analytes(conn)?, |v| &v.code),
            methods: by_code(reference::methods(conn)?, |v| &v.code),
            orderables: by_code(reference::orderables(conn)?, |v| &v.code),
        })
    }

    pub(crate) fn specimen_type(&self, item: &SpecimenType) -> StandardItemStatus {
        let existing = self.specimen_types.get(&item.code).map(|v| v.name.as_str());
        by_name(existing, &item.name)
    }

    /// The same analyte has the same code, name, LOINC code and drug class.
    pub(crate) fn analyte(&self, item: &Analyte) -> StandardItemStatus {
        let Some(existing) = self.analytes.get(&item.code) else {
            return StandardItemStatus::new(StandardItemState::New);
        };
        let loinc = item.loinc.as_ref().map(|l| l.code.as_str());
        let mut differences = Vec::new();
        if existing.name != item.name.trim() {
            differences.push("name");
        }
        if existing.loinc_code.as_deref() != loinc {
            differences.push("LOINC code");
        }
        if existing.drug_class_code != item.drug_class {
            differences.push("drug class");
        }
        if differences.is_empty() {
            StandardItemStatus::new(StandardItemState::AlreadyPresent)
        } else {
            code_in_use(&differences.join(", "))
        }
    }

    pub(crate) fn method(&self, item: &Method) -> StandardItemStatus {
        let existing = self.methods.get(&item.code).map(|v| v.name.as_str());
        by_name(existing, &item.name)
    }

    /// A new test or panel is blocked when any of its methods' codes exists.
    pub(crate) fn orderable(&self, item: &Orderable) -> StandardItemStatus {
        let existing = self.orderables.get(&item.code).map(|v| v.name.as_str());
        let status = by_name(existing, &item.name);
        let method_exists = item
            .method_codes()
            .iter()
            .any(|code| self.methods.contains_key(*code));
        if status.state == StandardItemState::New && method_exists {
            return StandardItemStatus::refused(
                StandardItemState::Blocked,
                "Method already in your catalog; add this test by hand.".to_owned(),
            );
        }
        status
    }
}

fn by_code<V>(rows: Vec<V>, code: impl Fn(&V) -> &String) -> HashMap<String, V> {
    rows.into_iter()
        .map(|row| (code(&row).clone(), row))
        .collect()
}

/// For specimen types, methods and orderables the same item has the same name.
fn by_name(existing: Option<&str>, name: &str) -> StandardItemStatus {
    match existing {
        None => StandardItemStatus::new(StandardItemState::New),
        Some(existing) if existing == name.trim() => {
            StandardItemStatus::new(StandardItemState::AlreadyPresent)
        }
        Some(_) => code_in_use("name"),
    }
}

fn code_in_use(differences: &str) -> StandardItemStatus {
    StandardItemStatus::refused(
        StandardItemState::CodeInUse,
        format!("Your catalog uses this code with a different {differences}."),
    )
}
