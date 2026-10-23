//! Result import, quarantine and technical-review engine boundary
//! (REQ-ANA-003, REQ-RES-001–005, REQ-ORD-003, REQ-INT-003).

mod names;
mod receipt;
pub(crate) use receipt::GET_RECEIPT_IMPORT_ATTEMPT;
pub use receipt::GetReceiptImportAttemptInput;

use std::collections::BTreeSet;

use lis_audit::AuditDetail;
use lis_core::{Capability, CommandError};
use lis_orders::TriggerAssessmentView;
use lis_results::{
    ImportAttemptDetail, ImportAttemptId, ImportAttemptSummary, RecordReflexDecisionInput,
    RecordResultDispositionInput, RecordValidityAssessmentInput, ReflexTriggerView,
    ResolveQuarantineItemInput, ResultError, ResultRevisionView, RetryImportInput,
    ReviewQueueFilter, ReviewView, SelectResultEvidenceInput, SpecimenResults,
    SubmitTechnicalReviewInput, UseMeasuredResultInput, ValidityAssessmentView,
};
use serde::Deserialize;
use ts_rs::TS;

use crate::engine::Engine;
use crate::pipeline::{access, domain_error};
use crate::registry::{Access, CommandSpec, spec};

const REVIEW: Access = Access::Capability(Capability::ReviewResults);
pub(crate) const LIST_REVIEW_QUEUE: CommandSpec =
    spec("list_review_queue", "result.list_review_queue", REVIEW);
pub(crate) const GET_SPECIMEN_RESULTS: CommandSpec = spec(
    "get_specimen_results",
    "result.get_specimen_results",
    REVIEW,
);
pub(crate) const SELECT_RESULT_EVIDENCE: CommandSpec =
    spec("select_result_evidence", "result.select_evidence", REVIEW);
pub(crate) const RECORD_RESULT_DISPOSITION: CommandSpec = spec(
    "record_result_disposition",
    "result.record_disposition",
    REVIEW,
);
pub(crate) const SUBMIT_TECHNICAL_REVIEW: CommandSpec =
    spec("submit_technical_review", "result.submit_review", REVIEW);
pub(crate) const LIST_VALIDITY_POLICIES: CommandSpec = spec(
    "list_validity_policies",
    "result.list_validity_policies",
    REVIEW,
);
pub(crate) const RECORD_VALIDITY_ASSESSMENT: CommandSpec = spec(
    "record_validity_assessment",
    "result.assess_validity",
    REVIEW,
);
pub(crate) const LIST_IMPORT_ATTEMPTS: CommandSpec = spec(
    "list_import_attempts",
    "result.list_import_attempts",
    REVIEW,
);
pub(crate) const GET_IMPORT_ATTEMPT: CommandSpec =
    spec("get_import_attempt", "result.get_import_attempt", REVIEW);
pub(crate) const RESOLVE_QUARANTINE_ITEM: CommandSpec = spec(
    "resolve_quarantine_item",
    "result.resolve_quarantine",
    REVIEW,
);
pub(crate) const RETRY_IMPORT: CommandSpec = spec("retry_import", "result.retry_import", REVIEW);
/// Pulls a measurement kept outside the ordered scope into an accepted
/// add-on (result files brief F17).
pub(crate) const USE_MEASURED_RESULT: CommandSpec =
    spec("use_measured_result", "result.use_measured_result", REVIEW);
pub(crate) const LIST_REFLEX_TRIGGERS: CommandSpec = spec(
    "list_reflex_triggers",
    "result.list_reflex_triggers",
    REVIEW,
);
/// The verified laboratory-result path into orders: the reviewer's
/// `review_results` grant authorizes it, not `enter_orders` (REQ-ORD-003).
pub(crate) const RECORD_REFLEX_DECISION: CommandSpec = spec(
    "record_reflex_decision",
    "result.record_reflex_decision",
    REVIEW,
);

domain_error!(
    ResultError,
    |error: &ResultError| error.field_errors().to_vec(),
    |error: &ResultError| error.gate_reasons().to_vec()
);

#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GetSpecimenResultsInput {
    pub specimen_id: lis_specimens::SpecimenId,
}

#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListReviewQueueInput {
    pub filter: ReviewQueueFilter,
    pub limit: u32,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub after: Option<lis_results::ReviewQueueCursor>,
}

