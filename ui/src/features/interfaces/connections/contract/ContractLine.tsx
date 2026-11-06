import { useState } from "react";
import { Button } from "@/components/Button";
import type { PartnerConnectionDetail } from "@/generated/PartnerConnectionDetail";
import { useFormatTime } from "@/lib/format";
import { ContractDialog } from "./ContractDialog";
import { contractText } from "./contractLabels";

/**
 * The Overview's interface contract line, as mockup 5 (`v3 · acceptance tests passed …`). It
 * opens the contract, its draft and its acceptance tests.
 */
export function ContractLine({ detail }: { detail: PartnerConnectionDetail }) {
  const [open, setOpen] = useState(false);
  const formatTime = useFormatTime();
  return (
    <>
      <Button variant="quiet" onPress={() => setOpen(true)}>
        {contractText(detail.connection.contract, formatTime)}
      </Button>
      {open && <ContractDialog detail={detail} onClose={() => setOpen(false)} />}
    </>
  );
}
