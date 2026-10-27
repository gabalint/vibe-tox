// Operations (REQ-OPS-002, 003, 006, 007, 008, 009, 010, 013): command names of
// docs/planning/m4-contract.md. Every type is generated from `lis-engine` and `lis-ops` into
// `@/generated`. The engine decides every state, due time, lag, gate and health result. "Dialog" commands open the OS dialog in Rust and answer `null` when it
// is cancelled: the UI never sends a path (D10).
import type { CommandName } from "@/generated/CommandName";

export const BACKUP_RESTORE = "backup_restore";
export const EXPORT = "export";
export const MANAGE_CONFIGURATION = "manage_configuration";
export const APPROVE_CONFIGURATION = "approve_configuration";
export const MANAGE_QUALITY = "manage_quality";
export const MANAGE_USERS = "manage_users";
export const PERFORM_ANALYSIS = "perform_analysis";

export const LIST_LIMIT = 100;
/**
 * Page size for `list_app_runs` (1–200). History uses a stable cursor; queue links
 * retrieve their exact run independently of the loaded history pages.
 */
export const APP_RUNS_LIMIT = 200;

// Backups.
export const LIST_BACKUP_TARGETS = "list_backup_targets";
export const ADD_BACKUP_TARGET = "add_backup_target"; // folder dialog
export const RETIRE_BACKUP_TARGET = "retire_backup_target";
export const GET_BACKUP_STATUS = "get_backup_status";
export const REQUEST_BACKUP = "request_backup";
export const LIST_BACKUP_RUNS = "list_backup_runs";
export const LIST_BACKUP_SETS = "list_backup_sets";
export const REQUEST_RESTORE_TEST = "request_restore_test";
export const LIST_RESTORE_TESTS = "list_restore_tests";
export const RECORD_MEDIA_DISPOSAL = "record_media_disposal";
export const LIST_MEDIA_DISPOSALS = "list_media_disposals";

// Recovery kit.
export const GET_RECOVERY_KIT_STATUS = "get_recovery_kit_status";
export const EXPORT_RECOVERY_KIT = "export_recovery_kit"; // save dialog
export const CONFIRM_RECOVERY_KIT_STORAGE = "confirm_recovery_kit_storage";

// Restore onto a new installation (public, only while no user exists).
export const BEGIN_RESTORE = "begin_restore";
export const CHOOSE_RESTORE_SOURCE = "choose_restore_source"; // folder dialog
export const STAGE_RESTORE = "stage_restore"; // long-running, awaited
export const CHECK_RESTORE_ANCHORS = "check_restore_anchors"; // folder dialog
export const CONFIRM_RESTORE = "confirm_restore";
export const CANCEL_RESTORE = "cancel_restore";

// Recovery reconciliation.
export const GET_RECOVERY_RECONCILIATION = "get_recovery_reconciliation"; // any session
export const CLOSE_RECOVERY_RECONCILIATION = "close_recovery_reconciliation";
export const RAISE_NUMBER_FLOORS = "raise_number_floors";

// Exit export.
export const REQUEST_EXIT_EXPORT = "request_exit_export"; // folder dialog
export const LIST_EXIT_EXPORTS = "list_exit_exports";

// Health and runs (any session unless noted).
export const GET_HOST_HEALTH = "get_host_health";
export const REFRESH_HOST_HEALTH = "refresh_host_health"; // the engine allows one call per minute
/** The gate reason of a refresh refused because the previous one was under a minute ago. */
export const REFRESH_RATE_LIMITED = "refresh_rate_limited";
export const LIST_HEALTH_HISTORY = "list_health_history";
export const ATTEST_HEALTH_CHECK = "attest_health_check";
export const LIST_APP_RUNS = "list_app_runs";
export const ACKNOWLEDGE_LISTENING_GAP = "acknowledge_listening_gap";

// Settings.
export const GET_OPERATIONS_SETTINGS = "get_operations_settings";
export const SET_OPERATIONS_SETTINGS = "set_operations_settings";
export const GET_SECURITY_POLICY = "get_security_policy";
export const SET_SECURITY_POLICY = "set_security_policy";
/** Which duties one person may do alone (ADR 0015); both need `manage_quality`. */
export const GET_SEPARATION_POLICY = "get_separation_policy";
export const SET_SEPARATION_POLICY = "set_separation_policy";

// Updates.
export const LIST_INSTALLED_VERSIONS = "list_installed_versions";
export const LIST_UPDATE_APPROVALS = "list_update_approvals";
export const RECORD_UPDATE_APPROVAL = "record_update_approval"; // file dialog
export const WITHDRAW_UPDATE_APPROVAL = "withdraw_update_approval";
export const ACCEPT_INSTALLED_VERSION = "accept_installed_version";
/** In-app updates (ADR 0020): the shell contacts the release repository after the engine allows it. */
export const CHECK_RELEASE_UPDATE = "check_release_update";
export const LIST_NIGHTLY_BUILDS = "list_nightly_builds";
export const DOWNLOAD_UPDATE = "download_update";
export const APPROVE_DOWNLOADED_UPDATE = "approve_downloaded_update";
export const INSTALL_UPDATE = "install_update";

export const STARTUP_STATUS = "startup_status"; // shell-only: answered even when the engine failed to start

/** What each change makes stale: health and the work queues follow most operations records. */
const HEALTH_AND_QUEUES: readonly CommandName[] = [GET_HOST_HEALTH, "list_work_queues"];
export const BACKUP_QUERIES: readonly CommandName[] = [
  LIST_BACKUP_TARGETS,
  GET_BACKUP_STATUS,
  LIST_BACKUP_RUNS,
  LIST_BACKUP_SETS,
  ...HEALTH_AND_QUEUES,
];
export const RESTORE_TEST_QUERIES: readonly CommandName[] = [
  LIST_RESTORE_TESTS,
  ...HEALTH_AND_QUEUES,
];
export const KIT_QUERIES: readonly CommandName[] = [GET_RECOVERY_KIT_STATUS, ...HEALTH_AND_QUEUES];
export const EXIT_EXPORT_QUERIES: readonly CommandName[] = [LIST_EXIT_EXPORTS];
export const DISPOSAL_QUERIES: readonly CommandName[] = [LIST_MEDIA_DISPOSALS];
export const RECONCILIATION_QUERIES: readonly CommandName[] = [
  GET_RECOVERY_RECONCILIATION,
  ...HEALTH_AND_QUEUES,
];
export const HEALTH_QUERIES: readonly CommandName[] = [LIST_HEALTH_HISTORY, ...HEALTH_AND_QUEUES];
export const APP_RUN_QUERIES: readonly CommandName[] = [LIST_APP_RUNS, ...HEALTH_AND_QUEUES];
export const SETTINGS_QUERIES: readonly CommandName[] = [GET_OPERATIONS_SETTINGS, GET_HOST_HEALTH];
export const VERSION_QUERIES: readonly CommandName[] = [
  LIST_INSTALLED_VERSIONS,
  LIST_UPDATE_APPROVALS,
  ...HEALTH_AND_QUEUES,
];

// Engine types the frame and the updates screen still import from here.
export type { BackupRunView } from "@/generated/BackupRunView";
export type { RecoveryReconciliation } from "@/generated/RecoveryReconciliation";
