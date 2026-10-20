//! Reuse shared facts only within one read-only SQLite snapshot.
//! Per-specimen entries are discarded between candidates; no clinical cache is persisted.
use super::review::{EvidencePosition, RevisionFacts};
use super::*;

pub(super) struct ReviewRead<'conn> {
    pub(super) conn: &'conn Connection,
    pub(super) analytical: lis_analytical::DependencyRead<'conn>,
    memo: bool,
    methods: BTreeMap<lis_catalog::MethodVersionId, lis_catalog::MethodVersionView>,
    facts: BTreeMap<ResultRevisionId, RevisionFacts>,
    evidence: BTreeMap<ResultRevisionId, Vec<EvidencePosition>>,
    testing: BTreeMap<OrderComponentId, Vec<GateReason>>,
    material: BTreeMap<SpecimenId, Vec<GateReason>>,
    validity_required: BTreeMap<(OrderComponentId, SpecimenId), bool>,
    validity: BTreeMap<SpecimenId, Option<ValidityAssessmentId>>,
    audits: BTreeMap<TechnicalReviewId, Option<i64>>,
    collection: BTreeMap<SpecimenId, Vec<lis_specimens::CollectionCorrectionView>>,
}

impl<'conn> ReviewRead<'conn> {
    pub(super) fn new(conn: &'conn Connection) -> Self {
        Self {
            conn,
            analytical: lis_analytical::DependencyRead::new(conn),
            memo: !conn.is_autocommit() && conn.is_readonly("main").unwrap_or(false),
            methods: BTreeMap::new(),
            facts: BTreeMap::new(),
            evidence: BTreeMap::new(),
            testing: BTreeMap::new(),
            material: BTreeMap::new(),
            validity_required: BTreeMap::new(),
            validity: BTreeMap::new(),
            audits: BTreeMap::new(),
            collection: BTreeMap::new(),
        }
    }

    pub(super) fn begin_specimen(&mut self) {
        self.facts.clear();
        self.evidence.clear();
        self.testing.clear();
        self.material.clear();
        self.validity_required.clear();
        self.validity.clear();
        self.audits.clear();
        self.collection.clear();
    }

    pub(super) fn method(
        &mut self,
        id: lis_catalog::MethodVersionId,
    ) -> Result<&lis_catalog::MethodVersionView, ResultError> {
        if !self.memo {
            self.methods.remove(&id);
        }
        Ok(match self.methods.entry(id) {
            std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(lis_catalog::get_method_version(self.conn, id)?)
            }
        })
    }

    pub(super) fn facts(&mut self, id: ResultRevisionId) -> Result<RevisionFacts, ResultError> {
        cached(self.memo, &mut self.facts, id, || {
            super::review::revision_facts(self.conn, id)
        })
    }

    pub(super) fn evidence(
        &mut self,
        id: ResultRevisionId,
    ) -> Result<Vec<EvidencePosition>, ResultError> {
        cached(self.memo, &mut self.evidence, id, || {
            super::review::evidence_positions(self.conn, id)
        })
    }

    pub(super) fn testing(&mut self, id: OrderComponentId) -> Result<Vec<GateReason>, ResultError> {
        cached(self.memo, &mut self.testing, id, || {
            Ok(lis_orders::testing_gate(self.conn, id)?
                .err()
                .unwrap_or_default())
        })
    }

    pub(super) fn material(&mut self, id: SpecimenId) -> Result<Vec<GateReason>, ResultError> {
        cached(self.memo, &mut self.material, id, || {
            Ok(lis_specimens::release_gate(self.conn, id)?
                .err()
                .unwrap_or_default())
        })
    }

    pub(super) fn validity_required(
        &mut self,
        component: OrderComponentId,
        specimen: SpecimenId,
    ) -> Result<bool, ResultError> {
        cached(
            self.memo,
            &mut self.validity_required,
            (component, specimen),
            || super::review::validity_required(self.conn, component, specimen),
        )
    }

    pub(super) fn validity(
        &mut self,
        id: SpecimenId,
    ) -> Result<Option<ValidityAssessmentId>, ResultError> {
        cached(self.memo, &mut self.validity, id, || {
            super::review::current_validity_id(self.conn, id)
        })
    }

    pub(super) fn audit(&mut self, id: TechnicalReviewId) -> Result<Option<i64>, ResultError> {
        cached(self.memo, &mut self.audits, id, || {
            super::review_chronology::unique_audit_sequence(self.conn, id)
        })
    }

    pub(super) fn collection(
        &mut self,
        id: SpecimenId,
    ) -> Result<Vec<lis_specimens::CollectionCorrectionView>, ResultError> {
        cached(self.memo, &mut self.collection, id, || {
            Ok(lis_specimens::collection_corrections_for_specimen(
                self.conn, id,
            )?)
        })
    }
}

fn cached<K: Ord + Clone, V: Clone, E>(
    enabled: bool,
    values: &mut BTreeMap<K, V>,
    key: K,
    load: impl FnOnce() -> Result<V, E>,
) -> Result<V, E> {
    if enabled && let Some(value) = values.get(&key) {
        return Ok(value.clone());
    }
    let value = load()?;
    if enabled {
        values.insert(key, value.clone());
    }
    Ok(value)
}
