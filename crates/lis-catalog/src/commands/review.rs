//! Catalog Preview records and All Data Reviewed (REQ-OPS-015, proposed;
//! REQ-ANA-009). A preview records exactly which drafts its printed report
//! lists. A review approves exactly those drafts through the ordinary
//! approval commands, so each gets its usual audit event and trigger check.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Capability, Principal, UserId};
use lis_store::CommandContext;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::commands::lifecycle::{acting_user, authorize_self_approval};
mod approve;
use crate::error::{CatalogError, invalid};
use crate::model::{
    AnalyteId, CatalogDraft, CatalogPreviewId, CatalogReviewId, MethodVersion, MethodVersionId,
    OrderableVersion, ValidityPolicyVersion,
};
use crate::repo::review::{self as repo, NewPreview, NewReview};
use crate::repo::{lifecycle, methods};
use approve::approve_drafts;

/// Input of [`record_catalog_preview`], built by the engine after it has
/// rendered and stored the report.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct RecordCatalogPreview {
    /// The report's ID, printed on it; the engine generates it before rendering.
    pub preview_id: CatalogPreviewId,
    /// The snapshot's `contentDigest`.
    pub content_digest: String,
    /// SHA-256 of the stored report artifact.
    pub artifact_digest: String,
    /// The snapshot's `drafts`: every open draft, at the revision printed.
    pub drafts: Vec<CatalogDraft>,
}

/// Records a generated Catalog Preview report and the drafts it lists.
/// Needs `manage_configuration` or `approve_configuration`. Refused when a
/// listed draft's revision or status differs from the database, or when an
/// open draft is not listed: the catalog changed while the report was
/// generated.
pub fn record_catalog_preview(
    ctx: &mut CommandContext<'_>,
    input: RecordCatalogPreview,
) -> Result<CatalogPreviewId, CatalogError> {
    // Either configuration capability; the one held becomes the audit authority.
    ctx.require(catalog_preview_capability(ctx.principal()))?;
    let generated_by = acting_user(ctx.principal())?;
    let content_digest = sha256("contentDigest", &input.content_digest)?;
    let artifact_digest = sha256("artifactDigest", &input.artifact_digest)?;
    for (index, draft) in input.drafts.iter().enumerate() {
        if input.drafts[..index].iter().any(|d| d.same_record(*draft)) {
            return Err(invalid(
                &format!("drafts[{index}]"),
                "duplicate",
                "This draft is listed twice.",
            ));
        }
    }
    ensure_unchanged(ctx.tx(), &input.drafts)?;
    let id = input.preview_id;
    let preview = NewPreview {
        id,
        generated_by,
        generated_at: ctx.now(),
        content_digest: &input.content_digest,
        artifact_digest: &input.artifact_digest,
    };
    repo::insert_preview(ctx.tx(), &preview, &input.drafts)?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "catalog.record_catalog_preview",
            Outcome::Succeeded,
        )
        .record("catalog_preview", id)
        .detail("content_digest", content_digest)
        .detail("artifact_digest", artifact_digest)
        .detail("drafts", AuditDetail::count(input.drafts.len())),
    )?;
    Ok(id)
}

/// The capability that lets `principal` generate, list and export Catalog
/// Previews (REQ-OPS-015): `manage_configuration` when held, otherwise
/// `approve_configuration`, which a denial then names. The one home of this
/// either-of rule; the engine applies it to the preview queries.
#[must_use]
pub fn catalog_preview_capability(principal: &Principal) -> Capability {
    if principal.require(Capability::ManageConfiguration).is_ok() {
        Capability::ManageConfiguration
    } else {
        Capability::ApproveConfiguration
    }
}

/// A lowercase hex SHA-256 digest, as an audit detail.
fn sha256(field: &str, value: &str) -> Result<AuditDetail, CatalogError> {
    AuditDetail::sha256(value).map_err(|_| {
        invalid(
            field,
            "invalid_digest",
            "Use a lowercase hex SHA-256 digest.",
        )
    })
}

/// Input of [`approve_reviewed_catalog`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct ApproveReviewedCatalog {
    /// The Catalog Preview the reviewer checked.
    pub preview_id: CatalogPreviewId,
    /// Every blank cutoff of the listed method drafts, each confirmed as not applicable.
    pub confirmed_blank_cutoffs: Vec<CatalogBlankCutoff>,
    /// Confirms the approver wrote listed drafts and approves them alone, as the
    /// separation-of-duties policy allows (ADR 0015); needed only then.
    pub self_approval: bool,
    /// Optional reference to laboratory approval records held outside the LIS (1–1000 characters).
    #[serde(default)]
    #[ts(optional)]
    pub external_approval_reference: Option<String>,
}

