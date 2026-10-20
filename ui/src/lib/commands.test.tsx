import { onlineManager } from "@tanstack/react-query";
import { act, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Button } from "@/components/Button";
import { QueryContent } from "@/components/QueryContent";
import type { CommandName } from "@/generated/CommandName";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { useCommandMutation, useCommandPages, useCommandQuery } from "./commands";

function Probe({ invalidates }: { invalidates: readonly CommandName[] | "all" }) {
  const query = useCommandQuery<string>("get_laboratory_profile");
  const mutation = useCommandMutation<null>("set_laboratory_profile", { invalidates });
  return (
    <>
      <output>{query.data ?? "no data"}</output>
      <Button onPress={() => mutation.mutate({})}>Change</Button>
    </>
  );
}

function mockNames() {
  let calls = 0;
  mockCommands({
    get_laboratory_profile: () => {
      calls += 1;
      // The second read never answers, so the test sees what the cache shows meanwhile.
      return calls === 1 ? "first person" : new Promise(() => undefined);
    },
    set_laboratory_profile: () => null,
  });
}

describe("useCommandMutation", () => {
  it("keeps showing named queries while they refresh", async () => {
    mockNames();
    const { user } = renderWithProviders(<Probe invalidates={["get_laboratory_profile"]} />);
    expect(await screen.findByText("first person")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Change" }));
    await waitFor(() => expect(screen.getByText("first person")).toBeTruthy());
  });

  // REQ-IDN-004: after a session change nothing from the previous person stays on screen.
  it("clears every cached result for session changes", async () => {
    mockNames();
    const { user } = renderWithProviders(<Probe invalidates="all" />);
    expect(await screen.findByText("first person")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Change" }));
    expect(await screen.findByText("no data")).toBeTruthy();
  });
});

describe("createQueryClient", () => {
  // REQ-IDN-004: whichever screen finds the session locked, the session is asked for again.
  it("asks for the session again when a command reports it locked, but not in a loop", async () => {
    const currentSession = vi.fn(() =>
      Promise.reject(engineError("session_locked", "The session is locked.")),
    );
    mockCommands({
      current_session: currentSession,
      get_laboratory_profile: () => "first person",
      set_laboratory_profile: () =>
        Promise.reject(engineError("session_locked", "The session is locked.")),
    });
    function Session() {
      useCommandQuery<null>("current_session");
      return null;
    }
    const { user } = renderWithProviders(
      <>
        <Session />
        <Probe invalidates={[]} />
      </>,
    );
    await waitFor(() => expect(currentSession).toHaveBeenCalledTimes(1));
    await user.click(screen.getByRole("button", { name: "Change" }));
    await waitFor(() => expect(currentSession).toHaveBeenCalledTimes(2));
  });
});

// The engine runs in this process: the web view's online state never holds a command back.
describe("offline web view", () => {
  afterEach(() => onlineManager.setOnline(true));

  it("still reads and saves, and a reconnect re-runs no read", async () => {
    onlineManager.setOnline(false);
    const read = vi.fn(() => "first person");
    const save = vi.fn(() => null);
    mockCommands({ get_laboratory_profile: read, set_laboratory_profile: save });
    const { user } = renderWithProviders(<Probe invalidates={[]} />);
    expect(await screen.findByText("first person")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Change" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    act(() => onlineManager.setOnline(true));
    expect(read).toHaveBeenCalledTimes(1);
  });
});

// NFR-31: a name the engine does not register fails the type check, not integration.
describe("command names", () => {
  it("are checked against the engine's registry", () => {
    // @ts-expect-error: not a registered command
    const misspelled: CommandName = "list_user";
    expect(misspelled).not.toBe("list_users");
  });
});

// UI-07: object cursors survive paging, a failed page and explicit retry without losing rows.
it("retains loaded pages and retries the exact composite cursor", async () => {
  type Row = { id: string; since: string };
  let failNext = true;
  const read = vi.fn((args: import("@tauri-apps/api/core").InvokeArgs | undefined) => {
    const { input } = args as { input: { after?: Row } };
    if (!input.after) return [{ id: "first", since: "same-time" }];
    expect(input.after).toEqual({ id: "first", since: "same-time" });
    if (failNext) {
      failNext = false;
      throw engineError("conflict", "Refresh required");
    }
    return [{ id: "second", since: "same-time" }];
  });
  mockCommands({ list_reports: read });
  function Pages() {
    const query = useCommandPages<Row, Row>("list_reports", {
      key: {},
      pageArgs: (after) => ({ input: { after } }),
      nextCursor: (rows) => (rows[0]?.id === "first" ? rows[0] : undefined),
    });
    return (
      <>
        <QueryContent query={query} what="queue pages">
          {(data) => (
            <output>
              {data.pages
                .flat()
                .map((row) => row.id)
                .join(",")}
            </output>
          )}
        </QueryContent>
        <Button isDisabled={!query.hasNextPage} onPress={() => void query.fetchNextPage()}>
          More
        </Button>
      </>
    );
  }
  const { user } = renderWithProviders(<Pages />);
  expect(await screen.findByText("first")).toBeTruthy();
  await user.click(screen.getByRole("button", { name: "More" }));
  expect(
    await screen.findByText(
      "More queue pages could not be loaded. The loaded rows remain available.",
    ),
  ).toBeTruthy();
  expect(screen.getByText("first")).toBeTruthy();
  await user.click(screen.getByRole("button", { name: "Try again" }));
  expect(await screen.findByText("first,second")).toBeTruthy();
  expect(read).toHaveBeenCalledTimes(3);
});
