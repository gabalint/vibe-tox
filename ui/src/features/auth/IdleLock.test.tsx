import { fireEvent, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SessionInfo } from "@/generated/SessionInfo";
import { mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { AuthGate } from "./AuthGate";
import { IdleLock } from "./IdleLock";
import { TEST_SESSION } from "./testSession";

const MINUTE = 60_000;
const TIMEOUT_SECONDS = 900;
/**
 * The fake clock also follows real time (`shouldAdvanceTime`), so a check just before the
 * timeout keeps this margin: a busy test run can spend a few real milliseconds between steps.
 */
const MARGIN_MS = 1000;

function mockSessionCommands() {
  const touch = vi.fn(() => null);
  const lock = vi.fn(() => null);
  mockCommands({ touch_session: touch, lock_session: lock });
  return { touch, lock };
}

/** Advances the clock, then lets the IPC promises settle. */
async function wait(ms: number) {
  await vi.advanceTimersByTimeAsync(ms);
}

describe("IdleLock (REQ-IDN-004, NFR-19)", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("locks the session after the idle timeout without input", async () => {
    const { lock } = mockSessionCommands();
    renderWithProviders(<IdleLock idleTimeoutSeconds={TIMEOUT_SECONDS} />);
    await wait(TIMEOUT_SECONDS * 1000 - MARGIN_MS);
    expect(lock).not.toHaveBeenCalled();
    await wait(MARGIN_MS);
    expect(lock).toHaveBeenCalledTimes(1);
  });

  it("restarts the idle timeout on input", async () => {
    const { lock } = mockSessionCommands();
    renderWithProviders(<IdleLock idleTimeoutSeconds={TIMEOUT_SECONDS} />);
    await wait(10 * MINUTE);
    fireEvent.keyDown(document.body, { key: "a" });
    await wait(10 * MINUTE);
    fireEvent.pointerDown(document.body);
    await wait(TIMEOUT_SECONDS * 1000 - MARGIN_MS);
    expect(lock).not.toHaveBeenCalled();
    await wait(MARGIN_MS);
    expect(lock).toHaveBeenCalledTimes(1);
  });

  it("reports activity to the engine at most once a minute", async () => {
    const { touch } = mockSessionCommands();
    renderWithProviders(<IdleLock idleTimeoutSeconds={TIMEOUT_SECONDS} />);
    for (let second = 0; second < 59; second += 1) {
      fireEvent.keyDown(document.body, { key: "a" });
      await wait(1000);
    }
    expect(touch).not.toHaveBeenCalled();
    await wait(1000);
    fireEvent.pointerMove(document.body);
    fireEvent.keyDown(document.body, { key: "b" });
    fireEvent.wheel(document.body);
    await wait(0);
    expect(touch).toHaveBeenCalledTimes(1);
    await wait(30_000);
    fireEvent.keyDown(document.body, { key: "c" });
    await wait(0);
    expect(touch).toHaveBeenCalledTimes(1);
    await wait(MINUTE);
    fireEvent.keyDown(document.body, { key: "d" });
    await wait(0);
    expect(touch).toHaveBeenCalledTimes(2);
  });

  it("does not report activity without input", async () => {
    const { touch } = mockSessionCommands();
    renderWithProviders(<IdleLock idleTimeoutSeconds={TIMEOUT_SECONDS} />);
    await wait(10 * MINUTE);
    expect(touch).not.toHaveBeenCalled();
  });

  it("shows the lock screen once the idle session is locked", async () => {
    let session: SessionInfo = { ...TEST_SESSION, idleTimeoutSeconds: 60 };
    mockCommands({
      current_session: () => session,
      lock_session: () => {
        session = { ...session, locked: true };
        return null;
      },
    });
    renderWithProviders(<AuthGate>{() => <p>Synthetic worklist</p>}</AuthGate>);
    expect(await screen.findByText("Synthetic worklist")).toBeTruthy();
    await wait(MINUTE);
    expect(await screen.findByRole("heading", { name: "Session locked" })).toBeTruthy();
    expect(screen.queryByText("Synthetic worklist")).toBeNull();
  });
});
