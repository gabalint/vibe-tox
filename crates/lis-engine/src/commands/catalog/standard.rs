//! Standard Catalog and catalog review IPC (REQ-OPS-014, REQ-OPS-015, both
//! proposed): the load tree and load, the banner status, the recorded
//! Catalog Previews and All Data Reviewed. Generating, showing and
//! exporting a preview live in [`super::preview`].
//!
//! The preview commands need `manage_configuration` **or**
//! `approve_configuration`. The registry has no either-of access, so they are
//! registered as `Session` and checked here and in the domain with
//! [`lis_catalog::catalog_preview_capability`], the one home of that rule.

use std::collections::BTreeSet;

use lis_auth::Reauthentication;
use lis_auth::{Attempt, AuthError, ReauthPurpose};
use lis_catalog::{
    ApproveReviewedCatalog, CatalogBlankCutoff, CatalogError, CatalogPreviewId,
    CatalogPreviewSummary, CatalogReviewApproved, LoadStandardCatalog, StandardCatalogLoaded,
    StandardCatalogStatus, StandardCatalogTree, StandardFile,
};
use lis_core::{CommandError, Denied};
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use super::{APPROVE, MANAGE, READ, versions};
use crate::engine::Engine;
use crate::pipeline::{Call, Failure, ModuleError, accepted};
use crate::registry::{CommandSpec, spec};

pub(crate) const STANDARD_CATALOG_TREE: CommandSpec = spec(
    "standard_catalog_tree",
    "catalog.standard_catalog_tree",
    MANAGE,
);
pub(crate) const LOAD_STANDARD_CATALOG: CommandSpec = spec(
    "load_standard_catalog",
    "catalog.load_standard_catalog",
    MANAGE,
);
pub(crate) const STANDARD_CATALOG_STATUS: CommandSpec = spec(
    "standard_catalog_status",
    "catalog.standard_catalog_status",
    READ,
);
/// Either-of access, checked by [`Engine::require_catalog_preview_access`].
pub(crate) const GENERATE_CATALOG_PREVIEW: CommandSpec = spec(
    "generate_catalog_preview",
    "catalog.generate_catalog_preview",
    READ,
);
/// Either-of access, checked by [`Engine::require_catalog_preview_access`].
pub(crate) const LIST_CATALOG_PREVIEWS: CommandSpec = spec(
    "list_catalog_previews",
    "catalog.list_catalog_previews",
    READ,
);
/// Either-of access, checked by [`Engine::require_catalog_preview_access`].
pub(crate) const EXPORT_CATALOG_PREVIEW_PDF: CommandSpec = spec(
    "export_catalog_preview_pdf",
    "catalog.export_catalog_preview_pdf",
    READ,
);
/// Either-of access, checked by [`Engine::require_catalog_preview_access`].
pub(crate) const VIEW_CATALOG_PREVIEW_PDF: CommandSpec = spec(
    "view_catalog_preview_pdf",
    "catalog.view_catalog_preview_pdf",
    READ,
);
pub(crate) const APPROVE_REVIEWED_CATALOG: CommandSpec = spec(
    "approve_reviewed_catalog",
    "catalog.approve_reviewed_catalog",
    APPROVE,
);

/// Input of `approve_reviewed_catalog` (All Data Reviewed).
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ApproveReviewedCatalogInput {
    /// The Catalog Preview the reviewer checked.
    pub preview_id: CatalogPreviewId,
    /// Every blank cutoff the preview lists, each confirmed as not applicable.
    pub confirmed_blank_cutoffs: Vec<CatalogBlankCutoff>,
    /// The approver confirms they wrote listed drafts and approve them with no
    /// second person, as the laboratory's separation-of-duties policy allows
    /// (ADR 0015); needed only when they wrote one.
    pub self_approval: bool,
    /// Optional external laboratory approval record (up to 1000 characters).
    #[serde(default)]
    #[ts(optional)]
    pub external_approval_reference: Option<String>,
    /// The reviewer's password (fresh authentication).
    pub password: Reauthentication,
}

/// Re-authentication and the review share one transaction.
#[derive(Debug, thiserror::Error)]
enum ReviewError {
    #[error(transparent)]
    Auth(#[from] AuthError),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
}

impl From<lis_store::StoreError> for ReviewError {
    fn from(error: lis_store::StoreError) -> Self {
        Self::Catalog(error.into())
    }
}

impl ModuleError for ReviewError {
    fn denied(&self) -> Option<Denied> {
        match self {
            Self::Auth(error) => ModuleError::denied(error),
            Self::Catalog(error) => ModuleError::denied(error),
        }
    }
}

impl From<ReviewError> for Failure {
    fn from(error: ReviewError) -> Self {
        match error {
            ReviewError::Auth(error) => error.into(),
            ReviewError::Catalog(error) => error.into(),
        }
    }
}

/// The Standard Catalog file this build ships; an error is a build defect.
pub(super) fn standard_file() -> Result<StandardFile, Failure> {
    StandardFile::embedded().map_err(Failure::from)
}

impl Engine {
    /// `standard_catalog_tree` (`manage_configuration`): every item of the
    /// shipped file with its state in this laboratory's catalog.
    pub async fn standard_catalog_tree(&self) -> Result<StandardCatalogTree, CommandError> {
        self.run(&STANDARD_CATALOG_TREE, |call| async move {
            let file = standard_file()?;
            self.read(&call, move |conn| {
                lis_catalog::standard_catalog_tree(conn, &file)
            })
            .await
        })
        .await
    }

