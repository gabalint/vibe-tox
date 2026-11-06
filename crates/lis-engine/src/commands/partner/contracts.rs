//! Interface contracts and acceptance tests of connections (ticket #4555 step 5a;
//! REQ-INT-001): drafting the next contract version, approving it, and running the
//! synthetic acceptance tests, which send nothing. The domain rules live in
//! `lis_interfaces::partner`; the HL7 Orders cases run in the engine, which knows orders.

use std::sync::Arc;

use lis_core::{CommandError, ErrorCode};
use lis_interfaces::partner::{
    AcceptanceCase, ApprovePartnerContract, DraftPartnerContract, PartnerConnectionDetail,
    PartnerContract, PartnerError,
};
use rusqlite::Connection;

use super::connections::PartnerConnectionInput;
use super::folders::blocking;
use super::{APPROVE_PARTNER_CONTRACT, DRAFT_PARTNER_CONTRACT, RUN_PARTNER_ACCEPTANCE_TESTS};
use crate::commands::people::{display_names, name_of};
use crate::engine::Engine;
use crate::partner_intake::IntakeError;
use crate::pipeline::Failure;

impl Engine {
    /// `draft_partner_contract`: the next contract version, until a second person approves
    /// it; a newer draft replaces it.
    pub async fn draft_partner_contract(
        &self,
        input: DraftPartnerContract,
    ) -> Result<PartnerConnectionDetail, CommandError> {
        self.run(&DRAFT_PARTNER_CONTRACT, |call| async move {
            let id = input.connection_id;
            self.write(&call, move |ctx| {
                lis_interfaces::partner::draft_partner_contract(ctx, input)
            })
            .await?;
            self.connection_detail(&call, id).await
        })
        .await
    }

    /// `approve_partner_contract`: the draft becomes the next version; the acceptance tests
    /// must then run again before patient data goes or comes.
    pub async fn approve_partner_contract(
        &self,
        input: ApprovePartnerContract,
    ) -> Result<PartnerConnectionDetail, CommandError> {
        self.run(&APPROVE_PARTNER_CONTRACT, |call| async move {
            let id = input.connection_id;
            self.change(&call, move |ctx| {
                lis_interfaces::partner::approve_partner_contract(ctx, input).map(|_| id)
            })
            .await
        })
        .await
    }

    /// `run_partner_acceptance_tests`: the synthetic acceptance cases of the connection as
    /// it is now, recorded with what they cover. Nothing is sent, and nothing the HL7 Orders
    /// cases make is kept.
    pub async fn run_partner_acceptance_tests(
        &self,
        input: PartnerConnectionInput,
    ) -> Result<PartnerConnectionDetail, CommandError> {
        self.run(&RUN_PARTNER_ACCEPTANCE_TESTS, |call| async move {
            self.authorize(&call).await?;
            let id = input.connection_id;
            let (covers, outgoing, source) = self
                .read(&call, move |conn| {
                    Ok::<_, PartnerError>((
                        lis_interfaces::partner::current_acceptance_key(conn, id)?,
                        lis_interfaces::partner::outgoing_cases(conn, id)?,
                        lis_interfaces::partner::intake_source(conn, id)?,
                    ))
                })
                .await?;
            let cases = match source {
                Some(source) => self.order_cases(source).await?,
                None => outgoing,
            };
            self.change(&call, move |ctx| {
                lis_interfaces::partner::record_acceptance_run(ctx, id, &covers, cases).map(|_| id)
            })
            .await
        })
        .await
    }

    /// The HL7 Orders cases, in a transaction that is rolled back.
    async fn order_cases(
        &self,
        source: lis_interfaces::partner::IntakeSource,
    ) -> Result<Vec<AcceptanceCase>, Failure> {
        let db = Arc::clone(&self.db);
        blocking(move || crate::partner_intake::acceptance::order_cases(&db, source))
            .await?
            .map_err(|error: IntakeError| {
                tracing::warn!(%error, "order acceptance tests not run");
                Failure::refused(
                    ErrorCode::Conflict,
                    "The order acceptance tests could not run. Check that the connection's \
                     client is active and its mapped tests are in the catalog.",
                )
            })
    }
}

/// Current account names on the contract's and the last run's stamps.
pub(super) fn name_contract(
    conn: &Connection,
    contract: &mut PartnerContract,
) -> Result<(), PartnerError> {
    let mut stamps = Vec::new();
    if let Some(current) = &mut contract.current {
        stamps.push(&mut current.drafted);
        stamps.push(&mut current.approved);
    }
    if let Some(draft) = &mut contract.draft {
        stamps.push(&mut draft.drafted);
    }
    if let Some(run) = &mut contract.last_run {
        stamps.push(&mut run.ran);
    }
    let names = display_names(conn, stamps.iter().map(|stamp| stamp.by.as_str()))?;
    for stamp in stamps {
        stamp.by_display = name_of(&names, &stamp.by);
    }
    Ok(())
}
