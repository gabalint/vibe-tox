//! Method and orderable versions (REQ-ANA-002, REQ-ANA-009).

use std::collections::BTreeSet;

use lis_catalog::{
    ApproveMethodVersion, ApproveOrderableVersion, DraftMethodVersion, DraftOrderableVersion,
    MethodId, MethodVersion, MethodVersionId, MethodVersionView, OrderableId, OrderableVersion,
    OrderableVersionId, OrderableVersionView, RetireMethodVersion, RetireOrderableVersion,
    UpdateMethodDraft, UpdateOrderableDraft, VersionLifecycle, VersionRef, VersionSummary,
};
use lis_core::CommandError;
use serde::Deserialize;
use ts_rs::TS;

use super::{APPROVE, MANAGE, READ};
use crate::engine::Engine;
use crate::registry::{CommandSpec, spec};

pub(crate) const LIST_METHOD_VERSIONS: CommandSpec =
    spec("list_method_versions", "catalog.list_method_versions", READ);
pub(crate) const GET_METHOD_VERSION: CommandSpec =
    spec("get_method_version", "catalog.get_method_version", READ);
pub(crate) const DRAFT_METHOD_VERSION: CommandSpec = spec(
    "draft_method_version",
    "catalog.draft_method_version",
    MANAGE,
);
pub(crate) const UPDATE_METHOD_DRAFT: CommandSpec =
    spec("update_method_draft", "catalog.update_method_draft", MANAGE);
pub(crate) const APPROVE_METHOD_VERSION: CommandSpec = spec(
    "approve_method_version",
    "catalog.approve_method_version",
    APPROVE,
);
pub(crate) const RETIRE_METHOD_VERSION: CommandSpec = spec(
    "retire_method_version",
    "catalog.retire_method_version",
    APPROVE,
);
pub(crate) const LIST_ORDERABLE_VERSIONS: CommandSpec = spec(
    "list_orderable_versions",
    "catalog.list_orderable_versions",
    READ,
);
pub(crate) const GET_ORDERABLE_VERSION: CommandSpec = spec(
    "get_orderable_version",
    "catalog.get_orderable_version",
    READ,
);
pub(crate) const DRAFT_ORDERABLE_VERSION: CommandSpec = spec(
    "draft_orderable_version",
    "catalog.draft_orderable_version",
    MANAGE,
);
pub(crate) const UPDATE_ORDERABLE_DRAFT: CommandSpec = spec(
    "update_orderable_draft",
    "catalog.update_orderable_draft",
    MANAGE,
);
pub(crate) const APPROVE_ORDERABLE_VERSION: CommandSpec = spec(
    "approve_orderable_version",
    "catalog.approve_orderable_version",
    APPROVE,
);
pub(crate) const RETIRE_ORDERABLE_VERSION: CommandSpec = spec(
    "retire_orderable_version",
    "catalog.retire_orderable_version",
    APPROVE,
);

/// Input of `list_method_versions`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListMethodVersionsInput {
    /// The method.
    pub method_id: MethodId,
}

/// Input of `get_method_version`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GetMethodVersionInput {
    /// The version.
    pub method_version_id: MethodVersionId,
}

/// Input of `list_orderable_versions`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListOrderableVersionsInput {
    /// The orderable.
    pub orderable_id: OrderableId,
}

/// Input of `get_orderable_version`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GetOrderableVersionInput {
    /// The version.
    pub orderable_version_id: OrderableVersionId,
}

impl Engine {
    /// `list_method_versions`: every version of a method, oldest first.
    pub async fn list_method_versions(
        &self,
        input: ListMethodVersionsInput,
    ) -> Result<Vec<VersionSummary<MethodVersion>>, CommandError> {
        self.run(&LIST_METHOD_VERSIONS, |call| async move {
            self.read(&call, move |conn| -> Result<_, lis_catalog::CatalogError> {
                let mut versions = lis_catalog::list_method_versions(conn, input.method_id)?;
                resolve_version_names(conn, &mut versions)?;
                Ok(versions)
            })
            .await
        })
        .await
    }

    /// `get_method_version`: a version with its analytes and limits.
    pub async fn get_method_version(
        &self,
        input: GetMethodVersionInput,
    ) -> Result<MethodVersionView, CommandError> {
        self.run(&GET_METHOD_VERSION, |call| async move {
            self.read(&call, move |conn| -> Result<_, lis_catalog::CatalogError> {
                let mut version = lis_catalog::get_method_version(conn, input.method_version_id)?;
                resolve_lifecycle_names(conn, &mut version.lifecycle)?;
                Ok(version)
            })
            .await
        })
        .await
    }

    /// `draft_method_version`.
    pub async fn draft_method_version(
        &self,
        input: DraftMethodVersion,
    ) -> Result<VersionRef<MethodVersion>, CommandError> {
        self.run(&DRAFT_METHOD_VERSION, |call| async move {
            self.write(&call, move |ctx| {
                lis_catalog::draft_method_version(ctx, input)
            })
            .await
        })
        .await
    }

    /// `update_method_draft`.
    pub async fn update_method_draft(
        &self,
        input: UpdateMethodDraft,
    ) -> Result<VersionRef<MethodVersion>, CommandError> {
        self.run(&UPDATE_METHOD_DRAFT, |call| async move {
            self.write(&call, move |ctx| {
                lis_catalog::update_method_draft(ctx, input)
            })
            .await
        })
        .await
    }

    /// `approve_method_version`.
    pub async fn approve_method_version(
        &self,
        input: ApproveMethodVersion,
    ) -> Result<VersionRef<MethodVersion>, CommandError> {
        self.run(&APPROVE_METHOD_VERSION, |call| async move {
            self.write(&call, move |ctx| {
                lis_catalog::approve_method_version(ctx, input)
            })
            .await
        })
        .await
    }

