import { screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { renderWithProviders } from "@/lib/test/render";
import { FieldList } from "./FieldList";

function Harness() {
  const [items, setItems] = useState<{ id: string }[]>([]);
  return (
    <FieldList
      legend="Diagnoses"
      emptyText="No diagnoses on the requisition."
      items={items}
      itemLabel={(index) => `Diagnosis ${index + 1}`}
      addLabel="Add diagnosis"
      onAdd={() => setItems((rows) => [...rows, { id: `row-${rows.length}` }])}
      onRemove={(index) => setItems((rows) => rows.filter((_, each) => each !== index))}
      renderItem={(index) => <p>Fields of row {index + 1}</p>}
    />
  );
}

describe("FieldList", () => {
  it("starts empty, adds and removes rows", async () => {
    const { user, container } = renderWithProviders(<Harness />);
    expect(screen.getByText("No diagnoses on the requisition.")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Add diagnosis" }));
    await user.click(screen.getByRole("button", { name: "Add diagnosis" }));
    expect(screen.getByRole("group", { name: "Diagnosis 2" })).toBeTruthy();
    await expectNoA11yViolations(container);
    await user.click(screen.getByRole("button", { name: "Remove diagnosis 1" }));
    expect(screen.queryByRole("group", { name: "Diagnosis 2" })).toBeNull();
    expect(screen.getByText("Fields of row 1")).toBeTruthy();
  });
});
