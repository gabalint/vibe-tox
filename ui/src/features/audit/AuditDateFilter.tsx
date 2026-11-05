import { useContext, useState } from "react";
import { DatePickerButton } from "@/components/FieldPicker";
import { TextField } from "@/components/TextField";
import { calendarDayToday, isCalendarDate, LabTimeZoneContext } from "@/lib/format";

interface AuditDateFilterProps {
  label: string;
  name: string;
  /** The applied day, `YYYY-MM-DD`. */
  value: string | undefined;
  /** The earliest day allowed: the other end of the range. */
  earliest?: string | undefined;
  /** The latest day allowed. */
  latest?: string | undefined;
  /** Called with a real day in range, or `undefined` when the field is emptied. */
  onChange: (day: string | undefined) => void;
}

/**
 * A day typed as `YYYY-MM-DD` or picked from the calendar, applied once it is a real day
 * within the range; until then the field says what is wrong. Days are the laboratory's.
 */
export function AuditDateFilter({
  label,
  name,
  value,
  earliest,
  latest,
  onChange,
}: AuditDateFilterProps) {
  const labTimeZone = useContext(LabTimeZoneContext);
  const [text, setText] = useState(value ?? "");
  const [applied, setApplied] = useState(value);
  // Clear filters and the address change the applied day; the field follows.
  if (value !== applied) {
    setApplied(value);
    setText(value ?? "");
  }
  const day = text.trim();
  const error = dayError(day, earliest, latest);
  const change = (next: string) => {
    setText(next);
    const trimmed = next.trim();
    if (dayError(trimmed, earliest, latest) === undefined) onChange(trimmed || undefined);
  };
  const today = calendarDayToday(new Date(), labTimeZone);
  return (
    <TextField
      layout="inline"
      label={label}
      name={name}
      value={text}
      onChange={change}
      placeholder="YYYY-MM-DD"
      errorMessage={error}
      autoComplete="off"
      adornment={
        <DatePickerButton
          label={`Choose the ${label.toLowerCase()} day`}
          latest={latest !== undefined && latest < today ? latest : today}
          onPick={change}
        />
      }
    />
  );
}

function dayError(day: string, earliest?: string, latest?: string): string | undefined {
  if (day === "") return undefined;
  if (!isCalendarDate(day)) return "Enter the day as YYYY-MM-DD.";
  if (earliest !== undefined && day < earliest) return "Choose a day on or after the start day.";
  if (latest !== undefined && day > latest) return "Choose a day on or before the end day.";
  return undefined;
}
