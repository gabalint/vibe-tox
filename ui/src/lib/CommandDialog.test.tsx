import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { TextInput } from "@/components/FormFields";
import { CommandDialog } from "./CommandDialog";
import { engineError, mockCommands } from "./test/ipc";
import { renderWithProviders } from "./test/render";

interface Values {
  name: string;
}

function renderDialog(props: Partial<Parameters<typeof CommandDialog<Values, Values, string>>[0]>) {
  const onClose = vi.fn();
  const result = renderWithProviders(
    <CommandDialog<Values, Values, string>
      title="Add synthetic client"
      submitLabel="Add client"
      command={{ name: "create_client", invalidates: [] }}
      defaultValues={{ name: "" }}
      toInput={(values) => ({ name: values.name.trim() })}
      successTitle="Recorded"
      onClose={onClose}
      {...props}
    >
      <TextInput name="name" label="Name" isRequired />
    </CommandDialog>,
  );
  return { ...result, onClose };
}

describe("CommandDialog", () => {
  it("words the notice from the values and the engine's answer", async () => {
    mockCommands({ create_client: () => "client-1" });
    const onDone = vi.fn();
    const { user, onClose } = renderDialog({
      successTitle: (values, id) => `Client ${values.name.trim()} added as ${id}`,
      onDone,
    });
    await user.type(screen.getByRole("textbox", { name: /Name/ }), "Synthetic West");
    await user.click(screen.getByRole("button", { name: "Add client" }));
    expect(await screen.findByText("Client Synthetic West added as client-1")).toBeTruthy();
    expect(onDone).toHaveBeenCalledWith("client-1");
    expect(onClose).toHaveBeenCalled();
  });

  it("titles the engine's refusal with the screen's wording", async () => {
    mockCommands({
      create_client: () => Promise.reject(engineError("conflict", "The code is taken.")),
    });
    const { user, onClose } = renderDialog({ errorTitle: "Not saved" });
    await user.type(screen.getByRole("textbox", { name: /Name/ }), "Synthetic West");
    await user.click(screen.getByRole("button", { name: "Add client" }));
    expect(await screen.findByText("Not saved")).toBeTruthy();
    expect(screen.getByText("The code is taken.")).toBeTruthy();
    expect(onClose).not.toHaveBeenCalled();
  });
});
