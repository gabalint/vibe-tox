import type { ReactNode } from "react";
import { Button } from "@/components/Button";
import { Checkbox } from "@/components/Checkbox";
import { DayFilter } from "@/components/DayFilter";
import { Select } from "@/components/Select";
import { Toolbar } from "@/components/Toolbar";
import { AuditFind, type PickedRecord } from "./AuditFind";
import { type AuditFilter, hasFilters, isOutcome } from "./contract";
import { ALL_STREAMS, isStream, STREAMS } from "./streams";

/** The option for "no filter" in the code lists. */
const ANY = "any";

/** The stream choices: every stream first, the default. */
const STREAM_OPTIONS = [{ id: ALL_STREAMS, label: "All streams" }, ...STREAMS];

/** Codes and their labels, as the engine lists them for the shown stream or streams. */
export type CodeLabels = readonly (readonly [string, string])[];

/** The kinds of principal offered before the users in "By". */
const BY_KINDS: CodeLabels = [
  ["people", "People only"],
  ["system", "System only"],
];

/** Outcomes as the engine records them; Denied is a refusal by authorization. */
const OUTCOMES: CodeLabels = [
  ["succeeded", "Succeeded"],
  ["failed", "Failed"],
  ["denied", "Denied"],
];

interface AuditFiltersProps {
  filter: AuditFilter;
  count?: ReactNode;
  /** Every action and record type recorded in the shown stream or streams, from the engine. */
  actions: CodeLabels;
  recordTypes: CodeLabels;
  /** Every user, by ID and name, from the engine. */
  users: CodeLabels;
  /** The label of the record found, as the trail shows it. */
  pickedLabel?: string;
  onApply: (filter: AuditFilter) => void;
  /** The record picked in Find; nothing shows every record. */
  onFind: (record: PickedRecord | undefined) => void;
}

/**
 * Each filter applies when chosen, Find's record when picked, and the engine applies them all
 * in its query. Actions and record types belong to a stream, so changing the stream clears
 * them, and a record of another type than the one chosen is no longer found. "Self-approvals only" lists the steps one person did alone under the laboratory's
 * separation-of-duties policy (ADR 0015), for the laboratory's review of them.
 */
export function AuditFilters({
  filter,
  count,
  actions,
  recordTypes,
  users,
  pickedLabel,
  onApply,
  onFind,
}: AuditFiltersProps) {
  return (
    <Toolbar
      variant="collection"
      align="end"
      count={count}
      actions={
        hasFilters(filter) && (
          <Button variant="quiet" onPress={() => onApply({ stream: filter.stream })}>
            Clear filters
          </Button>
        )
      }
      moreFilters={{
        content: <MoreFilters filter={filter} recordTypes={recordTypes} onApply={onApply} />,
        applied: moreApplied(filter, recordTypes),
      }}
    >
      <AuditFind filter={filter} pickedLabel={pickedLabel} onPick={onFind} />
      <Select
        label="Stream"
        name="stream"
        options={STREAM_OPTIONS}
        value={filter.stream ?? ALL_STREAMS}
        onChange={(stream) =>
          onApply({
            ...filter,
            stream: isStream(stream) ? stream : undefined,
            action: undefined,
            recordType: undefined,
            recordId: undefined,
          })
        }
      />
      <CodeSelect
        label="Action"
        any="Any action"
        codes={actions}
        value={filter.action}
        onChange={(action) => onApply({ ...filter, action })}
      />
      <CodeSelect
        label="By"
        any="Anyone"
        codes={[...BY_KINDS, ...users]}
        value={filter.by}
        onChange={(by) => onApply({ ...filter, by })}
      />
      <CodeSelect
        label="Outcome"
        any="Any outcome"
        codes={OUTCOMES}
        value={filter.outcome}
        onChange={(outcome) =>
          onApply({ ...filter, outcome: isOutcome(outcome) ? outcome : undefined })
        }
      />
      <Checkbox
        name="selfApprovalsOnly"
        isSelected={filter.selfApprovalsOnly === true}
        onChange={(only) => onApply({ ...filter, selfApprovalsOnly: only || undefined })}
      >
        Self-approvals only
      </Checkbox>
    </Toolbar>
  );
}

interface MoreFiltersProps {
  filter: AuditFilter;
  recordTypes: CodeLabels;
  onApply: (filter: AuditFilter) => void;
}

/** The record type and the days recorded, less used than the filters in the row. */
function MoreFilters({ filter, recordTypes, onApply }: MoreFiltersProps) {
  return (
    <>
      <CodeSelect
        label="Record type"
        any="Any record type"
        codes={recordTypes}
        value={filter.recordType}
        onChange={(recordType) =>
          onApply({
            ...filter,
            recordType,
            // A record found as another type is not one of this type.
            recordId: recordType === filter.recordType ? filter.recordId : undefined,
          })
        }
      />
      <DayFilter
        label="Recorded from"
        name="from"
        value={filter.from}
        latest={filter.to}
        onChange={(from) => onApply({ ...filter, from })}
      />
      <DayFilter
        label="Recorded to"
        name="to"
        value={filter.to}
        earliest={filter.from}
        onChange={(to) => onApply({ ...filter, to })}
      />
    </>
  );
}

/** The filters applied in "More filters", shown while it is closed. */
function moreApplied(filter: AuditFilter, recordTypes: CodeLabels): string | undefined {
  const recordType =
    filter.recordType &&
    `Record type: ${recordTypes.find(([id]) => id === filter.recordType)?.[1] ?? filter.recordType}`;
  const { from, to } = filter;
  const days =
    from && to
      ? `Recorded ${from} to ${to}`
      : from
        ? `Recorded from ${from}`
        : to && `Recorded to ${to}`;
  return [recordType, days].filter(Boolean).join(" · ") || undefined;
}

interface CodeSelectProps {
  label: string;
  any: string;
  codes: CodeLabels;
  value: string | undefined;
  onChange: (value: string | undefined) => void;
}

/** A filter over engine codes, shown by their labels; a chosen code stays offered. */
function CodeSelect({ label, any, codes, value, onChange }: CodeSelectProps) {
  const options = [{ id: ANY, label: any }, ...codes.map(([id, text]) => ({ id, label: text }))];
  if (value !== undefined && !codes.some(([id]) => id === value)) {
    options.push({ id: value, label: value });
  }
  return (
    <Select
      label={label}
      name={label}
      options={options}
      value={value ?? ANY}
      onChange={(next) => onChange(next === null || next === ANY ? undefined : next)}
    />
  );
}
