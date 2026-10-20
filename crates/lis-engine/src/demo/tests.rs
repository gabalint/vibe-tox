//! The shipped demo file: its structure, its fictitious-identifier rules (starter data
//! brief, "Fictitious identifier rules") and its demo cutoffs. Synthetic data only.

use std::collections::BTreeSet;

use lis_ops::InstallationKind;
use serde_json::Value;

use super::demo_cutoff_values;
use super::file::{DEMO_CUTOFF_LABEL, DemoFile};

const CATALOG: &str = include_str!("../../../lis-catalog/standard/catalog-v1.json");

fn file() -> DemoFile {
    DemoFile::embedded()
        .expect("the shipped demo file is valid")
        .0
}

/// NANPA reserves 555-0100 to 555-0199 for fictitious use (DOM-108).
fn fictitious_phone(number: &str) -> bool {
    let digits: String = number.chars().filter(char::is_ascii_digit).collect();
    digits.len() == 10 && digits[3..8] == *"55501"
}

/// RFC 2606 reserved names (DOM-109).
fn example_email(address: &str) -> bool {
    address
        .rsplit_once('@')
        .is_some_and(|(_, domain)| domain.ends_with(".example") || domain == "example.com")
}

/// The NPI check digit: Luhn over `80840` and the first nine digits (DOM-103).
fn npi_check_digit_valid(npi: &str) -> bool {
    let digits: Vec<u32> = npi.chars().filter_map(|c| c.to_digit(10)).collect();
    if digits.len() != 10 || npi.len() != 10 {
        return false;
    }
    let body: Vec<u32> = [8, 0, 8, 4, 0]
        .into_iter()
        .chain(digits[..9].iter().copied())
        .collect();
    let sum: u32 = body
        .iter()
        .rev()
        .enumerate()
        .map(|(position, &digit)| {
            if position % 2 == 0 {
                let doubled = digit * 2;
                if doubled > 9 { doubled - 9 } else { doubled }
            } else {
                digit
            }
        })
        .sum();
    (10 - sum % 10) % 10 == digits[9]
}

#[test]
fn the_shipped_file_is_valid_and_complete() {
    let file = file();
    assert_eq!(file.data_version, "1.1.0");
    assert_eq!(file.clients.len(), 4);
    assert_eq!(file.practitioners.len(), 8);
    assert_eq!(file.patients.len(), 12);
    let locations: usize = file.clients.iter().map(|c| c.locations.len()).sum();
    let destinations: usize = file.clients.iter().map(|c| c.destinations.len()).sum();
    assert_eq!((locations, destinations), (8, 15));
}

// Proposed REQ-OPS-016: every demo identifier follows the fictitious-identifier rules.
#[test]
fn every_phone_and_fax_number_is_reserved_for_fiction() {
    let file = file();
    assert!(fictitious_phone(&file.laboratory_profile.phone));
    let mut checked = 0;
    for destination in file.clients.iter().flat_map(|c| &c.destinations) {
        let Some(address) = &destination.address else {
            continue;
        };
        match destination.channel {
            lis_reporting::DeliveryChannel::Fax => {
                assert!(fictitious_phone(address), "{address}");
                checked += 1;
            }
            lis_reporting::DeliveryChannel::Email => {
                assert!(example_email(address), "{address}");
                checked += 1;
            }
            lis_reporting::DeliveryChannel::Mail
            | lis_reporting::DeliveryChannel::Courier
            | lis_reporting::DeliveryChannel::Pickup => {
                assert!(address.contains("(Demo)"), "{address}");
            }
        }
    }
    assert_eq!(checked, 10, "6 faxes and 4 email addresses");
}

#[test]
fn every_npi_has_a_valid_check_digit_and_is_unique() {
    let file = file();
    let npis: BTreeSet<&str> = file.practitioners.iter().map(|p| p.npi.as_str()).collect();
    assert_eq!(npis.len(), file.practitioners.len());
    for npi in npis {
        assert!(npi_check_digit_valid(npi), "{npi}");
    }
    assert!(npi_check_digit_valid("1234567893"), "positive control");
    assert!(!npi_check_digit_valid("1234567890"), "negative control");
}

