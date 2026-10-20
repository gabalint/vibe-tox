// The addresses of the reports feature's record pages, one home each: lists open them and the
// pages use them for their breadcrumb and Previous and Next.

const segment = encodeURIComponent;

export const reportPath = (reportId: string) => `/reports/${segment(reportId)}`;

export const revisionPath = (revisionId: string) => `/reports/revisions/${segment(revisionId)}`;

/** A specimen's report workspace. */
export const workspacePath = (specimenId: string) => `/reports/specimens/${segment(specimenId)}`;

export const candidatePath = (specimenId: string, candidateId: string) =>
  `${workspacePath(specimenId)}/candidates/${segment(candidateId)}`;

/**
 * One obligation in the delivery workspace's panel, under "All open" so it shows whatever its
 * status. Closed obligations leave the workspace.
 */
export const obligationPath = (obligationId: string) =>
  `/delivery?${new URLSearchParams({ filter: "all", open: obligationId })}`;
