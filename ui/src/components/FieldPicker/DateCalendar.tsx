import { ChevronLeft, ChevronRight } from "lucide-react";
import { useContext, useState } from "react";
import {
  Button,
  Calendar,
  CalendarCell,
  CalendarGrid,
  CalendarStateContext,
  useLocale,
} from "react-aria-components";
import styles from "./FieldPicker.module.css";
import { MonthsView, YearsView } from "./PeriodViews";

type View = "days" | "months" | "years";

/** The days of the focused month; its heading opens the months. */
function DaysView({ onHeading }: { onHeading: () => void }) {
  const state = useContext(CalendarStateContext);
  const { locale } = useLocale();
  if (!state) return null;
  const heading = new Intl.DateTimeFormat(locale, {
    month: "long",
    year: "numeric",
    timeZone: state.timeZone,
  }).format(state.focusedDate.toDate(state.timeZone));
  return (
    <>
      <header className={styles.calendarHeader}>
        <Button slot="previous" aria-label="Previous month" className={styles.trigger}>
          <ChevronLeft aria-hidden="true" />
        </Button>
        <Button
          slot={null}
          aria-label={`${heading}, choose the month`}
          onPress={onHeading}
          className={styles.heading}
        >
          {heading}
        </Button>
        <Button slot="next" aria-label="Next month" className={styles.trigger}>
          <ChevronRight aria-hidden="true" />
        </Button>
      </header>
      <CalendarGrid className={styles.grid}>
        {(date) => <CalendarCell date={date} className={styles.day} />}
      </CalendarGrid>
    </>
  );
}

/**
 * Days, months of a year and years of a decade: a heading opens the next level up, and a
 * pick or Escape comes back down. Days after `latest` (`YYYY-MM-DD`) can't be picked, nor can
 * the months and years that hold only such days.
 */
export function DateCalendar({
  label,
  latest,
  onPick,
}: {
  label: string;
  latest?: string;
  onPick: (value: string) => void;
}) {
  const [view, setView] = useState<View>("days");
  return (
    <Calendar
      aria-label={label}
      // ISO dates compare correctly as text.
      isDateUnavailable={latest ? (date) => date.toString() > latest : undefined}
      onChange={(date) => onPick(date.toString())}
      className={styles.calendar}
    >
      {view === "days" && <DaysView onHeading={() => setView("months")} />}
      {view === "months" && (
        <MonthsView onHeading={() => setView("years")} onDone={() => setView("days")} />
      )}
      {view === "years" && (
        <YearsView onPicked={() => setView("months")} onDone={() => setView("days")} />
      )}
    </Calendar>
  );
}
