import { useNavigate } from "react-router";
import { Button } from "@/components/Button";
import { Facts } from "@/components/Facts";
import { Inset } from "@/components/Inset";
import { QueryContent } from "@/components/QueryContent";
import { StatusBadge } from "@/components/StatusBadge";
import { Toolbar } from "@/components/Toolbar";
import type { InstrumentReadinessView } from "@/generated/InstrumentReadinessView";
import type { SourceRegistrationView } from "@/generated/SourceRegistrationView";
import { labelOf } from "@/lib/choices";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { useFormatTime } from "@/lib/format";
import styles from "@/styles/layout.module.css";
import { APPROVE_CONFIGURATION, LIST_SOURCE_REGISTRATIONS } from "../contract";
import { shapeText } from "../fileTemplates/templateLabels";
import { REVISION_INDICATORS, registrationStatus, SAMPLE_ID_FIELDS } from "../labels";
import { fileSourcePath } from "../paths";
import { useAnalyteName } from "../useAnalyteName";
import { ApproveSource } from "./ApproveSource";

/**
 * The sources registered for one instrument (REQ-ANA-003): an analyzer's ASTM sender, or a
 * result-file instrument's file template. Results are imported only from an approved source.
 * A source is drafted with `manage_configuration` and approved by someone else with
 * `approve_configuration`; a file template is checked with a file and approved on its page.
 */
export function InstrumentSources({ instrument }: { instrument: InstrumentReadinessView }) {
  const sources = useCommandQuery<SourceRegistrationView[]>(LIST_SOURCE_REGISTRATIONS, {
    input: { instrumentId: instrument.instrumentId },
  });
  const analyteName = useAnalyteName();
  return (
    <div className={styles.stack}>
      <h3 className={styles.subheading}>Result sources</h3>
      <QueryContent
        query={sources}
        what="result sources"
        empty={{
          when: (rows) => rows.length === 0,
          title: "No result sources",
          description:
            "Register the analyzer's sender and its test codes, or draft the file template of a result-file instrument, before importing results.",
        }}
      >
        {(rows) => (
          <ul className={styles.stack}>
            {rows.map((source) => (
              <SourceItem
                key={source.registrationId}
                source={source}
                instrumentName={instrument.name}
                analyteName={analyteName}
              />
            ))}
          </ul>
        )}
      </QueryContent>
    </div>
  );
}

interface SourceItemProps {
  source: SourceRegistrationView;
  instrumentName: string;
  analyteName: (id: string) => string;
}

function SourceItem({ source, instrumentName, analyteName }: SourceItemProps) {
  const formatTime = useFormatTime();
  const status = registrationStatus(source.state);
  const name = `${source.senderName} ${source.senderVersion}`;
  const layout = source.fileLayout;
  const kindFacts =
    layout === null
      ? [
          { label: "Sample ID read from", value: labelOf(SAMPLE_ID_FIELDS, source.sampleIdField) },
          { label: "Corrections", value: labelOf(REVISION_INDICATORS, source.revisionIndicator) },
        ]
      : [{ label: "Result file", value: shapeText(layout.shape) }];
  return (
    <Inset as="li">
      <Toolbar>
        <h4 className={styles.subheading}>{name}</h4>
        <StatusBadge tone={status.tone}>{status.label}</StatusBadge>
        {layout === null ? (
          <ApproveSource source={source}>
            <p>
              Results that {instrumentName} sends as {name} will be imported with this test-code
              map. The person who registered the source cannot approve it unless your laboratory's
              separation-of-duties policy allows self-approval.
            </p>
          </ApproveSource>
        ) : (
          <OpenFileSource source={source} />
        )}
      </Toolbar>
      <Facts
        items={[
          ...kindFacts,
          {
            label: "Reports final concentration",
            value: source.reportsFinalConcentration ? "Yes" : "No",
          },
          {
            label: "Registered",
            value: `${formatTime(source.authoredAt)} by ${source.authoredBy}`,
          },
          {
            label: "Approved",
            value:
              source.approvedAt === null
                ? "Not yet"
                : `${formatTime(source.approvedAt)} by ${source.approvedBy ?? "unknown"}`,
          },
        ]}
      />
      <ul aria-label={`Test codes of ${name}`} className={styles.list}>
        {source.testCodes.map((code) => (
          <li key={code.sourceTestCode}>
            <span className={styles.code}>{code.sourceTestCode}</span> →{" "}
            {analyteName(code.analyteId)}
          </li>
        ))}
      </ul>
    </Inset>
  );
}

/**
 * A file template opens on its own page, where it is checked with a file and, for a draft,
 * approved (result files brief §7).
 */
function OpenFileSource({ source }: { source: SourceRegistrationView }) {
  const navigate = useNavigate();
  const canApprove = useHasCapability(APPROVE_CONFIGURATION);
  const open = () => navigate(fileSourcePath(source.instrumentId, source.registrationId));
  if (canApprove && source.allowedActions.includes("approve_source")) {
    return (
      <Button variant="primary" onPress={open}>
        Check and approve
      </Button>
    );
  }
  return <Button onPress={open}>Open template</Button>;
}
