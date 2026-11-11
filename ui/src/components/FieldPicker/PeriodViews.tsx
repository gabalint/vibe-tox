import { ChevronLeft, ChevronRight } from "lucide-react";
import { type KeyboardEvent, type ReactNode, useContext } from "react";
import {
  Button,
  CalendarMonthPicker,
  type CalendarState,
  CalendarStateContext,
  Group,
  ListBox,
  ListBoxItem,
} from "react-aria-components";
import styles from "./FieldPicker.module.css";

type CalendarDate = CalendarState["focusedDate"];

/** Whether no day from `start` to `end` can be picked, judged by its first and last day. */
const isClosed = (
  state: Pick<CalendarState, "isInvalid" | "isCellUnavailable">,
  start: CalendarDate,
  end: CalendarDate,
) => [start, end].every((date) => state.isInvalid(date) || state.isCellUnavailable(date));

interface Period {
  id: number;
  label: string;
  isDisabled: boolean;
}

/** ‹ title › over a months or years grid; the title opens the next level up when it can. */
function PeriodHeader({
  title,
  unit,
  onMove,
  onHeading,
}: {
  title: string;
  unit: string;
  onMove: (direction: 1 | -1) => void;
  onHeading?: () => void;
}) {
  return (
    <header className={styles.calendarHeader}>
      <Button
        slot={null}
        aria-label={`Previous ${unit}`}
        onPress={() => onMove(-1)}
        className={styles.trigger}
      >
        <ChevronLeft aria-hidden="true" />
      </Button>
      {onHeading ? (
        <Button
          slot={null}
          aria-label={`${title}, choose the year`}
          onPress={onHeading}
          className={styles.heading}
        >
          {title}
        </Button>
      ) : (
        <span className={styles.month}>{title}</span>
      )}
      <Button
        slot={null}
        aria-label={`Next ${unit}`}
        onPress={() => onMove(1)}
        className={styles.trigger}
      >
        <ChevronRight aria-hidden="true" />
      </Button>
    </header>
  );
}

/**
 * A grid of months or years under its header. Arrow keys move, Enter or a click picks, and
 * Escape goes back to the days instead of closing the calendar.
 */
function PeriodGrid({
  label,
  header,
  periods,
  selected,
  onPick,
  onDone,
}: {
  label: string;
  header: ReactNode;
  periods: Period[];
  selected: number;
  onPick: (id: number) => void;
  onDone: () => void;
}) {
  const state = useContext(CalendarStateContext);
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key !== "Escape") return;
    event.stopPropagation();
    event.preventDefault();
    state?.setFocused(true);
    onDone();
  };
  return (
    <Group onKeyDown={onKeyDown}>
      {header}
      <ListBox
        aria-label={label}
        layout="grid"
        items={periods}
        selectionMode="single"
        selectedKeys={[selected]}
        disabledKeys={periods.filter((period) => period.isDisabled).map((period) => period.id)}
        escapeKeyBehavior="none"
        autoFocus
        // Picking the selected period again empties the selection: it is still a pick.
        onSelectionChange={(keys) =>
          onPick(keys === "all" ? selected : Number([...keys][0] ?? selected))
        }
        className={styles.periods}
      >
        {(period) => <ListBoxItem className={styles.period}>{period.label}</ListBoxItem>}
      </ListBox>
    </Group>
  );
}

/** The months of the focused year; picking one shows its days. */
export function MonthsView({ onHeading, onDone }: { onHeading: () => void; onDone: () => void }) {
  const state = useContext(CalendarStateContext);
  if (!state) return null;
  const focused = state.focusedDate;
  return (
    <CalendarMonthPicker format="short">
      {({ items }) => (
        <PeriodGrid
          label="Month"
          header={
            <PeriodHeader
              title={String(focused.year)}
              unit="year"
              onMove={(years) => state.setFocusedDate(focused.add({ years }))}
              onHeading={onHeading}
            />
          }
          periods={items.map(({ id, date, formatted }) => {
            const start = date.set({ day: 1 });
            const end = start.add({ months: 1 }).subtract({ days: 1 });
            return { id, label: formatted, isDisabled: isClosed(state, start, end) };
          })}
          selected={focused.month}
          onPick={(month) => {
            state.setFocusedDate(focused.set({ month }));
            state.setFocused(true);
            onDone();
          }}
          onDone={onDone}
        />
      )}
    </CalendarMonthPicker>
  );
}

/** The decade of the focused year; picking a year shows its months. */
export function YearsView({ onPicked, onDone }: { onPicked: () => void; onDone: () => void }) {
  const state = useContext(CalendarStateContext);
  if (!state) return null;
  const focused = state.focusedDate;
  const first = focused.year - (focused.year % 10);
  const periods = Array.from({ length: 10 }, (_, index) => {
    const start = focused.set({ year: first + index, month: 1, day: 1 });
    const end = start.add({ years: 1 }).subtract({ days: 1 });
    return { id: start.year, label: String(start.year), isDisabled: isClosed(state, start, end) };
  });
  return (
    <PeriodGrid
      label="Year"
      header={
        <PeriodHeader
          title={`${first} – ${first + 9}`}
          unit="decade"
          onMove={(direction) => state.setFocusedDate(focused.add({ years: 10 * direction }))}
        />
      }
      periods={periods}
      selected={focused.year}
      onPick={(year) => {
        state.setFocusedDate(focused.set({ year }));
        onPicked();
      }}
      onDone={onDone}
    />
  );
}
