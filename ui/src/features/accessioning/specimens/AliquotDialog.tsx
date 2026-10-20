import { useState } from "react";
import { useForm } from "react-hook-form";
import { FormDialog } from "@/components/FormDialog";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import { useToast } from "@/components/Toast";
import type { ContainerView } from "@/generated/ContainerView";
import type { CreateAliquotInput } from "@/generated/CreateAliquotInput";
import { checked, choiceOf, omittedIfBlank } from "@/lib/choices";
import { VOLUME_FORMAT } from "./formats";
import { LabelCheckDialog } from "./LabelCheckDialog";
import { VOLUME_KINDS, volumeText } from "./labels";
import { useAccessionCommand } from "./useAccessionCommand";

interface AliquotDialogProps {
  parent: ContainerView;
  onClose: () => void;
}

/**
 * Takes an aliquot from a container, keeping its lineage (REQ-SPC-003). The aliquot carries
 * the parent's identity, so the person first scans or types the parent's label.
 */
export function AliquotDialog({ parent, onClose }: AliquotDialogProps) {
  const [labelChecked, setLabelChecked] = useState(false);
  if (!labelChecked) {
    return (
      <LabelCheckDialog
        title={aliquotTitle(parent)}
        expected={parent.labelCode}
        labelCodes={[parent.labelCode]}
        onConfirmed={() => setLabelChecked(true)}
        onClose={onClose}
      />
    );
  }
  return <AliquotForm parent={parent} onClose={onClose} />;
}

const aliquotTitle = (parent: ContainerView) => `Create aliquot from ${parent.labelCode}`;

/**
 * The aliquot's volume. The engine checks the parent's state and holds, and subtracts volume
 * only when both use the same unit.
 */
function AliquotForm({ parent, onClose }: AliquotDialogProps) {
  const toast = useToast();
  const create = useAccessionCommand<CreateAliquotInput>("create_aliquot");
  const form = useForm<{ volumeTaken: string; volumeUnit: string; volumeKind: string | null }>({
    defaultValues: { volumeTaken: "", volumeUnit: "", volumeKind: null },
  });
  const onSubmit = form.handleSubmit((values) =>
    create.mutate(
      {
        input: {
          parentContainerId: parent.containerId,
          expectedRevision: parent.revision,
          volumeTaken: omittedIfBlank(values.volumeTaken),
          volumeUnit: omittedIfBlank(values.volumeUnit),
          volumeKind: checked(choiceOf(VOLUME_KINDS, values.volumeKind)),
        },
      },
      {
        onSuccess: () => {
          toast.show({ tone: "success", title: `Aliquot created from ${parent.labelCode}` });
          onClose();
        },
      },
    ),
  );
  return (
    <FormDialog
      title={aliquotTitle(parent)}
      submitLabel="Create aliquot"
      form={form}
      onSubmit={onSubmit}
      command={create}
      errorTitle="The aliquot was not created"
      onClose={onClose}
    >
      <p>Parent volume: {volumeText(parent)}</p>
      <TextInput name="volumeTaken" label="Volume taken" format={VOLUME_FORMAT} />
      <TextInput name="volumeUnit" label="Volume unit" description="Such as mL" />
      <ChoiceInput name="volumeKind" label="Volume taken is" options={VOLUME_KINDS} isRequired />
    </FormDialog>
  );
}
