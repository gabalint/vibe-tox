import { useEffect, useRef } from "react";
import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import { LoadingState } from "@/components/LoadingState";
import { CATALOG_QUERIES } from "@/lib/catalogQueries";
import { useCommandMutation } from "@/lib/commands";
import styles from "@/styles/layout.module.css";

/** Completes the persisted first-run choice after authentication (REQ-OPS-016). */
export function DemoSetupProgress() {
  const { mutate, error } = useCommandMutation<null>("complete_demo_setup", {
    invalidates: [
      "setup_status",
      "demo_status",
      "standard_catalog_status",
      "standard_catalog_tree",
      ...CATALOG_QUERIES,
      "get_laboratory_profile",
      "list_clients",
      "list_practitioners",
      "list_delivery_destinations",
      "list_identity_holds",
      "list_work_queues",
    ],
  });
  const signOut = useCommandMutation<null>("sign_out", { invalidates: "all" });
  const started = useRef(false);

  // Synchronize the engine's persisted setup request once, including StrictMode's replay.
  // Failure waits for an explicit retry; the engine makes completion atomic and idempotent.
  useEffect(() => {
    if (started.current) return;
    started.current = true;
    mutate({});
  }, [mutate]);

  return (
    <div className={styles.stack}>
      <h1>Preparing your demo laboratory</h1>
      <p>
        Your account is ready. The standard catalog and fictitious clinics, practitioners and
        patients will be ready to explore when this step finishes.
      </p>
      {error ? (
        <>
          <ErrorState
            title="The demo could not be prepared"
            error={error}
            onRetry={signOut.isPending ? undefined : () => mutate({})}
          />
          {signOut.error && <ErrorState title="Sign-out failed" error={signOut.error} />}
          <div>
            <Button onPress={() => signOut.mutate({})} isPending={signOut.isPending}>
              Sign out
            </Button>
          </div>
        </>
      ) : (
        <LoadingState label="Loading the catalog and sample data…" />
      )}
      <p className={styles.hint}>
        Catalog entries start as drafts. Review and approve them before trying laboratory work.
      </p>
    </div>
  );
}
