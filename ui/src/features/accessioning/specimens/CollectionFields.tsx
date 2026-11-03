import { useContext } from "react";
import { useFormContext, useWatch } from "react-hook-form";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { TimePickerButton } from "@/components/FieldPicker";
import { FieldAction, FieldGrid } from "@/components/Form";
import { ChoiceInput, DATE_FORMAT, TextInput } from "@/components/FormFields";
import { Section } from "@/components/Section";
import { LabTimeZoneContext } from "@/lib/format";
import type { AccessionFormValues } from "./accessionValues";
import { OFFSET_FORMAT, TIME_FORMAT } from "./formats";
import { COLLECTION_TIME_KINDS } from "./labels";

/**
 * When and by whom the specimens were collected (REQ-SPC-002). A missing time or date is
 * recorded as such: the form never fills one in.
 */
export function CollectionFields({ includeCollector = true }: { includeCollector?: boolean }) {
  const { control, setValue } = useFormContext<AccessionFormValues>();
  const labTimeZone = useContext(LabTimeZoneContext);
  const timeKind = useWatch({ control, name: "timeKind" });
  return (
    <Section title="Collection" variant="plain">
      <FieldGrid>
        <ChoiceInput
          name="timeKind"
          label="Collection time known"
          options={COLLECTION_TIME_KINDS}
          isRequired
        />
        {(timeKind === "datetime" || timeKind === "zoned" || timeKind === "date") && (
          <TextInput
            name="collectedDate"
            label="Collection date"
            format={DATE_FORMAT}
            isRequired
            isPastOnly
          />
        )}
        {(timeKind === "datetime" || timeKind === "zoned") && (
          <>
            <TextInput
              name="collectedTime"
              label="Collection time"
              format={TIME_FORMAT}
              isRequired
              picker={(value, pick) => (
                <TimePickerButton label="Choose the collection time" value={value} onPick={pick} />
              )}
            />
            {timeKind === "datetime" && (
              <TextInput
                name="utcOffset"
                label="Collection timezone (UTC offset)"
                description="The offset recorded at collection, such as -05:00. Use the supplied value."
                format={OFFSET_FORMAT}
                isRequired
              />
            )}
            {timeKind === "zoned" && (
              <>
                <TextInput
                  name="timeZone"
                  label="Collection site timezone"
                  description="The collection site's named timezone, such as America/New_York."
                  isRequired
                />
                {labTimeZone && (
                  <FieldAction>
                    <Button
                      onPress={() =>
                        setValue("timeZone", labTimeZone, {
                          shouldDirty: true,
                          shouldValidate: true,
                        })
                      }
                    >
                      Use laboratory timezone ({labTimeZone})
                    </Button>
                  </FieldAction>
                )}
              </>
            )}
          </>
        )}
        {includeCollector && (
          <TextInput
            name="collector"
            label="Collected by"
            description="As written on the requisition."
          />
        )}
      </FieldGrid>
      {timeKind === "unknown" && (
        <Alert tone="warning" compact>
          Without a collection date, billing for this order will be held ("Date of service
          unknown"), and correcting it later requires a collection correction and quality review.
          Choose Unknown only if no date was supplied.
        </Alert>
      )}
      {timeKind === "date" && (
        <Alert tone="info" compact>
          The report will show "time not recorded".
        </Alert>
      )}
    </Section>
  );
}
