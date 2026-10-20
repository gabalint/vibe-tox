// Addresses of the bench and review record pages, and the lists they open from.

export const runPath = (runId: string) => `/bench/runs/${runId}`;
export const runQcPath = (runId: string) => `/bench/runs/${runId}/qc`;
export const specimenResultsPath = (specimenId: string) => `/review/specimens/${specimenId}`;
export const importAttemptPath = (importAttemptId: string) => `/review/imports/${importAttemptId}`;

/** Each list's `h1` and address: the breadcrumb of a record page not opened from a list. */
export const RUNS_LIST = { label: "Runs", href: "/bench/runs" };
export const REVIEW_LIST = { label: "Result review", href: "/review/queue" };
export const IMPORTS_LIST = { label: "Result imports", href: "/review/imports" };

/** Bench setup's instruments, and one instrument's panel there. */
export const BENCH_INSTRUMENTS = { label: "Bench setup", href: "/bench/setup/instruments" };
export const benchInstrumentPath = (instrumentId: string) =>
  `${BENCH_INSTRUMENTS.href}?open=${instrumentId}`;
/** The page that drafts a file template for a result-file instrument. */
export const fileTemplatePath = (instrumentId: string) =>
  `${BENCH_INSTRUMENTS.href}/${instrumentId}/file-template`;
/** One file source's template, where it is checked and approved. */
export const fileSourcePath = (instrumentId: string, registrationId: string) =>
  `${BENCH_INSTRUMENTS.href}/${instrumentId}/sources/${registrationId}`;
