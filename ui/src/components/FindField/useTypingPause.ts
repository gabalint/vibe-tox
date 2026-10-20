import { useEffect, useRef } from "react";

/** How long typing must pause before a find or search runs. */
const TYPING_PAUSE_MS = 250;

/**
 * Calls `callback` once typing has paused (`later`), or at once (`now`), which cancels a
 * pending call. A pending call is dropped when the component unmounts.
 */
export function useTypingPause(callback: (text: string) => void) {
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  // Synchronizes with the timer, outside React: a pending call must not outlive the field.
  useEffect(() => () => clearTimeout(timer.current), []);
  return {
    later: (text: string) => {
      clearTimeout(timer.current);
      timer.current = setTimeout(() => callback(text), TYPING_PAUSE_MS);
    },
    now: (text: string) => {
      clearTimeout(timer.current);
      callback(text);
    },
  };
}
