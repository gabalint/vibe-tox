//! Value words never apply on a dilution execution (result files brief F7,
//! "Dilutions"; wave-10 review H1): the method version's limit is the
//! undiluted one, so a word on a diluted injection is quarantined as
//! `dilution_not_supported` (set aside, if not ordered), and every path that
//! would turn a stored word into a result refuses it. The LIS never computes a
//! diluted bound. Measurements and quarantine items stored before this rule
//! are built directly in SQL, as a database written by an earlier build holds
//! them.

use lis_analytical::AttemptKind;
use lis_core::{Comparator, ErrorCode};
use lis_results::{
    MeasurementId, QuarantineItemId, QuarantineReason, QuarantineResolution,
    ResolveQuarantineItemInput, ResultOutcome, SelectResultEvidenceInput, SetAsideKind,
    UseMeasuredResultInput,
};

use crate::kept::add_fentanyl;
use crate::rows::{current, quarantined, row};
use crate::*;

/// `sample` placed again as a dilution with `factor` on a new open run; the
/// fixture's run is closed first, so its labels match only the new run.
fn dilute(f: &Fixture, samples: Vec<Sample>, factor: &str) -> Vec<Sample> {
    f.close_run(f.run);
    let run = f.open_run(false);
    samples
        .into_iter()
        .map(|sample| f.place(sample, run, AttemptKind::Dilution, Some(factor)))
        .collect()
}

/// The ID of the only measurement whose value text is `value`.
fn measurement_with(f: &Fixture, value: &'static str) -> MeasurementId {
    f.read(move |conn| {
        Ok(conn.query_row(
            "SELECT id FROM res_measurements WHERE value_text = ?1",
            [value],
            |row| row.get(0),
        )?)
    })
}

/// A word measurement as an earlier build could have stored it: a copy of
/// `from` (same position, attempt and analyte unless `analyte` is given) whose
/// value is the approved word `N/F`.
fn legacy_word(f: &Fixture, from: MeasurementId, analyte: Option<AnalyteId>) -> MeasurementId {
    let id = MeasurementId::generate();
    f.setup(move |ctx| {
        ctx.tx().execute(
            "INSERT INTO res_measurements (id, import_attempt_id, processing_revision_id, \
             position_id, analyte_id, source_test_code, value_text, units_text, flags_text, \
             status_text, operator_text, completed_text, record_path, form, comparator, \
             magnitude, code_text, recorded_at) \
             SELECT ?1, import_attempt_id, processing_revision_id, position_id, \
             coalesce(?3, analyte_id), source_test_code, 'N/F', units_text, flags_text, \
             status_text, operator_text, completed_text, record_path, 'word', NULL, NULL, NULL, \
             recorded_at FROM res_measurements WHERE id = ?2",
            rusqlite::params![id, from, analyte],
        )?;
        Ok::<_, ResultError>(())
    });
    id
}

fn refused_as_diluted<T: std::fmt::Debug>(result: Result<T, ResultError>) {
    let error = result.unwrap_err();
    assert_eq!(error.code(), ErrorCode::GateBlocked, "{error:?}");
    assert_eq!(error.gate_reasons()[0].code, "dilution_not_supported");
}

// Import: an ordered row is quarantined, a kept row set aside, whatever the word
// means; an execution whose factor is numerically 1 is no dilution.
#[test]
fn a_value_word_on_a_dilution_is_quarantined_or_set_aside() {
    let f = fixture();
    let placed = dilute(&f, vec![f.sample(), f.sample()], "10");
    let tenfold = &placed[0];
    let attempt = f.import(&[
        row(&tenfold.label, "Unknown", "Morphine", "N/F", UNIT, "10"),
        row(&tenfold.label, "Unknown", "Oxycodone", "NP", UNIT, "10"),
        row(&tenfold.label, "Unknown", "Fentanyl", "N/F", UNIT, "10"),
        row(&placed[1].label, "Unknown", "Morphine", "220", UNIT, "10"),
    ]);
    assert_eq!(
        (
            attempt.rows_quarantined,
            attempt.rows_set_aside,
            attempt.rows_new
        ),
        (2, 1, 1)
    );
    let detail = f.detail(&attempt);
    assert_eq!(
        quarantined(&detail),
        [
            (QuarantineReason::DilutionNotSupported, "L2".to_owned()),
            (QuarantineReason::DilutionNotSupported, "L3".to_owned()),
        ]
    );
    let set_aside = &detail.set_aside[0];
    assert_eq!(
        (set_aside.kind, set_aside.reason),
        (
            SetAsideKind::NotOrderedUnusable,
            Some(QuarantineReason::DilutionNotSupported)
        )
    );
    assert!(f.measurements(tenfold, f.analytes.mor).is_empty());
    assert!(f.measurements(tenfold, f.analytes.fen).is_empty());

    let g = fixture();
    let undiluted = dilute(&g, vec![g.sample()], "1.0").remove(0);
    let attempt = g.import(&[row(
        &undiluted.label,
        "Unknown",
        "Morphine",
        "N/F",
        UNIT,
        "1",
    )]);
    assert_eq!(attempt.rows_new, 1);
    let word = current(&g.results(&undiluted), "SYN-MOR");
    assert_eq!(
        (word.outcome, word.comparator, word.magnitude.as_deref()),
        (ResultOutcome::Censored, Some(Comparator::Lt), Some("5"))
    );
}

