import { screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { renderWithProviders } from "@/lib/test/render";
import { Select } from "./Select";

const OPTIONS = [
  { id: "a", label: "Synthetic panel A" },
  { id: "b", label: "Synthetic panel B" },
];

function Harness() {
  const [value, setValue] = useState<string | null>(null);
  return (
    <>
      <Select label="Panel" name="panel" options={OPTIONS} value={value} onChange={setValue} />
      <output>{value ?? "none"}</output>
    </>
  );
}

describe("Select", () => {
  it("chooses an option with the keyboard", async () => {
    const { user, container } = renderWithProviders(<Harness />);
    await expectNoA11yViolations(container);
    await user.tab();
    await user.keyboard("{ArrowDown}");
    await user.click(await screen.findByRole("option", { name: "Synthetic panel B" }));
    expect(screen.getByText("b", { selector: "output" })).toBeTruthy();
  });
});
