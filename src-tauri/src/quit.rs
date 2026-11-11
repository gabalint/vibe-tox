//! Closing the main window or quitting (REQ-OPS-013, §6.3): the shell asks the engine what
//! is still in progress and combines it with the full-page forms the UI reports as unsaved
//! ([`UnsavedForms`], ticket #4585). If either is non-empty, a native dialog off the main
//! thread asks first, with Cancel in the default position so Enter keeps the app open.
//! Quitting with work in progress is recorded; unsaved forms are UI state and are not.
//! A second close or quit request while the dialog is open, such as an OS shutdown, is
//! never blocked.

use std::sync::{Mutex, PoisonError};

use lis_engine::Engine;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{
    DialogExt, MessageDialogButtons, MessageDialogKind, MessageDialogResult,
};

/// Where a close or quit request stands; one prompt at a time.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Quit {
    /// No request is being handled.
    #[default]
    Idle,
    /// The pending work is being checked or the dialog is open.
    Prompting,
    /// The app may exit.
    Confirmed,
}

/// The shell's quit state, managed for the event handlers.
#[derive(Debug, Default)]
pub(crate) struct QuitState(Mutex<Quit>);

impl QuitState {
    fn get(&self) -> Quit {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn set(&self, quit: Quit) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = quit;
    }
}

/// The page names of the UI's full-page forms with unsaved input, such as "New order", as
/// the UI last reported them. PHI-free: names of pages, never what was typed.
#[derive(Debug, Default)]
pub(crate) struct UnsavedForms(Mutex<Vec<String>>);

impl UnsavedForms {
    pub(crate) fn set(&self, forms: Vec<String>) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = forms;
    }

    fn get(&self) -> Vec<String> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// The close warning: its title, text and the label of the button that leaves.
#[derive(Debug, PartialEq, Eq)]
struct QuitPrompt {
    title: &'static str,
    text: String,
    leave: &'static str,
}

/// The warning for these unsaved forms and work in progress, or `None` when the app may
/// close at once.
fn quit_prompt(unsaved: &[String], in_progress: &[String]) -> Option<QuitPrompt> {
    let unsaved_text = format!("You have unsaved changes in {}.", unsaved.join(", "));
    let in_progress_text = format!(
        "These are still in progress:\n\n• {}\n\nQuitting now may leave results or copies \
         unfinished.",
        in_progress.join("\n• ")
    );
    match (unsaved.is_empty(), in_progress.is_empty()) {
        (true, true) => None,
        (false, true) => Some(QuitPrompt {
            title: "Leave without saving?",
            text: format!("{unsaved_text} Leave without saving?"),
            leave: "Leave",
        }),
        (true, false) => Some(QuitPrompt {
            title: "Quit Vibe Toxicology LIS?",
            text: in_progress_text,
            leave: "Quit anyway",
        }),
        (false, false) => Some(QuitPrompt {
            title: "Quit Vibe Toxicology LIS?",
            text: format!("{unsaved_text} What you typed is lost.\n\n{in_progress_text}"),
            leave: "Quit anyway",
        }),
    }
}

/// Lets the next close or quit request go ahead without the warning: a switch of
/// installations or an update checked the pending work itself.
pub(crate) fn confirm_exit<R: tauri::Runtime>(app: &AppHandle<R>) {
    if let Some(quit) = app.try_state::<QuitState>() {
        quit.set(Quit::Confirmed);
    }
}

/// Whether a close or quit request may go ahead now. The first request starts the check
/// and is held; a request while the check or dialog is open (for example an OS shutdown)
/// goes ahead and is recorded as a quit with warnings.
pub(crate) fn may_exit(app: &AppHandle) -> bool {
    let (Some(engine), Some(quit)) = (app.try_state::<Engine>(), app.try_state::<QuitState>())
    else {
        return true;
    };
    match quit.get() {
        Quit::Confirmed => true,
        Quit::Prompting => {
            quit.set(Quit::Confirmed);
            tauri::async_runtime::block_on(engine.record_quit_with_warnings());
            true
        }
        Quit::Idle => {
            quit.set(Quit::Prompting);
            let app = app.clone();
            tauri::async_runtime::spawn(async move { confirm_quit(app).await });
            false
        }
    }
}

/// Exits at once when nothing is unsaved or pending; otherwise asks, off the main thread.
async fn confirm_quit(app: AppHandle) {
    let Some(engine) = app.try_state::<Engine>() else {
        return;
    };
    let warnings = engine.quit_warnings().await;
    let unsaved = app
        .try_state::<UnsavedForms>()
        .map(|forms| forms.get())
        .unwrap_or_default();
    let quit = match quit_prompt(&unsaved, &warnings.0) {
        None => true,
        Some(prompt) => {
            let chosen = ask(&app, prompt).await;
            if chosen && !warnings.is_empty() {
                engine.record_quit_with_warnings().await;
            }
            chosen
        }
    };
    let Some(state) = app.try_state::<QuitState>() else {
        return;
    };
    if state.get() == Quit::Confirmed {
        // An OS shutdown already went ahead while the dialog was open.
        return;
    }
    if quit {
        state.set(Quit::Confirmed);
        app.exit(0);
    } else {
        state.set(Quit::Idle);
    }
}

/// Shows the warning and whether the leave button was pressed. The dialog cannot choose its
/// default button, so Cancel takes the default (OK) position and leaving is the second
/// button; closing the dialog keeps the app open.
async fn ask(app: &AppHandle, prompt: QuitPrompt) -> bool {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .message(prompt.text)
            .title(prompt.title)
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Cancel".to_owned(),
                prompt.leave.to_owned(),
            ))
            .blocking_show_with_result()
    })
    .await
    .is_ok_and(|result| result == MessageDialogResult::Custom(prompt.leave.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(names: &[&str]) -> Vec<String> {
        names.iter().map(|&name| name.to_owned()).collect()
    }

    #[test]
    fn nothing_unsaved_or_pending_closes_at_once() {
        assert_eq!(quit_prompt(&[], &[]), None);
    }

    #[test]
    fn unsaved_forms_alone_ask_to_leave_without_saving() {
        let prompt = quit_prompt(&names(&["New order", "Register patient"]), &[]).unwrap();
        assert_eq!(prompt.title, "Leave without saving?");
        assert_eq!(
            prompt.text,
            "You have unsaved changes in New order, Register patient. Leave without saving?"
        );
        assert_eq!(prompt.leave, "Leave");
    }

    #[test]
    fn work_in_progress_alone_keeps_the_quit_warning() {
        let prompt = quit_prompt(&[], &names(&["Analyzer A is listening"])).unwrap();
        assert_eq!(prompt.title, "Quit Vibe Toxicology LIS?");
        assert_eq!(
            prompt.text,
            "These are still in progress:\n\n• Analyzer A is listening\n\nQuitting now may \
             leave results or copies unfinished."
        );
        assert_eq!(prompt.leave, "Quit anyway");
    }

    #[test]
    fn both_are_listed_in_one_warning() {
        let prompt = quit_prompt(
            &names(&["New order"]),
            &names(&["Analyzer A is listening", "A backup is running"]),
        )
        .unwrap();
        assert_eq!(prompt.title, "Quit Vibe Toxicology LIS?");
        assert_eq!(
            prompt.text,
            "You have unsaved changes in New order. What you typed is lost.\n\nThese are \
             still in progress:\n\n• Analyzer A is listening\n• A backup is running\n\n\
             Quitting now may leave results or copies unfinished."
        );
        assert_eq!(prompt.leave, "Quit anyway");
    }
}
