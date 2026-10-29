import type { WorkItemKind, WorkQueueItem } from "./contract";

/** The parts of an item that say where it opens. */
type ItemRef = Pick<WorkQueueItem, "itemKind" | "itemId" | "state">;

/** Where an item opens: the record's ID as its screen knows it, and the screen's address. */
interface ItemTarget {
  recordId: string;
  path: string;
}

/** The record part of a compound ID, `<record>:<part>`: the run of a QC item or position. */
const recordOf = (id: string) => id.split(":")[0] ?? id;

/** A record page, `<base>/<record><rest>`. */
const page =
  (base: string, rest = "") =>
  (item: ItemRef) => {
    const recordId = recordOf(item.itemId);
    return { recordId, path: `${base}/${encodeURIComponent(recordId)}${rest}` };
  };

/** A record in a list's side panel (`?open=`), on the screen that owns it. */
const panel = (path: string) => (item: ItemRef) => ({
  recordId: item.itemId,
  path: `${path}?open=${encodeURIComponent(item.itemId)}`,
});

/**
 * A part of a record in its page's side panel, `<base>/<record><rest>?open=<part>`, for an
 * item named `<record>:<part>`. The sequence walks the records, so items of one record share
 * one entry.
 */
const part =
  (base: string, rest = "") =>
  (item: ItemRef) => {
    const [recordId = item.itemId, partId] = item.itemId.split(":");
    const path = `${base}/${encodeURIComponent(recordId)}${rest}`;
    return { recordId, path: partId ? `${path}?${new URLSearchParams({ open: partId })}` : path };
  };

/**
 * An obligation in the delivery workspace's panel, under "All open": the delivery queues list
 * every status (awaiting, failed, blocked, paused, handed off), which one filter would miss.
 */
function obligation(item: ItemRef): ItemTarget {
  const query = new URLSearchParams({ filter: "all", open: item.itemId });
  return { recordId: item.itemId, path: `/delivery?${query}` };
}

/**
 * The exact record of each kind of item (REQ-OPS-004). Label print failures and specimens
 * awaiting a receipt decision open their accession when the engine names one (kind
 * `accession`); a label job or a specimen outside the review and release queues has no screen
 * of its own. Deviations have an exact record page independent of run scope.
 */
const TARGETS: Readonly<Record<WorkItemKind, (item: ItemRef) => ItemTarget>> = {
  deviation: page("/quality/deviations"),
  order: page("/orders"),
  subject: page("/patients"),
  accession: page("/accessions"),
  collection_review: page("/accessions", "/collection-review"),
  review_specimen: page("/review/specimens"),
  specimen_report: page("/reports/specimens"),
  // The engine names a QC item `<run>:<analyte>`; its run's QC assessments open the
  // analyte's assessment by the analyte ID.
  run_analyte_qc: part("/bench/runs", "/qc"),
  // The engine names a position awaiting results `<run>:<position>`; its run's positions open
  // it by the position ID.
  run_position: part("/bench/runs"),
  import_attempt: page("/review/imports"),
  report_revision: page("/reports/revisions"),
  obligation,
  delivery_destination: panel("/configuration/destinations"),
  // Known incidents retain their ID; newly discovered damage names the exact revision.
  preservation_incident: (item) => {
    const [reportId = item.itemId, kindOrIncident, revisionId] = item.itemId.split(":");
    const query =
      kindOrIncident === "revision" && revisionId
        ? new URLSearchParams({ preservationRevisionId: revisionId })
        : kindOrIncident
          ? new URLSearchParams({ incidentId: kindOrIncident })
          : null;
    return {
      recordId: reportId,
      path: `/reports/${encodeURIComponent(reportId)}${query ? `?${query}` : ""}`,
    };
  },
  billing_derivation: page("/billing/derivations"),
  // Patient/date work and handoff items retain their exact identities across page limits.
  billing_subject: billingWork,
  billing_handoff_item: part("/billing/handoffs"),
  // Operations (REQ-OPS-002, 006, 008): the engine names a check by its code.
  health_check: panel("/operations/health"),
  app_run: panel("/operations/health/history"),
  recovery_reconciliation: (item) => ({ recordId: item.itemId, path: "/operations/recovery" }),
  app_version: panel("/operations/versions/installed"),
};

function billingWork(item: ItemRef): ItemTarget {
  const filter = item.state === "approved" || item.state === "candidate" ? "ready" : item.state;
  const query = new URLSearchParams({ filter, open: item.itemId });
  if (item.itemKind === "billing_subject") {
    const [subjectId = item.itemId, dateOfService = ""] = item.itemId.split("|");
    query.set("subjectId", subjectId);
    query.set("dateOfService", dateOfService);
  }
  return { recordId: item.itemId, path: `/billing/work?${query}` };
}

function isKnownKind(kind: string): kind is WorkItemKind {
  return Object.hasOwn(TARGETS, kind);
}

/** Where opening `item` goes, or `null` when no screen shows it. */
export function itemTarget(item: ItemRef): ItemTarget | null {
  return isKnownKind(item.itemKind) ? TARGETS[item.itemKind](item) : null;
}
