import { useState } from "react";
import { Button } from "@/components/Button";
import { Checkbox } from "@/components/Checkbox";
import { CheckboxGroup } from "@/components/CheckboxGroup";
import { Form } from "@/components/Form";
import { Select } from "@/components/Select";
import { TextField } from "@/components/TextField";
import { GallerySection } from "./GallerySection";
import styles from "./gallery.module.css";

const PANELS = [
  { id: "panel-a", label: "Synthetic panel A" },
  { id: "panel-b", label: "Synthetic panel B" },
];

export function FieldsSection() {
  const [text, setText] = useState("SYN-000123");
  const [password, setPassword] = useState("");
  const [panel, setPanel] = useState<string | null>(null);
  const [checked, setChecked] = useState(true);
  const [panels, setPanels] = useState<string[]>([]);

  return (
    <GallerySection title="Fields and forms">
      <div className={styles.stack}>
        <Form
          aria-label="Field examples"
          onSubmit={(event) => event.preventDefault()}
          error={{
            code: "validation",
            message: "An engine error shows here, and its field errors on their fields.",
            fieldErrors: [
              { field: "engineField", code: "synthetic", message: "An engine field error." },
            ],
            gateReasons: [],
            correlationId: "corr-0000-synthetic",
          }}
          errorTitle="Engine error"
          actions={
            <Button type="submit" variant="primary">
              Submit
            </Button>
          }
        >
          <TextField
            label="Accession"
            name="accession"
            value={text}
            onChange={setText}
            isRequired
          />
          <TextField
            label="Password"
            name="password"
            type="password"
            value={password}
            onChange={setPassword}
            description="Typed characters are hidden."
          />
          <TextField
            label="Client-side check"
            name="client"
            value=""
            onChange={setText}
            errorMessage="Enter a value."
          />
          <TextField label="Engine check" name="engineField" value="" onChange={setText} />
          <TextField
            label="Disabled"
            name="disabled"
            value="Read only"
            onChange={setText}
            isDisabled
          />
          <Select
            label="Panel"
            name="panel"
            options={PANELS}
            value={panel}
            onChange={setPanel}
            isRequired
          />
          <Checkbox isSelected={checked} onChange={setChecked}>
            Print labels on receipt
          </Checkbox>
          <CheckboxGroup
            legend="Panels to add"
            options={PANELS.map((each) => ({ ...each, details: ["Synthetic description."] }))}
            value={panels}
            onChange={setPanels}
            isRequired
          />
          <Checkbox isSelected={false} onChange={setChecked} isIndeterminate>
            Some specimens selected
          </Checkbox>
        </Form>
      </div>
    </GallerySection>
  );
}
