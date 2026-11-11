//! One IPC command's entry in the registry.

use super::Access;

/// One IPC command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSpec {
    /// The Tauri command name.
    pub name: &'static str,
    /// `<module>.<verb>`, for log lines and denial events.
    pub action: &'static str,
    /// Who may call it.
    pub access: Access,
}

pub(crate) const fn spec(name: &'static str, action: &'static str, access: Access) -> CommandSpec {
    CommandSpec {
        name,
        action,
        access,
    }
}
