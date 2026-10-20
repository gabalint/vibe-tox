import type { CheckCode } from "@/generated/CheckCode";
import type { ChecklistItem } from "@/generated/ChecklistItem";

// Type guards for text that reaches the screen as a plain string (an address parameter, a
// form value) but must go back to the engine as one of its codes. Each record lists every
// code, so adding a code in the engine fails to compile here until it is listed.

const CHECK_CODES: Readonly<Record<CheckCode, true>> = {
  backup_configured: true,
  backup_age: true,
  backup_failures: true,
  offhost_copy_age: true,
  offline_copy_age: true,
  restore_test_age: true,
  recovery_kit: true,
  anchor_lag: true,
  audit_chain: true,
  artifact_store: true,
  database_integrity: true,
  disk_free: true,
  disk_encryption: true,
  os_support: true,
  webview: true,
  listeners: true,
  keep_awake: true,
  listening_gaps: true,
  unclean_shutdowns: true,
  clock: true,
  outbox: true,
  version: true,
  recovery_reconciliation: true,
  ups: true,
  cloud_services: true,
};

const CHECKLIST_ITEMS: Readonly<Record<ChecklistItem, true>> = {
  production_computer: true,
  lost_interval_listed: true,
  numbers_checked: true,
  analyzer_results_resent: true,
  reports_reconciled: true,
  orders_reentered: true,
  billing_reconciled: true,
  accounts_reapplied: true,
  configuration_reapplied: true,
  anchors_reviewed: true,
  labels_waiting: true,
};

export function isCheckCode(value: string | null): value is CheckCode {
  return value !== null && Object.hasOwn(CHECK_CODES, value);
}

export function isChecklistItem(value: string): value is ChecklistItem {
  return Object.hasOwn(CHECKLIST_ITEMS, value);
}
