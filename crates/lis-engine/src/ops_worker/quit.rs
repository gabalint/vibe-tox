//! The close and quit warning (REQ-OPS-013; M4 design brief §6.3): what is still pending
//! when the window closes or the app quits, and the app run's stop record, once, as a clean
//! stop or a quit with warnings, attributed to the signed-in user or the sign-in screen.

use std::sync::atomic::Ordering;

use lis_core::{CorrelationId, Principal};
use lis_interfaces::ListenerState;
use lis_ops::RuntimeError;
use lis_ops::runs::StopKind;

use super::locked;
use crate::engine::Engine;
use crate::pipeline::SIGN_IN_SCREEN;
use crate::session::SessionState;

/// What the close warning lists: PHI-free sentences, empty when nothing is pending
/// (REQ-OPS-013, §6.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuitWarnings(pub Vec<String>);

impl QuitWarnings {
    /// True when the app may close without asking.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Engine {
    /// What is still pending, for the close and quit warning (§6.3): active analyzer
    /// listeners, receipts being received, outbox items per topic, and the running job.
    /// PHI-free: instrument names, counts and job kinds.
    pub async fn quit_warnings(&self) -> QuitWarnings {
        let statuses = self.listeners.status_snapshot();
        // Without names the warning still lists each active listener.
        let names = self
            .db
            .read(lis_interfaces::instrument_names)
            .await
            .unwrap_or_default();
        let name_of = |id| {
            names
                .iter()
                .find(|(instrument, _)| *instrument == id)
                .map_or("An analyzer", |(_, name)| name.as_str())
        };
        let mut warnings = Vec::new();
        for status in &statuses {
            match status.state {
                ListenerState::Receiving => warnings.push(format!(
                    "{} is sending results now.",
                    name_of(status.instrument_id)
                )),
                ListenerState::Listening | ListenerState::Connected => warnings.push(format!(
                    "{} is listening for results.",
                    name_of(status.instrument_id)
                )),
                ListenerState::Disabled | ListenerState::Error | ListenerState::ManualOnly => {}
            }
        }
        let now = self.clock.now();
        let backlog = match self
            .db
            .read(move |conn| lis_store::outbox::backlog(conn, now))
            .await
        {
            Ok(backlog) => backlog,
            Err(error) => {
                tracing::warn!(%error, "outbox backlog unreadable for the close warning");
                warnings.push("Waiting work could not be checked.".to_owned());
                Vec::new()
            }
        };
        for topic in backlog.into_iter().filter(|topic| topic.outstanding > 0) {
            warnings.push(format!(
                "{} waiting: {}.",
                topic_label(&topic.topic),
                topic.outstanding
            ));
        }
        if let Some(job) = *locked(&self.ops.running) {
            warnings.push(job.label().to_owned());
        }
        QuitWarnings(warnings)
    }

    /// Records `ops.app_stop` as `quit_with_warnings`, once, when the user chose "Quit
    /// anyway" (§6.3); shutdown then records nothing more.
    pub async fn record_quit_with_warnings(&self) {
        self.record_stop(StopKind::QuitWithWarnings).await;
    }

    /// Records the run's stop once, attributed to the signed-in user or to the sign-in
    /// screen.
    pub(crate) async fn record_stop(&self, kind: StopKind) {
        if self.ops.stopped.swap(true, Ordering::SeqCst) {
            return;
        }
        let principal = match *self.session_state() {
            SessionState::Active(session) | SessionState::Locked(session) => {
                self.user_principal(session, [])
            }
            SessionState::None => Principal::system(SIGN_IN_SCREEN, []),
        };
        let run_id = self.ops.run_id;
        let stopped: Result<(), RuntimeError> = self
            .db
            .write(principal, CorrelationId::generate(), move |ctx| {
                lis_ops::runs::stop_run(ctx, run_id, kind)
            })
            .await;
        match stopped {
            Ok(()) => tracing::info!(stop_kind = kind.as_str(), "app run stopped"),
            Err(error) => tracing::error!(%error, "app run stop not recorded"),
        }
    }
}

/// PHI-free names of the outbox topics for the close warning.
fn topic_label(topic: &str) -> &str {
    match topic {
        lis_specimens::LABEL_PRINT_TOPIC => "Label print jobs",
        lis_interfaces::RECEIPT_COMPLETED_TOPIC => "Analyzer results to import",
        lis_results::RETRY_IMPORT_TOPIC => "Result imports to retry",
        other => other,
    }
}
