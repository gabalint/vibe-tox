import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Button } from "@/components/Button";
import { TextField } from "@/components/TextField";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { Form } from "./Form";

describe("Form", () => {
  it("submits", async () => {
    const onSubmit = vi.fn((event: { preventDefault: () => void }) => event.preventDefault());
    const { user, container } = renderWithProviders(
      <Form
        aria-label="Receive"
        stickyActions
        onSubmit={onSubmit}
        actions={<Button type="submit">Receive</Button>}
      >
        <TextField label="Accession" name="accession" value="SYN-1" onChange={() => undefined} />
      </Form>,
    );
    await user.click(screen.getByRole("button", { name: "Receive" }));
    expect(onSubmit).toHaveBeenCalledOnce();
    await expectNoA11yViolations(container);
  });

  it("shows an engine error and its field errors on the matching field", async () => {
    const error = engineError("validation", "Check the highlighted fields.", {
      fieldErrors: [
        { field: "accession", code: "duplicate", message: "This accession is already received." },
      ],
    });
    const { container } = renderWithProviders(
      <Form
        aria-label="Receive"
        onSubmit={() => undefined}
        error={error}
        errorTitle="Receipt failed"
        actions={<Button type="submit">Receive</Button>}
      >
        <TextField label="Accession" name="accession" value="SYN-1" onChange={() => undefined} />
      </Form>,
    );
    expect(screen.getByRole("alert").textContent).toContain("Receipt failed");
    const field = screen.getByRole("textbox", { name: "Accession" });
    expect(field.getAttribute("aria-invalid")).toBe("true");
    // Focus moves to the first invalid field.
    expect(document.activeElement).toBe(field);
    expect(screen.getByText("This accession is already received.")).toBeTruthy();
    await expectNoA11yViolations(container);
  });
});
