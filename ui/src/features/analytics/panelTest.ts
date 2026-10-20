import { screen, waitFor, within } from "@testing-library/react";
import type { UserEvent } from "@testing-library/user-event";
import { expect } from "vitest";
import { openRowActions } from "@/lib/test/grid";

/** The side panel named by its identifier (its `h2`). */
export const panel = (identifier: string) => screen.getByRole("region", { name: identifier });

/** Escape in the open panel closes it, and focus returns to the row holding `rowText`. */
export async function closePanelWithEscape(user: UserEvent, identifier: string, rowText: string) {
  await user.click(within(panel(identifier)).getByRole("heading", { level: 2 }));
  await user.keyboard("{Escape}");
  await waitFor(() => expect(screen.queryByRole("region", { name: identifier })).toBeNull());
  await waitFor(() =>
    expect(document.activeElement?.closest("[role=row]")?.textContent).toContain(rowText),
  );
}

/** The labels of a row's actions menu, which is closed again. */
export async function rowActionLabels(user: UserEvent, rowIdentifier: string) {
  const menu = await openRowActions(user, rowIdentifier);
  const labels = within(menu)
    .getAllByRole("menuitem")
    .map((item) => item.textContent);
  await user.keyboard("{Escape}");
  return labels;
}
