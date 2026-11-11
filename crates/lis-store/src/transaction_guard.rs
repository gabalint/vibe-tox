//! Prevents callback SQL from controlling store-owned transactions.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rusqlite::Connection;
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};

use crate::error::StoreError;

/// The authorizer that denies transaction and savepoint statements while armed.
///
/// rusqlite registers an authorizer only on a `&mut Connection`, so it is installed
/// before the store begins its transaction, disarmed, and a
/// [`TransactionControlGuard`] arms it for the callback alone. Disarmed, it allows
/// every statement, so the store's own `BEGIN`, `COMMIT` and `ROLLBACK` run.
pub(crate) struct TransactionControl {
    armed: Arc<AtomicBool>,
    denied: Arc<AtomicBool>,
}

impl TransactionControl {
    /// Installs the disarmed authorizer on `connection`, replacing any earlier one.
    pub(crate) fn install(connection: &mut Connection) -> Result<Self, StoreError> {
        let armed = Arc::new(AtomicBool::new(false));
        let denied = Arc::new(AtomicBool::new(false));
        let (callback_armed, callback_denied) = (Arc::clone(&armed), Arc::clone(&denied));
        connection.authorizer(Some(move |context: AuthContext<'_>| {
            if callback_armed.load(Ordering::Relaxed)
                && matches!(
                    context.action,
                    AuthAction::Transaction { .. } | AuthAction::Savepoint { .. }
                )
            {
                callback_denied.store(true, Ordering::Relaxed);
                Authorization::Deny
            } else {
                Authorization::Allow
            }
        }))?;
        Ok(Self { armed, denied })
    }

    /// Denies transaction control on `connection` until the guard finishes or drops.
    pub(crate) fn guard<'a>(&self, connection: &'a Connection) -> TransactionControlGuard<'a> {
        connection.flush_prepared_statement_cache();
        self.denied.store(false, Ordering::Relaxed);
        self.armed.store(true, Ordering::Relaxed);
        TransactionControlGuard {
            connection,
            armed: Arc::clone(&self.armed),
            denied: Arc::clone(&self.denied),
            active: true,
        }
    }
}

/// Denies transaction and savepoint statements until explicitly finished.
///
/// SQLite authorizes statements when they are prepared. Flushing on both
/// sides prevents a statement prepared under another authorization policy
/// from crossing this boundary.
pub(crate) struct TransactionControlGuard<'a> {
    connection: &'a Connection,
    armed: Arc<AtomicBool>,
    denied: Arc<AtomicBool>,
    active: bool,
}

impl TransactionControlGuard<'_> {
    /// Disarms the authorizer and reports whether it denied transaction control.
    pub(crate) fn finish(mut self) -> bool {
        self.clear();
        self.denied.load(Ordering::Relaxed)
    }

    fn clear(&mut self) {
        if self.active {
            self.armed.store(false, Ordering::Relaxed);
            self.connection.flush_prepared_statement_cache();
            self.active = false;
        }
    }
}

impl Drop for TransactionControlGuard<'_> {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_transaction_statement_cannot_bypass_guard() {
        let mut connection = Connection::open_in_memory().unwrap();
        drop(connection.prepare_cached("COMMIT").unwrap());
        let control = TransactionControl::install(&mut connection).unwrap();
        let transaction = connection.transaction().unwrap();
        let guard = control.guard(&transaction);
        assert!(transaction.prepare_cached("COMMIT").is_err());
        assert!(guard.finish());
    }

    #[test]
    fn every_transaction_control_form_is_denied() {
        for sql in [
            "BEGIN",
            "COMMIT",
            "ROLLBACK",
            "SAVEPOINT nested",
            "RELEASE nested",
            "ROLLBACK TO nested",
        ] {
            let mut connection = Connection::open_in_memory().unwrap();
            let control = TransactionControl::install(&mut connection).unwrap();
            let transaction = connection.transaction().unwrap();
            let guard = control.guard(&transaction);
            assert!(transaction.execute_batch(sql).is_err(), "{sql}");
            assert!(guard.finish(), "{sql}");
        }
    }

    #[test]
    fn the_store_commits_once_the_guard_is_finished() {
        let mut connection = Connection::open_in_memory().unwrap();
        let control = TransactionControl::install(&mut connection).unwrap();
        let transaction = connection.transaction().unwrap();
        let guard = control.guard(&transaction);
        transaction.execute_batch("CREATE TABLE t (x)").unwrap();
        assert!(!guard.finish());
        transaction.commit().unwrap();
        connection.execute_batch("BEGIN; COMMIT;").unwrap();
    }
}
