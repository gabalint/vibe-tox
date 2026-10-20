import { screen, waitFor, within } from "@testing-library/react";
import type { ReactElement } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SessionInfo } from "@/generated/SessionInfo";
import type { SubjectSummary } from "@/generated/SubjectSummary";
import type { SubjectView } from "@/generated/SubjectView";
import { IdentityBanner } from "./IdentityBanner";
import { expectNoA11yViolations } from "./test/a11y";
import { engineError, mockCommands } from "./test/ipc";
import { renderRoutes } from "./test/router";

const SUBJECT_ID = "00000000-0000-7000-8000-000000000901";

/** A synthetic signed-in session holding `capabilities`. */
const sessionWith = (capabilities: string[]): SessionInfo => ({
  userId: "00000000-0000-7000-8000-000000000904",
  username: "synthetic.user",
  displayName: "Synthetic User",
  roles: [],
  capabilities,
  idleTimeoutSeconds: 900,
  locked: false,
});

const identifier = (identifierType: string, assigningAuthority: string, value: string) => ({
  id: `00000000-0000-7000-8000-${value.replace(/\D/g, "").padStart(12, "0")}`,
  assigningAuthority,
  identifierType,
  value,
  active: true,
  addedAt: "2026-10-12T14:00:00Z",
  deactivatedAt: null,
  deactivationReason: null,
});

/** A synthetic patient; no real person. */
const SUBJECT: SubjectSummary = {
  subjectId: SUBJECT_ID,
  revision: 3,
  familyName: "Synthetic-Example",
  givenNames: "Avery Jo",
  birthDate: "1961-11-09",
  administrativeSex: null,
  identifiers: [identifier("PI", "SYN-LAB", "PI-0042"), identifier("MR", "SYN-CLINIC", "MR-0007")],
  openIdentityHolds: 0,
};

const VIEW: SubjectView = {
  id: SUBJECT_ID,
  revision: 3,
  assertions: [
    {
      id: "00000000-0000-7000-8000-000000000902",
      sequence: 1,
      familyName: "Synthetic-Example",
      givenNames: "Avery Jo",
      birthDate: "1961-11-09",
      administrativeSex: null,
      source: "Synthetic requisition",
      reason: null,
      assertedAt: "2026-10-12T14:00:00Z",
      assertedByKind: "user",
      assertedById: "00000000-0000-7000-8000-000000000903",
    },
  ],
  identifiers: [
    identifier("MR", "SYN-CLINIC", "MR-0007"),
    { ...identifier("MR", "SYN-OLD", "MR-0001"), active: false },
  ],
  openHolds: [],
};

const renderBanner = (banner: ReactElement) =>
  renderRoutes([{ path: "/orders/1", element: banner }], "/orders/1");
const region = (name = "Patient identity") => screen.getByRole("region", { name });
const facts = (name?: string) =>
  within(region(name))
    .getAllByRole("term")
    .map((term) => `${term.textContent}: ${term.nextElementSibling?.textContent}`);

describe("IdentityBanner", () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date("2026-10-13T15:00:00Z"));
  });
  afterEach(() => vi.useRealTimers());

  it("shows the name, birth date with age and identifiers, the MRN in bold", async () => {
    mockCommands({ current_session: () => sessionWith(["enter_orders"]) });
    const { container } = renderBanner(<IdentityBanner subject={SUBJECT} />);
    expect(facts()).toEqual([
      "Patient: Synthetic-Example, Avery Jo",
      "Date of birth: Nov 9, 1961 (64 y)",
      "PI (SYN-LAB): PI-0042",
      "MRN (SYN-CLINIC): MR-0007",
    ]);
    // The family name is shown in capitals by style only; the stored text is unchanged.
    const link = await screen.findByRole("link", { name: "Synthetic-Example, Avery Jo" });
    expect(link.getAttribute("href")).toBe(`/patients/${SUBJECT_ID}`);
    expect(screen.getByText("MR-0007").tagName).toBe("STRONG");
    expect(screen.getByText("PI-0042").tagName).toBe("DD");
    expect(within(region()).queryByText(/identity hold/i)).toBeNull();
    await expectNoA11yViolations(container);
  });

  it("shows the name as plain text to a session that cannot open patients", async () => {
    const calls: string[] = [];
    mockCommands({
      current_session: () => {
        calls.push("current_session");
        return sessionWith(["review_results"]);
      },
    });
    renderBanner(<IdentityBanner subject={SUBJECT} />);
    await waitFor(() => expect(calls).toEqual(["current_session"]));
    expect(within(region()).getByText("Synthetic-Example")).toBeTruthy();
    expect(screen.queryByRole("link")).toBeNull();
  });

  it("names open identity holds and possible matches", () => {
    renderBanner(<IdentityBanner subject={{ ...SUBJECT, openIdentityHolds: 2 }} />);
    expect(within(region()).getByText("2 identity holds")).toBeTruthy();
  });

  it("adds the accession and container label on specimen screens", () => {
    renderBanner(
      <IdentityBanner
        subject={{ ...SUBJECT, identifiers: [], openIdentityHolds: 1 }}
        specimen={{ accessionNumber: "SYN-ACC-0001", labelCode: null }}
      />,
    );
    expect(facts("Patient and specimen identity").slice(2)).toEqual([
      "Identifiers: None recorded",
      "Accession: SYN-ACC-0001",
      "Container: Not recorded",
    ]);
    expect(screen.getByText("Identity hold")).toBeTruthy();
  });

  it("loads the patient by ID, with its active identifiers only", async () => {
    const calls: unknown[] = [];
    mockCommands({
      get_subject: (args) => {
        calls.push(args);
        return VIEW;
      },
    });
    renderBanner(<IdentityBanner subjectId={SUBJECT_ID} specimen={{ accessionNumber: "A-1" }} />);
    const name = "Patient and specimen identity";
    expect(within(region(name)).getByText("Loading patient…")).toBeTruthy();
    expect(await screen.findByText("MR-0007")).toBeTruthy();
    expect(screen.queryByText("MR-0001")).toBeNull();
    expect(facts(name).at(-1)).toBe("Accession: A-1");
    expect(calls).toEqual([{ input: { subjectId: SUBJECT_ID } }]);
  });

  it("shows a failed load inside the banner, with retry", async () => {
    mockCommands({ get_subject: () => Promise.reject(engineError("unavailable", "Busy.")) });
    renderBanner(<IdentityBanner subjectId={SUBJECT_ID} />);
    expect(await within(region()).findByText("The patient could not be loaded")).toBeTruthy();
  });
});
