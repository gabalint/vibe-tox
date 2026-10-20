import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
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
      <li>{person(OTHER, "Synthetic Reviewer")}</li>
      <li>{person(OTHER)}</li>
    </ul>
  );
}

const shown = () => screen.getAllByRole("listitem").map((item) => item.textContent);

describe("usePersonText", () => {
  it("says you for the signed-in person, others by display name, else by user ID", async () => {
    mockCommands({ current_session: () => SESSION });
    renderWithProviders(<People />);
    expect(await screen.findByText("you")).toBeTruthy();
    expect(shown()).toEqual(["you", "Synthetic Reviewer", `user ${OTHER}`]);
  });

  it("says the screen's wording for someone the engine sent no name for", async () => {
    mockCommands({ current_session: () => SESSION });
    renderWithProviders(<People unnamed="another user" />);
    expect(await screen.findByText("you")).toBeTruthy();
    expect(shown()).toEqual(["you", "Synthetic Reviewer", "another user"]);
  });
});
