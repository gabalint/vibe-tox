import { useRef } from "react";
import { Controller, useFormContext } from "react-hook-form";
import { Button } from "@/components/Button";
import { type RecordOption, RecordPicker } from "@/components/RecordPicker";
import type { AccessionFormValues } from "./accessionValues";
import styles from "./receiving.module.css";

/** Search long ordered-test lists while keeping the selected material scope visible. */
export function AccessionTestPicker({ options }: { options: readonly RecordOption[] }) {
  const { control } = useFormContext<AccessionFormValues>();
  const input = useRef<HTMLInputElement>(null);
  return (
    <Controller
      control={control}
      name="orderItemIds"
      rules={{ validate: (value) => value.length > 0 || "Choose at least one test." }}
      render={({ field, fieldState }) => (
        <>
          <RecordPicker
            selectionMode="multiple"
            name={field.name}
            label="Find ordered tests"
            description="Search by test code, name or required specimen type."
            options={options}
            value={field.value}
            onChange={field.onChange}
            inputRef={(element) => {
              input.current = element;
              field.ref(element);
            }}
            isRequired
            errorMessage={fieldState.error?.message}
          />
          {field.value.length > 0 && (
            <ul className={styles.selectedTests} aria-label="Selected tests for this material">
              {options
                .filter((option) => field.value.includes(option.id))
                .map((option) => (
                  <li key={option.id} className={styles.selectedTest}>
                    <span>{option.label}</span>
                    <Button
                      variant="quiet"
                      aria-label={`Remove ${option.label}`}
                      onPress={() => {
                        field.onChange(field.value.filter((id) => id !== option.id));
                        input.current?.focus();
                      }}
                    >
                      Remove
                    </Button>
                  </li>
                ))}
            </ul>
          )}
        </>
      )}
    />
  );
}
