import { useState } from "react";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import { Section } from "@/components/Section";
import type { CheckFileLayoutInput } from "@/generated/CheckFileLayoutInput";
import type { LayoutCheck } from "@/generated/LayoutCheck";
import { useCommandMutation } from "@/lib/commands";
import { CHECK_FILE_LAYOUT } from "../contract";
import { useAnalyteName } from "../useAnalyteName";
import { LayoutCheckResult } from "./LayoutCheckResult";

/**
 * Step 8: runs the template as it stands on the sample and shows the outcome (brief §7): what
 * each row would become, with the counts, or why the whole file gives no rows. It writes
 * nothing; whether a label is on an open run is known only when a file is imported.
 */
export function CheckSection({ input }: { input: CheckFileLayoutInput }) {
  const check = useCommandMutation<LayoutCheck, { input: CheckFileLayoutInput }>(
    CHECK_FILE_LAYOUT,
    { invalidates: [] },
  );
  // The input last checked, to say when the template has changed since.
  const [checked, setChecked] = useState<string | null>(null);
  const analyteName = useAnalyteName();
  const current = JSON.stringify(input);
  const run = () => check.mutate({ input }, { onSuccess: () => setChecked(current) });
  const changed = check.data !== undefined && checked !== current;
  return (
    <Section
      title="8. Check"
      actions={
        <Button onPress={run} isPending={check.isPending}>
          Check the sample
        </Button>
      }
    >
      <p>
        Reads the sample as an import would, row by row, and writes nothing. Whether each label is
        on an open run is known only when a file is imported.
      </p>
      {check.error && <ErrorState title="The check did not run" error={check.error} />}
      {changed && (
        <Alert tone="info">
          The template changed after this check. Check again to see the change.
        </Alert>
      )}
      {check.data && <LayoutCheckResult check={check.data} analyteName={analyteName} />}
    </Section>
  );
}
