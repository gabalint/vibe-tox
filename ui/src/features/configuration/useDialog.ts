import { useState } from "react";

/** Whether one dialog is open, for a section that offers a single dialog. */
export function useDialog() {
  const [isOpen, setOpen] = useState(false);
  return { isOpen, open: () => setOpen(true), close: () => setOpen(false) };
}