/// A method draft's analyte whose cutoff is blank.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct CatalogBlankCutoff {
    /// The method draft.
    pub method_version_id: MethodVersionId,
    /// The analyte.
    pub analyte_id: AnalyteId,
}

/// What All Data Reviewed approved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CatalogReviewApproved {
    /// The review record.
    pub review_id: CatalogReviewId,
    /// Validity policy drafts approved.
    pub validity_policies: usize,
    /// Method drafts approved.
    pub methods: usize,
    /// Test and panel drafts approved.
    pub orderables: usize,
    /// Blank cutoffs confirmed as not applicable.
    pub confirmed_blank_cutoffs: usize,
}

/// All Data Reviewed: approves exactly the drafts one Catalog Preview lists,
/// in one transaction (REQ-OPS-015, REQ-ANA-009). Needs
/// `approve_configuration`. **The engine must re-authenticate the user
/// before calling it.** Refused, naming the records, when the preview is
/// unknown or already reviewed, when a listed draft changed or is no longer
/// a draft, when an unlisted draft exists, when the reviewer authored a
/// listed draft and the laboratory's separation-of-duties policy needs a
/// second person or `self_approval` is not confirmed (ADR 0015), and when the
/// confirmed blank cutoffs differ from the blank cutoffs of the listed method
/// drafts. Approves validity policies, then methods, then tests and panels,
/// each at its listed revision. A method draft may replace a version that
/// current tests use when the review also approves their new versions; the
/// whole review is refused if any current test or panel is left on a replaced
/// method version.
pub fn approve_reviewed_catalog(
    ctx: &mut CommandContext<'_>,
    input: ApproveReviewedCatalog,
) -> Result<CatalogReviewApproved, CatalogError> {
    ctx.require(Capability::ApproveConfiguration)?;
    let reviewer = acting_user(ctx.principal())?;
    if !repo::preview_exists(ctx.tx(), input.preview_id)? {
        return Err(CatalogError::NotFound("catalog_preview"));
    }
    if repo::is_reviewed(ctx.tx(), input.preview_id)? {
        return Err(CatalogError::PreviewAlreadyReviewed);
    }
    let drafts = repo::preview_drafts(ctx.tx(), input.preview_id)?;
    ensure_unchanged(ctx.tx(), &drafts)?;
    let self_approval = separation_of_duties(ctx.tx(), &drafts, reviewer, input.self_approval)?;
    let confirmed = confirmed_blank_cutoffs(ctx.tx(), &drafts, &input.confirmed_blank_cutoffs)?;
    let reference =
        crate::approval_references::validate(input.external_approval_reference.as_deref())?;
    let approved = approve_drafts(ctx, &drafts, input.self_approval, reference.as_deref())?;
    let review_id = CatalogReviewId::generate();
    let review = NewReview {
        id: review_id,
        preview_id: input.preview_id,
        reviewed_by: reviewer,
        reviewed_at: ctx.now(),
    };
    repo::insert_review(ctx.tx(), &review, &confirmed)?;
    crate::approval_references::record(ctx, "catalog_review", review_id, reference.as_deref())?;
    let result = CatalogReviewApproved {
        review_id,
        validity_policies: approved[0],
        methods: approved[1],
        orderables: approved[2],
        confirmed_blank_cutoffs: confirmed.len(),
    };
    let mut event = AuditEvent::new(
        Stream::Clinical,
        "catalog.approve_reviewed_catalog",
        Outcome::Succeeded,
    )
    .record("catalog_review", review_id)
    .detail("preview_id", AuditDetail::id(input.preview_id))
    .detail(
        "validity_policies",
        AuditDetail::count(result.validity_policies),
    )
    .detail("methods", AuditDetail::count(result.methods))
    .detail("orderables", AuditDetail::count(result.orderables))
    .detail(
        "confirmed_blank_cutoffs",
        AuditDetail::count(result.confirmed_blank_cutoffs),
    );
    if self_approval {
        event = event.self_approval();
    }
    ctx.audit(event)?;
    Ok(result)
}

