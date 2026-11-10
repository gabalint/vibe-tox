import { useEffect } from "react";
import { call } from "./ipc";

/** The open full-page forms with unsaved input, by page name, in the order they began. */
const unsaved = new Map<symbol, string>();

function publish() {
  // Shell state only: the close warning names these forms (ticket #4585). If the shell cannot
  // hear it, closing behaves as before.
  call("set_unsaved_forms", { forms: unsavedFormNames() }).catch(() => undefined);
}

/** The page names of the forms that would lose input now, such as `["New order"]`. */
export function unsavedFormNames(): string[] {
  return [...new Set(unsaved.values())];
}

/**
 * Lists the form as unsaved while `when` holds, for the sign-out and close warnings. Used by
 * `UnsavedChangesGuard`; leaving the page or saving removes it.
 */
export function useUnsavedForm(name: string, when: boolean) {
  useEffect(() => {
    if (!when) return;
    const key = Symbol(name);
    unsaved.set(key, name);
    publish();
    return () => {
      unsaved.delete(key);
      publish();
    };
  }, [name, when]);
}
