import { useState } from "react";
import { Button } from "react-aria-components";
import { useSearchParams } from "react-router";
import type { PartnerConnectionDetail } from "@/generated/PartnerConnectionDetail";
import { useFormatTime } from "@/lib/format";
import { ContractDialog } from "./ContractDialog";
import styles from "./ContractLine.module.css";
import { contractText } from "./contractLabels";

/**
 * The Overview's interface contract line, as mockup 5 (`v3 · acceptance tests passed …`). It
 * opens the contract, its draft and its acceptance tests; a held message's link in Logs opens
 * it too, with `?contract=open` (#4726).
 */
export function ContractLine({ detail }: { detail: PartnerConnectionDetail }) {
  const [params, setParams] = useSearchParams();
  const [open, setOpen] = useState(params.get("contract") === "open");
  const close = () => {
    setOpen(false);
    setParams({}, { replace: true });
  };
  const formatTime = useFormatTime();
  return (
    <>
      <Button className={styles.link} onPress={() => setOpen(true)}>
        {contractText(detail.connection.contract, formatTime)}
      </Button>
      {open && <ContractDialog detail={detail} onClose={close} />}
    </>
  );
}
