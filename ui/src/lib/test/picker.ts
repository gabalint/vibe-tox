import { screen, waitFor } from "@testing-library/react";
import type { UserEvent } from "@testing-library/user-event";
import { expect } from "vitest";
import { tabTo } from "./keyboard";

/**
 * Chooses a record in the `RecordPicker` labelled `label` by keyboard only, as a keyboard
 * user would (NFR-09): Tab to the field, type `text` (enough to make the wanted record the
 * first match), then the down arrow and Enter. Waits for the list to close.
 */
export async function pickRecord(user: UserEvent, label: string | RegExp, text: string) {
  await tabTo(user, screen.getByRole("combobox", { name: label }));
  await user.keyboard(text);
  await screen.findByRole("listbox");
  await user.keyboard("{ArrowDown}{Enter}");
  await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());
}
