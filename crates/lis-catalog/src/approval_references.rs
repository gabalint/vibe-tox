//! Optional external approval evidence; deliberately outside clinical method content.
use crate::{commands::lifecycle::acting_user, error::CatalogError, model::text};
use lis_audit::{AuditEvent, Outcome, Stream};
use lis_core::Id;
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension, params};

pub(crate) fn validate(value: Option<&str>) -> Result<Option<String>, CatalogError> {
    value
        .map(|value| text("externalApprovalReference", value, 1000))
        .transpose()
}

pub(crate) fn record<K>(
    ctx: &CommandContext<'_>,
    kind: &'static str,
    id: Id<K>,
    reference: Option<&str>,
) -> Result<(), CatalogError> {
    let Some(reference) = reference else {
        return Ok(());
    };
    let actor = acting_user(ctx.principal())?;
    ctx.tx().execute(
        "INSERT INTO cat_approval_references (record_kind, record_id, external_reference, recorded_by, recorded_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![kind, id, reference, actor, ctx.now()],
    )?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "catalog.external_approval_reference",
            Outcome::Succeeded,
        )
        .record(kind, id)
        .reason(reference.to_owned()),
    )?;
    Ok(())
}

pub(crate) fn get<K>(
    conn: &Connection,
    kind: &str,
    id: Id<K>,
) -> Result<Option<String>, CatalogError> {
    Ok(conn.query_row("SELECT external_reference FROM cat_approval_references WHERE record_kind = ?1 AND record_id = ?2", params![kind, id], |row| row.get(0)).optional()?)
}
