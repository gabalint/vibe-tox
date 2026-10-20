import { render, screen, within } from "@testing-library/react";
import { expect, it } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { IntakeSteps } from "./IntakeSteps";

it("announces task position without treating earlier steps as clinical approval", async () => {
  const { container } = render(<IntakeSteps current="receive" />);
  const steps = within(screen.getByRole("list", { name: "Intake progress" })).getAllByRole(
    "listitem",
  );
  expect(steps).toHaveLength(4);
  expect(steps.filter((step) => step.getAttribute("aria-current") === "step")).toHaveLength(1);
  expect(screen.getByText("Receive specimens").getAttribute("aria-current")).toBe("step");
  expect(screen.queryByText(/accepted|approved|complete/i)).toBeNull();
  await expectNoA11yViolations(container);
});