#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListImportAttemptsInput {
    pub instrument_id: Option<lis_core::InstrumentId>,
    pub limit: u32,
    pub before: Option<lis_core::UtcTimestamp>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub after_import_attempt_id: Option<ImportAttemptId>,
}

#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GetImportAttemptInput {
    pub import_attempt_id: ImportAttemptId,
}

#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListReflexTriggersInput {
    pub specimen_id: lis_specimens::SpecimenId,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListValidityPoliciesInput {
    pub specimen_type_code: String,
}

impl Engine {
    pub async fn list_review_queue(
        &self,
        input: ListReviewQueueInput,
    ) -> Result<lis_results::ReviewQueuePage, CommandError> {
        self.run(&LIST_REVIEW_QUEUE, |call| async move {
            self.audited_read(
                &call,
                move |conn| {
                    lis_results::scan_review_queue(conn, input.filter, input.limit, input.after)
                },
                move |entries| {
                    access(LIST_REVIEW_QUEUE.action)
                        .detail("filter", AuditDetail::code(input.filter.as_str()))
                        .detail("returned", AuditDetail::count(entries.items.len()))
                        .detail("scanned", AuditDetail::count(entries.scanned))
                },
            )
            .await
        })
        .await
    }

    pub async fn get_specimen_results(
        &self,
        input: GetSpecimenResultsInput,
    ) -> Result<SpecimenResults, CommandError> {
        self.run(&GET_SPECIMEN_RESULTS, |call| async move {
            self.audited_read(
                &call,
                move |conn| names::specimen_results(conn, input.specimen_id),
                move |_| access(GET_SPECIMEN_RESULTS.action).record("specimen", input.specimen_id),
            )
            .await
        })
        .await
    }

