// The M1 session contract (REQ-IDN-004, NFR-19). The engine owns the session; the UI holds
// no token. Types are generated from the engine into `@/generated`. Command names:
// setup_status, create_first_administrator, sign_in, change_initial_password, verify_mfa,
// current_session, touch_session, lock_session, unlock_session, sign_out, change_password.

/** `setup_status() → SetupStatus`; refreshed after the first administrator is created. */
export const SETUP_STATUS = "setup_status";

// Restore onto a new installation and installation-key custody (M4 design brief §3.7, §3.9).
// Public commands. The restore flow itself belongs to the operations feature (`restoreSlot.ts`).

/**
 * `get_restore_availability() → RestoreAvailability`: whether a restore is offered (only while
 * no user account exists), a confirmed restore applied at the next start, and why the last one
 * could not be applied.
 */
export const GET_RESTORE_AVAILABILITY = "get_restore_availability";

/** `get_installation_key_state() → InstallationKeyStateView`. */
export const GET_INSTALLATION_KEY_STATE = "get_installation_key_state";
/**
 * `restore_installation_key({input:{installationKey}}) → InstallationKeyStateView`. A key that
 * does not open this installation's data is refused on `installationKey` (`does_not_open`).
 */
export const RESTORE_INSTALLATION_KEY = "restore_installation_key";
