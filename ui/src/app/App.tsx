import { QueryClientProvider } from "@tanstack/react-query";
import { lazy, Suspense, useState } from "react";
import { createHashRouter, RouterProvider } from "react-router";
import { LoadingState } from "@/components/LoadingState";
import { ToastProvider } from "@/components/Toast";
import { DemoSetupContext } from "@/features/auth/demoSetupSlot";
import { DemoEntryContext } from "@/features/auth/demoSlot";
import { RestoreFlowContext } from "@/features/auth/restoreSlot";
import { DemoFrame } from "@/features/demo/DemoFrame";
import { DemoSetupProgress } from "@/features/demo/DemoSetupProgress";
import { ExploreDemo } from "@/features/demo/ExploreDemo";
import { createQueryClient } from "@/lib/commands";
import { routes } from "./routes";
import { StartupGate } from "./StartupFailure";

/**
 * Providers and routing. A hash router keeps every route inside the bundled
 * `index.html`, so the window never navigates to another document (NFR-31). A failed engine
 * start shows its own screen instead of the app (D11); the setup screen offers the restore
 * flow, which the operations feature owns and which loads only when someone starts it, and the
 * way into the demo laboratory. The demo laboratory's every screen carries its banner.
 */
export function App() {
  const [queryClient] = useState(createQueryClient);
  const [router] = useState(() => createHashRouter(routes));
  return (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <DemoFrame>
          <StartupGate>
            <RestoreFlowContext.Provider value={RestoreFlow}>
              <DemoEntryContext.Provider value={ExploreDemo}>
                <DemoSetupContext.Provider value={DemoSetupProgress}>
                  <RouterProvider router={router} />
                </DemoSetupContext.Provider>
              </DemoEntryContext.Provider>
            </RestoreFlowContext.Provider>
          </StartupGate>
        </DemoFrame>
      </ToastProvider>
    </QueryClientProvider>
  );
}

const RestoreWizard = lazy(() =>
  import("@/features/operations/restore/RestoreWizard").then((m) => ({ default: m.RestoreWizard })),
);

function RestoreFlow({ onExit }: { onExit: () => void }) {
  return (
    <Suspense fallback={<LoadingState />}>
      <RestoreWizard onExit={onExit} />
    </Suspense>
  );
}
