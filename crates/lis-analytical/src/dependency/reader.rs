//! Shared analytical facts within one read-only SQLite snapshot.
use std::collections::BTreeMap;

use super::*;
use crate::model::DeviationScope;
use crate::views::QcAssessmentView;

/// Reuses common analytical facts during one read-only transaction.
///
/// Create this inside a read callback and discard it before that callback returns.
/// Never retain it across transactions or reuse it after a write. In autocommit mode
/// reads remain uncached. Reuse also requires a read-only database connection.
/// Command gates use fresh readers on every invocation.
pub struct DependencyRead<'conn> {
    conn: &'conn Connection,
    methods: BTreeMap<lis_catalog::MethodVersionId, bool>,
    instruments: BTreeMap<InstrumentId, Option<InstrumentStatus>>,
    current: BTreeMap<ScopeKey, Option<QcAssessmentView>>,
    qc_changes: BTreeMap<(RunId, AnalyteId), bool>,
    conflicts: BTreeMap<(RunId, AnalyteId), bool>,
    preparations: BTreeMap<PrepBatchId, Prep>,
    assessments: BTreeMap<ScopeKey, Vec<(&'static str, String)>>,
    run_targets: BTreeMap<RunId, Vec<(DeviationScope, String)>>,
    deviation_states: BTreeMap<(String, String, AnalyteId), (Option<i64>, bool)>,
    deviation_records: BTreeMap<(String, String, AnalyteId), Vec<String>>,
    last_position: Option<(PositionId, PositionRow)>,
    last_lot_expiry: Option<(PositionId, bool)>,
}

impl<'conn> DependencyRead<'conn> {
    /// Begins request-local reuse; the caller owns the read-only snapshot boundary.
    #[must_use]
    pub fn new(conn: &'conn Connection) -> Self {
        Self {
            conn,
            methods: BTreeMap::new(),
            instruments: BTreeMap::new(),
            current: BTreeMap::new(),
            qc_changes: BTreeMap::new(),
            conflicts: BTreeMap::new(),
            preparations: BTreeMap::new(),
            assessments: BTreeMap::new(),
            run_targets: BTreeMap::new(),
            deviation_states: BTreeMap::new(),
            deviation_records: BTreeMap::new(),
            last_position: None,
            last_lot_expiry: None,
        }
    }

    /// Uses the existing analytical gate, with shared run/method facts read once.
    pub fn status(
        &mut self,
        position_id: PositionId,
        analyte_id: AnalyteId,
    ) -> Result<Result<DependencySnapshot, Vec<GateReason>>, AnalyticalError> {
        let position = self.position(position_id)?;
        let qc = self.current(ScopeKey::RunAnalyte(position.run_id, analyte_id))?;
        let run = self.current(ScopeKey::Run(position.run_id))?;
        let (deviation_watermark, deviation_open) = self.deviations(&position, analyte_id)?;
        let facts = Facts {
            instrument: self.instrument(position.instrument_id)?,
            method_retired: self.method_retired(position.method_version_id)?,
            lot_expired: self.lot_expired(&position)?,
            execution_failed: position.execution_state == Some(ExecutionState::Failed),
            deviation_open,
            deviation_watermark,
            prep: position
                .prep_batch_id
                .map(|id| self.preparation(id))
                .transpose()?,
            qc: qc
                .as_ref()
                .map(|value| (value.assessment_id, value.outcome)),
            qc_stale: self.qc_changed(position.run_id, analyte_id, qc.as_ref())?,
            qc_conflict: self.conflict(position.run_id, analyte_id)?,
            run_rejected: run.is_some_and(|value| value.outcome == QcOutcome::Rejected),
        };
        Ok(decide(&facts))
    }

