import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { SessionInfo } from "@/generated/SessionInfo";
import { usePersonText } from "./person";
import { mockCommands } from "./test/ipc";
import { renderWithProviders } from "./test/render";

const ME = "00000000-0000-7000-8000-000000000a01";
const OTHER = "00000000-0000-7000-8000-000000000a02";

/** A synthetic signed-in session. */
const SESSION: SessionInfo = {
  userId: ME,
  username: "synthetic.user",
  displayName: "Synthetic User",
  roles: [],
  capabilities: [],
  idleTimeoutSeconds: 900,
  locked: false,
};

function People({ unnamed }: { unnamed?: string }) {
  const person = usePersonText(unnamed);
  return (
    <ul>
      <li>{person(ME, "Synthetic User")}</li>
      <li>{person(ME)}</li>
      <li>{person(OTHER, "Synthetic Reviewer")}</li>
      <li>{person(OTHER)}</li>
      <li>{person("result-import")}</li>
    </ul>
  );
}

const shown = () => screen.getAllByRole("listitem").map((item) => item.textContent);

describe("usePersonText", () => {
  it("names everyone by display name, the signed-in person included, else by user ID or component", async () => {
    mockCommands({ current_session: () => SESSION });
    renderWithProviders(<People />);
    expect(await screen.findByText("Synthetic Reviewer")).toBeTruthy();
    await vi.waitFor(() => expect(shown()[1]).toBe("Synthetic User"));
    expect(shown()).toEqual([
      "Synthetic User",
      "Synthetic User",
      "Synthetic Reviewer",
      `user ${OTHER}`,
      "result-import",
    ]);
  });

  it("says the screen's wording for someone the engine sent no name for", async () => {
    mockCommands({ current_session: () => SESSION });
    renderWithProviders(<People unnamed="another user" />);
    await vi.waitFor(() => expect(shown()[1]).toBe("Synthetic User"));
    expect(shown()).toEqual([
      "Synthetic User",
      "Synthetic User",
      "Synthetic Reviewer",
      "another user",
      "another user",
    ]);
  });
});
