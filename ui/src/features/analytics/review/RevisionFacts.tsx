import { Facts } from "@/components/Facts";
import type { ResultRevisionView } from "@/generated/ResultRevisionView";
import { useFormatTime } from "@/lib/format";
import { causeText, classificationText, methodText, resultValueText } from "../resultLabels";
import { MeasurementList } from "./MeasurementList";

/** One immutable result revision: its value, classification, method and evidence. */
export function RevisionFacts({
  revision,
  analyte,
}: {
  revision: ResultRevisionView;
  analyte: string;
}) {
  const formatTime = useFormatTime();
  return (
    <Facts
      items={[
        { label: "Result", value: resultValueText(revision) },
        { label: "Classification", value: classificationText(revision) },
        { label: "Method", value: methodText(revision) },
        {
          label: "Revision",
          value: `${revision.revisionNo}: ${causeText(revision.cause)}, ${formatTime(revision.createdAt)} by ${revision.createdBy}`,
        },
        ...(revision.reason ? [{ label: "Reason", value: revision.reason }] : []),
        {
          label: "Evidence",
          value:
            revision.evidence.length === 0 ? (
              "None"
            ) : (
              <MeasurementList
                label={`Evidence for ${analyte} revision ${revision.revisionNo}`}
                measurements={revision.evidence}
              />
            ),
        },
      ]}
    />
  );
}