    pub async fn select_result_evidence(
        &self,
        input: SelectResultEvidenceInput,
    ) -> Result<ResultRevisionView, CommandError> {
        self.run(&SELECT_RESULT_EVIDENCE, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_results::select_result_evidence(ctx, input)?;
                ctx.audit(
                    access(SELECT_RESULT_EVIDENCE.action)
                        .record("result_revision", detail.revision_id),
                )?;
                Ok::<_, ResultError>(detail)
            })
            .await
        })
        .await
    }

    pub async fn record_result_disposition(
        &self,
        input: RecordResultDispositionInput,
    ) -> Result<ResultRevisionView, CommandError> {
        self.run(&RECORD_RESULT_DISPOSITION, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_results::record_result_disposition(ctx, input)?;
                ctx.audit(
                    access(RECORD_RESULT_DISPOSITION.action)
                        .record("result_revision", detail.revision_id),
                )?;
                Ok::<_, ResultError>(detail)
            })
            .await
        })
        .await
    }

    pub async fn submit_technical_review(
        &self,
        input: SubmitTechnicalReviewInput,
    ) -> Result<ReviewView, CommandError> {
        self.run(&SUBMIT_TECHNICAL_REVIEW, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_results::submit_technical_review(ctx, input)?;
                ctx.audit(
                    access(SUBMIT_TECHNICAL_REVIEW.action)
                        .record("technical_review", detail.review_id),
                )?;
                Ok::<_, ResultError>(detail)
            })
            .await
        })
        .await
    }

    pub async fn list_validity_policies(
        &self,
        input: ListValidityPoliciesInput,
    ) -> Result<Vec<lis_catalog::VersionSummary<lis_catalog::ValidityPolicyVersion>>, CommandError>
    {
        self.run(&LIST_VALIDITY_POLICIES, |call| async move {
            self.read(&call, move |conn| {
                let specimen = lis_catalog::list_specimen_types(conn)?
                    .into_iter()
                    .find(|item| item.code == input.specimen_type_code)
                    .ok_or(lis_catalog::CatalogError::NotFound("specimen_type"))?;
                lis_catalog::list_validity_policy_versions(conn, specimen.id)
            })
            .await
        })
        .await
    }

    pub async fn record_validity_assessment(
        &self,
        input: RecordValidityAssessmentInput,
    ) -> Result<ValidityAssessmentView, CommandError> {
        self.run(&RECORD_VALIDITY_ASSESSMENT, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_results::record_validity_assessment(ctx, input)?;
                ctx.audit(
                    access(RECORD_VALIDITY_ASSESSMENT.action)
                        .record("validity_assessment", detail.assessment_id),
                )?;
                Ok::<_, ResultError>(detail)
            })
            .await
        })
        .await
    }

    pub async fn list_import_attempts(
        &self,
        input: ListImportAttemptsInput,
    ) -> Result<Vec<ImportAttemptSummary>, CommandError> {
        self.run(&LIST_IMPORT_ATTEMPTS, |call| async move {
            self.read(&call, move |conn| {
                lis_results::list_import_attempts_page(
                    conn,
                    input.instrument_id,
                    input.limit,
                    input.before,
                    input.after_import_attempt_id,
                )
            })
            .await
        })
        .await
    }

    pub async fn get_import_attempt(
        &self,
        input: GetImportAttemptInput,
    ) -> Result<ImportAttemptDetail, CommandError> {
        self.run(&GET_IMPORT_ATTEMPT, |call| async move {
            self.audited_read(
                &call,
                move |conn| {
                    let detail = lis_results::get_import_attempt(conn, input.import_attempt_id)?;
                    with_loader_name(conn, detail)
                },
                move |_| {
                    access(GET_IMPORT_ATTEMPT.action)
                        .record("import_attempt", input.import_attempt_id)
                },
            )
            .await
        })
        .await
    }

    pub async fn resolve_quarantine_item(
        &self,
        input: ResolveQuarantineItemInput,
    ) -> Result<ImportAttemptDetail, CommandError> {
        self.run(&RESOLVE_QUARANTINE_ITEM, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_results::resolve_quarantine_item(ctx, input)?;
                ctx.audit(
                    access(RESOLVE_QUARANTINE_ITEM.action)
                        .record("import_attempt", detail.summary.import_attempt_id),
                )?;
                with_loader_name(ctx.tx(), detail)
            })
            .await
        })
        .await
    }

    pub async fn use_measured_result(
        &self,
        input: UseMeasuredResultInput,
    ) -> Result<SpecimenResults, CommandError> {
        self.run(&USE_MEASURED_RESULT, |call| async move {
            self.write(&call, move |ctx| {
                let results = names::use_measured_result(ctx, input)?;
                ctx.audit(
                    access(USE_MEASURED_RESULT.action).record("specimen", results.specimen_id),
                )?;
                Ok::<_, ResultError>(results)
            })
            .await
        })
        .await
    }

    pub async fn retry_import(&self, input: RetryImportInput) -> Result<i64, CommandError> {
        self.run(&RETRY_IMPORT, |call| async move {
            self.write(&call, move |ctx| lis_results::retry_import(ctx, input))
                .await
        })
        .await
    }

    pub async fn list_reflex_triggers(
        &self,
        input: ListReflexTriggersInput,
    ) -> Result<Vec<ReflexTriggerView>, CommandError> {
        self.run(&LIST_REFLEX_TRIGGERS, |call| async move {
            self.read(&call, move |conn| {
                lis_results::pending_reflex_triggers(conn, input.specimen_id)
            })
            .await
        })
        .await
    }

    pub async fn record_reflex_decision(
        &self,
        input: RecordReflexDecisionInput,
    ) -> Result<TriggerAssessmentView, CommandError> {
        self.run(&RECORD_REFLEX_DECISION, |call| async move {
            self.write(&call, move |ctx| {
                lis_results::record_reflex_decision(ctx, input)
            })
            .await
        })
        .await
    }
}

/// The import attempt with the display name of the user who loaded its file by hand,
/// read from the accounts module (result files brief §7).
fn with_loader_name(
    conn: &rusqlite::Connection,
    mut detail: ImportAttemptDetail,
) -> Result<ImportAttemptDetail, ResultError> {
    let Some(file) = detail.file.as_mut() else {
        return Ok(detail);
    };
    let Some(user) = file.loaded_by else {
        return Ok(detail);
    };
    let names =
        lis_auth::display_names(conn, &BTreeSet::from([user])).map_err(|error| match error {
            lis_auth::AuthError::Store(error) => ResultError::Store(error),
            _ => ResultError::NotFound("loader"),
        })?;
    file.loaded_by_display = names.get(&user).cloned();
    Ok(detail)
}
