import { useQueryClient } from "@tanstack/react-query";
import { screen, waitFor } from "@testing-library/react";
import { FormProvider, useForm } from "react-hook-form";
import { expect, it, vi } from "vitest";
import { Button } from "@/components/Button";
import { Form } from "@/components/Form";
import { TEST_SESSION } from "@/features/auth/testSession";
import { AuthenticationField } from "./securityKeyAuthenticationField";
import { mockCommands } from "./test/ipc";
import { renderWithProviders } from "./test/render";

function Confirmation({ onConfirm }: { onConfirm: (values: { password: string }) => void }) {
  const client = useQueryClient();
  const form = useForm({ defaultValues: { password: "" } });
  return (
    <FormProvider {...form}>
      <Form
        onSubmit={form.handleSubmit(onConfirm)}
        actions={<Button type="submit">Confirm</Button>}
      >
        <AuthenticationField />
        <Button
          onPress={() =>
            client.setQueryData(["get_authentication_status"], {
              method: "security_key",
              credentials: [{ credentialId: "synthetic-key", label: "Everyday key", revision: 1 }],
              recoveryAvailable: true,
            })
          }
        >
          Refresh changed account
        </Button>
      </Form>
    </FormProvider>
  );
}

it("removes an old password validation rule when the account changes to a security key", async () => {
  mockCommands({ current_session: () => TEST_SESSION });
  const confirm = vi.fn();
  const { user } = renderWithProviders(<Confirmation onConfirm={confirm} />);
  await screen.findByLabelText(/^Your password/);
  await user.click(screen.getByRole("button", { name: "Confirm" }));
  expect(await screen.findByText("Enter your password.")).toBeTruthy();
  await user.click(screen.getByRole("button", { name: "Refresh changed account" }));
  await screen.findByRole("button", { name: "Everyday key Security key" });
  expect(screen.queryByLabelText(/^Your password/)).toBeNull();
  await user.click(screen.getByRole("button", { name: "Confirm" }));
  await waitFor(() => expect(confirm).toHaveBeenCalledOnce());
  expect(confirm.mock.calls[0]?.[0]).toEqual({ password: "" });
});
