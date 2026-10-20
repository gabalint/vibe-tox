import type { UseQueryResult } from "@tanstack/react-query";
import { Alert } from "@/components/Alert";
import { QueryContent } from "@/components/QueryContent";
import type { CommandError } from "@/generated/CommandError";
import type { FileLayout } from "@/generated/FileLayout";
import type { LayoutCheck } from "@/generated/LayoutCheck";
import { AnalytesSection } from "./AnalytesSection";
import type { Mappings } from "./mappings";
import { failureText } from "./templateLabels";
import { analyteNames, type Found } from "./templateValues";
import { WordsSection } from "./WordsSection";

interface MappingStepsProps {
  /** The structure the sample was read with. */
  reading: FileLayout;
  /** The reading of the sample with that structure. */
  query: UseQueryResult<LayoutCheck, CommandError>;
  /** What the reading found, or `null` when the whole sample could not be read. */
  found: Found | null;
  mappings: Mappings;
  onChange: (mappings: Mappings) => void;
}

/** Steps 5 and 6: the analyte names and the words the sample holds, each to map. */
export function MappingSteps({ reading, query, found, mappings, onChange }: MappingStepsProps) {
  return (
    <QueryContent query={query} what="names and words of the sample" loadingRows={4}>
      {(check) =>
        found === null ? (
          <Alert tone="danger" title="The sample cannot be read with these choices">
            {check.failure === null ? null : failureText(check.failure)}
          </Alert>
        ) : (
          <>
            <AnalytesSection
              names={analyteNames(reading, found)}
              isLong={reading.shape.kind === "long"}
              mappings={mappings}
              onChange={onChange}
            />
            <WordsSection
              found={found}
              hasTypeColumn={reading.columns.sampleType !== null}
              mappings={mappings}
              onChange={onChange}
            />
          </>
        )
      }
    </QueryContent>
  );
}
