import { useIsMutating, useQuery } from "@tanstack/react-query";
import { Controller, FormProvider, useForm } from "react-hook-form";
import { Button } from "@/components/Button";
import { Dialog } from "@/components/Dialog";
import { ErrorState } from "@/components/ErrorState";
import { Form } from "@/components/Form";
import { LoadingState } from "@/components/LoadingState";
import { TextField } from "@/components/TextField";
import type { SecurityKeyOperationInput } from "@/generated/SecurityKeyOperationInput";
import type { SecurityKeyPinInput } from "@/generated/SecurityKeyPinInput";
import { useCommandMutation } from "./commands";
import { textFieldProps } from "./form";
import { call } from "./ipc";
import type { SecurityKeyStatus } from "./securityKey";

/** One transient PIN prompt for all native ceremonies; no PIN enters query data or storage. */
export function SecurityKeyPrompt() {
  const pending = useIsMutating({ mutationKey: ["security-key"] }) > 0;
  return pending ? <PendingKeyPrompt /> : null;
}

function PendingKeyPrompt() {
  const status = useQuery({
    queryKey: ["security_key_status"],
    queryFn: () => call<SecurityKeyStatus | null>("security_key_status"),
    refetchInterval: 250,
    gcTime: 0,
  });
  const cancel = useCommandMutation<null, { input: SecurityKeyOperationInput }>(
    "security_key_cancel",
    { invalidates: [] },
  );
  const current = status.data;
  if (!current)
    return status.isError ? (
      <ErrorState
        title="The key prompt could not be loaded"
        error={status.error}
        onRetry={() => void status.refetch()}
      />
    ) : null;
  const onCancel = () => cancel.mutate({ input: { operationId: current.operationId } });
  return (
    <Dialog
      title="Verify with your security key"
      isOpen
      onOpenChange={(open) => {
        if (!open) onCancel();
      }}
      isDismissable={false}
      isKeyboardDismissDisabled={false}
    >
      <KeyInteraction
        key={`${current.operationId}:${current.status}`}
        status={current}
        onCancel={onCancel}
      />
      {cancel.error && (
        <ErrorState title="The key operation could not be cancelled" error={cancel.error} />
      )}
    </Dialog>
  );
}

function KeyInteraction({ status, onCancel }: { status: SecurityKeyStatus; onCancel: () => void }) {
  const pin = useCommandMutation<null, { input: SecurityKeyPinInput }>("security_key_pin", {
    invalidates: [],
  });
  const form = useForm<{ pin: string }>({ defaultValues: { pin: "" } });
  if (status.status === "waitingForReplacement")
    return <ReplacementKeyPrompt operationId={status.operationId} onCancel={onCancel} />;
  const needsPin = status.status === "pinRequired";
  const submit = form.handleSubmit(({ pin: value }) => {
    form.resetField("pin");
    pin.mutate({ input: { operationId: status.operationId, pin: value } });
  });
  return (
    <FormProvider {...form}>
      <Form
        aria-label="Security key verification"
        onSubmit={submit}
        error={pin.error}
        errorTitle="The key PIN was not accepted"
        actions={
          <>
            {needsPin && (
              <Button type="submit" variant="primary" isPending={pin.isPending}>
                Continue
              </Button>
            )}
            <Button onPress={onCancel}>Cancel</Button>
          </>
        }
      >
        {needsPin ? (
          <Controller
            control={form.control}
            name="pin"
            rules={{ required: "Enter this security key’s PIN." }}
            render={(controller) => (
              <TextField
                {...textFieldProps(controller)}
                label="Security key PIN"
                type="password"
                autoComplete="off"
                autoFocus
                isRequired
                description="Use the PIN belonging to this key. It is not your LIS password."
              />
            )}
          />
        ) : (
          <LoadingState
            label={
              status.status === "waitingForTouch"
                ? "Insert your security key and touch it when it signals."
                : "Checking the security key…"
            }
          />
        )}
        <p>If Windows shows its own security dialog, finish or cancel there as well.</p>
      </Form>
    </FormProvider>
  );
}

function ReplacementKeyPrompt({
  operationId,
  onCancel,
}: {
  operationId: string;
  onCancel: () => void;
}) {
  const proceed = useCommandMutation<null, { input: SecurityKeyOperationInput }>(
    "security_key_continue",
    { invalidates: [] },
  );
  return (
    <>
      <p>
        Existing key verified. Disconnect it, then connect the different physical key you want to
        add. Keep only that replacement key connected.
      </p>
      {proceed.error && (
        <ErrorState title="Registration could not continue" error={proceed.error} />
      )}
      <Button
        variant="primary"
        isPending={proceed.isPending}
        onPress={() => proceed.mutate({ input: { operationId } })}
      >
        Continue with the new key
      </Button>
      <Button onPress={onCancel}>Cancel</Button>
    </>
  );
}