    /// Every original causal assessment/deviation record, including cleared conditions.
    pub fn causal_records(
        &mut self,
        position_id: PositionId,
        analyte_id: AnalyteId,
    ) -> Result<Vec<(&'static str, String)>, AnalyticalError> {
        let position = self.position(position_id)?;
        let mut records = self.assessments(ScopeKey::RunAnalyte(position.run_id, analyte_id))?;
        records.extend(self.assessments(ScopeKey::Run(position.run_id))?);
        if let Some(batch) = position.prep_batch_id {
            records.extend(self.assessments(ScopeKey::PrepBatch(batch))?);
        }
        for (scope, id) in self.targets(&position)? {
            records.extend(
                self.deviation_records(scope, &id, analyte_id)?
                    .into_iter()
                    .map(|id| ("deviation_open", id)),
            );
        }
        records.sort_unstable();
        records.dedup();
        Ok(records)
    }

    fn position(&mut self, id: PositionId) -> Result<PositionRow, AnalyticalError> {
        memo_last(self.conn, &mut self.last_position, id, || {
            runs::position(self.conn, id)?.ok_or(AnalyticalError::NotFound("position"))
        })
    }

    fn lot_expired(&mut self, position: &PositionRow) -> Result<bool, AnalyticalError> {
        memo_last(self.conn, &mut self.last_lot_expiry, position.id, || {
            lot_expired(self.conn, position)
        })
    }

    fn method_retired(
        &mut self,
        id: lis_catalog::MethodVersionId,
    ) -> Result<bool, AnalyticalError> {
        memo(self.conn, &mut self.methods, id, || {
            Ok(lis_catalog::get_method_version(self.conn, id)?
                .lifecycle
                .status
                != VersionStatus::Approved)
        })
    }

    fn instrument(
        &mut self,
        id: InstrumentId,
    ) -> Result<Option<InstrumentStatus>, AnalyticalError> {
        memo(self.conn, &mut self.instruments, id, || {
            Ok(readiness::latest_status(self.conn, id)?.map(|(status, _)| status))
        })
    }

    fn current(&mut self, key: ScopeKey) -> Result<Option<QcAssessmentView>, AnalyticalError> {
        memo(self.conn, &mut self.current, key, || {
            Ok(qc::current_assessment(self.conn, key)?)
        })
    }

    fn qc_changed(
        &mut self,
        run: RunId,
        analyte: AnalyteId,
        current: Option<&QcAssessmentView>,
    ) -> Result<bool, AnalyticalError> {
        memo(
            self.conn,
            &mut self.qc_changes,
            (run, analyte),
            || match current {
                Some(current) => evidence_changed(self.conn, current.assessment_id, run, analyte),
                None => Ok(false),
            },
        )
    }

    fn conflict(&mut self, run: RunId, analyte: AnalyteId) -> Result<bool, AnalyticalError> {
        memo(self.conn, &mut self.conflicts, (run, analyte), || {
            Ok(conflicts::open_on_run_analyte(self.conn, run, analyte)?)
        })
    }

    fn preparation(&mut self, id: PrepBatchId) -> Result<Prep, AnalyticalError> {
        memo(self.conn, &mut self.preparations, id, || {
            prep(self.conn, id)
        })
    }

    fn assessments(
        &mut self,
        key: ScopeKey,
    ) -> Result<Vec<(&'static str, String)>, AnalyticalError> {
        memo(self.conn, &mut self.assessments, key, || {
            Ok(qc::assessments(self.conn, key)?
                .into_iter()
                .map(|assessment| {
                    let code = match (assessment.scope, assessment.outcome) {
                        (QcScope::PrepBatch, QcOutcome::Rejected) => "prep_rejected",
                        (_, QcOutcome::Rejected) => "qc_rejected",
                        (_, QcOutcome::Accepted) => "qc_superseded",
                    };
                    (code, assessment.assessment_id.to_string())
                })
                .collect())
        })
    }

    fn targets(
        &mut self,
        position: &PositionRow,
    ) -> Result<Vec<(DeviationScope, String)>, AnalyticalError> {
        let mut targets = memo(self.conn, &mut self.run_targets, position.run_id, || {
            crate::deviation_targets::run_level_targets(
                self.conn,
                position.instrument_id,
                position.run_id,
            )
        })?;
        targets.extend(crate::deviation_targets::position_level_targets(
            self.conn, position,
        )?);
        Ok(targets)
    }