#[test]
fn every_code_and_record_number_carries_the_demo_prefix() {
    let file = file();
    let profile = &file.laboratory_profile;
    assert_eq!(profile.clia_id, "00D0000000");
    assert!(profile.name.starts_with("Demo "));
    for client in &file.clients {
        assert!(client.code.starts_with("DEMO-"), "{}", client.code);
        assert!(client.name.ends_with(" (Demo)"), "{}", client.name);
        for location in &client.locations {
            assert!(location.code.starts_with("DEMO-"), "{}", location.code);
            assert!(location.name.ends_with(" (Demo)"), "{}", location.name);
        }
    }
    for patient in &file.patients {
        assert_eq!(patient.source, "demo-data");
        for identifier in &patient.identifiers {
            assert!(identifier.assigning_authority.starts_with("DEMO-"));
            assert!(identifier.value.expose().starts_with("DEMO-"));
        }
    }
}

#[test]
fn the_identity_hold_case_acknowledges_the_earlier_patient() {
    let file = file();
    let acknowledging: Vec<(&str, &[String])> = file
        .patients
        .iter()
        .filter(|p| !p.acknowledge_candidates.is_empty())
        .map(|p| (p.key.as_str(), p.acknowledge_candidates.as_slice()))
        .collect();
    assert_eq!(acknowledging, [("P08", ["P07".to_owned()].as_slice())]);
}

/// Every `(method, analyte)` whose cutoff the Standard Catalog leaves blank.
fn blank_catalog_cutoffs() -> BTreeSet<(String, String)> {
    let catalog: Value = serde_json::from_str(CATALOG).unwrap();
    assert_eq!(catalog["catalogVersion"], "1.1.0");
    let mut blanks = BTreeSet::new();
    for method in catalog["methods"].as_array().unwrap() {
        for analyte in method["analytes"].as_array().unwrap() {
            if analyte["cutoff"].is_null() {
                blanks.insert((
                    method["code"].as_str().unwrap().to_owned(),
                    analyte["analyte"].as_str().unwrap().to_owned(),
                ));
            }
        }
    }
    blanks
}

// Decision 3: demo values fill only blanks, never replace a published value, never cover
// validity criteria (laboratory policy, Q-24), and each carries the demo label.
#[test]
fn demo_cutoffs_fill_only_blank_drug_cutoffs_of_the_standard_catalog() {
    let file = file();
    let blanks = blank_catalog_cutoffs();
    let mut keys = BTreeSet::new();
    for cutoff in &file.demo_cutoffs {
        let key = (cutoff.method.clone(), cutoff.analyte.clone());
        assert!(blanks.contains(&key), "{key:?} is not blank in the catalog");
        assert_ne!(cutoff.method, "UR-VALIDITY");
        assert!(keys.insert(key), "duplicate demo cutoff");
        assert!(["100", "10", "50", "5"].contains(&cutoff.value.as_str()));
    }
    let drug_blanks = blanks.iter().filter(|(m, _)| m != "UR-VALIDITY").count();
    assert_eq!(
        keys.len(),
        drug_blanks,
        "every blank drug cutoff has a demo value"
    );
    assert_eq!(DEMO_CUTOFF_LABEL, "Demo value, not a recommended cutoff");
}

#[test]
fn demo_values_exist_only_in_a_demo_installation() {
    assert!(demo_cutoff_values(InstallationKind::Laboratory).is_empty());
    let values = demo_cutoff_values(InstallationKind::Demo);
    assert_eq!(values.len(), file().demo_cutoffs.len());
    let key = ("OF-LCMSMS-BENZO".to_owned(), "OF-7ACLO".to_owned());
    assert_eq!(
        values.get(&key).map(lis_core::DecimalText::as_str),
        Some("5")
    );
}

// Proposed REQ-OPS-016: the demo laboratory's reports and labels are marked; a laboratory's
// are not.
#[test]
fn only_the_demo_installation_marks_its_reports_and_labels() {
    use super::{label_marking, report_marking};
    assert_eq!(
        report_marking(InstallationKind::Laboratory),
        lis_report::Marking::Unmarked
    );
    assert_eq!(
        report_marking(InstallationKind::Demo),
        lis_report::Marking::Demo
    );
    assert_eq!(
        label_marking(InstallationKind::Laboratory),
        lis_labels::Marking::Unmarked
    );
    assert_eq!(
        label_marking(InstallationKind::Demo),
        lis_labels::Marking::Demo
    );
}
