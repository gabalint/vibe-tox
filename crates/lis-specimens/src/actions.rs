//! Audit action names (`specimen.<verb>`), shared by the commands and the
//! engine's command registry.

/// `create_accession`.
pub const CREATE_ACCESSION: &str = "specimen.create_accession";
/// `record_receipt_decision`.
pub const RECORD_RECEIPT_DECISION: &str = "specimen.record_receipt_decision";
/// `create_aliquot`.
pub const CREATE_ALIQUOT: &str = "specimen.create_aliquot";
/// `mark_container_depleted`.
pub const MARK_DEPLETED: &str = "specimen.mark_depleted";
/// `move_container`.
pub const MOVE_CONTAINER: &str = "specimen.move_container";
/// `dispose_container`.
pub const DISPOSE_CONTAINER: &str = "specimen.dispose_container";
/// `place_hold`, and each hold an excursion assessment places.
pub const PLACE_HOLD: &str = "specimen.place_hold";
/// `release_hold`.
pub const RELEASE_HOLD: &str = "specimen.release_hold";
/// `record_excursion`.
pub const RECORD_EXCURSION: &str = "specimen.record_excursion";
/// `assess_excursion`.
pub const ASSESS_EXCURSION: &str = "specimen.assess_excursion";
/// `configure_location`.
pub const CONFIGURE_LOCATION: &str = "specimen.configure_location";
/// `configure_printer`.
pub const CONFIGURE_PRINTER: &str = "specimen.configure_printer";
/// `print_labels`: one event per label job.
pub const PRINT_LABEL: &str = "specimen.print_label";
/// The label worker's outcome of one job.
pub const RECORD_LABEL_OUTCOME: &str = "specimen.record_label_outcome";
/// `raise_number_floors` and recording a restore: one event per floor (REQ-OPS-002).
pub const RAISE_NUMBER_FLOOR: &str = "specimen.raise_number_floor";
/// `get_accession`, an audited read.
pub const VIEW_ACCESSION: &str = "specimen.view_accession";
/// `get_order_accession_context`, an audited order-to-material read.
pub const VIEW_ORDER_ACCESSIONS: &str = "specimen.view_order_accessions";
/// `find_container`, an audited read.
pub const FIND_CONTAINER: &str = "specimen.find_container";
