import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { AuditEventChanges } from "@/generated/AuditEventChanges";
import type { AuditEventView } from "@/generated/AuditEventView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { AuditChanges } from "./AuditChanges";

/** A synthetic clinical event on a synthetic patient record. */
const EVENT: AuditEventView = {
  stream: "clinical",
  seq: 42,
  occurredAt: "2026-11-05T10:00:00Z",
  recordedAt: "2026-11-05T10:00:00Z",
  principalKind: "user",
  principalId: "00000000-0000-7000-8000-000000000101",
  principalDisplay: "Test Quality Manager",
  action: "identity.assert_demographics",
  outcome: "succeeded",
  recordType: "subject",
  recordId: "00000000-0000-7000-8000-000000000201",
  reason: "Typo in the requisition",
  authority: "manage_quality",
  correlationId: "corr-42",
};

function changes(extra: Partial<AuditEventChanges>): AuditEventChanges {
  return {
    status: "changed",
    oldRevision: 1,
    newRevision: 2,
    patient: null,
    fields: [],
    facts: [],
    readCapabilities: [],
    ...extra,
  };
}

function renderChanges(result: AuditEventChanges | Promise<never>) {
  const get = vi.fn(() => result);
  mockCommands({ get_audit_event_changes: get });
  return { get, ...renderWithProviders(<AuditChanges event={EVENT} />) };
}

describe("AuditChanges (REQ-OPS-001)", () => {
  it("shows the patient and each changed field's old and new value", async () => {
    const { get, container } = renderChanges(
      changes({
        patient: {
          subjectId: "00000000-0000-7000-8000-000000000201",
          name: "Doe, Jane",
          birthDate: "1980-01-01",
          identifiers: ["MR (TEST): MRN-00000"],
        },
        fields: [
          { field: "Family name", old: "Dae", new: "Doe" },
          { field: "Given names", old: null, new: "Jane" },
        ],
        facts: [["Assertion sequence", "2"]],
      }),
    );
    expect(await screen.findByRole("row", { name: "Family name Dae Doe" })).toBeTruthy();
    expect(get).toHaveBeenCalledWith({ input: { stream: "clinical", seq: 42 } });
    expect(screen.getByRole("heading", { name: "Changes" })).toBeTruthy();
    expect(screen.getByText("Doe, Jane")).toBeTruthy();
    expect(screen.getByText("MR (TEST): MRN-00000")).toBeTruthy();
    expect(screen.getByRole("row", { name: "Given names — Jane" })).toBeTruthy();
    expect(screen.getByRole("columnheader", { name: "Old" })).toBeTruthy();
    expect(screen.getByRole("row", { name: "Revision 1 2" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Recorded details" })).toBeTruthy();
    expect(screen.getByText("Assertion sequence")).toBeTruthy();
    await expectNoA11yViolations(container);
  });

  it("names the changed fields without values when the roles cannot read the patient", async () => {
    renderChanges(
      changes({
        status: "hidden",
        fields: [{ field: "Birth date", old: null, new: null }],
        readCapabilities: ["enter_orders", "manage_quality"],
      }),
    );
    expect(await screen.findByText(/which your roles cannot read/)).toBeTruthy();
    expect(screen.getByText(/Enter orders or Manage quality/)).toBeTruthy();
    expect(screen.getByText("Birth date")).toBeTruthy();
    expect(screen.getByText("Hidden")).toBeTruthy();
    expect(screen.queryByText("Patient")).toBeNull();
  });

  it("says when a revision changed no value, and when an event changed no data", async () => {
    renderChanges(changes({ status: "unchanged_revision", oldRevision: 3, newRevision: 4 }));
    expect(await screen.findByText(/changed no field value/)).toBeTruthy();
    expect(screen.getByRole("row", { name: "Revision 3 4" })).toBeTruthy();
  });

  it("says no data changed for an event without a revision", async () => {
    renderChanges(changes({ status: "no_change", oldRevision: null, newRevision: null }));
    expect(await screen.findByText("No data changed.")).toBeTruthy();
    expect(screen.queryByText("Revision")).toBeNull();
  });

  it("says when the record type keeps no history of values", async () => {
    renderChanges(changes({ status: "values_not_kept" }));
    expect(await screen.findByText(/not kept in a history/)).toBeTruthy();
  });

  it("shows why the changes could not be read", async () => {
    renderChanges(
      Promise.reject(engineError("forbidden", "You do not have permission for this action.")),
    );
    expect(await screen.findByText("You do not have permission for this action.")).toBeTruthy();
  });
});
