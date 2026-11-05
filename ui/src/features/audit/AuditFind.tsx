import { useContext, useState } from "react";
import { type RecordOption, RecordPicker } from "@/components/RecordPicker";
import type { FindAuditRecordsInput } from "@/generated/FindAuditRecordsInput";
import type { FoundAuditRecord } from "@/generated/FoundAuditRecord";
import type { FoundAuditRecords } from "@/generated/FoundAuditRecords";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { isCalendarDate, LabTimeZoneContext, labDayStart, useFormatTime } from "@/lib/format";
import styles from "./audit.module.css";
import type { AuditFilter } from "./contract";
import { findHint } from "./findHints";
import { streamLabel } from "./streams";

/** The record that Find filters the trail by. */
export interface PickedRecord {
  recordType: string;
  recordId: string;
}

interface AuditFindProps {
  filter: AuditFilter;
  /** The picked record's label, as the trail shows it, until a search lists it. */
  pickedLabel?: string;
  /** Called with the record picked, or nothing when the field is cleared. */
  onPick: (record: PickedRecord | undefined) => void;
}

/** A record's option ID: its type and ID, which together are unique. */
const optionId = (recordType: string | undefined, recordId: string) =>
  JSON.stringify([recordType ?? null, recordId]);

/** Two characters, or one digit (a billing handoff number), as the engine searches. */
const isSearchable = (text: string) => text.length >= 2 || /^\d$/.test(text);

/**
 * Find (REQ-OPS-001): the records whose readable values start with the typed text, such as an
 * accession, an instrument code or a username, or whose record ID it is; the user picks one
 * and the trail shows its events. Its wording follows the chosen record type. Patients are
 * found by name only for people who may read them, and the engine records each search
 * without its text.
 */
export function AuditFind({ filter, pickedLabel, onPick }: AuditFindProps) {
  const [text, setText] = useState("");
  const ordersPatients = useHasCapability("enter_orders");
  const reviewsPatients = useHasCapability("manage_quality");
  const found = useFoundRecords(text.trim(), filter);
  const formatTime = useFormatTime();
  const hint = findHint(filter.recordType, ordersPatients || reviewsPatients);
  const records = found.data?.records ?? [];
  const options: RecordOption[] = records.map((record) => recordOption(record, formatTime));
  const value = filter.recordId === undefined ? null : optionId(filter.recordType, filter.recordId);
  if (value !== null && !options.some(({ id }) => id === value)) {
    options.unshift({ id: value, label: pickedLabel ?? filter.recordId ?? "" });
  }
  return (
    <div className={styles.find}>
      <RecordPicker
        label={hint.label}
        isLabelHidden
        placeholder={hint.placeholder}
        name="find"
        options={options}
        isLoading={found.isFetching}
        onInputChange={setText}
        description={
          found.data?.more
            ? "More records match than are listed: type more of the value."
            : undefined
        }
        value={value}
        onChange={(id) => {
          const record = records.find((each) => optionId(each.recordType, each.recordId) === id);
          if (id === null || record) onPick(record && pickedRecord(record));
        }}
      />
    </div>
  );
}

/** The engine's matches for `text`, in the record type and stream shown. */
function useFoundRecords(text: string, { recordType, stream }: AuditFilter) {
  const timeZone = useContext(LabTimeZoneContext);
  const start = isCalendarDate(text) ? labDayStart(text, timeZone) : null;
  const end = isCalendarDate(text) ? labDayStart(text, timeZone, 1) : null;
  const input: FindAuditRecordsInput = {
    text,
    recordType,
    stream,
    day: start && end ? { start, end } : undefined,
  };
  return useCommandQuery<FoundAuditRecords>(
    "find_audit_records",
    { input },
    { enabled: isSearchable(text) },
  );
}

/** A found record as an option: its label, then its type, streams and latest event. */
function recordOption(record: FoundAuditRecord, formatTime: (time: string) => string) {
  const streams = record.streams.map(streamLabel).join(", ");
  return {
    id: optionId(record.recordType, record.recordId),
    label: record.label ?? record.recordId,
    description: `${record.recordTypeLabel} · ${streams} · last event ${formatTime(record.lastRecordedAt)}`,
  } satisfies RecordOption;
}

function pickedRecord({ recordType, recordId }: FoundAuditRecord): PickedRecord {
  return { recordType, recordId };
}
