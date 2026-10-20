import { screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it } from "vitest";
import { Button } from "@/components/Button";
import type { CommandError } from "@/generated/CommandError";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { asksSelfApproval, SelfApprovalConfirm } from "./SelfApprovalConfirm";

const MESSAGE =
  "You wrote this configuration. Your laboratory allows you to approve it yourself: confirm that no second person reviewed it.";

const ASK = engineError("validation", "Check the highlighted fields.", {
  fieldErrors: [{ field: "selfApproval", code: "self_approval_unconfirmed", message: MESSAGE }],
});
const WRONG_PASSWORD = engineError("validation", "Check the highlighted fields.", {
  fieldErrors: [{ field: "password", code: "incorrect", message: "Incorrect password." }],
});
const REFUSED = engineError("gate_blocked", "Your laboratory requires someone else to approve it.");

/** A dialog's part: each button stands for one answer of the engine. */
function Harness() {
  const [error, setError] = useState<CommandError | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  return (
    <>
      <Button onPress={() => setError(ASK)}>Engine asks</Button>
      <Button onPress={() => setError(WRONG_PASSWORD)}>Wrong password</Button>
      <SelfApprovalConfirm error={error} isConfirmed={confirmed} onChange={setConfirmed} />
      <output>{confirmed ? "confirmed" : "not confirmed"}</output>
    </>
  );
}

describe("SelfApprovalConfirm (ADR 0015)", () => {
  it("appears only when the engine asks, unticked and focused, and stays for the retry", async () => {
    const { user, container } = renderWithProviders(<Harness />);
    expect(screen.queryByRole("checkbox")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Engine asks" }));
    const checkbox = screen.getByRole("checkbox", {
      name: "I confirm that no second person reviewed this",
    });
    expect(screen.getByRole("alert").textContent).toContain(MESSAGE);
    expect(checkbox).toHaveProperty("checked", false);
    expect(document.activeElement).toBe(checkbox);
    await expectNoA11yViolations(container);
    await user.keyboard(" ");
    expect(screen.getByText("confirmed")).toBeTruthy();
    // A later answer, such as a wrong password, keeps the confirmation and its words.
    await user.click(screen.getByRole("button", { name: "Wrong password" }));
    expect(screen.getByRole("checkbox")).toHaveProperty("checked", true);
    expect(screen.getByText(MESSAGE)).toBeTruthy();
  });

  it("tells a request for confirmation from other answers", () => {
    expect(asksSelfApproval(ASK)).toBe(true);
    expect(asksSelfApproval(WRONG_PASSWORD)).toBe(false);
    expect(asksSelfApproval(REFUSED)).toBe(false);
    expect(asksSelfApproval(null)).toBe(false);
  });
});
