import { useForm } from "react-hook-form";
import { Link } from "react-router";
import { Alert } from "@/components/Alert";
import { ErrorState } from "@/components/ErrorState";
import { FormDialog } from "@/components/FormDialog";
import { ChoiceInput } from "@/components/FormFields";
import { LoadingState } from "@/components/LoadingState";
import { ReasonField } from "@/components/ReasonField";
import { useToast } from "@/components/Toast";
import type { CommandError } from "@/generated/CommandError";
import type { ContainerView } from "@/generated/ContainerView";
import type { LabelJobView } from "@/generated/LabelJobView";
import type { LabelPrinterView } from "@/generated/LabelPrinterView";
import type { PrintLabelsInput } from "@/generated/PrintLabelsInput";
import { checked } from "@/lib/choices";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { useAccessionCommand } from "./useAccessionCommand";

interface PrintLabelsDialogProps {
  containers: readonly ContainerView[];
  kind: PrintLabelsInput["kind"];
  onClose: () => void;
}

/**
 * Sends label jobs to a printer (REQ-SPC-001). A reprint keeps each container's barcode and
 * records who asked and why. A job reports `queued`, `handed_off` or `failed`; nothing here
 * can tell that a label actually printed.
 */
export function PrintLabelsDialog({ containers, kind, onClose }: PrintLabelsDialogProps) {
  const toast = useToast();
  const printers = useCommandQuery<LabelPrinterView[]>("list_printers");
  const print = useAccessionCommand<PrintLabelsInput, LabelJobView[]>("print_labels");
  const reprint = kind === "reprint";
  const form = useForm<{ printerId: string | null; reason: string }>({
    defaultValues: { printerId: null, reason: "" },
  });
  const onSubmit = form.handleSubmit(({ printerId, reason }) =>
    print.mutate(
      {
        input: {
          containerIds: containers.map((container) => container.containerId),
          printerId: checked(printerId),
          kind,
          reason: reprint ? reason.trim() : undefined,
        },
      },
      {
        onSuccess: (jobs) => {
          toast.show({
            tone: "info",
            title: `${jobs.length} label ${jobs.length === 1 ? "job" : "jobs"} queued`,
            description: "Check the label status on the accession once the printer has answered.",
          });
          onClose();
        },
      },
    ),
  );
  const barcodes = containers.map((container) => container.labelCode).join(", ");
  return (
    <FormDialog
      title={reprint ? "Reprint labels" : "Print labels"}
      submitLabel={reprint ? "Reprint labels" : "Print labels"}
      form={form}
      onSubmit={onSubmit}
      command={print}
      errorTitle="The labels were not sent"
      onClose={onClose}
    >
      <p>Containers: {barcodes}</p>
      <PrinterChoice printers={printers} />
      {reprint && <ReasonField />}
    </FormDialog>
  );
}

interface PrinterChoiceProps {
  printers: { data?: LabelPrinterView[]; error: CommandError | null };
}

function printerDestination(printer: LabelPrinterView) {
  if (printer.transport === "tcp") {
    return `${printer.ipAddress ?? "address missing"}:${printer.port ?? "port missing"}`;
  }
  return `queue ${printer.queue ?? "missing"}`;
}

function PrinterChoice({ printers }: PrinterChoiceProps) {
  const canConfigure = useHasCapability("manage_configuration");
  if (printers.error) {
    return <ErrorState title="The printers could not be loaded" error={printers.error} />;
  }
  if (!printers.data) return <LoadingState label="Loading printers…" />;
  // Inactive printers stay configured for history but are not offered.
  const active = printers.data.filter((printer) => printer.active);
  if (active.length === 0) {
    return (
      <Alert tone="warning" title="No active label printer is set up">
        An administrator adds label printers before labels can be sent.{" "}
        {canConfigure && <Link to="/setup/material?tab=printers">Configure label printers</Link>}
      </Alert>
    );
  }
  return (
    <ChoiceInput
      name="printerId"
      label="Printer"
      options={active.map((printer) => ({
        id: printer.printerId,
        label: `${printer.name} (${printer.code} — ${printerDestination(printer)})`,
      }))}
      isRequired
    />
  );
}
