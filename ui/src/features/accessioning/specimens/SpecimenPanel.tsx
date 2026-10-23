import { useMemo, useState } from "react";
import { Link } from "react-router";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { DataGrid, type GridRowActions } from "@/components/DataGrid";
import type { ActionItem } from "@/components/RowActions";
import { Section } from "@/components/Section";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { AccessionSpecimenView } from "@/generated/AccessionSpecimenView";
import type { ContainerView } from "@/generated/ContainerView";
import { labelOf } from "@/lib/choices";
import { useHasCapability } from "@/lib/commands";
import { usePersonText } from "@/lib/person";
import styles from "../accessioning.module.css";
import { AliquotDialog } from "./AliquotDialog";
import { containerColumns } from "./containerColumns";
import { acceptanceStatus, discrepancyText, RECEIPT_DECISIONS } from "./labels";
import { PrintLabelsDialog } from "./PrintLabelsDialog";
import { ReceiptChronologyDialog } from "./ReceiptChronologyDialog";
import { ReceiptDecisionDialog } from "./ReceiptDecisionDialog";

type OpenDialog =
  | { kind: "receipt" | "chronology" }
  | { kind: "print" | "reprint"; containers: readonly ContainerView[] }
  | { kind: "aliquot"; parent: ContainerView }
  | null;

interface SpecimenPanelProps {
  specimen: AccessionSpecimenView;
  /** "Specimen 1 (urine)": how the screen names this specimen. */
  title: string;
  accessionNumber: string;
}

const getRowId = (container: ContainerView) => container.containerId;
const canPrint = (container: ContainerView) => container.allowedActions.includes("print_labels");
const canAliquot = (container: ContainerView) =>
  container.allowedActions.includes("create_aliquot");

/**
 * One specimen: its receipt decisions and its containers, with their label jobs. Actions come
 * from the engine's `allowedActions`, per specimen and per container; labels for several
 * containers print together from the selection.
 */
export function SpecimenPanel({ specimen, title, accessionNumber }: SpecimenPanelProps) {
  const accession = useHasCapability("accession");
  const quality = useHasCapability("manage_quality");
  const unresolved = specimen.decisions.some((d) => d.causalSequence === null);
  const [selectedIds, setSelectedIds] = useState<ReadonlySet<string>>(new Set());
  const [dialog, setDialog] = useState<OpenDialog>(null);
  const columns = useMemo(() => {
    const labels = new Map(specimen.containers.map((each) => [each.containerId, each.labelCode]));
    return containerColumns(accessionNumber, (id) => labels.get(id) ?? "an unknown container");
  }, [accessionNumber, specimen.containers]);
  const selected = specimen.containers.filter((each) => selectedIds.has(each.containerId));
  const printable = accession ? specimen.containers.filter(canPrint) : [];
  const status = acceptanceStatus(specimen.acceptance);
  const close = () => setDialog(null);
  const rowActions: GridRowActions<ContainerView> = {
    label: (container) => container.labelCode,
    hasItems: (container) => accession && (canPrint(container) || canAliquot(container)),
    items: (container) => (accession ? containerActions(container, setDialog) : []),
  };

  return (
    <Section
      title={title}
      actions={
        <>
          <StatusBadge tone={status.tone}>{status.label}</StatusBadge>
          {selectedIds.size === 0 && printable.length > 0 && (
            <Button onPress={() => setDialog({ kind: "print", containers: printable })}>
              Print labels
            </Button>
          )}
          {accession && specimen.allowedActions.includes("record_receipt_decision") && (
            <Button onPress={() => setDialog({ kind: "receipt" })}>Record receipt decision</Button>
          )}
        </>
      }
    >
      {unresolved && (
        <Alert tone="warning" title="Receipt chronology unresolved">
          Testing and release remain blocked until reliable evidence establishes the decision order.{" "}
          {quality && (
            <Button onPress={() => setDialog({ kind: "chronology" })}>
              Review receipt chronology
            </Button>
          )}
        </Alert>
      )}
      <ReceiptDecisions specimen={specimen} title={title} />
      <DataGrid
        label={`Containers of ${title}`}
        columns={columns}
        rows={specimen.containers}
        getRowId={getRowId}
        {...(accession
          ? {
              rowActions,
              bulk: {
                selectedIds,
                onChange: setSelectedIds,
                actions: selected.length > 0 && selected.every(canPrint) && (
                  <>
                    <Button onPress={() => setDialog({ kind: "print", containers: selected })}>
                      Print labels
                    </Button>
                    <Button onPress={() => setDialog({ kind: "reprint", containers: selected })}>
                      Reprint labels
                    </Button>
                  </>
                ),
              },
            }
          : {})}
      />
      <ul aria-label="Container material records">
        {specimen.containers.map((container) => (
          <li key={container.containerId}>
            <Link to={`/containers?labelCode=${encodeURIComponent(container.labelCode)}`}>
              Material history and actions: {container.labelCode}
            </Link>
          </li>
        ))}
      </ul>
      {dialog?.kind === "chronology" && (
        <ReceiptChronologyDialog specimen={specimen} onClose={close} />
      )}
      {dialog?.kind === "receipt" && (
        <ReceiptDecisionDialog specimen={specimen} title={title} onClose={close} />
      )}
      {(dialog?.kind === "print" || dialog?.kind === "reprint") && (
        <PrintLabelsDialog containers={dialog.containers} kind={dialog.kind} onClose={close} />
      )}
      {dialog?.kind === "aliquot" && <AliquotDialog parent={dialog.parent} onClose={close} />}
    </Section>
  );
}

/** One container's actions, as its row menu offers them. */
function containerActions(
  container: ContainerView,
  open: (dialog: OpenDialog) => void,
): ActionItem[] {
  const items: ActionItem[] = [];
  if (canPrint(container)) {
    items.push(
      {
        id: "print",
        label: "Print label",
        onAction: () => open({ kind: "print", containers: [container] }),
      },
      {
        id: "reprint",
        label: "Reprint label",
        onAction: () => open({ kind: "reprint", containers: [container] }),
      },
    );
  }
  if (canAliquot(container)) {
    items.push({
      id: "create_aliquot",
      label: "Create aliquot",
      onAction: () => open({ kind: "aliquot", parent: container }),
    });
  }
  return items;
}

/** The specimen's receipt decisions, oldest first. */
function ReceiptDecisions({ specimen, title }: Omit<SpecimenPanelProps, "accessionNumber">) {
  const person = usePersonText();
  return (
    <ul className={styles.list} aria-label={`Receipt decisions for ${title}`}>
      {specimen.decisions.length === 0 && <li className={styles.hint}>No receipt decision yet.</li>}
      {specimen.decisions.map((decision) => (
        <li key={decision.decisionId}>
          {labelOf(RECEIPT_DECISIONS, decision.decision)} by{" "}
          {person(decision.decidedBy, decision.decidedByDisplay)},{" "}
          <Timestamp value={decision.decidedAt} />. {discrepancyText(decision.discrepancies)}.
          {decision.policyReference && ` Policy ${decision.policyReference}: ${decision.scope}.`}
          {decision.reason && ` Reason: ${decision.reason}`}
        </li>
      ))}
    </ul>
  );
}
