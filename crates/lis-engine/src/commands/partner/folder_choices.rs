//! Folders chosen in the native dialog, kept in memory for the session that
//! chose them, at most [`MAX_CHOICES`]; the oldest goes first. The UI holds
//! only the handle, so it never sends a path the engine would write to.

use std::sync::{Mutex, MutexGuard, PoisonError};

use lis_core::{Id, SessionId};
use lis_interfaces::partner::MovedToChoice;

use super::connections::refused;
use crate::commands::sign_in::session_of;
use crate::engine::Engine;
use crate::pipeline::{Call, Failure};

/// Folders one session may hold chosen at a time.
const MAX_CHOICES: usize = 8;

/// Marker for one folder chosen in the dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderChoiceMarker {}
/// The handle of a chosen folder.
pub(crate) type FolderChoiceId = Id<FolderChoiceMarker>;

struct Choice {
    handle: FolderChoiceId,
    session: SessionId,
    folder: String,
}

/// The chosen folders.
#[derive(Default)]
pub(crate) struct FolderChoices(Mutex<Vec<Choice>>);

impl FolderChoices {
    fn choices(&self) -> MutexGuard<'_, Vec<Choice>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Keeps `folder` for `session`, dropping other sessions' choices and the
    /// session's oldest beyond the limit.
    pub(super) fn insert(&self, session: SessionId, folder: String) -> FolderChoiceId {
        let mut choices = self.choices();
        choices.retain(|choice| choice.session == session);
        if choices.len() >= MAX_CHOICES {
            choices.remove(0);
        }
        let handle = FolderChoiceId::generate();
        choices.push(Choice {
            handle,
            session,
            folder,
        });
        handle
    }

    /// The folder behind `handle`, if `session` chose it.
    pub(super) fn get(&self, session: SessionId, handle: FolderChoiceId) -> Option<String> {
        self.choices()
            .iter()
            .find(|choice| choice.handle == handle && choice.session == session)
            .map(|choice| choice.folder.clone())
    }
}

impl Engine {
    /// The folder behind `handle`, chosen in this session; `None` without a handle.
    pub(super) fn chosen_folder(
        &self,
        call: &Call,
        handle: Option<FolderChoiceId>,
    ) -> Result<Option<String>, Failure> {
        self.chosen_folder_as(call, handle, "folderChoice")
    }

    /// The Processed and Failed folders chosen in this session, each `None` without a handle.
    pub(super) fn chosen_moved_to(
        &self,
        call: &Call,
        processed: Option<FolderChoiceId>,
        failed: Option<FolderChoiceId>,
    ) -> Result<MovedToChoice, Failure> {
        Ok(MovedToChoice {
            processed: self.chosen_folder_as(call, processed, "processedFolderChoice")?,
            failed: self.chosen_folder_as(call, failed, "failedFolderChoice")?,
        })
    }

    fn chosen_folder_as(
        &self,
        call: &Call,
        handle: Option<FolderChoiceId>,
        field: &str,
    ) -> Result<Option<String>, Failure> {
        let Some(handle) = handle else {
            return Ok(None);
        };
        let (_, session) = session_of(&call.principal)?;
        self.folder_choices
            .get(session, handle)
            .map(Some)
            .ok_or_else(|| refused(field, "expired", "Choose the folder again."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_choice_belongs_to_its_session() {
        let choices = FolderChoices::default();
        let (mine, other) = (SessionId::generate(), SessionId::generate());
        let handle = choices.insert(mine, "/srv/out".to_owned());
        assert_eq!(choices.get(mine, handle).as_deref(), Some("/srv/out"));
        assert_eq!(choices.get(other, handle), None);
        choices.insert(other, "/srv/other".to_owned());
        assert_eq!(choices.get(mine, handle), None);
    }

    #[test]
    fn the_oldest_choice_goes_first() {
        let choices = FolderChoices::default();
        let session = SessionId::generate();
        let first = choices.insert(session, "/0".to_owned());
        for n in 1..=MAX_CHOICES {
            choices.insert(session, format!("/{n}"));
        }
        assert_eq!(choices.get(session, first), None);
    }
}
