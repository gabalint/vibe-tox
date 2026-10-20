//! Shared persistence and evidence helpers for result revision commands.

use super::*;

mod au;

pub(crate) struct NewRevisionRecord<'a> {
    pub id: ResultRevisionId,
    pub result_id: ResultId,
    pub revision_no: i64,
    pub cause: ResultCause,
    pub value: &'a ResultValue,
    pub method: &'a lis_catalog::MethodVersionView,
    pub analyte: &'a MethodAnalyte,
    pub sequence_role: SequenceRole,
    pub specimen_id: SpecimenId,
    pub reason: Option<&'a str>,
    pub created_by: &'a str,
}

/// Inserts one immutable revision, classified under its frozen method
/// version, with the measurements it cites; a word-derived value records its
/// rule (result files brief F7).
pub(crate) fn insert_revision_record(
    ctx: &CommandContext<'_>,
    record: NewRevisionRecord<'_>,
    evidence: impl IntoIterator<Item = MeasurementId>,
) -> Result<(), ResultError> {
    let classification = record.value.classification(record.method, record.analyte);
    let classification_reason = match classification {
        Classification::NotClassifiable(reason) => Some(reason.code()),
        Classification::MeetsCutoff
        | Classification::DoesNotMeetCutoff
        | Classification::NotApplicable => None,
    };
    let (outcome, comparator, magnitude, code) = record.value.columns();
    let rule = record.value.rule.as_ref();
    ctx.tx().execute(
        "INSERT INTO res_result_revisions (id, result_id, revision_no, cause, outcome, comparator, \
         magnitude, units_text, code_text, classification, classification_reason, method_version_id, \
         method_certainty, output_scale, sequence_role, specimen_id, reason, created_by, created_at, \
         value_rule_registration_id, value_rule_word) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, \
                 ?17, ?18, ?19, ?20, ?21)",
        params![record.id, record.result_id, record.revision_no, record.cause, outcome,
            comparator, magnitude, record.value.units, code, classification.code(),
            classification_reason, record.method.id, record.method.certainty,
            record.method.output_scale, record.sequence_role, record.specimen_id,
            record.reason, record.created_by, ctx.now(),
            rule.map(|rule| rule.registration_id), rule.map(|rule| rule.word.as_str())],
    )?;
    for measurement_id in evidence {
        ctx.tx().execute(
            "INSERT INTO res_revision_evidence (revision_id, measurement_id) VALUES (?1, ?2)",
            params![record.id, measurement_id],
        )?;
    }
    Ok(())
}

/// A new result line for an ordered component and analyte, with no revision yet.
pub(crate) fn insert_result_line(
    conn: &rusqlite::Connection,
    component_id: OrderComponentId,
    analyte_id: AnalyteId,
    purpose: lis_catalog::ComponentPurpose,
) -> Result<ResultId, ResultError> {
    let result_id = ResultId::generate();
    conn.execute(
        "INSERT INTO res_results \
         (id, order_component_id, analyte_id, purpose, current_revision_id, revision) \
         VALUES (?1, ?2, ?3, ?4, NULL, 1)",
        params![result_id, component_id, analyte_id, purpose],
    )?;
    Ok(result_id)
}

/// Makes `revision_id` the line's current revision. The open-review queue
/// follows through its trigger (REQ-OPS-004).
pub(crate) fn point_to(
    conn: &rusqlite::Connection,
    result_id: ResultId,
    revision_id: ResultRevisionId,
) -> Result<(), ResultError> {
    conn.execute(
        "UPDATE res_results SET current_revision_id = ?2, revision = revision + 1 WHERE id = ?1",
        params![result_id, revision_id],
    )?;
    Ok(())
}

/// A current measurement with the result value it asserts under its frozen
/// method version.
pub(super) struct MeasurementRecord {
    pub position_id: lis_analytical::PositionId,
    pub analyte_id: AnalyteId,
    pub specimen_id: SpecimenId,
    pub method: lis_catalog::MethodVersionView,
    pub analyte: MethodAnalyte,
    pub value: ResultValue,
}

impl MeasurementRecord {
    pub(super) fn equivalent(&self, other: &Self) -> bool {
        self.analyte_id == other.analyte_id
            && self.method.id == other.method.id
            && self.value == other.value
    }
}

