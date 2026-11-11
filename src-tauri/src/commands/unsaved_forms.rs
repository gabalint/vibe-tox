//! The shell-only `set_unsaved_forms` command: the UI reports the page names of its full-page
//! forms with unsaved input, so closing the app can name them (ticket #4585). Shell state
//! only, so not an engine command: no clinical state changes and nothing is recorded.

use tauri::State;

use crate::quit::UnsavedForms;

/// Longest page name kept; the UI's names are short page titles.
const MAX_NAME: usize = 80;
/// Most names kept; the UI shows one page at a time.
const MAX_FORMS: usize = 10;

#[tauri::command]
pub(crate) fn set_unsaved_forms(state: State<'_, UnsavedForms>, forms: Vec<String>) {
    state.set(
        forms
            .into_iter()
            .take(MAX_FORMS)
            .map(|name| name.chars().take(MAX_NAME).collect())
            .collect(),
    );
}
