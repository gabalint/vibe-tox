import { Controller, useFormContext } from "react-hook-form";
import { Checkbox } from "@/components/Checkbox";
import { ChoiceInput } from "@/components/FormFields";
import styles from "./setup.module.css";

const PEOPLE = [
  { id: "several", label: "Several people" },
  { id: "alone", label: "Only me: I run this laboratory alone" },
];

/** Demo loading is an explicit choice; ordinary laboratories retain their role choice. */
export function SetupDataOptions({ isDemo, isPending }: { isDemo: boolean; isPending: boolean }) {
  const { control, watch } = useFormContext();
  const prepareDemo = isDemo && watch("loadDemoData");
  return (
    <>
      {isDemo && (
        <div className={styles.dataset}>
          <Controller
            control={control}
            name="loadDemoData"
            render={({ field }) => (
              <Checkbox
                name={field.name}
                isSelected={field.value}
                onChange={field.onChange}
                isDisabled={isPending}
              >
                Load Standard Catalog and demo data
              </Checkbox>
            )}
          />
          <p>
            After sign-in, we’ll load catalog drafts and fictional clinics, practitioners and
            patients automatically.
          </p>
          {prepareDemo && (
            <p>
              This demo account gets every role so you can explore on your own. Self-approval still
              asks for confirmation.
            </p>
          )}
        </div>
      )}
      {!prepareDemo && (
        <ChoiceInput
          name="people"
          label="Who will use this laboratory?"
          options={PEOPLE}
          isDisabled={isPending}
          description="Only me gives this account every role, and lets one person approve configuration they wrote, release reports whose results they reviewed and verify destinations they created. The app asks you to confirm each time. An administrator can change this later."
        />
      )}
    </>
  );
}
