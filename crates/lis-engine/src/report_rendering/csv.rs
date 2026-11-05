//! Cells of the CSV files the engine exports (interface log, audit trail).

/// One quoted CSV cell. A leading `=`, `+`, `-` or `@` is prefixed with `'` so a
/// spreadsheet never runs it as a formula.
pub(crate) fn cell(value: &str) -> String {
    let guarded = if value.starts_with(['=', '+', '-', '@']) {
        format!("'{value}")
    } else {
        value.to_owned()
    };
    format!("\"{}\"", guarded.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_are_quoted_and_formulas_disarmed() {
        assert_eq!(cell("a\"b"), "\"a\"\"b\"");
        assert_eq!(cell("=SUM(A1)"), "\"'=SUM(A1)\"");
    }
}
