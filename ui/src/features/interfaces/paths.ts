// The addresses of the Interfaces screens, one home each.

export const CONNECTIONS_PATH = "/interfaces/connections";
export const LOGS_PATH = "/interfaces/logs";
export const UNROUTED_PATH = "/interfaces/unrouted";

export const connectionPath = (connectionId: string) =>
  `${CONNECTIONS_PATH}/${encodeURIComponent(connectionId)}`;

const segment = encodeURIComponent;

/** The records a message is made from, on their own pages: Logs links its Record to them. */
export const reportPath = (reportId: string) => `/reports/${segment(reportId)}`;
export const revisionPath = (revisionId: string) => `/reports/revisions/${segment(revisionId)}`;
export const handoffPath = (handoffId: string) => `/billing/handoffs/${segment(handoffId)}`;
