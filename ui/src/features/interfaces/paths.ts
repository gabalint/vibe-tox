// The addresses of the Interfaces screens, one home each.

export const CONNECTIONS_PATH = "/interfaces/connections";
export const LOGS_PATH = "/interfaces/logs";
export const UNROUTED_PATH = "/interfaces/unrouted";

export const connectionPath = (connectionId: string) =>
  `${CONNECTIONS_PATH}/${encodeURIComponent(connectionId)}`;
