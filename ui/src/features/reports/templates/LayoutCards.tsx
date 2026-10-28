import { useState } from "react";
import { Button } from "@/components/Button";
import { StatusBadge } from "@/components/StatusBadge";
import type { DraftReportLayoutInput } from "@/generated/DraftReportLayoutInput";
import type { ReportLayout } from "@/generated/ReportLayout";
import type { ReportLayoutOption } from "@/generated/ReportLayoutOption";
import type { ReportLayoutsView } from "@/generated/ReportLayoutsView";
import { CommandConfirmDialog } from "@/lib/CommandConfirmDialog";
import { DRAFT_REPORT_LAYOUT, LAYOUT_QUERIES } from "./contract";
import { versionStates } from "./labels";
import styles from "./templates.module.css";
import classic from "./thumbnails/classic.png";
import compact from "./thumbnails/compact.png";
import resultsFirst from "./thumbnails/results-first.png";
import standard from "./thumbnails/standard.png";

/** First pages of each layout's preview, rendered from the pinned templates. */
const thumbnails: Record<ReportLayout, string> = {
  standard,
  compact,
  results_first: resultsFirst,
  classic,
};

interface LayoutCardsProps {
  view: ReportLayoutsView;
  onPreview: (layout: ReportLayout) => void;
}

/**
 * The ready-made layouts, each with a thumbnail, its description, whether it is active or
 * waiting for approval, Preview, and Save as draft when the engine allows a draft.
 */
export function LayoutCards({ view, onPreview }: LayoutCardsProps) {
  const [drafting, setDrafting] = useState<ReportLayoutOption | null>(null);
  const canDraft = view.allowedActions.includes(DRAFT_REPORT_LAYOUT);
  const stateOf = (layout: ReportLayout) =>
    view.draft?.layout === layout
      ? versionStates.draft
      : view.inForce.layout === layout
        ? versionStates.active
        : null;
  return (
    <>
      <ul className={styles.cards}>
        {view.layouts.map((option) => {
          const state = stateOf(option.layout);
          return (
            <li key={option.layout} className={styles.card}>
              <img
                className={styles.thumbnail}
                src={thumbnails[option.layout]}
                alt={`First page of the ${option.name} layout on sample data`}
              />
              <div className={styles.body}>
                <h3 className={styles.name}>{option.name}</h3>
                {state && <StatusBadge tone={state.tone}>{state.label}</StatusBadge>}
                <p className={styles.description}>{option.description}</p>
              </div>
              <div className={styles.actions}>
                <Button onPress={() => onPreview(option.layout)}>Preview</Button>
                {canDraft && state === null && (
                  <Button variant="primary" onPress={() => setDrafting(option)}>
                    Save as draft
                  </Button>
                )}
              </div>
            </li>
          );
        })}
      </ul>
      {drafting && (
        <CommandConfirmDialog<DraftReportLayoutInput>
          title={`Save the ${drafting.name} layout as a draft?`}
          confirmLabel="Save as draft"
          variant="primary"
          command={{ name: DRAFT_REPORT_LAYOUT, invalidates: LAYOUT_QUERIES }}
          input={{ layout: drafting.layout }}
          successTitle={`${drafting.name} saved as a draft`}
          onClose={() => setDrafting(null)}
        >
          <p>The draft waits for a quality manager's approval and changes no report until then.</p>
        </CommandConfirmDialog>
      )}
    </>
  );
}
