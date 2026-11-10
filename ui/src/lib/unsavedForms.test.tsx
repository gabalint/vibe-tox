import { render, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { mockCommands } from "@/lib/test/ipc";
import { unsavedFormNames, useUnsavedForm } from "./unsavedForms";

function Form({ name, when }: { name: string; when: boolean }) {
  useUnsavedForm(name, when);
  return null;
}

describe("unsaved forms", () => {
  it("lists each unsaved form once and tells the shell as it changes", async () => {
    const published: unknown[] = [];
    mockCommands({
      set_unsaved_forms: (args) => {
        published.push((args as { forms: string[] }).forms);
        return null;
      },
    });
    const { rerender, unmount } = render(
      <>
        <Form name="Register patient" when />
        <Form name="Register patient" when />
        <Form name="New order" when={false} />
      </>,
    );
    expect(unsavedFormNames()).toEqual(["Register patient"]);
    rerender(
      <>
        <Form name="Register patient" when />
        <Form name="Register patient" when />
        <Form name="New order" when />
      </>,
    );
    expect(unsavedFormNames()).toEqual(["Register patient", "New order"]);
    unmount();
    expect(unsavedFormNames()).toEqual([]);
    await waitFor(() => expect(published.at(-1)).toEqual([]));
    expect(published).toContainEqual(["Register patient", "New order"]);
  });
});