/// A current (not superseded) measurement and its result value.
pub(super) fn load_measurement(
    ctx: &CommandContext<'_>,
    id: MeasurementId,
) -> Result<MeasurementRecord, ResultError> {
    let (position_id, analyte_id, specimen_id, method_version_id, stored, source) = ctx
        .tx()
        .query_row(
            "SELECT m.position_id, m.analyte_id, c.specimen_id, p.method_version_id, m.form, \
             m.comparator, m.magnitude, m.units_text, m.code_text, m.value_text, \
             a.instrument_id, a.registration_id, p.execution_id, a.parser_version, \
             m.source_test_code, m.flags_text, m.status_text FROM res_measurements m \
             JOIN res_import_attempts a ON a.id = m.import_attempt_id \
             JOIN ana_v_positions p ON p.position_id = m.position_id \
             JOIN spc_v_containers c ON c.container_id = p.container_id \
             LEFT JOIN res_measurement_supersessions s ON s.measurement_id = m.id \
             WHERE m.id = ?1 AND s.measurement_id IS NULL",
            [id],
            |row| {
                Ok((
                    row.get::<_, lis_analytical::PositionId>(0)?,
                    row.get::<_, AnalyteId>(1)?,
                    row.get::<_, SpecimenId>(2)?,
                    row.get::<_, MethodVersionId>(3)?,
                    StoredValue {
                        form: row.get(4)?,
                        comparator: row.get(5)?,
                        magnitude: row.get(6)?,
                        units: row.get(7)?,
                        code: row.get(8)?,
                        value_text: row.get(9)?,
                    },
                    MeasuredBy {
                        instrument_id: row.get(10)?,
                        registration_id: row.get(11)?,
                        execution_id: row.get(12)?,
                        parser_version: row.get(13)?,
                        test_code: row.get(14)?,
                        flags_text: row.get(15)?,
                        status_text: row.get(16)?,
                    },
                ))
            },
        )
        .optional()?
        .ok_or(ResultError::NotFound("current measurement"))?;
    let method = lis_catalog::get_method_version(ctx.tx(), method_version_id)?;
    let analyte = method
        .analytes
        .iter()
        .find(|a| a.limits.analyte_id == analyte_id)
        .map(|a| a.limits.clone())
        .ok_or(ResultError::WrongState("method analyte"))?;
    let value = stored.result_value(ctx.tx(), source, &method, &analyte)?;
    Ok(MeasurementRecord {
        position_id,
        analyte_id,
        specimen_id,
        method,
        analyte,
        value,
    })
}

/// The source and execution of a measurement.
struct MeasuredBy {
    instrument_id: lis_core::InstrumentId,
    registration_id: Option<lis_analytical::SourceRegistrationId>,
    execution_id: lis_analytical::ExecutionId,
    parser_version: String,
    test_code: String,
    flags_text: Option<String>,
    status_text: Option<String>,
}

/// A measurement's stored value columns.
struct StoredValue {
    form: MeasurementForm,
    comparator: Option<Comparator>,
    magnitude: Option<String>,
    units: Option<String>,
    code: Option<String>,
    value_text: Option<String>,
}

impl StoredValue {
    /// The value a revision records from this measurement. A word is read
    /// again under the template of the registration that imported it, which
    /// is immutable once approved, and never applies on a dilution (F7).
    fn result_value(
        self,
        conn: &rusqlite::Connection,
        source: MeasuredBy,
        method: &lis_catalog::MethodVersionView,
        analyte: &MethodAnalyte,
    ) -> Result<ResultValue, ResultError> {
        if source.parser_version == crate::import::AU_PARSER_VERSION {
            return self.au_result_value(conn, source, method, analyte);
        }
        let decimal = |text: Option<String>| {
            lis_core::DecimalText::parse(text.as_deref().unwrap_or_default())
                .map_err(|_| ResultError::WrongState("measurement"))
        };
        let value = match self.form {
            MeasurementForm::Quantity => Some(SourceValue::Quantity(decimal(self.magnitude)?)),
            MeasurementForm::Censored => Some(SourceValue::Censored {
                comparator: self
                    .comparator
                    .ok_or(ResultError::WrongState("measurement"))?,
                bound: decimal(self.magnitude)?,
            }),
            MeasurementForm::Qualitative => {
                Some(SourceValue::Qualitative(self.code.unwrap_or_default()))
            }
            MeasurementForm::NotPerformed => None,
            MeasurementForm::Word => {
                let registration = source_registration(conn, &source)?;
                let layout = registration
                    .file_layout
                    .ok_or(ResultError::WrongState("measurement"))?;
                let word = self.value_text.unwrap_or_default();
                let FileValueReading::Word(meaning) =
                    read_value(&word, &layout.value_words, method.output_scale)
                else {
                    return Err(ResultError::WrongState("measurement"));
                };
                let rule = ValueRule {
                    registration_id: registration.registration_id,
                    word,
                };
                let factor = lis_analytical::execution_dilution_factor(conn, source.execution_id)?;
                return ResultValue::from_word(meaning, analyte, rule, factor.as_ref())
                    .map_err(WordRefusal::error);
            }
        };
        Ok(ResultValue {
            value,
            units: self.units,
            rule: None,
        })
    }
}

