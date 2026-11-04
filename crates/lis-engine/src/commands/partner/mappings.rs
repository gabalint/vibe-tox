//! Code mappings of HL7 Results connections (REQ-INT-004): the list with what waiting
//! messages are held for, drafting a change, discarding it and approving the drafts as a
//! new version. The domain rules live in `lis_interfaces::partner`.

use lis_core::CommandError;
use lis_interfaces::partner::{
    ApprovePartnerMappings, DiscardPartnerMappingDraft, DraftPartnerMapping, LisRecordId,
    ListPartnerMessages, MappingKind, MessageView, NewMappingDraft, PartnerConnectionId,
    PartnerError, PartnerMappings, PartnerMessageRow, PartnerMessageType, TransportOutcome,
};
use lis_reporting::ReportingError;
use rusqlite::Connection;

use super::connections::PartnerConnectionInput;
use super::{
    APPROVE_PARTNER_MAPPINGS, DISCARD_PARTNER_MAPPING_DRAFT, DRAFT_PARTNER_MAPPING,
    LIST_PARTNER_MAPPINGS, ReportRoutingError,
};
use crate::engine::Engine;
use crate::partner_delivery::hl7_message::Missing;
use crate::partner_delivery::hl7_payload::{Target, make};

impl Engine {
    /// `list_partner_mappings`: an HL7 connection's approved mappings, drafts, and the LIS
    /// tests and analytes its waiting messages are held for.
    pub async fn list_partner_mappings(
        &self,
        input: PartnerConnectionInput,
    ) -> Result<PartnerMappings, CommandError> {
        self.run(&LIST_PARTNER_MAPPINGS, |call| async move {
            self.read(&call, move |conn| mappings_view(conn, input.connection_id))
                .await
        })
        .await
    }

    /// `draft_partner_mapping`: proposes a mapping, or its removal, for the next version.
    pub async fn draft_partner_mapping(
        &self,
        input: DraftPartnerMapping,
    ) -> Result<PartnerMappings, CommandError> {
        self.run(&DRAFT_PARTNER_MAPPING, |call| async move {
            // The catalog lookup runs before the domain command's own check.
            self.authorize(&call).await?;
            self.write(&call, move |ctx| {
                let (lis_code, lis_name) = lis_record(ctx.tx(), input.kind, input.lis_id)?;
                let connection_id = input.connection_id;
                lis_interfaces::partner::draft_partner_mapping(
                    ctx,
                    NewMappingDraft {
                        input,
                        lis_code,
                        lis_name,
                    },
                )?;
                mappings_view(ctx.tx(), connection_id)
            })
            .await
        })
        .await
    }

    /// `discard_partner_mapping_draft`: drops one proposed change.
    pub async fn discard_partner_mapping_draft(
        &self,
        input: DiscardPartnerMappingDraft,
    ) -> Result<PartnerMappings, CommandError> {
        self.run(&DISCARD_PARTNER_MAPPING_DRAFT, |call| async move {
            self.write(&call, move |ctx| {
                let connection_id = input.connection_id;
                lis_interfaces::partner::discard_partner_mapping_draft(ctx, input)?;
                mappings_view(ctx.tx(), connection_id)
            })
            .await
        })
        .await
    }

    /// `approve_partner_mappings`: the drafts become the next version, for messages not
    /// written yet; a second person approves, as the separation-of-duties policy requires.
    pub async fn approve_partner_mappings(
        &self,
        input: ApprovePartnerMappings,
    ) -> Result<PartnerMappings, CommandError> {
        self.run(&APPROVE_PARTNER_MAPPINGS, |call| async move {
            self.write(&call, move |ctx| {
                let connection_id = input.connection_id;
                lis_interfaces::partner::approve_partner_mappings(ctx, input)?;
                mappings_view(ctx.tx(), connection_id)
            })
            .await
        })
        .await
    }
}

fn mappings_view(
    conn: &Connection,
    connection_id: PartnerConnectionId,
) -> Result<PartnerMappings, ReportRoutingError> {
    let mut held = Vec::new();
    for missing in held_for(conn, connection_id)? {
        let (code, name) = lis_record(conn, missing.kind, missing.lis_id)?;
        held.push((missing.kind, missing.lis_id, code, name));
    }
    Ok(lis_interfaces::partner::partner_mappings(
        conn,
        connection_id,
        &held,
    )?)
}

/// The tests and analytes the connection's waiting HL7 messages miss a mapping for.
fn held_for(
    conn: &Connection,
    connection_id: PartnerConnectionId,
) -> Result<Vec<Missing>, ReportRoutingError> {
    let page = lis_interfaces::partner::list_partner_messages(
        conn,
        &ListPartnerMessages {
            since: None,
            connection_id: Some(connection_id),
            view: MessageView::NotSent,
            search: None,
        },
    )?;
    let mut held: Vec<Missing> = Vec::new();
    for row in &page.rows {
        for missing in missing_for(conn, row)? {
            if !held.contains(&missing) {
                held.push(missing);
            }
        }
    }
    Ok(held)
}

/// What a waiting HL7 row misses; nothing for any other row.
pub(super) fn missing_for(
    conn: &Connection,
    row: &PartnerMessageRow,
) -> Result<Vec<Missing>, ReportRoutingError> {
    let (Some(delivery_id), Some(connection)) = (row.delivery_id, &row.connection) else {
        return Ok(Vec::new());
    };
    if row.message_type != PartnerMessageType::Hl7Oru
        || row.transport != TransportOutcome::NotSent
        || row.superseded
    {
        return Ok(Vec::new());
    }
    let target = Target {
        delivery_id,
        connection_id: connection.id,
        created_at: row.at,
        partner: String::new(),
        source: row.source,
        source_id: row.source_id.clone(),
    };
    Ok(make(conn, &target)?.err().unwrap_or_default())
}

/// The code and name of the LIS orderable or analyte `lis_id`.
fn lis_record(
    conn: &Connection,
    kind: MappingKind,
    lis_id: LisRecordId,
) -> Result<(String, String), ReportRoutingError> {
    let found = match kind {
        MappingKind::Test => lis_catalog::list_orderables(conn)
            .map_err(ReportingError::from)?
            .into_iter()
            .find(|orderable| orderable.id.cast() == lis_id)
            .map(|orderable| (orderable.code, orderable.name)),
        MappingKind::Analyte => lis_catalog::list_analytes(conn)
            .map_err(ReportingError::from)?
            .into_iter()
            .find(|analyte| analyte.id.cast() == lis_id)
            .map(|analyte| (analyte.code, analyte.name)),
    };
    Ok(found.ok_or(PartnerError::NotFound("test or analyte"))?)
}
