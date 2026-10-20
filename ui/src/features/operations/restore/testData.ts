import { vi } from "vitest";
import type { AnchorSummary } from "@/generated/AnchorSummary";
import type { RestoreSessionView } from "@/generated/RestoreSessionView";
import type { RestoreSourceView } from "@/generated/RestoreSourceView";
import type { StagedRestoreView } from "@/generated/StagedRestoreView";
import { mockCommands } from "@/lib/test/ipc";
import { backupSet, SET_ID } from "../testData";

/** Synthetic restore records for the restore tests. No real keys, paths or laboratory data. */

export const KEY = "SYN0-TEST-KEY0-0000-0000-0000";
export const SESSION_ID = "00000000-0000-7000-8000-0000000000c1";
export const FINGERPRINT = "5Y7H-0000-TEST-0001";
const OTHER_SET_ID = "20261012T140000Z-0000b2aa";

const SESSION: RestoreSessionView = { restoreSessionId: SESSION_ID, keyFingerprint: FINGERPRINT };

const SOURCE: RestoreSourceView = {
  installationId: "00000000-0000-7000-8000-0000000000c2",
  sets: [
    backupSet(),
    backupSet({
      setId: OTHER_SET_ID,
      createdAt: "2026-10-12T14:00:00Z",
      appVersion: "0.9.0",
      compatible: false,
      incompatibleReason: "newer app version",
    }),
  ],
};

export const OTHER_DRIVE: AnchorSummary = {
  compared: 12,
  matched: 12,
  lostInterval: 0,
  unverifiedRestoreBoundary: 0,
  mismatched: 0,
  localEventsMissing: 0,
  conflicts: 0,
  gaps: 0,
  unanchored: 0,
};

export function staged(overrides: Partial<StagedRestoreView> = {}): StagedRestoreView {
  return {
    setId: SET_ID,
    setCreatedAt: "2026-10-13T14:00:00Z",
    appVersion: "0.4.0",
    databaseBytes: 52_428_800,
    artifactCount: 12,
    integrityOk: true,
    chains: [
      { stream: "clinical", events: 1204, intact: true, headMatchesManifest: true },
      { stream: "security", events: 311, intact: true, headMatchesManifest: true },
    ],
    anchors: null,
    lostIntervalUntil: "2026-10-13T15:30:00Z",
    findings: [{ code: "lost_interval", count: 2 }],
    canConfirm: true,
    ...overrides,
  };
}

type Handler = (args: unknown) => unknown;

/**
 * Answers the restore commands along the main path; `overrides` replace single answers. Returns
 * the handlers, so tests can check what was sent.
 */
export function restoreEngine(overrides: Record<string, Handler> = {}) {
  const handlers: Record<string, ReturnType<typeof vi.fn<Handler>>> = {
    begin_restore: vi.fn<Handler>(() => SESSION),
    choose_restore_source: vi.fn<Handler>(() => SOURCE),
    stage_restore: vi.fn<Handler>(() => staged()),
    check_restore_anchors: vi.fn<Handler>(() => OTHER_DRIVE),
    confirm_restore: vi.fn<Handler>(() => ({ closing: true })),
    cancel_restore: vi.fn<Handler>(() => null),
  };
  for (const [command, handler] of Object.entries(overrides)) {
    handlers[command] = vi.fn<Handler>(handler);
  }
  mockCommands(handlers);
  return handlers;
}
