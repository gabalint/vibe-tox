import { CalendarDays, Clock } from "lucide-react";
import { type ReactNode, useState } from "react";
import {
  Button,
  Dialog,
  DialogTrigger,
  ListBox,
  ListBoxItem,
  Popover,
  type Selection,
} from "react-aria-components";
import { DateCalendar } from "./DateCalendar";
import styles from "./FieldPicker.module.css";

/** Called with the picked value in the field's own text format. */
type Pick = (value: string) => void;

/** The icon button inside a text field and the popover it opens. */
function PickerTrigger({
  label,
  icon,
  children,
}: {
  label: string;
  icon: ReactNode;
  children: (close: () => void) => ReactNode;
}) {
  const [isOpen, setOpen] = useState(false);
  return (
    <DialogTrigger isOpen={isOpen} onOpenChange={setOpen}>
      <Button aria-label={label} className={styles.trigger}>
        {icon}
      </Button>
      <Popover placement="bottom end" className={styles.popover}>
        <Dialog aria-label={label} className={styles.dialog}>
          {children(() => setOpen(false))}
        </Dialog>
      </Popover>
    </DialogTrigger>
  );
}

/**
 * A calendar that fills a `YYYY-MM-DD` field. Days after `latest` (`YYYY-MM-DD`) can't be
 * picked; the field still takes typed dates, which its own validation checks.
 */
export function DatePickerButton({
  label,
  latest,
  onPick,
}: {
  label: string;
  latest?: string;
  onPick: Pick;
}) {
  return (
    <PickerTrigger label={label} icon={<CalendarDays aria-hidden="true" />}>
      {(close) => (
        <DateCalendar
          label={label}
          latest={latest}
          onPick={(value) => {
            onPick(value);
            close();
          }}
        />
      )}
    </PickerTrigger>
  );
}

const twoDigits = (count: number) =>
  Array.from({ length: count }, (_, index) => {
    const id = String(index).padStart(2, "0");
    return { id, label: id };
  });
const HOURS = twoDigits(24);
const first = (keys: Selection) => (keys === "all" ? "" : String([...keys][0] ?? ""));
const MINUTES = twoDigits(60);

/**
 * Hour and minute lists (24-hour) that fill an `HH:MM` field: picking the hour keeps the
 * minutes already there, and picking the minutes closes the picker.
 */
export function TimePickerButton({
  label,
  value,
  onPick,
}: {
  label: string;
  value: string;
  onPick: Pick;
}) {
  const [hour = "", minute = ""] = /^\d{2}:\d{2}$/.test(value) ? value.split(":") : [];
  return (
    <PickerTrigger label={label} icon={<Clock aria-hidden="true" />}>
      {(close) => (
        <div className={styles.time}>
          <ListBox
            aria-label="Hour"
            items={HOURS}
            selectionMode="single"
            selectedKeys={hour ? [hour] : []}
            disallowEmptySelection
            onSelectionChange={(keys) => onPick(`${first(keys)}:${minute || "00"}`)}
            className={styles.column}
          >
            {(item) => <ListBoxItem className={styles.option}>{item.label}</ListBoxItem>}
          </ListBox>
          <ListBox
            aria-label="Minute"
            items={MINUTES}
            selectionMode="single"
            selectedKeys={minute ? [minute] : []}
            disallowEmptySelection
            onSelectionChange={(keys) => {
              onPick(`${hour || "00"}:${first(keys)}`);
              close();
            }}
            className={styles.column}
          >
            {(item) => <ListBoxItem className={styles.option}>{item.label}</ListBoxItem>}
          </ListBox>
        </div>
      )}
    </PickerTrigger>
  );
}