    /// `retire_method_version`.
    pub async fn retire_method_version(
        &self,
        input: RetireMethodVersion,
    ) -> Result<VersionRef<MethodVersion>, CommandError> {
        self.run(&RETIRE_METHOD_VERSION, |call| async move {
            self.write(&call, move |ctx| {
                lis_catalog::retire_method_version(ctx, input)
            })
            .await
        })
        .await
    }
}

impl Engine {
    /// `list_orderable_versions`: every version of an orderable, oldest first.
    pub async fn list_orderable_versions(
        &self,
        input: ListOrderableVersionsInput,
    ) -> Result<Vec<VersionSummary<OrderableVersion>>, CommandError> {
        self.run(&LIST_ORDERABLE_VERSIONS, |call| async move {
            self.read(&call, move |conn| -> Result<_, lis_catalog::CatalogError> {
                let mut versions = lis_catalog::list_orderable_versions(conn, input.orderable_id)?;
                resolve_version_names(conn, &mut versions)?;
                Ok(versions)
            })
            .await
        })
        .await
    }

    /// `get_orderable_version`: a version with its component manifest.
    pub async fn get_orderable_version(
        &self,
        input: GetOrderableVersionInput,
    ) -> Result<OrderableVersionView, CommandError> {
        self.run(&GET_ORDERABLE_VERSION, |call| async move {
            self.read(&call, move |conn| -> Result<_, lis_catalog::CatalogError> {
                let mut version =
                    lis_catalog::get_orderable_version(conn, input.orderable_version_id)?;
                resolve_lifecycle_names(conn, &mut version.lifecycle)?;
                Ok(version)
            })
            .await
        })
        .await
    }

    /// `draft_orderable_version`.
    pub async fn draft_orderable_version(
        &self,
        input: DraftOrderableVersion,
    ) -> Result<VersionRef<OrderableVersion>, CommandError> {
        self.run(&DRAFT_ORDERABLE_VERSION, |call| async move {
            self.write(&call, move |ctx| {
                lis_catalog::draft_orderable_version(ctx, input)
            })
            .await
        })
        .await
    }

    /// `update_orderable_draft`.
    pub async fn update_orderable_draft(
        &self,
        input: UpdateOrderableDraft,
    ) -> Result<VersionRef<OrderableVersion>, CommandError> {
        self.run(&UPDATE_ORDERABLE_DRAFT, |call| async move {
            self.write(&call, move |ctx| {
                lis_catalog::update_orderable_draft(ctx, input)
            })
            .await
        })
        .await
    }

    /// `approve_orderable_version`.
    pub async fn approve_orderable_version(
        &self,
        input: ApproveOrderableVersion,
    ) -> Result<VersionRef<OrderableVersion>, CommandError> {
        self.run(&APPROVE_ORDERABLE_VERSION, |call| async move {
            self.write(&call, move |ctx| {
                lis_catalog::approve_orderable_version(ctx, input)
            })
            .await
        })
        .await
    }

    /// `retire_orderable_version`.
    pub async fn retire_orderable_version(
        &self,
        input: RetireOrderableVersion,
    ) -> Result<VersionRef<OrderableVersion>, CommandError> {
        self.run(&RETIRE_ORDERABLE_VERSION, |call| async move {
            self.write(&call, move |ctx| {
                lis_catalog::retire_orderable_version(ctx, input)
            })
            .await
        })
        .await
    }
}

pub(super) fn resolve_version_names<K>(
    conn: &rusqlite::Connection,
    versions: &mut [VersionSummary<K>],
) -> Result<(), lis_catalog::CatalogError> {
    resolve_lifecycles_names(
        conn,
        versions.iter_mut().map(|version| &mut version.lifecycle),
    )
}

pub(super) fn resolve_lifecycle_names<K>(
    conn: &rusqlite::Connection,
    lifecycle: &mut VersionLifecycle<K>,
) -> Result<(), lis_catalog::CatalogError> {
    resolve_lifecycles_names(conn, std::iter::once(lifecycle))
}

pub(super) fn resolve_lifecycles_names<'a, K: 'a>(
    conn: &rusqlite::Connection,
    lifecycles: impl Iterator<Item = &'a mut VersionLifecycle<K>>,
) -> Result<(), lis_catalog::CatalogError> {
    let mut lifecycles: Vec<_> = lifecycles.collect();
    let mut users = BTreeSet::new();
    for lifecycle in &lifecycles {
        users.insert(lifecycle.created_by);
        users.extend(lifecycle.approved_by);
        users.extend(lifecycle.retired_by);
    }
    let names = display_names(conn, &users)?;
    for lifecycle in &mut lifecycles {
        set_lifecycle_names(lifecycle, &names);
    }
    Ok(())
}

pub(super) fn display_names(
    conn: &rusqlite::Connection,
    users: &BTreeSet<lis_core::UserId>,
) -> Result<std::collections::HashMap<lis_core::UserId, String>, lis_catalog::CatalogError> {
    lis_auth::display_names(conn, users).map_err(|error| match error {
        lis_auth::AuthError::Store(error) => lis_catalog::CatalogError::Store(error),
        _ => lis_catalog::CatalogError::NotFound("configuration actor"),
    })
}

fn set_lifecycle_names<K>(
    lifecycle: &mut VersionLifecycle<K>,
    names: &std::collections::HashMap<lis_core::UserId, String>,
) {
    let name = |user: Option<lis_core::UserId>| user.and_then(|user| names.get(&user).cloned());
    lifecycle.created_by_display = name(Some(lifecycle.created_by));
    lifecycle.approved_by_display = name(lifecycle.approved_by);
    lifecycle.retired_by_display = name(lifecycle.retired_by);
}