    /// `load_standard_catalog` (`manage_configuration`): the chosen items and
    /// what they need, as drafts, in one transaction.
    pub async fn load_standard_catalog(
        &self,
        input: LoadStandardCatalog,
    ) -> Result<StandardCatalogLoaded, CommandError> {
        self.run(&LOAD_STANDARD_CATALOG, |call| async move {
            let file = standard_file()?;
            self.write(&call, move |ctx| {
                lis_catalog::load_standard_catalog(ctx, &file, input)
            })
            .await
        })
        .await
    }

    /// `standard_catalog_status` (any unlocked session): the loads and the
    /// loaded drafts remaining, which drive the "starter content" banner.
    pub async fn standard_catalog_status(&self) -> Result<StandardCatalogStatus, CommandError> {
        self.run(&STANDARD_CATALOG_STATUS, |call| async move {
            self.read(&call, lis_catalog::standard_catalog_status).await
        })
        .await
    }

    /// `list_catalog_previews` (`manage_configuration` or
    /// `approve_configuration`): every recorded preview, newest first.
    pub async fn list_catalog_previews(&self) -> Result<Vec<CatalogPreviewSummary>, CommandError> {
        self.run(&LIST_CATALOG_PREVIEWS, |call| async move {
            self.require_catalog_preview_access(&call).await?;
            self.read(&call, named_previews).await
        })
        .await
    }

    /// `approve_reviewed_catalog` (`approve_configuration`, fresh
    /// authentication): All Data Reviewed. The password is checked in the
    /// approval's own transaction; a wrong one commits only its failure count.
    pub async fn approve_reviewed_catalog(
        &self,
        input: ApproveReviewedCatalogInput,
    ) -> Result<CatalogReviewApproved, CommandError> {
        self.run(&APPROVE_REVIEWED_CATALOG, |call| async move {
            // Before fresh authentication, so a refusal counts no password attempt.
            self.authorize(&call).await?;
            let ApproveReviewedCatalogInput {
                preview_id,
                confirmed_blank_cutoffs,
                self_approval,
                external_approval_reference,
                password,
            } = input;
            let review = ApproveReviewedCatalog {
                preview_id,
                confirmed_blank_cutoffs,
                self_approval,
                external_approval_reference,
            };
            let attempt = self
                .write(&call, move |ctx| approve(ctx, &password, review))
                .await?;
            accepted(attempt)
        })
        .await
    }

    /// The either-of check of the preview commands, before any work. A denial
    /// is recorded by the pipeline: the domain check runs in a transaction
    /// that fails, so only the denial event is written.
    pub(super) async fn require_catalog_preview_access(&self, call: &Call) -> Result<(), Failure> {
        let capability = lis_catalog::catalog_preview_capability(&call.principal);
        if call.principal.require(capability).is_ok() {
            return Ok(());
        }
        self.write(call, move |ctx| ctx.require(capability)).await
    }
}

/// The recorded previews with the generating and reviewing users' names.
fn named_previews(conn: &rusqlite::Connection) -> Result<Vec<CatalogPreviewSummary>, CatalogError> {
    let mut previews = lis_catalog::list_catalog_previews(conn)?;
    let mut users = BTreeSet::new();
    for p in &previews {
        users.insert(p.generated_by);
        users.extend(p.reviewed_by);
    }
    let names = versions::display_names(conn, &users)?;
    for p in &mut previews {
        p.generated_by_display = names.get(&p.generated_by).cloned();
        p.reviewed_by_display = p.reviewed_by.and_then(|by| names.get(&by).cloned());
    }
    Ok(previews)
}

fn approve(
    ctx: &mut CommandContext<'_>,
    password: &Reauthentication,
    review: ApproveReviewedCatalog,
) -> Result<Attempt<CatalogReviewApproved>, ReviewError> {
    match lis_auth::reauthenticate(ctx, password, ReauthPurpose::CatalogReview)? {
        Attempt::Rejected(rejection) => Ok(Attempt::Rejected(rejection)),
        Attempt::Accepted(()) => Ok(Attempt::Accepted(lis_catalog::approve_reviewed_catalog(
            ctx, review,
        )?)),
    }
}
