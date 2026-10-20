// The addresses of the billing feature's record pages, one home each: lists open them and the
// pages use them for their breadcrumb and Previous and Next.

const segment = encodeURIComponent;

export const derivationPath = (derivationId: string) =>
  `/billing/derivations/${segment(derivationId)}`;

export const handoffPath = (handoffId: string) => `/billing/handoffs/${segment(handoffId)}`;

/** The class mapping, one versioned record, showing version `version` (`?version=`). */
export const classMappingPath = (version: number) =>
  `/billing/class-mappings?${new URLSearchParams({ version: String(version) })}`;
