//! Prevents callback SQL from controlling store-owned transactions.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rusqlite::Connection;
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};

use crate::error::StoreError;

/// Installs the hook before a transaction borrows the connection, and removes
/// it after that transaction ends. Callback guards enable its restrictions.
pub(crate) fn with_authorizer<T, E>(
    connection: &mut Connection,
    run: impl FnOnce(&mut Connection, &TransactionControl) -> Result<T, E>,
) -> Result<T, E>
where
    E: From<StoreError>,
{
    let mut authorizer = AuthorizerGuard::install(connection).map_err(E::from)?;
    let result = run(authorizer.connection, &authorizer.control);
    // Bundled SQLite removes the hook and its client data without allocating
    // or failing for a live connection. Keep this cleanup free of fallible
    // work that could report failure after the transaction already committed.
    authorizer.clear().map_err(E::from)?;
    result
}

struct AuthorizerGuard<'a> {
    connection: &'a mut Connection,
    control: Arc<TransactionControl>,
    active: bool,
}

impl<'a> AuthorizerGuard<'a> {
    fn install(connection: &'a mut Connection) -> Result<Self, StoreError> {
        let control = Arc::new(TransactionControl::default());
        let callback_control = Arc::clone(&control);
        connection.authorizer(Some(move |context: AuthContext<'_>| {
            if callback_control.protected.load(Ordering::Relaxed)
                && matches!(
                    context.action,
                    AuthAction::Transaction { .. } | AuthAction::Savepoint { .. }
                )
            {
                callback_control.denied.store(true, Ordering::Relaxed);
                Authorization::Deny
            } else {
                Authorization::Allow
            }
        }))?;
        Ok(Self {
            connection,
            control,
            active: true,
        })
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

impl Drop for AuthorizerGuard<'_> {
    fn drop(&mut self) {
        let _ = self.clear();
    }
}

#[derive(Default)]
pub(crate) struct TransactionControl {
    protected: AtomicBool,
    denied: AtomicBool,
}

impl TransactionControl {
    /// Protects callback SQL after the store has begun its transaction.
    pub(crate) fn protect<'a>(&'a self, connection: &'a Connection) -> TransactionControlGuard<'a> {
        connection.flush_prepared_statement_cache();
        self.denied.store(false, Ordering::Relaxed);
        self.protected.store(true, Ordering::Relaxed);
        TransactionControlGuard {
            connection,
            control: self,
        }
    }
}

/// Disables restrictions before the transaction commits or rolls back, including
/// during unwinding. SQLite authorizes prepared statements, so both callback
/// boundaries flush statements prepared under the other authorization policy.
pub(crate) struct TransactionControlGuard<'a> {
    connection: &'a Connection,
    control: &'a TransactionControl,
}

impl TransactionControlGuard<'_> {
    /// Ends protection and reports whether it denied transaction control.
    pub(crate) fn finish(self) -> bool {
        self.control.denied.load(Ordering::Relaxed)
    }
}

impl Drop for TransactionControlGuard<'_> {
    fn drop(&mut self) {
        self.control.protected.store(false, Ordering::Relaxed);
        self.connection.flush_prepared_statement_cache();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_transaction_statement_cannot_bypass_guard() {
        let mut connection = Connection::open_in_memory().unwrap();
        with_authorizer(&mut connection, |connection, control| {
            drop(connection.prepare_cached("COMMIT")?);
            let transaction = connection.transaction()?;
            let guard = control.protect(&transaction);
            assert!(transaction.prepare_cached("COMMIT").is_err());
            assert!(guard.finish());
            transaction.commit()?;
            Ok::<_, StoreError>(())
        })
        .unwrap();
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
            with_authorizer(&mut connection, |connection, control| {
                let transaction = connection.transaction()?;
                let guard = control.protect(&transaction);
                assert!(transaction.execute_batch(sql).is_err(), "{sql}");
                assert!(guard.finish(), "{sql}");
                transaction.rollback()?;
                Ok::<_, StoreError>(())
            })
            .unwrap();
        }
    }

    #[test]
    fn callback_error_rolls_back_and_allows_connection_reuse() {
        let mut connection = Connection::open_in_memory().unwrap();
        let result: Result<(), StoreError> =
            with_authorizer(&mut connection, |connection, control| {
                let transaction = connection.transaction()?;
                let _guard = control.protect(&transaction);
                transaction.execute_batch("CREATE TABLE items (id INTEGER PRIMARY KEY) STRICT")?;
                Err(StoreError::OutboxConflict)
            });
        assert!(matches!(result, Err(StoreError::OutboxConflict)));
        assert!(connection.is_autocommit());
        connection
            .execute_batch(
                "BEGIN IMMEDIATE; CREATE TABLE items (id INTEGER PRIMARY KEY) STRICT; COMMIT",
            )
            .unwrap();
    }

    #[test]
    fn panicking_callback_allows_rollback_and_connection_reuse() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("CREATE TABLE items (id INTEGER PRIMARY KEY) STRICT")
            .unwrap();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_authorizer(
                &mut connection,
                |connection, control| -> Result<(), StoreError> {
                    let transaction = connection.transaction()?;
                    let _guard = control.protect(&transaction);
                    transaction.execute("INSERT INTO items VALUES (1)", [])?;
                    panic!("synthetic panic inside a guarded callback");
                },
            )
        }));
        assert!(panic.is_err());
        assert!(connection.is_autocommit());
        let count: i64 = connection
            .query_row("SELECT count(*) FROM items", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
        connection
            .execute_batch("BEGIN IMMEDIATE; INSERT INTO items VALUES (1); COMMIT")
            .unwrap();
    }

    #[test]
    fn dropping_authorizer_removes_its_hook() {
        let mut connection = Connection::open_in_memory().unwrap();
        let control = {
            let authorizer = AuthorizerGuard::install(&mut connection).unwrap();
            Arc::downgrade(&authorizer.control)
        };
        assert!(control.upgrade().is_none());
    }
}
