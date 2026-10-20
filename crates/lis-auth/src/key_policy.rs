//! Frozen laboratory policy and session invalidation for key ceremonies.

use lis_core::SessionId;
use lis_store::CommandContext;
use rusqlite::Connection;

use crate::error::AuthError;
use crate::policy::get_security_policy;
use crate::sessions::{EndReason, end_session};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct KeyPolicy {
    version: Option<i64>,
    pub(crate) allow_touch_only: bool,
}

impl KeyPolicy {
    pub(crate) fn current(conn: &Connection) -> Result<Self, AuthError> {
        let policy = get_security_policy(conn)?;
        Ok(Self {
            version: policy.version,
            allow_touch_only: policy.values.allow_security_key_touch_only,
        })
    }

    pub(crate) fn ensure_current(self, conn: &Connection) -> Result<(), AuthError> {
        if self != Self::current(conn)? {
            return Err(AuthError::ChallengeExpired);
        }
        Ok(())
    }
}

/// The fresh-confirmed caller remains signed in. Every other person's open
/// session and every pending action receipt must respect the new policy.
pub(crate) fn invalidate_policy_sessions(
    ctx: &mut CommandContext<'_>,
    keep: SessionId,
) -> Result<(), AuthError> {
    let open = ctx
        .tx()
        .prepare_cached("SELECT id FROM auth_sessions WHERE ended_at IS NULL AND id <> ?1")?
        .query_map([keep], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<SessionId>>>()?;
    for id in open {
        end_session(ctx, id, EndReason::Expired)?;
    }
    ctx.tx().execute("DELETE FROM auth_key_action_proofs", [])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_migration_preserves_history_and_defaults_to_verified_keys() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../migrations/0002_security_policy.sql"))
            .unwrap();
        conn.execute_batch(
            "INSERT INTO auth_security_policies VALUES (1, 15, 5, 15, NULL, 'actor', 'time');
             INSERT INTO auth_security_policies VALUES (2, 20, 4, 10, 'Historical reason', 'actor2', 'time2');",
        ).unwrap();
        let snapshot = |conn: &Connection| -> Vec<Vec<rusqlite::types::Value>> {
            conn.prepare("SELECT version, idle_timeout_minutes, lockout_failures, lockout_minutes, reason, set_by, set_at FROM auth_security_policies ORDER BY version")
                .unwrap().query_map([], |row| (0..7).map(|column| row.get(column)).collect())
                .unwrap().collect::<rusqlite::Result<_>>().unwrap()
        };
        let before = snapshot(&conn);
        conn.execute_batch(include_str!("../migrations/0009_security_key_policy.sql"))
            .unwrap();
        assert_eq!(snapshot(&conn), before);
        assert_eq!(
            conn.query_row(
                "SELECT sum(allow_security_key_touch_only) FROM auth_security_policies",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        assert!(
            conn.execute(
                "UPDATE auth_security_policies SET allow_security_key_touch_only=1",
                []
            )
            .is_err()
        );
        assert!(
            conn.execute("DELETE FROM auth_security_policies", [])
                .is_err()
        );
    }
}
