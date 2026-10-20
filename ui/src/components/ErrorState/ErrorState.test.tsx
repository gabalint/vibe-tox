import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { ErrorState } from "./ErrorState";

describe("ErrorState", () => {
  it("shows the message, the support reference and a retry", async () => {
    const onRetry = vi.fn();
    const error = engineError("internal", "The app could not complete the action.", {
      correlationId: "corr-42",
    });
    const { user, container } = renderWithProviders(<ErrorState error={error} onRetry={onRetry} />);
    const alert = screen.getByRole("alert");
    expect(alert.textContent).toContain("The app could not complete the action.");
    expect(alert.textContent).toContain("Support reference: corr-42");
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(onRetry).toHaveBeenCalledOnce();
    await expectNoA11yViolations(container);
  });

  it("lists the gates that blocked the action", async () => {
    const error = engineError(
      "gate_blocked",
      "This action is blocked by one or more review gates.",
      {
        gateReasons: [
          { code: "qc_not_accepted", message: "Run QC is not accepted." },
          { code: "identity_hold", message: "The patient has an open identity hold." },
          { code: "qc_conflict", message: "A changed QC row for this analyte awaits a decision." },
        ],
      },
    );
    const { container } = renderWithProviders(<ErrorState error={error} />);
    const gates = screen.getByRole("list", { name: "Blocked by" });
    expect(gates.textContent).toContain("Run QC is not accepted.");
    expect(gates.textContent).toContain("The patient has an open identity hold.");
    // A known code leads with its plain label; an unknown one shows the message alone.
    expect(gates.textContent).toContain(
      "Changed QC row awaits a decision: A changed QC row for this analyte awaits a decision.",
    );
    expect(gates.textContent).not.toContain("identity_hold");
    await expectNoA11yViolations(container);
  });

  it("omits an empty support reference", () => {
    renderWithProviders(
      <ErrorState error={engineError("unavailable", "Not available.", { correlationId: "" })} />,
    );
    expect(screen.getByRole("alert").textContent).not.toContain("Support reference");
  });

  it("offers no retry when the roles do not allow the work, and says who can help", async () => {
    const onRetry = vi.fn();
    const { container } = renderWithProviders(
      <ErrorState
        error={engineError("forbidden", "You do not have permission for this action.")}
        onRetry={onRetry}
      />,
    );
    const alert = screen.getByRole("alert");
    expect(alert.textContent).toContain(
      "Your roles do not include this work. A laboratory administrator can change roles.",
    );
    expect(screen.queryByRole("button", { name: "Try again" })).toBeNull();
    await expectNoA11yViolations(container);
  });
});
