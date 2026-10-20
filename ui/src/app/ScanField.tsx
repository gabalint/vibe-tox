import { ScanBarcode } from "lucide-react";
import { useState } from "react";
import { Input, SearchField } from "react-aria-components";
import { useNavigate } from "react-router";
import styles from "./AppShell.module.css";
import { containerLookupPath } from "./navigation";

/**
 * The top-bar "Scan or search" field (ui-standards §Forms): a scanner types a container
 * barcode and presses Enter, from any screen, and the container lookup opens with it. The
 * field clears for the next scan. The lookup page runs the audited `find_container` read.
 */
export function ScanField() {
  const navigate = useNavigate();
  const [value, setValue] = useState("");
  const submit = (text: string) => {
    const labelCode = text.trim();
    if (labelCode === "") return;
    setValue("");
    navigate(containerLookupPath(labelCode));
  };
  return (
    <search className={styles.scan}>
      <SearchField
        aria-label="Scan or search"
        value={value}
        onChange={setValue}
        onSubmit={submit}
        className={styles.scanField}
      >
        <ScanBarcode aria-hidden="true" className={styles.scanIcon} />
        <Input placeholder="Scan or search" className={styles.scanInput} />
      </SearchField>
    </search>
  );
}
