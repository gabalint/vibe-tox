//! Routing new billing handoffs, released report revisions and report withdrawals to their
//! connections, in the transaction that creates them, so a committed source always has its
//! message (REQ-BIL-002, REQ-RPT-003, REQ-RPT-013).

use lis_billing::{BillingError, HandoffView};
use lis_core::Denied;
use lis_interfaces::partner::{
    MessageSource, NewPartnerMessage, PartnerError, PartnerMessageType, ReportRef, RoutingFacts,
    RoutingValue,
};
use lis_orders::{OrderParties, Party};
use lis_reporting::{ReportDetail, ReportRevisionId, ReportingError};
use lis_store::{CommandContext, StoreError};

use crate::pipeline::{Failure, ModuleError};

/// A command whose source and routing share one transaction: either may fail.
macro_rules! routing_error {
    ($(#[$doc:meta])* $name:ident, $variant:ident, $error:ty) => {
        $(#[$doc])*
        #[derive(Debug, thiserror::Error)]
        pub(crate) enum $name {
            #[error(transparent)]
            $variant(#[from] $error),
            #[error(transparent)]
            Partner(#[from] PartnerError),
        }

        impl From<StoreError> for $name {
            fn from(error: StoreError) -> Self {
                Self::$variant(error.into())
            }
        }
        impl ModuleError for $name {
            fn denied(&self) -> Option<Denied> {
                match self {
                    Self::$variant(error) => ModuleError::denied(error),
                    Self::Partner(error) => ModuleError::denied(error),
                }
            }
        }

        impl From<$name> for Failure {
            fn from(error: $name) -> Self {
                match error {
                    $name::$variant(error) => error.into(),
                    $name::Partner(error) => error.into(),
                }
            }
        }
    };
}

routing_error!(
    /// Creating a handoff and routing it.
    HandoffError,
    Billing,
    BillingError
);
routing_error!(
    /// Releasing or withdrawing a report and routing it, or reading the message log with
    /// the reports' impact pauses.
    ReportRoutingError,
    Reporting,
    ReportingError
);

/// Makes the handoff's message for every Billing JSON connection whose rules match, or
/// puts it in Unrouted messages. Without any Billing JSON connection nothing is made.
pub(crate) fn route_billing_handoff(
    ctx: &mut CommandContext<'_>,
    handoff: &HandoffView,
) -> Result<(), HandoffError> {
    if !lis_interfaces::partner::carries(ctx.tx(), PartnerMessageType::BillingJson)? {
        return Ok(());
    }
    let parties = lis_billing::handoff_parties(ctx.tx(), handoff.handoff_id)?;
    lis_interfaces::partner::route_partner_message(
        ctx,
        NewPartnerMessage {
            message_type: PartnerMessageType::BillingJson,
            source: MessageSource::BillingHandoff,
            source_id: handoff.handoff_id.to_string(),
            source_number: handoff.handoff_no,
            report: None,
            artifact_sha256: handoff.artifact_sha256.clone(),
            facts: facts(parties),
        },
    )?;
    Ok(())
}

/// The message types a released report goes out as.
const REPORT_TYPES: [PartnerMessageType; 2] =
    [PartnerMessageType::ReportPdf, PartnerMessageType::Hl7Oru];

/// Makes a released revision's message for the Report PDF and the HL7 Results connections:
/// first those that had an earlier revision, then every one whose rules match, or puts it
/// in Unrouted messages. Without any connection of a type nothing of that type is made.
pub(crate) fn route_report_revision(
    ctx: &mut CommandContext<'_>,
    revision_id: ReportRevisionId,
) -> Result<(), ReportRoutingError> {
    let mut revision = None;
    for message_type in REPORT_TYPES {
        if !lis_interfaces::partner::carries(ctx.tx(), message_type)? {
            continue;
        }
        let revision = match &revision {
            Some(revision) => revision,
            None => revision.insert(lis_reporting::revision_outbound(ctx.tx(), revision_id)?),
        };
        lis_interfaces::partner::route_partner_message(
            ctx,
            NewPartnerMessage {
                message_type,
                source: MessageSource::ReportRevision,
                source_id: revision_id.to_string(),
                source_number: revision.revision_no,
                report: Some(ReportRef {
                    report_id: revision.report_id.to_string(),
                    report_number: revision.report_number.clone(),
                }),
                artifact_sha256: revision.artifact_sha256.clone(),
                facts: facts(revision.parties.clone()),
            },
        )?;
    }
    Ok(())
}

/// Sends a withdrawn report's notice to every Report PDF and HL7 Results connection an
/// earlier revision was written to; with none, nothing is made.
pub(crate) fn route_report_withdrawal(
    ctx: &mut CommandContext<'_>,
    report: &ReportDetail,
) -> Result<(), ReportRoutingError> {
    let Some(withdrawal) = &report.withdrawal else {
        return Ok(());
    };
    for message_type in REPORT_TYPES {
        if !lis_interfaces::partner::carries(ctx.tx(), message_type)? {
            continue;
        }
        lis_interfaces::partner::route_partner_message(
            ctx,
            NewPartnerMessage {
                message_type,
                source: MessageSource::ReportWithdrawal,
                source_id: report.report_id.to_string(),
                source_number: withdrawal.revision_no,
                report: Some(ReportRef {
                    report_id: report.report_id.to_string(),
                    report_number: report.report_number.clone(),
                }),
                artifact_sha256: withdrawal.notice_artifact_sha256.clone(),
                facts: RoutingFacts::default(),
            },
        )?;
    }
    Ok(())
}

fn facts(parties: OrderParties) -> RoutingFacts {
    let values = |parties: std::collections::BTreeSet<Party>| {
        parties
            .into_iter()
            .map(|(value, label)| RoutingValue { value, label })
            .collect()
    };
    RoutingFacts {
        clients: values(parties.clients),
        practitioners: values(parties.practitioners),
        locations: values(parties.locations),
        tests: values(parties.tests),
    }
}
