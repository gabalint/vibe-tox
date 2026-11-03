import { useRef } from "react";
import { useForm } from "react-hook-form";
import { Button } from "@/components/Button";
import { FormDialog } from "@/components/FormDialog";
import { ReasonField } from "@/components/ReasonField";
import { useToast } from "@/components/Toast";
import type { CreatePartnerConnectionInput } from "@/generated/CreatePartnerConnectionInput";
import type { PartnerConnection } from "@/generated/PartnerConnection";
import type { PartnerConnectionDetail } from "@/generated/PartnerConnectionDetail";
import type { UpdatePartnerConnectionInput } from "@/generated/UpdatePartnerConnectionInput";
import { useCommandMutation } from "@/lib/commands";
import { CONNECTION_QUERIES, CREATE_CONNECTION, UPDATE_CONNECTION } from "../contract";
import { Box, GeneralSection, WhenSection } from "./ConnectionSections";
import { type ConnectionValues, FIELD_PATHS, toSettings, toValues } from "./connectionForm";
import { FolderFields } from "./FolderFields";
import { ProtocolCards } from "./ProtocolCards";
import { RuleFields } from "./RuleFields";

interface ConnectionDialogProps {
  /** The connection to change, or `null` to add one. */
  connection: PartnerConnection | null;
  onClose: () => void;
  onSaved?: (saved: PartnerConnectionDetail) => void;
}

/**
 * Adds or changes a connection (REQ-INT-001, REQ-RPT-012), laid out as the Interfaces mockup:
 * numbered boxes for general, protocol, folder, routing rules and when to send, and the note
 * beside Cancel, Save as stopped and Save. A new connection is saved started, or stopped with
 * Save as stopped; either way it writes no patient data until its destination check is
 * recorded. A change needs a reason; a new folder needs a new destination check.
 */
export function ConnectionDialog({ connection, onClose, onSaved }: ConnectionDialogProps) {
  const toast = useToast();
  const form = useForm<ConnectionValues>({ defaultValues: toValues(connection) });
  const active = useRef(true);
  const create = useCommandMutation<
    PartnerConnectionDetail,
    { input: CreatePartnerConnectionInput }
  >(CREATE_CONNECTION, { invalidates: CONNECTION_QUERIES });
  const update = useCommandMutation<
    PartnerConnectionDetail,
    { input: UpdatePartnerConnectionInput }
  >(UPDATE_CONNECTION, { invalidates: CONNECTION_QUERIES });
  const command = connection ? update : create;
  const saved = (detail: PartnerConnectionDetail) => {
    toast.show({ tone: "success", title: `${detail.connection.settings.name} saved` });
    onSaved?.(detail);
    onClose();
  };
  const submit = form.handleSubmit((values) => {
    const settings = toSettings(values);
    if (connection) {
      update.mutate(
        {
          input: {
            connectionId: connection.id,
            expectedRevision: connection.revision,
            settings,
            ...(values.folderChoice ? { folderChoice: values.folderChoice } : {}),
            reason: values.reason.trim(),
          },
        },
        { onSuccess: saved },
      );
    } else {
      const input = { settings, folderChoice: values.folderChoice, active: active.current };
      create.mutate({ input }, { onSuccess: saved });
    }
  });
  const saveStopped = () => {
    active.current = false;
    void submit().finally(() => {
      active.current = true;
    });
  };
  return (
    <FormDialog
      form={form}
      title={connection ? `Change ${connection.settings.name}` : "Add connection"}
      description="Where messages go or come from, and which ones."
      submitLabel="Save"
      errorTitle="Not saved"
      fieldPaths={FIELD_PATHS}
      size="extra-wide"
      command={command}
      onSubmit={submit}
      onClose={onClose}
      footnote="A new connection can't send patient data until its destination check is recorded."
      extraActions={
        !connection && (
          <Button isDisabled={command.isPending} onPress={saveStopped}>
            Save as stopped
          </Button>
        )
      }
    >
      <GeneralSection isEdit={connection !== null} />
      <Box title="2. Protocol">
        <ProtocolCards />
      </Box>
      <Box title="3. Folder">
        <FolderFields connectionId={connection?.id ?? null} />
      </Box>
      <Box title="4. Routing rules – which messages go here">
        <RuleFields />
      </Box>
      <WhenSection />
      {connection && <ReasonField />}
    </FormDialog>
  );
}
