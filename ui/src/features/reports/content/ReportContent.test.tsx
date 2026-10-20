import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { CONTENT } from "../testData";
import { ReportContent } from "./ReportContent";

describe("preserved report content", () => {
  it("shows generation semantics, exact values and historical performing provenance", async () => {
    const { container } = render(<ReportContent content={CONTENT} />);
    expect(screen.getByText("Generated")).toBeTruthy();
    expect(screen.getByText("Prepared for release by")).toBeTruthy();
    expect(screen.getByText("512.4")).toBeTruthy();
    expect(
      screen.getAllByText(/Performing location: Synthetic Toxicology Laboratory/),
    ).toHaveLength(2);
    expect(screen.getAllByText(/First reported: This revision/)).toHaveLength(2);
    await expectNoA11yViolations(container);
  });

  it("preserves the labels of historical v2 data", () => {
    const old = structuredClone(CONTENT);
    old.report.generatedAt = null;
    render(<ReportContent content={old} />);
    expect(screen.getByText("Issued")).toBeTruthy();
    expect(screen.getByText("Released by")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Performing laboratory" })).toBeTruthy();
  });

  it("shows correction identification and approved reference content as plain text", () => {
    const data = structuredClone(CONTENT);
    data.report.changeSummary = ["Synthetic amphetamine: quantity changed", "Patient identity"];
    const group = data.resultGroups[0];
    const row = group?.rows[0];
    if (!group || !row) throw new Error("Synthetic report fixture is incomplete");
    group.reportNotes = "Synthetic approved limitation <script>";
    row.referenceInterval = "Synthetic interval, not cutoff";
    row.firstReportedAt = "2026-10-11 12:00 CDT";
    render(<ReportContent content={data} />);
    expect(screen.getByRole("region", { name: "Changes in this correction" })).toBeTruthy();
    expect(screen.getByText("Synthetic amphetamine: quantity changed")).toBeTruthy();
    expect(
      screen.getByText("Method information: Synthetic approved limitation <script>"),
    ).toBeTruthy();
    expect(screen.getByText("Reference information: Synthetic interval, not cutoff")).toBeTruthy();
    expect(screen.getByText(/First reported: 2026-10-11 12:00 CDT/)).toBeTruthy();
  });
});
