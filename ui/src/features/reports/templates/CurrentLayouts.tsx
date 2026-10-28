import { useState } from "react";
import { Button } from "@/components/Button";
import { Facts } from "@/components/Facts";
import { ReasonConfirmDialog } from "@/components/ReasonConfirmDialog";
import { Section } from "@/components/Section";
import { Timestamp } from "@/components/Timestamp";
import type { ApproveReportLayoutInput } from "@/generated/ApproveReportLayoutInput";
import type { LayoutInForce } from "@/generated/LayoutInForce";
import type { LayoutVersionView } from "@/generated/LayoutVersionView";
import type { RejectReportLayoutInput } from "@/generated/RejectReportLayoutInput";
import type { ReportLayout } from "@/generated/ReportLayout";
import { CommandConfirmDialog } from "@/lib/CommandConfirmDialog";
import { useCommandMutation } from "@/lib/commands";
import { usePersonText } from "@/lib/person";
import { APPROVE_REPORT_LAYOUT, LAYOUT_QUERIES, REJECT_REPORT_LAYOUT } from "./contract";

interface ActiveLayoutProps {
  inForce: LayoutInForce;
  onPreview: (layout: ReportLayout) => void;
}

/** The layout reports prepared now use: its name, version and approval, or the built-in default. */
export function ActiveLayout({ inForce, onPreview }: ActiveLayoutProps) {
  const person = usePersonText();
  const version = inForce.version;
  const approval = version?.decidedBy
    ? [
        {
          label: "Approved by",
          value: person(version.decidedBy.userId, version.decidedBy.displayName),
        },
        {
          label: "Approved",
          value: version.decidedAt ? <Timestamp value={version.decidedAt} /> : "—",
        },
      ]
    : [];
  return (
    <Section
      title="Active layout"
      actions={<Button onPress={() => onPreview(inForce.layout)}>Preview</Button>}
    >
      <Facts
        items={[
          { label: "Layout", value: inForce.name },
          {
            label: "Version",
            value: version ? `Version ${version.versionNo}` : "Built-in default, never approved",
          },
          ...approval,
        ]}
      />
    </Section>
  );
}

interface DraftLayoutProps {
  draft: LayoutVersionView;
  allowedActions: readonly string[];
  onPreview: (layout: ReportLayout) => void;
}

/**
 * The draft waiting for approval, with Approve and Reject when the engine allows them. The
 * engine refuses an approval by the draft's author unless the laboratory's separation-of-duties
 * policy allows it and the author confirms it (ADR 0015).
 */
export function DraftLayout({ draft, allowedActions, onPreview }: DraftLayoutProps) {
  const person = usePersonText();
  const [dialog, setDialog] = useState<"approve" | "reject" | null>(null);
  const close = () => setDialog(null);
  const input = { versionId: draft.versionId, expectedRevision: draft.revision };
  return (
    <Section
      title="Waiting for approval"
      actions={
        <>
          <Button onPress={() => onPreview(draft.layout)}>Preview</Button>
          {allowedActions.includes(REJECT_REPORT_LAYOUT) && (
            <Button variant="danger" onPress={() => setDialog("reject")}>
              Reject
            </Button>
          )}
          {allowedActions.includes(APPROVE_REPORT_LAYOUT) && (
            <Button variant="primary" onPress={() => setDialog("approve")}>
              Approve
            </Button>
          )}
        </>
      }
    >
      <Facts
        items={[
          { label: "Layout", value: draft.name },
          { label: "Version", value: `Version ${draft.versionNo}` },
          {
            label: "Drafted by",
            value: person(draft.draftedBy.userId, draft.draftedBy.displayName),
          },
          { label: "Drafted", value: <Timestamp value={draft.draftedAt} /> },
        ]}
      />
      {dialog === "approve" && (
        <CommandConfirmDialog<ApproveReportLayoutInput>
          title={`Approve the ${draft.name} layout?`}
          confirmLabel="Approve layout"
          variant="primary"
          command={{ name: APPROVE_REPORT_LAYOUT, invalidates: LAYOUT_QUERIES }}
          selfApprovalInput={(selfApproval) => ({ ...input, selfApproval })}
          successTitle={`${draft.name} is now the active layout`}
          onClose={close}
        >
          <p>
            Reports prepared from now on use this layout. Reports already released, and their PDFs,
            stay exactly as they were.
          </p>
        </CommandConfirmDialog>
      )}
      {dialog === "reject" && <RejectDialog draft={draft} input={input} onClose={close} />}
    </Section>
  );
}

interface RejectDialogProps {
  draft: LayoutVersionView;
  input: Omit<RejectReportLayoutInput, "reason">;
  onClose: () => void;
}

function RejectDialog({ draft, input, onClose }: RejectDialogProps) {
  const reject = useCommandMutation<unknown, { input: RejectReportLayoutInput }>(
    REJECT_REPORT_LAYOUT,
    { invalidates: LAYOUT_QUERIES },
  );
  return (
    <ReasonConfirmDialog
      title={`Reject the ${draft.name} layout?`}
      confirmLabel="Reject layout"
      variant="danger"
      command={reject}
      input={(reason) => ({ ...input, reason })}
      successTitle={`${draft.name} layout rejected`}
      onClose={onClose}
    >
      <p>The active layout stays in use. The draft and your reason are kept in the history.</p>
    </ReasonConfirmDialog>
  );
}
