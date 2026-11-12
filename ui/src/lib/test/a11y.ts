import axe from "axe-core";
import { expect } from "vitest";

/**
 * Fails with the list of axe violations inside `container` (NFR-09). Components are
 * checked outside page landmarks, so the landmark rule runs only for whole screens.
 */
export async function expectNoA11yViolations(
  container: Element,
  { wholePage = false }: { wholePage?: boolean } = {},
) {
  // react-aria's hidden live announcer can announce a pending button by its id after the button
  // has unmounted; that library node is not the screen under test.
  const context = { include: container, exclude: "[data-live-announcer]" };
  const results = await axe.run(context, { rules: { region: { enabled: wholePage } } });
  const violations = results.violations.map(
    (violation) =>
      `${violation.id}: ${violation.help} (${violation.nodes.map((node) => node.target.join(" ")).join(", ")})`,
  );
  expect(violations).toEqual([]);
}
