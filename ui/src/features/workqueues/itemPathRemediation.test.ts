import { describe, expect, it } from "vitest";
import { itemTarget } from "./itemPath";

const HANDOFF = "00000000-0000-7000-8000-000000000001";
const ITEM = "00000000-0000-7000-8000-000000000002";
const PATIENT = "00000000-0000-7000-8000-000000000003";

describe("Queue exact targets after remediation (UI-10)", () => {
  it("opens a shared-lot deviation directly without inventing a run", () => {
    expect(itemTarget({ itemKind: "deviation", itemId: ITEM, state: "open" })).toEqual({
      recordId: ITEM,
      path: `/quality/deviations/${ITEM}`,
    });
  });
  it("opens a handed-off billing item independently of the first billing-work page", () => {
    expect(
      itemTarget({
        itemKind: "billing_handoff_item",
        itemId: `${HANDOFF}:${ITEM}`,
        state: "outstanding",
      }),
    ).toEqual({ recordId: HANDOFF, path: `/billing/handoffs/${HANDOFF}?open=${ITEM}` });
  });
  it.each(["2026-10-11", ""])(
    "retains exact patient/date group %s including explicit unknown",
    (date) => {
      const target = itemTarget({
        itemKind: "billing_subject",
        itemId: `${PATIENT}|${date}`,
        state: "needs_derivation",
      });
      expect(target).not.toBeNull();
      const url = new URL(target?.path ?? "", "http://synthetic.invalid");
      expect(url.pathname).toBe("/billing/work");
      expect(url.searchParams.get("subjectId")).toBe(PATIENT);
      expect(url.searchParams.get("dateOfService")).toBe(date);
      expect(url.searchParams.get("open")).toBe(`${PATIENT}|${date}`);
    },
  );
});
