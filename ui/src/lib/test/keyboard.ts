import { screen, waitFor, within } from "@testing-library/react";
import type { UserEvent } from "@testing-library/user-event";
import { expect } from "vitest";

/**
 * Time allowed for a test that walks a whole form by keyboard: each key press is a separate
 * render, so these tests run longer than the default on a busy machine.
 */
export const WALKTHROUGH_TIMEOUT_MS = 30_000;

/** Presses Tab until `target` has focus, as a keyboard-only user would (NFR-09). */
export async function tabTo(user: UserEvent, target: Element, maxTabs = 200) {
  for (let presses = 0; presses < maxTabs; presses += 1) {
    if (document.activeElement === target) return;
    await user.tab();
  }
  throw new Error("Tab never reached the element");
}

/**
 * Chooses `option` in the select whose trigger is labelled `label`, by keyboard only: Tab to
 * the trigger, open it with the down arrow, move to the option and press Enter.
 */
export async function chooseWithKeyboard(user: UserEvent, label: string | RegExp, option: string) {
  await tabTo(user, screen.getByRole("button", { name: label }));
  await user.keyboard("{ArrowDown}");
  const listbox = await screen.findByRole("listbox");
  // Opening moves focus into the list; wait for it rather than racing the next key.
  await waitFor(() => expect(listbox.contains(document.activeElement)).toBe(true));
  const target = within(listbox).getByRole("option", { name: option });
  for (let presses = 0; presses < 50; presses += 1) {
    if (document.activeElement === target) {
      await user.keyboard("{Enter}");
      await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());
      return;
    }
    await user.keyboard("{ArrowDown}");
  }
  throw new Error(`the keyboard never reached option ${option}`);
}

/** Tabs to the field labelled `label` and types `text` into it. */
export async function typeInto(user: UserEvent, label: string | RegExp, text: string) {
  await tabTo(user, screen.getByRole("textbox", { name: label }));
  await user.keyboard(text);
}