/// Every open draft, validity policies first, then methods, then orderables.
fn open_drafts(conn: &Connection) -> Result<Vec<CatalogDraft>, CatalogError> {
    let mut drafts = Vec::new();
    for (id, revision) in repo::drafts::<ValidityPolicyVersion>(conn)? {
        drafts.push(CatalogDraft::ValidityPolicyVersion { id, revision });
    }
    for (id, revision) in repo::drafts::<MethodVersion>(conn)? {
        drafts.push(CatalogDraft::MethodVersion { id, revision });
    }
    for (id, revision) in repo::drafts::<OrderableVersion>(conn)? {
        drafts.push(CatalogDraft::OrderableVersion { id, revision });
    }
    Ok(drafts)
}

/// Refuses when a listed draft is no longer an open draft at its listed
/// revision, or an open draft is not listed.
fn ensure_unchanged(conn: &Connection, listed: &[CatalogDraft]) -> Result<(), CatalogError> {
    let open = open_drafts(conn)?;
    let mut records = Vec::new();
    for draft in listed {
        if !open.contains(draft) {
            records.push(label(conn, *draft)?);
        }
    }
    for draft in &open {
        if !listed.iter().any(|l| l.same_record(*draft)) {
            records.push(label(conn, *draft)?);
        }
    }
    if records.is_empty() {
        Ok(())
    } else {
        Err(CatalogError::CatalogChanged { records })
    }
}

/// "method CODE v2", "test or panel CODE v1", "validity policy UR v1".
fn label(conn: &Connection, draft: CatalogDraft) -> Result<String, CatalogError> {
    let (noun, found) = match draft {
        CatalogDraft::MethodVersion { id, .. } => ("method", repo::code_and_version(conn, id)?),
        CatalogDraft::OrderableVersion { id, .. } => {
            ("test or panel", repo::code_and_version(conn, id)?)
        }
        CatalogDraft::ValidityPolicyVersion { id, .. } => {
            ("validity policy", repo::code_and_version(conn, id)?)
        }
    };
    Ok(match found {
        Some((code, version)) => format!("{noun} {code} v{version}"),
        None => format!("{noun} {}", draft.id_text()),
    })
}

/// Separation of duties for every listed draft (ADR 0015): returns whether the
/// reviewer authored any, which the policy then must allow and `confirmed` must
/// confirm. A refusal by the policy names the drafts the reviewer authored.
fn separation_of_duties(
    conn: &Connection,
    drafts: &[CatalogDraft],
    reviewer: UserId,
    confirmed: bool,
) -> Result<bool, CatalogError> {
    let mut records = Vec::new();
    for draft in drafts {
        let author = match *draft {
            CatalogDraft::MethodVersion { id, .. } => lifecycle::version_state(conn, id)?
                .map(|stored| stored.version.lifecycle.created_by),
            CatalogDraft::OrderableVersion { id, .. } => lifecycle::version_state(conn, id)?
                .map(|stored| stored.version.lifecycle.created_by),
            CatalogDraft::ValidityPolicyVersion { id, .. } => lifecycle::version_state(conn, id)?
                .map(|stored| stored.version.lifecycle.created_by),
        };
        if author == Some(reviewer) {
            records.push(label(conn, *draft)?);
        }
    }
    if records.is_empty() {
        return Ok(false);
    }
    authorize_self_approval(conn, confirmed, || CatalogError::ReviewerAuthoredDrafts {
        records,
    })?;
    Ok(true)
}

/// REQ-ANA-009: the confirmations must be exactly the blank cutoffs of the
/// listed method drafts. Returns them, deduplicated.
fn confirmed_blank_cutoffs(
    conn: &Connection,
    drafts: &[CatalogDraft],
    confirmed: &[CatalogBlankCutoff],
) -> Result<Vec<(MethodVersionId, AnalyteId)>, CatalogError> {
    let mut blank = Vec::new();
    let mut records = Vec::new();
    for draft in drafts {
        let CatalogDraft::MethodVersion { id, .. } = *draft else {
            continue;
        };
        for (analyte, code) in methods::blank_cutoffs(conn, id)? {
            let confirmed = confirmed
                .iter()
                .any(|c| c.method_version_id == id && c.analyte_id == analyte);
            if !confirmed {
                records.push(format!("{}: {code} not confirmed", label(conn, *draft)?));
            }
            blank.push((id, analyte));
        }
    }
    for c in confirmed {
        if !blank.contains(&(c.method_version_id, c.analyte_id)) {
            records.push(format!(
                "method version {} analyte {} has no blank cutoff in the report",
                c.method_version_id, c.analyte_id
            ));
        }
    }
    if records.is_empty() {
        Ok(blank)
    } else {
        Err(CatalogError::BlankCutoffsUnconfirmed { records })
    }
}
