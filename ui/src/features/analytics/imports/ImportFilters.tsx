import type { ReactNode } from "react";
import { Button } from "@/components/Button";
import { DayFilter } from "@/components/DayFilter";
import { FindField } from "@/components/FindField";
import { Select } from "@/components/Select";
import { Toolbar } from "@/components/Toolbar";
import type { InstrumentReadinessView } from "@/generated/InstrumentReadinessView";
import { choiceOf } from "@/lib/choices";
import { type ImportFilter, isFiltered, OUTCOME_FILTERS } from "./importFilter";

/** The option for "no filter" in the instrument and outcome lists. */
const ANY = "any";

interface ImportFiltersProps {
  filter: ImportFilter;
  instruments: readonly InstrumentReadinessView[];
  onApply: (filter: ImportFilter) => void;
  count?: ReactNode;
  /** Refresh and the list's own actions, after Clear filters. */
  actions?: ReactNode;
}

/**
 * The search, as a find after a pause in typing, and the instrument and outcome, applied when
 * chosen; the received days under More filters. Every filter combines with the others and
 * Clear filters removes them all. The engine applies them before the list is paged.
 */
export function ImportFilters({
  filter,
  instruments,
  onApply,
  count,
  actions,
}: ImportFiltersProps) {
  const received = receivedText(filter);
  return (
    <Toolbar
      variant="collection"
      align="end"
      count={count}
      actions={
        <>
          {isFiltered(filter) && (
            <Button variant="quiet" onPress={() => onApply({})}>
              Clear filters
            </Button>
          )}
          {actions}
        </>
      }
      moreFilters={{
        content: (
          <>
            <DayFilter
              label="Received from"
              name="from"
              value={filter.from}
              latest={filter.to}
              onChange={(from) => onApply({ ...filter, from })}
            />
            <DayFilter
              label="Received to"
              name="to"
              value={filter.to}
              earliest={filter.from}
              onChange={(to) => onApply({ ...filter, to })}
            />
          </>
        ),
        applied: received && `Received: ${received}`,
      }}
    >
      <FindField
        label="Find imports by sample label, accession, container label or file name"
        placeholder="Sample label, accession, container or file name"
        isLabelHidden
        value={filter.search ?? ""}
        onChange={(text) => onApply({ ...filter, search: text.trim() || undefined })}
      />
      <Select
        label="Instrument"
        name="instrument"
        options={[
          { id: ANY, label: "All instruments" },
          ...instruments.map((instrument) => ({
            id: instrument.instrumentId,
            label: instrument.name,
          })),
        ]}
        value={filter.instrumentId ?? ANY}
        onChange={(next) =>
          onApply({ ...filter, instrumentId: next === ANY ? undefined : (next ?? undefined) })
        }
      />
      <Select
        label="Outcome"
        name="outcome"
        options={[{ id: ANY, label: "Any outcome" }, ...OUTCOME_FILTERS]}
        value={filter.outcome ?? ANY}
        onChange={(next) =>
          onApply({ ...filter, outcome: choiceOf(OUTCOME_FILTERS, next) ?? undefined })
        }
      />
    </Toolbar>
  );
}

/** The applied received days in words, for the line under the toolbar. */
function receivedText({ from, to }: ImportFilter): string | undefined {
  if (from && to) return from === to ? from : `${from} to ${to}`;
  if (from) return `from ${from}`;
  if (to) return `up to ${to}`;
  return undefined;
}
