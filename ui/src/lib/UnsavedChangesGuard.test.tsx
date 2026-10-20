import { screen } from "@testing-library/react";
import { useState } from "react";
import { Link } from "react-router";
import { describe, expect, it } from "vitest";
import { TextField } from "@/components/TextField";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { renderRoutes } from "@/lib/test/router";
import { UnsavedChangesGuard } from "./UnsavedChangesGuard";

function OrderEntry() {
  const [code, setCode] = useState("");
  return (
    <>
      <TextField label="Placer order number" name="code" value={code} onChange={setCode} />
      <Link to="/elsewhere">Work queues</Link>
      <UnsavedChangesGuard when={code !== ""} />
    </>
  );
}

const renderEntry = () =>
  renderRoutes([{ path: "/orders/new", element: <OrderEntry /> }], "/orders/new");

describe("UnsavedChangesGuard", () => {
  it("asks before leaving a form with typed input, and stays on Cancel", async () => {
    const { user } = renderEntry();
    await user.type(screen.getByRole("textbox", { name: "Placer order number" }), "SYN-PO-1");
    await user.click(screen.getByRole("link", { name: "Work queues" }));
    const dialog = screen.getByRole("alertdialog", { name: "Leave without saving?" });
    await expectNoA11yViolations(dialog);
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(screen.getByRole("textbox", { name: "Placer order number" })).toBeTruthy();
    expect(screen.queryByText(/Navigated to/)).toBeNull();
  });

  it("leaves once confirmed", async () => {
    const { user } = renderEntry();
    await user.type(screen.getByRole("textbox", { name: "Placer order number" }), "SYN-PO-1");
    await user.click(screen.getByRole("link", { name: "Work queues" }));
    await user.click(screen.getByRole("button", { name: "Leave without saving" }));
    expect(await screen.findByText("Navigated to /elsewhere")).toBeTruthy();
  });

  it("does not ask when nothing is typed", async () => {
    const { user } = renderEntry();
    await user.click(screen.getByRole("link", { name: "Work queues" }));
    expect(await screen.findByText("Navigated to /elsewhere")).toBeTruthy();
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });
});
