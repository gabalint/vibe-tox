//! The unit of work every command runs in.

use lis_audit::{Attribution, AuditEvent, ChainLink};
use lis_core::{Capability, Clock, CorrelationId, Principal, UtcTimestamp};
use rusqlite::{Connection, Transaction, TransactionBehavior};

use crate::error::StoreError;
use crate::outbox::{self, OutboxItem, OutboxItemId};
use crate::transaction_guard::TransactionControlGuard;

/// A command's view of its `BEGIN IMMEDIATE` transaction (NFR-01).
///
/// Everything a command writes through [`tx`](Self::tx), [`audit`](Self::audit)
/// and [`enqueue`](Self::enqueue) commits together when the command returns
/// `Ok`, and rolls back together when it returns `Err`.
pub struct CommandContext<'a> {
    tx: &'a Transaction<'a>,
    principal: &'a Principal,
    clock: &'a dyn Clock,
    now: UtcTimestamp,
    correlation_id: CorrelationId,
    granted: Vec<Capability>,
}

impl<'a> CommandContext<'a> {
    /// The open transaction, for the owning module's SQL.
    #[must_use]
    pub fn tx(&self) -> &Transaction<'a> {
        self.tx
    }

    /// Who runs the command.
    #[must_use]
    pub fn principal(&self) -> &Principal {
        self.principal
    }

    /// The command's time, read once when the transaction began.
    #[must_use]
    pub fn now(&self) -> UtcTimestamp {
        self.now
    }

    /// Shared with the audit events, the diagnostic log line and any error.
    #[must_use]
    pub fn correlation_id(&self) -> CorrelationId {
        self.correlation_id
    }

    /// Refuses the command unless the principal holds `capability`.
    ///
    /// Granted capabilities become the `authority` of this command's audit events.
    pub fn require(&mut self, capability: Capability) -> Result<(), StoreError> {
        self.principal.require(capability)?;
        if !self.granted.contains(&capability) {
            self.granted.push(capability);
        }
        Ok(())
    }

    /// Appends an audit event in this transaction (REQ-OPS-001).
    ///
    /// Fills in the principal, the command time (`occurred_at`), the append
    /// time (`recorded_at`), the correlation ID and the capabilities granted by
    /// [`require`](Self::require). If the append fails, the command must fail.
    pub fn audit(&self, event: AuditEvent) -> Result<ChainLink, StoreError> {
        let attribution = Attribution {
            principal: self.principal,
            occurred_at: self.now,
            recorded_at: self.clock.now(),
            correlation_id: self.correlation_id,
            authority: &self.granted,
        };
        Ok(lis_audit::append(self.tx, &attribution, &event)?)
    }

    /// Records follow-up work in this transaction (NFR-02).
    ///
    /// The same topic and dedup key with the same payload is a no-op that
    /// returns the existing item; with a different payload it is
    /// [`StoreError::OutboxConflict`].
    pub fn enqueue(&self, item: OutboxItem) -> Result<OutboxItemId, StoreError> {
        outbox::enqueue(self.tx, &item, self.now)
    }
}

/// Runs `command` in a `BEGIN IMMEDIATE` transaction: commit on `Ok`, roll back on `Err`.
pub(crate) fn run_unit_of_work<T, E, F>(
    connection: &mut Connection,
    principal: &Principal,
    correlation_id: CorrelationId,
    clock: &dyn Clock,
    command: F,
) -> Result<T, E>
where
    F: FnOnce(&mut CommandContext<'_>) -> Result<T, E>,
    E: From<StoreError>,
{
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(StoreError::from)?;
    let guard = TransactionControlGuard::install(&tx).map_err(E::from)?;
    let result = command(&mut CommandContext {
        tx: &tx,
        principal,
        clock,
        now: clock.now(),
        correlation_id,
        granted: Vec::new(),
    });
    if guard.finish().map_err(E::from)? {
        return Err(StoreError::TransactionEnded.into());
    }
    if tx.is_autocommit() {
        return Err(StoreError::TransactionEnded.into());
    }
    // On `Err`, `tx` drops here and rolls back.
    let value = result?;
    tx.commit().map_err(StoreError::from)?;
    Ok(value)
}
