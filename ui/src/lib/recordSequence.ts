import { type NavigateOptions, useLocation, useNavigate } from "react-router";

/** A list as the breadcrumb shows it: its page title and its address. */
export interface ListCrumb {
  label: string;
  /** Path plus search, so the list's find text and filters come back with it. */
  href: string;
}

/** The list a record page was opened from: its crumb and its row IDs in display order. */
interface RecordSequence {
  list: ListCrumb;
  ids: readonly string[];
}

/** A link to a neighbouring record that carries the list's sequence on to it. */
interface SequenceLink {
  href: string;
  routerOptions: NavigateOptions;
}

/** What a record page shows of the list it came from (see `useRecordSequence`). */
export interface RecordSequenceLinks {
  /**
   * The breadcrumb's list: the one the record was opened from (such as a work queue), as it
   * was, or the record's own directory when that is unknown.
   */
  list: ListCrumb;
  /** Previous and Next in the list's order, `null` at either end; `null` without a list. */
  sequence: { previous: SequenceLink | null; next: SequenceLink | null } | null;
  /**
   * Router options for other links within the record, such as its `Tabs`, so the breadcrumb
   * and Previous and Next survive moving between them.
   */
  keep: NavigateOptions;
}

function crumbOf(value: unknown): ListCrumb | null {
  if (typeof value !== "object" || value === null) return null;
  if (!("label" in value) || typeof value.label !== "string") return null;
  if (!("href" in value) || typeof value.href !== "string") return null;
  return { label: value.label, href: value.href };
}

function sequenceOf(state: unknown): RecordSequence | null {
  if (typeof state !== "object" || state === null || !("recordSequence" in state)) return null;
  const sequence: unknown = state.recordSequence;
  if (typeof sequence !== "object" || sequence === null || !("list" in sequence)) return null;
  const list = crumbOf(sequence.list);
  if (list === null || !("ids" in sequence) || !Array.isArray(sequence.ids)) return null;
  const ids = sequence.ids.filter((id): id is string => typeof id === "string");
  return { list, ids };
}

/**
 * The list side: returns the row action that opens a row's record page, passing the list's
 * title (`listLabel`, its `h1`), its address and its rows' IDs in display order (as
 * `DataGrid` gives them) in navigation state. The state lives only in this window's
 * history; nothing is stored elsewhere.
 */
export function useOpenRecordPage(
  recordPath: (id: string) => string,
  listLabel: string,
): (id: string, orderedIds: readonly string[]) => void {
  const navigate = useNavigate();
  const { pathname, search } = useLocation();
  return (id, orderedIds) => {
    const list = { label: listLabel, href: `${pathname}${search}` };
    const recordSequence: RecordSequence = { list, ids: orderedIds };
    void navigate(recordPath(id), { state: { recordSequence } });
  };
}

/**
 * The record side: the breadcrumb's list and Previous and Next for the record `id`, from the
 * state `useOpenRecordPage` passed. Without that state (a link from elsewhere, a scan, a reload
 * that lost it) the breadcrumb goes to `home`, the record's own directory, and there is no
 * Previous or Next.
 * Previous and Next replace the history entry, so Back returns to the list. They are links:
 * a neighbour loads only when the person follows one, never in advance.
 */
export function useRecordSequence(
  id: string,
  home: ListCrumb,
  recordPath: (id: string) => string,
): RecordSequenceLinks {
  const { state } = useLocation();
  const keep: NavigateOptions = { state };
  const sequence = sequenceOf(state);
  const index = sequence === null ? -1 : sequence.ids.indexOf(id);
  if (sequence === null || index === -1) return { list: home, sequence: null, keep };
  const link = (neighbour: string | undefined): SequenceLink | null =>
    neighbour === undefined
      ? null
      : { href: recordPath(neighbour), routerOptions: { state, replace: true } };
  return {
    list: sequence.list,
    sequence: { previous: link(sequence.ids[index - 1]), next: link(sequence.ids[index + 1]) },
    keep,
  };
}