    fn deviations(
        &mut self,
        position: &PositionRow,
        analyte: AnalyteId,
    ) -> Result<(i64, bool), AnalyticalError> {
        let mut watermark = 0;
        let mut open = false;
        for (scope, id) in self.targets(position)? {
            let query = || Ok(deviations::applicable(self.conn, scope, &id, analyte)?);
            let (seq, any_open) = if scope == DeviationScope::Position {
                query()?
            } else {
                memo(
                    self.conn,
                    &mut self.deviation_states,
                    (scope.as_str().to_owned(), id.clone(), analyte),
                    query,
                )?
            };
            watermark = watermark.max(seq.unwrap_or(0));
            open |= any_open;
        }
        Ok((watermark, open))
    }

    fn deviation_records(
        &mut self,
        scope: DeviationScope,
        id: &str,
        analyte: AnalyteId,
    ) -> Result<Vec<String>, AnalyticalError> {
        let query = || {
            Ok(self
                .conn
                .prepare_cached(
                    "SELECT id FROM ana_deviations WHERE scope = ?1 AND scope_id = ?2 \
                 AND (analyte_id IS NULL OR analyte_id = ?3) ORDER BY seq",
                )?
                .query_map(rusqlite::params![scope, id, analyte], |row| {
                    row.get::<_, crate::model::DeviationId>(0)
                        .map(|id| id.to_string())
                })?
                .collect::<Result<Vec<String>, _>>()?)
        };
        if scope == DeviationScope::Position {
            return query();
        }
        memo(
            self.conn,
            &mut self.deviation_records,
            (scope.as_str().to_owned(), id.to_owned(), analyte),
            query,
        )
    }
}

// Concrete call sites above share this request-local map operation. Autocommit
// calls remain fresh; no error is memoized and no map survives its reader.
fn memo<K: Ord + Clone, V: Clone>(
    conn: &Connection,
    values: &mut BTreeMap<K, V>,
    key: K,
    load: impl FnOnce() -> Result<V, AnalyticalError>,
) -> Result<V, AnalyticalError> {
    if !can_memo(conn) {
        return load();
    }
    if let Some(value) = values.get(&key) {
        return Ok(value.clone());
    }
    let value = load()?;
    values.insert(key, value.clone());
    Ok(value)
}

fn can_memo(conn: &Connection) -> bool {
    !conn.is_autocommit() && conn.is_readonly("main").unwrap_or(false)
}

// Multi-analyte results commonly revisit the same position. Retain only the last
// successful lookup, and evaluate lot-at-use only when status actually needs it.
fn memo_last<V: Clone>(
    conn: &Connection,
    last: &mut Option<(PositionId, V)>,
    id: PositionId,
    load: impl FnOnce() -> Result<V, AnalyticalError>,
) -> Result<V, AnalyticalError> {
    if !can_memo(conn) {
        return load();
    }
    if let Some((previous, value)) = last
        && *previous == id
    {
        return Ok(value.clone());
    }
    let value = load()?;
    *last = Some((id, value.clone()));
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writable_transaction_never_memoizes_facts() {
        let mut conn = Connection::open_in_memory().unwrap();
        let tx = conn.transaction().unwrap();
        let mut values = BTreeMap::new();
        assert_eq!(memo(&tx, &mut values, 1, || Ok(10)).unwrap(), 10);
        assert_eq!(memo(&tx, &mut values, 1, || Ok(20)).unwrap(), 20);
        assert!(values.is_empty());
        let mut last = None;
        let position = PositionId::generate();
        assert_eq!(memo_last(&tx, &mut last, position, || Ok(10)).unwrap(), 10);
        assert_eq!(memo_last(&tx, &mut last, position, || Ok(20)).unwrap(), 20);
        assert!(last.is_none());
    }
}
