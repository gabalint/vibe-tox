import { screen, within } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { renderWithProviders } from "@/lib/test/render";
import { DatePickerButton, TimePickerButton } from "./FieldPicker";

function DateHarness({ latest }: { latest?: string }) {
  const [value, setValue] = useState("");
  return (
    <>
      <DatePickerButton label="Choose the date" latest={latest} onPick={setValue} />
      <output>{value}</output>
    </>
  );
}

function TimeHarness() {
  const [value, setValue] = useState("");
  return (
    <>
      <TimePickerButton label="Choose the time" value={value} onPick={setValue} />
      <output>{value}</output>
    </>
  );
}

const today = new Date();
const month = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, "0")}`;
const dayName = (day: number) =>
  new RegExp(
    new Date(today.getFullYear(), today.getMonth(), day).toLocaleDateString("en-US", {
      month: "long",
      day: "numeric",
      year: "numeric",
    }),
  );

describe("FieldPicker", () => {
  it("fills the picked day as YYYY-MM-DD and refuses days after the latest", async () => {
    const { user, container } = renderWithProviders(<DateHarness latest={`${month}-01`} />);
    await user.click(screen.getByRole("button", { name: "Choose the date" }));
    const grid = await screen.findByRole("grid");
    // Day 2 is after the latest day, so it can't be picked.
    expect(
      within(grid)
        .getByRole("button", { name: dayName(2) })
        .getAttribute("aria-disabled"),
    ).toBe("true");
    await user.click(within(grid).getByRole("button", { name: dayName(1) }));
    expect(container.querySelector("output")?.textContent).toBe(`${month}-01`);
    expect(screen.queryByRole("grid")).toBeNull();
  });

  it("jumps from days to months to years and back down to a day", async () => {
    const { user, container } = renderWithProviders(<DateHarness />);
    await user.click(screen.getByRole("button", { name: "Choose the date" }));
    await user.click(await screen.findByRole("button", { name: /choose the month$/ }));
    expect(screen.getByRole("listbox", { name: "Month" })).toBeTruthy();
    await user.click(
      screen.getByRole("button", { name: `${today.getFullYear()}, choose the year` }),
    );
    const years = screen.getByRole("listbox", { name: "Year" });
    expect(within(years).getAllByRole("option")).toHaveLength(10);
    while (
      !within(screen.getByRole("listbox", { name: "Year" })).queryByRole("option", { name: "1990" })
    ) {
      await user.click(screen.getByRole("button", { name: "Previous decade" }));
    }
    await user.click(screen.getByRole("option", { name: "1990" }));
    expect(screen.getByRole("button", { name: "1990, choose the year" })).toBeTruthy();
    await user.click(screen.getByRole("option", { name: "Jan" }));
    await user.click(screen.getByRole("button", { name: /January 1, 1990/ }));
    expect(container.querySelector("output")?.textContent).toBe("1990-01-01");
  });

  it("disables the months and years after the latest day", async () => {
    const { user } = renderWithProviders(<DateHarness latest={`${month}-01`} />);
    await user.click(screen.getByRole("button", { name: "Choose the date" }));
    await user.click(await screen.findByRole("button", { name: /choose the month$/ }));
    const thisMonth = screen.getAllByRole("option")[today.getMonth()];
    expect(thisMonth?.getAttribute("aria-disabled")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Next year" }));
    expect(screen.getByRole("option", { name: "Jan" }).getAttribute("aria-disabled")).toBe("true");
    await user.click(screen.getByRole("button", { name: /choose the year$/ }));
    const next = String(today.getFullYear() + 1);
    const nextYear = screen.queryByRole("option", { name: next });
    expect(nextYear?.getAttribute("aria-disabled") ?? "true").toBe("true");
    expect(
      screen
        .getByRole("option", { name: String(today.getFullYear()) })
        .getAttribute("aria-disabled"),
    ).toBeNull();
  });

  it("moves through months by keyboard, and Escape goes back to the days", async () => {
    const { user } = renderWithProviders(<DateHarness />);
    await user.click(screen.getByRole("button", { name: "Choose the date" }));
    await user.click(await screen.findByRole("button", { name: /choose the month$/ }));
    await user.keyboard("{Home}{ArrowRight}{Enter}");
    expect(await screen.findByRole("grid", { name: /February/ })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: /choose the month$/ }));
    await expectNoA11yViolations(document.body);
    await user.keyboard("{Escape}");
    expect(await screen.findByRole("grid", { name: /February/ })).toBeTruthy();
  });

  it("fills hours then minutes as HH:MM, 24-hour", async () => {
    const { user, container } = renderWithProviders(<TimeHarness />);
    await user.click(screen.getByRole("button", { name: "Choose the time" }));
    await user.click(
      within(await screen.findByRole("listbox", { name: "Hour" })).getByRole("option", {
        name: "17",
      }),
    );
    expect(container.querySelector("output")?.textContent).toBe("17:00");
    await user.click(
      within(screen.getByRole("listbox", { name: "Minute" })).getByRole("option", { name: "45" }),
    );
    expect(container.querySelector("output")?.textContent).toBe("17:45");
    expect(screen.queryByRole("listbox", { name: "Minute" })).toBeNull();
  });

  it("is accessible when open", async () => {
    const { user } = renderWithProviders(<TimeHarness />);
    await user.click(screen.getByRole("button", { name: "Choose the time" }));
    await screen.findByRole("listbox", { name: "Hour" });
    await expectNoA11yViolations(document.body);
  });
});
