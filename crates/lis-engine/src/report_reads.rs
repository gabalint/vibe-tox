//! Reads of report records (NFR-07, REQ-RPT-002): who may read report
//! history, and the reads that record a preservation incident for stored
//! report content that fails its digest, before refusing it.

use lis_audit::{AuditDetail, AuditEvent};
use lis_core::{Capability, CommandError};
use lis_reporting::{
    DeliveryQueuePage, DestinationView, ListDeliveryDestinationsInput,
    ListDeliveryObligationsInput, ListReportsInput, ReportId, ReportSummary, ReportingError,
};
use rusqlite::Connection;

use crate::commands::reporting::{
    LIST_DELIVERY_DESTINATIONS, LIST_DELIVERY_OBLIGATIONS, LIST_REPORTS,
};
use crate::engine::Engine;
use crate::pipeline::{Call, Failure, access};

impl Engine {
    pub(crate) async fn record_workspace_incident(
        &self,
        call: &crate::pipeline::Call,
        incident: Option<lis_reporting::ContentIncident>,
    ) -> Result<(), Failure> {
        let Some(incident) = incident else {
            return Ok(());
        };
        self.write(call, move |ctx| {
            lis_reporting::record_content_incident(ctx, Capability::ReleaseReports, &incident)
        })
        .await?;
        Ok(())
    }

    /// `list_reports`: identifiers and states, with one access event.
    pub async fn list_reports(
        &self,
        input: ListReportsInput,
    ) -> Result<Vec<ReportSummary>, CommandError> {
        self.run(&LIST_REPORTS, |call| async move {
            let capability = self.require_report_reader(&call).await?;
            let filter = input.filter.as_str();
            let has_search = input.search.is_some();
            self.audited_report_read(
                &call,
                capability,
                move |conn| lis_reporting::list_reports(conn, &input),
                move |rows| {
                    access(LIST_REPORTS.action)
                        .detail("filter", AuditDetail::code(filter))
                        .detail("search_used", AuditDetail::boolean(has_search))
                        .detail("returned", AuditDetail::count(rows.len()))
                },
            )
            .await
        })
        .await
    }

    /// `list_delivery_obligations`: an audited page of the delivery queue.
    pub async fn list_delivery_obligations(
        &self,
        input: ListDeliveryObligationsInput,
    ) -> Result<DeliveryQueuePage, CommandError> {
        self.run(&LIST_DELIVERY_OBLIGATIONS, |call| async move {
            let filter = input.filter.as_str();
            let continued = input.after.is_some();
            self.audited_read(
                &call,
                move |conn| lis_reporting::delivery_queue_page(conn, input),
                move |page| {
                    access(LIST_DELIVERY_OBLIGATIONS.action)
                        .detail("filter", AuditDetail::code(filter))
                        .detail("continued", AuditDetail::boolean(continued))
                        .detail("returned", AuditDetail::count(page.items.len()))
                },
            )
            .await
        })
        .await
    }

    /// `list_delivery_destinations`: audited destination detail for any session.
    pub async fn list_delivery_destinations(
        &self,
        input: ListDeliveryDestinationsInput,
    ) -> Result<Vec<DestinationView>, CommandError> {
        self.run(&LIST_DELIVERY_DESTINATIONS, |call| async move {
            let client_scoped = input.client_id.is_some();
            let state_scoped = input.state.is_some();
            self.audited_read(
                &call,
                move |conn| lis_reporting::list_destinations(conn, input),
                move |rows| {
                    access(LIST_DELIVERY_DESTINATIONS.action)
                        .detail("client_scoped", AuditDetail::boolean(client_scoped))
                        .detail("state_scoped", AuditDetail::boolean(state_scoped))
                        .detail("returned", AuditDetail::count(rows.len()))
                },
            )
            .await
        })
        .await
    }

    /// Report history is read with `manage_quality` (impact assessment) or
    /// `deliver_reports`; a caller with neither is refused and the denial
    /// audited. Returns the capability the read runs under.
    pub(crate) async fn require_report_reader(&self, call: &Call) -> Result<Capability, Failure> {
        if call.principal.require(Capability::ManageQuality).is_ok() {
            return Ok(Capability::ManageQuality);
        }
        self.write(call, |ctx| ctx.require(Capability::DeliverReports))
            .await?;
        Ok(Capability::DeliverReports)
    }

    /// A reporting read under `capability`. Stored report content that fails
    /// its digest is recorded as a preservation incident in its own
    /// transaction, then refused: it is never shown (NFR-07).
    pub(crate) async fn report_read<T, F>(
        &self,
        call: &Call,
        capability: Capability,
        query: F,
    ) -> Result<T, Failure>
    where
        F: FnOnce(&Connection) -> Result<T, ReportingError> + Send + 'static,
        T: Send + 'static,
    {
        let read = self
            .read(call, move |conn| Ok::<_, ReportingError>(query(conn)))
            .await?;
        let Err(ReportingError::ContentMismatch(incident)) = read else {
            return read.map_err(Into::into);
        };
        let recorded = incident.clone();
        self.write(call, move |ctx| {
            lis_reporting::record_content_incident(ctx, capability, &recorded)
        })
        .await?;
        Err(ReportingError::ContentMismatch(incident).into())
    }

    /// Records the damaged stored content of a report's current revision
    /// that no incident covers yet, in its own transaction, so the report
    /// can be shown with its incident instead of being refused (NFR-07).
    pub(crate) async fn record_damaged_content(
        &self,
        call: &Call,
        capability: Capability,
        report_id: ReportId,
    ) -> Result<(), Failure> {
        let damaged = self
            .read(call, move |conn| {
                lis_reporting::unrecorded_content_incident(conn, report_id)
            })
            .await?;
        let Some(incident) = damaged else {
            return Ok(());
        };
        self.write(call, move |ctx| {
            lis_reporting::record_content_incident(ctx, capability, &incident)
        })
        .await
        .map(drop)
    }

    /// [`Self::report_read`] of PHI: the access event `event(&data)` is
    /// appended before the data is returned.
    pub(crate) async fn audited_report_read<T, F, A>(
        &self,
        call: &Call,
        capability: Capability,
        query: F,
        event: A,
    ) -> Result<T, Failure>
    where
        F: FnOnce(&Connection) -> Result<T, ReportingError> + Send + 'static,
        T: Send + 'static,
        A: FnOnce(&T) -> AuditEvent,
    {
        let data = self.report_read(call, capability, query).await?;
        let event = event(&data);
        self.write(call, move |ctx| ctx.audit(event).map(drop))
            .await
            .map(|()| data)
    }
}
