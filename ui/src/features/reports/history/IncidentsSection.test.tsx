import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderRoutes } from "@/lib/test/router";
import type { PreservationIncidentView } from "../contract";
import { REPORT_ID, REVISION_ID } from "../testData";
import { IncidentsSection } from "./IncidentsSection";

const incident: PreservationIncidentView = {
  incidentId: "synthetic-a",
  revision: 1,
  kind: "corrupt",
  record: "content_json",
  artifactSha256: "ab".repeat(32),
  reportId: REPORT_ID,
  revisionId: REVISION_ID,
  candidateId: null,
  withdrawalNotice: false,
  detectedAt: "2026-10-14T12:00:00Z",
  closure: null,
  allowedActions: [],
};
describe("Preservation incident targets", () => {
  it("focuses the exact incident instead of the first report incident", async () => {
    renderRoutes(
      [
        {
          path: "/reports/:reportId",
          element: (
            <IncidentsSection
              incidents={[incident, { ...incident, incidentId: "synthetic-b" }]}
              revisions={[]}
            />
          ),
        },
      ],
      `/reports/${REPORT_ID}?incidentId=synthetic-b`,
    );
    const selected = await screen.findByRole("listitem", {
      name: "Selected preservation incident",
    });
    expect(document.activeElement).toBe(selected);
    expect(selected.textContent).toContain("Selected incident.");
  });
  it("focuses a uniquely recorded incident for the queued damaged revision", async () => {
    renderRoutes(
      [
        {
          path: "/reports/:reportId",
          element: (
            <IncidentsSection
              incidents={[
                { ...incident, incidentId: "other", revisionId: "other-revision" },
                incident,
              ]}
              revisions={[]}
            />
          ),
        },
      ],
      `/reports/${REPORT_ID}?preservationRevisionId=${REVISION_ID}`,
    );
    const selected = await screen.findByRole("listitem", {
      name: "Selected preservation incident",
    });
    expect(document.activeElement).toBe(selected);
    expect(selected.textContent).toContain("Selected incident.");
  });
  it("requires review when more than one open incident matches the revision", async () => {
    renderRoutes(
      [
        {
          path: "/reports/:reportId",
          element: (
            <IncidentsSection
              incidents={[incident, { ...incident, incidentId: "second" }]}
              revisions={[]}
            />
          ),
        },
      ],
      `/reports/${REPORT_ID}?preservationRevisionId=${REVISION_ID}`,
    );
    expect(
      await screen.findByText("Multiple open incidents affect the requested revision"),
    ).toBeTruthy();
    expect(screen.queryByRole("listitem", { name: "Selected preservation incident" })).toBeNull();
  });
  it("explicitly reports a missing target without selecting another", async () => {
    renderRoutes(
      [
        {
          path: "/reports/:reportId",
          element: <IncidentsSection incidents={[incident]} revisions={[]} />,
        },
      ],
      `/reports/${REPORT_ID}?incidentId=not-on-report`,
    );
    expect(
      await screen.findByText("Requested preservation incident is not on this report"),
    ).toBeTruthy();
    expect(screen.queryByRole("listitem", { name: "Selected preservation incident" })).toBeNull();
  });
});
