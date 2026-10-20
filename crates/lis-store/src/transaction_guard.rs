//! Prevents callback SQL from controlling store-owned transactions.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rusqlite::Connection;
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};

use crate::error::StoreError;

/// Denies transaction and savepoint statements until explicitly finished.
///
/// SQLite authorizes statements when they are prepared. Flushing on both
/// sides prevents a statement prepared under another authorization policy
/// from crossing this boundary.
pub(crate) struct TransactionControlGuard<'a> {
    connection: &'a Connection,
    denied: Arc<AtomicBool>,
    active: bool,
}

impl<'a> TransactionControlGuard<'a> {
    pub(crate) fn install(connection: &'a Connection) -> Result<Self, StoreError> {
        connection.flush_prepared_statement_cache();
        let denied = Arc::new(AtomicBool::new(false));
        let callback_flag = Arc::clone(&denied);
        connection.authorizer(Some(move |context: AuthContext<'_>| {
            if matches!(
                context.action,
                AuthAction::Transaction { .. } | AuthAction::Savepoint { .. }
            ) {
                callback_flag.store(true, Ordering::Relaxed);
                Authorization::Deny
            } else {
                Authorization::Allow
            }
        }))?;
        Ok(Self {
            connection,
            denied,
            active: true,
        })
    }

    /// Removes the authorizer and reports whether it denied transaction control.
    pub(crate) fn finish(mut self) -> Result<bool, StoreError> {
        self.clear()?;
        Ok(self.denied.load(Ordering::Relaxed))
    }

    fn clear(&mut self) -> Result<(), StoreError> {
        if self.active {
            self.connection
                .authorizer(None::<fn(AuthContext<'_>) -> Authorization>)?;
            self.connection.flush_prepared_statement_cache();
            self.active = false;
        }
        Ok(())
    }
}

impl Drop for TransactionControlGuard<'_> {
    fn drop(&mut self) {
        let _ = self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_transaction_statement_cannot_bypass_guard() {
        let mut connection = Connection::open_in_memory().unwrap();
        drop(connection.prepare_cached("COMMIT").unwrap());
        let transaction = connection.transaction().unwrap();
        let guard = TransactionControlGuard::install(&transaction).unwrap();
        assert!(transaction.prepare_cached("COMMIT").is_err());
        assert!(guard.finish().unwrap());
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
            let transaction = connection.transaction().unwrap();
            let guard = TransactionControlGuard::install(&transaction).unwrap();
            assert!(transaction.execute_batch(sql).is_err(), "{sql}");
            assert!(guard.finish().unwrap(), "{sql}");
        }
    }
}