fn source_registration(
    conn: &rusqlite::Connection,
    source: &MeasuredBy,
) -> Result<lis_analytical::SourceRegistrationView, ResultError> {
    let id = source
        .registration_id
        .ok_or(ResultError::WrongState("measurement source"))?;
    let registration = lis_analytical::source_registration(conn, id)?;
    if registration.instrument_id != source.instrument_id {
        return Err(ResultError::WrongState("measurement source"));
    }
    Ok(registration)
}

pub(super) fn measurement_serves_component(
    conn: &rusqlite::Connection,
    position_id: lis_analytical::PositionId,
    component_id: lis_orders::OrderComponentId,
) -> Result<bool, ResultError> {
    Ok(crate::queries::measurement_component_ids(conn, position_id)?.contains(&component_id))
}

/// Appends a revision whose value comes from `measurement`, citing `evidence`.
pub(super) fn append_measurement_revision(
    ctx: &CommandContext<'_>,
    revision: MeasurementRevision<'_>,
    measurement: &MeasurementRecord,
    evidence: impl IntoIterator<Item = MeasurementId>,
) -> Result<ResultRevisionView, ResultError> {
    let id = ResultRevisionId::generate();
    let revision_no: i64 = ctx.tx().query_row(
        "SELECT coalesce(max(revision_no), 0) + 1 FROM res_result_revisions WHERE result_id = ?1",
        [revision.result_id],
        |row| row.get(0),
    )?;
    insert_revision_record(
        ctx,
        NewRevisionRecord {
            id,
            result_id: revision.result_id,
            revision_no,
            cause: revision.cause,
            value: &measurement.value,
            method: &measurement.method,
            analyte: &measurement.analyte,
            sequence_role: revision.sequence_role,
            specimen_id: measurement.specimen_id,
            reason: revision.reason,
            created_by: revision.actor,
        },
        evidence,
    )?;
    revision_view(ctx.tx(), id)
}

/// Who appends which kind of revision to which result line.
pub(super) struct MeasurementRevision<'a> {
    pub result_id: ResultId,
    pub cause: ResultCause,
    pub sequence_role: SequenceRole,
    pub reason: Option<&'a str>,
    pub actor: &'a str,
}

pub(super) fn advance_result(
    ctx: &CommandContext<'_>,
    result_id: ResultId,
    expected_current: ResultRevisionId,
    expected_revision: i64,
    next: ResultRevisionId,
) -> Result<(), ResultError> {
    let changed = ctx.tx().execute(
        "UPDATE res_results SET current_revision_id = ?4, revision = revision + 1 \
         WHERE id = ?1 AND current_revision_id = ?2 AND revision = ?3",
        params![result_id, expected_current, expected_revision, next],
    )?;
    if changed == 1 {
        Ok(())
    } else {
        Err(ResultError::RevisionConflict)
    }
}

pub(super) fn copy_revision_evidence(
    ctx: &CommandContext<'_>,
    from: ResultRevisionId,
    to: ResultRevisionId,
) -> Result<(), ResultError> {
    ctx.tx().execute(
        "INSERT INTO res_revision_evidence (revision_id, measurement_id) \
         SELECT ?2, measurement_id FROM res_revision_evidence WHERE revision_id = ?1",
        params![from, to],
    )?;
    Ok(())
}

#[derive(Debug)]
pub(super) struct RevisionRecord {
    pub revision_no: i64,
    pub method_version_id: MethodVersionId,
    pub method_certainty: lis_catalog::MethodCertainty,
    pub output_scale: lis_catalog::OutputScale,
}

pub(super) fn load_revision_record(
    conn: &rusqlite::Connection,
    id: ResultRevisionId,
) -> Result<RevisionRecord, ResultError> {
    conn.query_row(
        "SELECT revision_no, method_version_id, method_certainty, output_scale \
         FROM res_result_revisions WHERE id = ?1",
        [id],
        |row| {
            Ok(RevisionRecord {
                revision_no: row.get(0)?,
                method_version_id: row.get(1)?,
                method_certainty: row.get(2)?,
                output_scale: row.get(3)?,
            })
        },
    )
    .map_err(Into::into)
}

pub(super) fn revision_view(
    conn: &rusqlite::Connection,
    id: ResultRevisionId,
) -> Result<ResultRevisionView, ResultError> {
    let mut view = conn
        .query_row(
            "SELECT id, revision_no, cause, outcome, comparator, magnitude, units_text, code_text, \
             classification, classification_reason, method_version_id, method_certainty, output_scale, \
             sequence_role, reason, created_by, created_at FROM res_result_revisions WHERE id = ?1",
            [id],
            crate::views::result_revision_row,
        )
        .optional()?
        .ok_or(ResultError::NotFound("result revision"))?;
    view.evidence = crate::queries::measurements_for_revision(conn, id)?;
    Ok(view)
}
