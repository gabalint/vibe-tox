import row from "../actionRow.module.css";

/** What a test found, such as ✓ Connected or ✗ Not reachable; the next test replaces it. */
export interface TestOutcome {
  ok: boolean;
  text: string;
}

/** A test's latest result on its action row, as in the mockups. */
export function TestStatus({ status }: { status: TestOutcome | null }) {
  if (!status) return null;
  return (
    <span role="status" className={status.ok ? row.ok : row.problem}>
      {status.text}
    </span>
  );
}
