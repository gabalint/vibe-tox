import { useRef } from "react";
import { useForm, useWatch } from "react-hook-form";
import { Button } from "@/components/Button";
import { FormDialog } from "@/components/FormDialog";
import { ReasonField } from "@/components/ReasonField";
import { useToast } from "@/components/Toast";
import type { CreatePartnerConnectionInput } from "@/generated/CreatePartnerConnectionInput";
import type { PartnerConnection } from "@/generated/PartnerConnection";
import type { PartnerConnectionDetail } from "@/generated/PartnerConnectionDetail";
import type { PartnerProtocol } from "@/generated/PartnerProtocol";
import type { UpdatePartnerConnectionInput } from "@/generated/UpdatePartnerConnectionInput";
import { useCommandMutation } from "@/lib/commands";
import { CONNECTION_QUERIES, CREATE_CONNECTION, UPDATE_CONNECTION } from "../contract";
import { isIncoming } from "../labels";
import { Box, GeneralSection, WhenSection } from "./ConnectionSections";
import {
  type ConnectionValues,
  FIELD_PATHS,
  toSettings,
  toValues,
  typedPassword,
} from "./connectionForm";
import { FolderFields } from "./FolderFields";
import { AcceptSection, IntakeFolderFields } from "./IntakeSections";
import { MllpFields } from "./MllpFields";
import { ProtocolCards } from "./ProtocolCards";
import { RuleFields } from "./RuleFields";
import { SftpFields } from "./SftpFields";

interface ConnectionDialogProps {
  /** The connection to change, or `null` to add one. */
  connection: PartnerConnection | null;
  onClose: () => void;
  onSaved?: (saved: PartnerConnectionDetail) => void;
}

/**
 * Adds or changes a connection (REQ-INT-001, REQ-RPT-012), laid out as the Interfaces mockup:
 * numbered boxes for general, protocol, folder or SFTP server, routing rules and when to send,
 * and the note beside Cancel, Save as stopped and Save. A new connection is saved started, or
 * stopped with Save as stopped; either way it writes no patient data until its destination
 * check is recorded. A change needs a reason; a new folder or server needs a new check.
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
  const protocol = useWatch({ control: form.control, name: "protocol" });
  const incoming = isIncoming(useWatch({ control: form.control, name: "messageType" }));
  const submit = form.handleSubmit((values) => {
    const settings = toSettings(values);
    const password = values.protocol === "sftp" ? typedPassword(values.sftp) : undefined;
    // Folders come only from the dialog's handles; an SFTP connection has none.
    const local = values.protocol === "folder";
    const handle = (choice: string) => (local && choice !== "" ? choice : undefined);
    const destination = {
      folderChoice: handle(values.folderChoice),
      ...(isIncoming(values.messageType)
        ? {
            processedFolderChoice: handle(values.processedFolderChoice),
            failedFolderChoice: handle(values.failedFolderChoice),
          }
        : {}),
      ...(password ? { password } : {}),
    };
    if (connection) {
      update.mutate(
        {
          input: {
            connectionId: connection.id,
            expectedRevision: connection.revision,
            settings,
            ...destination,
            reason: values.reason.trim(),
          },
        },
        { onSuccess: saved },
      );
    } else {
      create.mutate(
        { input: { settings, ...destination, active: active.current } },
        { onSuccess: saved },
      );
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
      footnote={
        incoming
          ? "A new connection reads no orders until its destination check is recorded."
          : "A new connection can't send patient data until its destination check is recorded."
      }
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
      <ProtocolSection protocol={protocol} incoming={incoming} connection={connection} />
      {incoming ? (
        <AcceptSection />
      ) : (
        <>
          <Box title="4. Routing rules – which messages go here">
            <RuleFields />
          </Box>
          <WhenSection />
        </>
      )}
      {connection && <ReasonField />}
    </FormDialog>
  );
}

interface ProtocolSectionProps {
  protocol: PartnerProtocol;
  incoming: boolean;
  connection: PartnerConnection | null;
}

/** Section 3, which changes with the protocol: the folder, the SFTP server or the MLLP partner. */
function ProtocolSection({ protocol, incoming, connection }: ProtocolSectionProps) {
  switch (protocol) {
    case "sftp":
      return (
        <Box title="3. SFTP server">
          <SftpFields connection={connection} />
        </Box>
      );
    case "mllp_connect":
      return (
        <Box title="3. MLLP partner">
          <MllpFields />
        </Box>
      );
    case "folder":
      return (
        <Box title="3. Folder">
          {incoming ? (
            <IntakeFolderFields connectionId={connection?.id ?? null} />
          ) : (
            <FolderFields connectionId={connection?.id ?? null} />
          )}
        </Box>
      );
  }
}
