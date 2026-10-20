import type { ReactNode } from "react";
import { Button } from "@/components/Button";
import { Checkbox } from "@/components/Checkbox";
import { FindField } from "@/components/FindField";
import { Select } from "@/components/Select";
import { Toolbar } from "@/components/Toolbar";
import { optionalText } from "@/lib/choices";
import type { AuditFilter } from "./contract";
import { isStream, STREAMS } from "./streams";

/** The option for "no filter" in the action and record type lists. */
const ANY = "any";

/** Codes and their labels, as the engine lists them for one stream. */
export type CodeLabels = readonly (readonly [string, string])[];

interface AuditFiltersProps {
  filter: AuditFilter;
  count?: ReactNode;
  /** Every action and record type recorded in the shown stream, from the engine. */
  actions: CodeLabels;
  recordTypes: CodeLabels;
  onApply: (filter: AuditFilter) => void;
  /** The record ID to find, after a pause in typing or on Enter; empty finds every record. */
  onFind: (recordId: string | undefined) => void;
}

/**
 * The stream, action and record type apply when chosen; the record ID as a find, after a
 * pause in typing. Actions and record types belong to a stream, so changing the stream
 * clears them. "Self-approvals only" lists the steps one person did alone under the
 * laboratory's separation-of-duties policy (ADR 0015), for the laboratory's review of them.
 */
export function AuditFilters({
  filter,
  count,
  actions,
  recordTypes,
  onApply,
  onFind,
}: AuditFiltersProps) {
  const recordTypeLabel =
    recordTypes.find(([id]) => id === filter.recordType)?.[1] ?? filter.recordType;
  const filtered = Boolean(
    filter.action || filter.recordType || filter.recordId || filter.selfApprovalsOnly,
  );
  return (
    <Toolbar
      variant="collection"
      count={count}
      actions={
        filtered && (
          <Button variant="quiet" onPress={() => onApply({ stream: filter.stream })}>
            Clear filters
          </Button>
        )
      }
      moreFilters={{
        content: (
          <CodeSelect
            label="Record type"
            any="Any record type"
            codes={recordTypes}
            value={filter.recordType}
            onChange={(recordType) => onApply({ ...filter, recordType })}
          />
        ),
        applied: recordTypeLabel ? `Record type: ${recordTypeLabel}` : undefined,
      }}
    >
      <FindField
        label="Find by record ID"
        placeholder="Record ID"
        value={filter.recordId ?? ""}
        onChange={(text) => onFind(optionalText(text) ?? undefined)}
      />
      <Select
        layout="inline"
        label="Stream"
        name="stream"
        options={STREAMS}
        value={filter.stream}
        onChange={(stream) => {
          if (isStream(stream)) {
            onApply({
              stream,
              recordId: filter.recordId,
              selfApprovalsOnly: filter.selfApprovalsOnly,
            });
          }
        }}
      />
      <CodeSelect
        label="Action"
        any="Any action"
        codes={actions}
        value={filter.action}
        onChange={(action) => onApply({ ...filter, action })}
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
      layout="inline"
      label={label}
      name={label}
      options={options}
      value={value ?? ANY}
      onChange={(next) => onChange(next === null || next === ANY ? undefined : next)}
    />
  );
}
