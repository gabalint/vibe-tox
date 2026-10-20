import { QueryClientProvider } from "@tanstack/react-query";
import { render } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactElement } from "react";
import { ToastProvider } from "@/components/Toast";
import { createQueryClient } from "@/lib/commands";

/** Renders with the app's providers and a fresh engine cache; returns a `user` for events. */
export function renderWithProviders(ui: ReactElement) {
  const user = userEvent.setup();
  const result = render(
    <QueryClientProvider client={createQueryClient()}>
      <ToastProvider>{ui}</ToastProvider>
    </QueryClientProvider>,
  );
  return { ...result, user };
}
