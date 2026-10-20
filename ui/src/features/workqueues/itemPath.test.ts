import { describe, expect, it } from "vitest";
import { itemTarget } from "./itemPath";

const ID = "00000000-0000-7000-8000-0000000000a1";
const OTHER = "00000000-0000-7000-8000-0000000000a2";

describe("itemTarget (REQ-OPS-004)", () => {
  it.each([
    ["order", ID, `/orders/${ID}`, ID],
    ["subject", ID, `/patients/${ID}`, ID],
    ["accession", ID, `/accessions/${ID}`, ID],
    ["collection_review", ID, `/accessions/${ID}/collection-review`, ID],
    ["review_specimen", ID, `/review/specimens/${ID}`, ID],
    ["specimen_report", ID, `/reports/specimens/${ID}`, ID],
    // The run's QC assessments and positions open the item's own analyte or position.
    ["run_analyte_qc", `${ID}:${OTHER}`, `/bench/runs/${ID}/qc?open=${OTHER}`, ID],
    ["run_position", `${ID}:${OTHER}`, `/bench/runs/${ID}?open=${OTHER}`, ID],
    ["run_position", ID, `/bench/runs/${ID}`, ID],
    ["import_attempt", ID, `/review/imports/${ID}`, ID],
    ["report_revision", ID, `/reports/revisions/${ID}`, ID],
    // Under "All open", so a failed, blocked or paused obligation shows too.
    ["obligation", ID, `/delivery?filter=all&open=${ID}`, ID],
    ["delivery_destination", ID, `/configuration/destinations?open=${ID}`, ID],
    ["preservation_incident", `${ID}:${OTHER}`, `/reports/${ID}?incidentId=${OTHER}`, ID],
    [
      "preservation_incident",
      `${ID}:revision:${OTHER}`,
      `/reports/${ID}?preservationRevisionId=${OTHER}`,
      ID,
    ],
    ["deviation", ID, `/quality/deviations/${ID}`, ID],
    ["billing_handoff_item", `${ID}:${OTHER}`, `/billing/handoffs/${ID}?open=${OTHER}`, ID],
    ["billing_derivation", ID, `/billing/derivations/${ID}`, ID],
    ["health_check", "disk_space", "/operations/health?open=disk_space", "disk_space"],
    ["app_run", ID, `/operations/health/history?open=${ID}`, ID],
    ["recovery_reconciliation", ID, "/operations/recovery", ID],
    ["app_version", "1.4.0", "/operations/versions?open=1.4.0", "1.4.0"],
  ])("opens a %s item %s at its exact record", (itemKind, itemId, path, recordId) => {
    expect(itemTarget({ itemKind, itemId, state: "open" })).toEqual({ path, recordId });
  });

  it.each(["2026-10-13", ""])("opens the exact patient/date billing group %s", (date) => {
    const itemId = `${ID}|${date}`;
    const query = new URLSearchParams({
      filter: "needs_derivation",
      open: itemId,
      subjectId: ID,
      dateOfService: date,
    });
    expect(itemTarget({ itemKind: "billing_subject", itemId, state: "needs_derivation" })).toEqual({
      path: `/billing/work?${query}`,
      recordId: itemId,
    });
  });

  it("encodes identifiers in the address", () => {
    expect(itemTarget({ itemKind: "health_check", itemId: "a b/c?d", state: "x" })?.path).toBe(
      "/operations/health?open=a%20b%2Fc%3Fd",
    );
    expect(itemTarget({ itemKind: "order", itemId: "a/b", state: "x" })?.path).toBe(
      "/orders/a%2Fb",
    );
  });

  it.each(["label_job", "specimen", "outbox_message", "toString"])(
    "has no screen for an item of kind %s",
    (itemKind) => {
      expect(itemTarget({ itemKind, itemId: ID, state: "open" })).toBeNull();
    },
  );
});
