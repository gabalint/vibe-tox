import { useEffect } from "react";
import { useToast } from "@/components/Toast";
import { useCommandMutation } from "@/lib/commands";
import { toUserMessage } from "@/lib/errors";

/** Input that counts as the person being at the computer. */
const ACTIVITY_EVENTS: readonly string[] = ["keydown", "pointerdown", "pointermove", "wheel"];

/** Activity is reported to the engine at most this often. */
const TOUCH_INTERVAL_MS = 60_000;

/**
 * The UI side of the idle lock (REQ-IDN-004, NFR-19). Input keeps the engine's idle timer
 * current through `touch_session`, at most once a minute; after `idleTimeoutSeconds`
 * without input the session is locked. The engine enforces the timeout on its own as
 * well, so a missed touch can only lock the session sooner, never keep it open longer.
 */
export function IdleLock({ idleTimeoutSeconds }: { idleTimeoutSeconds: number }) {
  const toast = useToast();
  const { mutate: touch } = useCommandMutation<null>("touch_session", { invalidates: [] });
  const { mutate: lock } = useCommandMutation<null>("lock_session", { invalidates: "all" });

  // Synchronizes with input events and timers, which live outside React.
  useEffect(() => {
    const timeoutMs = idleTimeoutSeconds * 1000;
    const lockNow = () =>
      lock(
        {},
        {
          onError: (error) =>
            toast.show({
              tone: "danger",
              title: "The session could not be locked",
              description: toUserMessage(error),
            }),
        },
      );
    // The session was just started, unlocked or reported active by the engine.
    let lastTouch = Date.now();
    let idleTimer = setTimeout(lockNow, timeoutMs);
    const onActivity = () => {
      clearTimeout(idleTimer);
      idleTimer = setTimeout(lockNow, timeoutMs);
      const now = Date.now();
      if (now - lastTouch < TOUCH_INTERVAL_MS) return;
      lastTouch = now;
      touch({});
    };
    for (const type of ACTIVITY_EVENTS) {
      document.addEventListener(type, onActivity, { capture: true, passive: true });
    }
    return () => {
      clearTimeout(idleTimer);
      for (const type of ACTIVITY_EVENTS) {
        document.removeEventListener(type, onActivity, { capture: true });
      }
    };
  }, [idleTimeoutSeconds, touch, lock, toast]);

  return null;
}
