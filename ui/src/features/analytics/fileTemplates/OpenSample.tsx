import { useState } from "react";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import type { LayoutSampleOpened } from "@/generated/LayoutSampleOpened";
import { useCommandMutation } from "@/lib/commands";
import { OPEN_LAYOUT_SAMPLE } from "../contract";

interface OpenSampleProps {
  label: string;
  onOpened: (sample: LayoutSampleOpened) => void;
}

/**
 * Opens a sample export in the system file dialog (brief §7, step 1). The engine reads the
 * file and keeps it in memory for this session only; nothing is stored. Cancelling the
 * dialog opens nothing.
 */
export function OpenSample({ label, onOpened }: OpenSampleProps) {
  const [cancelled, setCancelled] = useState(false);
  const open = useCommandMutation<LayoutSampleOpened | null>(OPEN_LAYOUT_SAMPLE, {
    invalidates: [],
  });
  const choose = () => {
    setCancelled(false);
    open.mutate(
      {},
      {
        onSuccess: (sample) => {
          if (sample === null) setCancelled(true);
          else onOpened(sample);
        },
      },
    );
  };
  return (
    <>
      <div>
        <Button onPress={choose} isPending={open.isPending}>
          {label}
        </Button>
      </div>
      {cancelled && <Alert tone="info">The file dialog was cancelled. No file was opened.</Alert>}
      {open.error && <ErrorState title="The file was not opened" error={open.error} />}
    </>
  );
}
