//! What each folder or SFTP connection's latest reading found (ticket #4555 step 5c): when,
//! and the problem that stopped it, if any. The reader records it after each reading; the
//! connection views show it as the connection's last connection, and a started connection
//! whose reading failed is in error until a reading works again. Kept in memory: after a
//! start, the first reading sets it again.

use std::collections::HashMap;
use std::sync::Mutex;

use lis_core::UtcTimestamp;
use lis_interfaces::partner::{
    ConnectionStatus, LastAttempt, PartnerConnection, PartnerConnectionId,
};

use crate::ops_worker::locked;

/// Shared readings and serialization of folder/SFTP intake passes.
#[derive(Default)]
pub(crate) struct PartnerReadings {
    /// Manual and background readers must finish recording and moving files before
    /// another pass lists them. Separate from `readings`, which a pass updates.
    pub(super) pass: Mutex<()>,
    readings: Mutex<HashMap<PartnerConnectionId, LastAttempt>>,
}

impl PartnerReadings {
    /// Records a reading at `at`; `problem` is the PHI-free code of the first failure.
    pub(super) fn record(
        &self,
        id: PartnerConnectionId,
        at: UtcTimestamp,
        problem: Option<&'static str>,
    ) {
        let reading = LastAttempt {
            at,
            error_code: problem.map(str::to_owned),
        };
        locked(&self.readings).insert(id, reading);
    }

    /// Adds the latest reading to `connection`, as its last connection; a started one whose
    /// reading failed is in error.
    pub(crate) fn add_view(&self, connection: &mut PartnerConnection) {
        let Some(reading) = locked(&self.readings).get(&connection.id).cloned() else {
            return;
        };
        if connection.active && reading.error_code.is_some() {
            connection.status = ConnectionStatus::Error;
        }
        connection.last_attempt = Some(reading);
    }
}
