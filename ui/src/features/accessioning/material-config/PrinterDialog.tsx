import { useFormContext } from "react-hook-form";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import type { ConfigureLabelPrinterInput } from "@/generated/ConfigureLabelPrinterInput";
import type { LabelPrinterView } from "@/generated/LabelPrinterView";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked, omittedIfBlank } from "@/lib/choices";

interface Values {
  code: string;
  name: string;
  transport: "tcp" | "cups" | null;
  ipAddress: string;
  port: string;
  queue: string;
  active: string;
}
export function PrinterDialog({
  printer,
  onClose,
}: {
  printer?: LabelPrinterView;
  onClose: () => void;
}) {
  return (
    <CommandDialog<Values, ConfigureLabelPrinterInput>
      title={printer ? `Configure printer ${printer.code}` : "Add label printer"}
      submitLabel="Save printer"
      command={{ name: "configure_printer", invalidates: ["list_printers"] }}
      defaultValues={{
        code: printer?.code ?? "",
        name: printer?.name ?? "",
        transport: printer?.transport ?? "tcp",
        ipAddress: printer?.ipAddress ?? "",
        port: String(printer?.port ?? 9100),
        queue: printer?.queue ?? "",
        active: printer?.active === false ? "no" : "yes",
      }}
      toInput={(v) => ({
        printerId: printer?.printerId,
        expectedRevision: printer?.revision,
        code: v.code.trim(),
        name: v.name.trim(),
        transport: checked(v.transport),
        ipAddress: v.transport === "tcp" ? omittedIfBlank(v.ipAddress) : undefined,
        port: v.transport === "tcp" && v.port.trim() ? Number(v.port) : undefined,
        queue: v.transport === "cups" ? omittedIfBlank(v.queue) : undefined,
        active: v.active === "yes",
      })}
      successTitle="Label printer saved"
      onClose={onClose}
    >
      <TextInput
        name="code"
        label="Code"
        isRequired
        description="Unique; 1–64 lowercase letters, digits, dots, dashes or underscores."
      />
      <TextInput
        name="name"
        label="Name"
        isRequired
        description="Operator-facing name; up to 200 characters."
      />
      <ChoiceInput
        name="transport"
        label="Transport"
        options={[
          { id: "tcp", label: "TCP network printer" },
          { id: "cups", label: "CUPS raw queue" },
        ]}
        isRequired
      />
      <DestinationFields />
      <ChoiceInput
        name="active"
        label="Available for printing"
        options={[
          { id: "yes", label: "Active" },
          { id: "no", label: "Inactive" },
        ]}
        isRequired
      />
      <p>
        Saving configures the destination. Print a synthetic label and verify the physical output
        before laboratory use. Handed off does not prove printing completed.
      </p>
    </CommandDialog>
  );
}
function DestinationFields() {
  const { watch } = useFormContext<Values>();
  return watch("transport") === "cups" ? (
    <TextInput
      name="queue"
      label="Raw queue name"
      isRequired
      description="Existing host CUPS queue; letters, digits, dots, dashes or underscores; must not start with a dash."
    />
  ) : (
    <>
      <TextInput
        name="ipAddress"
        label="Printer IP address"
        isRequired
        description="Private or link-local laboratory IP address; hostnames are not accepted."
      />
      <TextInput
        name="port"
        label="TCP port"
        format={{
          pattern: /^\d{1,5}$/,
          message: "Enter a whole-number port.",
          description: "1–65535; blank uses 9100.",
        }}
      />
    </>
  );
}