// `select_result_evidence`: a stored word on a dilution never becomes the selected
// result; the dilution's own number still can.
#[test]
fn selecting_a_word_measured_on_a_dilution_is_refused() {
    let f = fixture();
    let s = f.sample();
    f.import(&[row(&s.label, "Unknown", "Morphine", "30", UNIT, "1")]);
    let diluted = dilute(&f, vec![s], "10").remove(0);
    f.import(&[row(
        &diluted.label,
        "Unknown",
        "Morphine",
        "300",
        UNIT,
        "10",
    )]);
    let number = measurement_with(&f, "300");
    let word = legacy_word(&f, number, None);
    let line = |f: &Fixture| {
        f.results(&diluted)
            .lines
            .into_iter()
            .find(|line| line.analyte_code == "SYN-MOR")
            .unwrap()
    };
    let select = |measurement_ids: Vec<MeasurementId>, line: &lis_results::ResultLineView| {
        SelectResultEvidenceInput {
            result_id: line.result_id,
            expected_revision: line.revision,
            measurement_ids,
            reason: "Synthetic dilution in range".to_owned(),
        }
    };
    let before = line(&f);
    let input = select(vec![word], &before);
    refused_as_diluted(f.run_as(principal(STAFF), move |ctx| {
        lis_results::select_result_evidence(ctx, input)
    }));
    assert_eq!(line(&f).revision, before.revision, "nothing changed");
    let input = select(vec![number], &before);
    let selected = f.setup(move |ctx| lis_results::select_result_evidence(ctx, input));
    assert_eq!(selected.magnitude.as_deref(), Some("300"));
}

// Accepting a conflicting re-export as reprocessing: a word on a dilution is refused
// as at import, and the item stays open.
#[test]
fn accepting_a_word_on_a_dilution_as_reprocessing_is_refused() {
    let f = fixture();
    let diluted = dilute(&f, vec![f.sample()], "10").remove(0);
    let fentanyl = |value: &str| row(&diluted.label, "Unknown", "Fentanyl", value, UNIT, "10");
    f.import(&[fentanyl("8.0")]);
    let conflict = f.detail(&f.import(&[fentanyl("8.4")])).quarantine[0].item_id;
    let legacy = QuarantineItemId::generate();
    f.setup(move |ctx| {
        ctx.tx().execute(
            "INSERT INTO res_quarantine_items (id, import_attempt_id, record_path, sample_label, \
             source_test_code, value_text, units_text, flags_text, status_text, operator_text, \
             completed_text, position_id, analyte_id, conflicting_measurement_id, reason, state, \
             revision) \
             SELECT ?1, import_attempt_id, record_path, sample_label, source_test_code, 'N/F', \
             units_text, flags_text, status_text, operator_text, completed_text, position_id, \
             analyte_id, conflicting_measurement_id, reason, state, revision \
             FROM res_quarantine_items WHERE id = ?2",
            rusqlite::params![legacy, conflict],
        )?;
        Ok::<_, ResultError>(())
    });
    let input = ResolveQuarantineItemInput {
        item_id: legacy,
        expected_revision: 1,
        resolution: QuarantineResolution::AcceptAsReprocessing,
        reason: "Synthetic reintegration".to_owned(),
    };
    refused_as_diluted(f.run_as(principal(STAFF), move |ctx| {
        lis_results::resolve_quarantine_item(ctx, input)
    }));
    let state: String = f.read(move |conn| {
        Ok(conn.query_row(
            "SELECT state FROM res_quarantine_items WHERE id = ?1",
            [legacy],
            |row| row.get(0),
        )?)
    });
    assert_eq!(state, "open");
}

// `use_measured_result`: an add-on cannot pull a word kept from a dilution; nothing
// is joined, attached or written.
#[test]
fn using_a_word_kept_from_a_dilution_is_refused() {
    let f = fixture();
    let s = f.sample();
    let diluted = dilute(&f, vec![s], "10").remove(0);
    f.import(&[row(
        &diluted.label,
        "Unknown",
        "Morphine",
        "300",
        UNIT,
        "10",
    )]);
    legacy_word(&f, measurement_with(&f, "300"), Some(f.analytes.fen));
    let component = add_fentanyl(&f, &diluted);
    let input = UseMeasuredResultInput {
        order_component_id: component,
        execution_id: diluted.execution_id,
        reason: "Synthetic add-on".to_owned(),
    };
    refused_as_diluted(f.run_as(principal(STAFF), move |ctx| {
        lis_results::use_measured_result(ctx, input)
    }));
    let accession = diluted.accession_id;
    let items = f.read(move |conn| {
        Ok(lis_specimens::get_accession(conn, accession)?
            .order_item_ids
            .len())
    });
    assert_eq!(items, 1, "the add-on did not join the accession");
    assert_eq!(f.results(&diluted).measured_not_ordered.len(), 1);
}
