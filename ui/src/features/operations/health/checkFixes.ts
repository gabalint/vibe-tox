/** Where a check's next action is done in the app: the screen, its link text and who may act there. */
export interface CheckFix {
  path: string;
  place: string;
  /** The capability the screen's action needs; people without it see the action as text only. */
  capability: string;
}

const BACKUPS: CheckFix = {
  path: "/operations/backups",
  place: "Backups",
  capability: "backup_restore",
};
const BACKUP_TARGETS: CheckFix = {
  ...BACKUPS,
  path: "/operations/backups/targets",
  place: "Backup targets",
};
const BACKUP_RUNS: CheckFix = {
  ...BACKUPS,
  path: "/operations/backups/runs",
  place: "Backup runs",
};

/**
 * The checks whose next action is done on one screen of the app, by check code. Audit anchors
 * go to a target whose purpose includes anchors, which is added on Backup targets. Not linked,
 * because the action is done outside the app or follows a procedure: database integrity, free
 * disk space, disk encryption, operating system support, web engine, sleep prevention, unclean
 * shutdowns, clock, power protection and cloud services; and pending work, which has no screen.
 */
const FIXES: Readonly<Record<string, CheckFix>> = {
  backup_configured: BACKUP_TARGETS,
  // Back up now is on every Backups tab; the targets come first.
  backup_age: BACKUPS,
  offhost_copy_age: BACKUPS,
  offline_copy_age: BACKUPS,
  backup_failures: BACKUP_RUNS,
  // A run's panel counts the stored-file problems it found.
  artifact_store: BACKUP_RUNS,
  restore_test_age: {
    ...BACKUPS,
    path: "/operations/backups/restore-tests",
    place: "Restore tests",
  },
  anchor_lag: BACKUP_TARGETS,
  recovery_kit: { ...BACKUPS, path: "/operations/recovery-kit", place: "Recovery kit" },
  audit_chain: { path: "/audit/anchors", place: "Audit anchors", capability: "view_audit" },
  listeners: {
    path: "/configuration/instruments",
    place: "Instruments",
    capability: "manage_configuration",
  },
  // A run with a gap is acknowledged from the application runs.
  listening_gaps: {
    path: "/operations/health/history",
    place: "Health history",
    capability: "perform_analysis",
  },
  version: {
    path: "/operations/versions",
    place: "Versions and updates",
    capability: "approve_configuration",
  },
  recovery_reconciliation: {
    path: "/operations/recovery",
    place: "Recovery reconciliation",
    capability: "manage_quality",
  },
};

/** Where the check's next action is done in the app; `null` when it is done elsewhere. */
export function checkFix(check: string): CheckFix | null {
  return FIXES[check] ?? null;
}
