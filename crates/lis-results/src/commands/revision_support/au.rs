//! Reconstructs AU evidence under the immutable registration that imported it.

use super::{
    MeasuredBy, MeasurementForm, MethodAnalyte, ResultError, ResultValue, StoredValue,
    source_registration,
};

impl StoredValue {
    pub(super) fn au_result_value(
        self,
        conn: &rusqlite::Connection,
        source: MeasuredBy,
        method: &lis_catalog::MethodVersionView,
        analyte: &MethodAnalyte,
    ) -> Result<ResultValue, ResultError> {
        if self.form != MeasurementForm::Quantity {
            return Err(ResultError::WrongState("AU measurement"));
        }
        let registration = source_registration(conn, &source)?;
        let profile = registration
            .au_profile
            .ok_or(ResultError::WrongState("AU source profile"))?;
        let factor = lis_analytical::execution_dilution_factor(conn, source.execution_id)?;
        if crate::result_value::diluted(factor.as_ref()) {
            return Err(ResultError::WrongState("AU dilution measurement"));
        }
        let row = crate::astm::ExtractedRow {
            record_path: String::new(),
            sample_label: None,
            test_code: Some(source.test_code),
            value_text: self.value_text,
            units_text: self.units,
            flags_text: source.flags_text,
            status_text: source.status_text,
            operator_text: None,
            completed_text: None,
            sample_type_text: None,
            dilution_text: None,
        };
        crate::au::patient_value(&row, method.output_scale, analyte, &profile)
            .map_err(|_| ResultError::WrongState("AU measurement"))
    }
}
