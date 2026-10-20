import { lazy, Suspense, useId, useRef, useState } from "react";
import { Outlet } from "react-router";
import { LoadingState } from "@/components/LoadingState";
import { UserMenu } from "@/features/auth/UserMenu";
import { ListenerSummary } from "@/features/listeners/ListenerStatus";
import { HealthIndicator } from "@/features/operations/frame/HealthIndicator";
import { OperationsBanners } from "@/features/operations/frame/OperationsBanners";
import { StarterBanner } from "@/features/starter/StarterBanner";
import type { LaboratoryProfileView } from "@/generated/LaboratoryProfileView";
import type { SessionInfo } from "@/generated/SessionInfo";
import { GET_LABORATORY_PROFILE, useCommandQuery } from "@/lib/commands";
import { LabTimeZoneContext } from "@/lib/format";
import styles from "./AppShell.module.css";
import { helpGridStyle, PageHelpButton } from "./help/HelpResize";
import { useHelpLayout } from "./help/useHelpLayout";
import { ScanField } from "./ScanField";
import { Sidebar } from "./Sidebar";
import { APP_NAME, usePageHeading } from "./usePageHeading";

const PageHelp = lazy(() => import("./help/PageHelp").then((m) => ({ default: m.PageHelp })));

/**
 * The signed-in frame: a skip link, the top bar, notices every person must see, the side
 * navigation and the screen. The top bar shows the host's health, one analyzer summary, the
 * scan field (for people who look up containers) and the user menu. Every screen shows times
 * in the laboratory's time zone; until a profile names one, they show in UTC and the frame
 * says so. Banners (an open recovery reconciliation, an unaccepted version) sit under the top
 * bar. Showing or hiding a control is presentation; the engine checks every command.
 */
export function AppShell({ session }: { session: SessionInfo }) {
  const profile = useCommandQuery<LaboratoryProfileView | null>(GET_LABORATORY_PROFILE);
  const main = useRef<HTMLElement>(null);
  const helpPane = useRef<HTMLDivElement>(null);
  const helpTrigger = useRef<HTMLButtonElement>(null);
  const helpId = useId();
  const [helpState, setHelpState] = useState<"unopened" | "open" | "closed">("unopened");
  const helpOpen = helpState === "open";
  const helpLayout = useHelpLayout(main, helpPane, helpOpen);
  function closeHelp() {
    setHelpState("closed");
    helpTrigger.current?.focus();
  }
  usePageHeading(main);
  return (
    <LabTimeZoneContext.Provider value={profile.data?.timeZone ?? null}>
      <div
        className={styles.shell}
        data-help-open={helpOpen}
        // Runtime panel geometry from the resize control.
        style={helpGridStyle(helpLayout, helpOpen)}
      >
        {/* A button, not a `#main` link: the hash router owns the URL fragment. */}
        <button type="button" className={styles.skipLink} onClick={() => main.current?.focus()}>
          Skip to main content
        </button>
        <header className={styles.topBar}>
          <span className={styles.appName}>{APP_NAME}</span>
          {session.capabilities.includes("accession") && <ScanField />}
          <div className={styles.topBarEnd}>
            <PageHelpButton
              buttonRef={helpTrigger}
              open={helpOpen}
              controls={helpId}
              onPress={() => (helpOpen ? closeHelp() : setHelpState("open"))}
            />
            <HealthIndicator />
            <ListenerSummary />
            <UserMenu session={session} />
          </div>
        </header>
        <section aria-label="Notices" className={styles.notices}>
          <TimeZoneNote profile={profile.data} isError={profile.isError} />
          <OperationsBanners />
          <StarterBanner />
        </section>
        <Sidebar capabilities={session.capabilities} />
        <main id="main" ref={main} tabIndex={-1} className={styles.main}>
          <Outlet />
        </main>
        <div ref={helpPane} id={helpId} className={styles.help} hidden={!helpOpen}>
          {helpState !== "unopened" && (
            <Suspense fallback={<LoadingState />}>
              <PageHelp
                isOpen={helpOpen}
                onClose={closeHelp}
                layout={helpLayout}
                controls={helpId}
              />
            </Suspense>
          )}
        </div>
      </div>
    </LabTimeZoneContext.Provider>
  );
}

function TimeZoneNote({
  profile,
  isError,
}: {
  profile: LaboratoryProfileView | null | undefined;
  isError: boolean;
}) {
  if (profile === null) {
    return (
      <p className={styles.zoneNote}>
        Times show in UTC until the laboratory profile and its time zone are set.
      </p>
    );
  }
  if (isError) {
    return (
      <p className={styles.zoneNote}>
        Times show in UTC: the laboratory time zone could not be read.
      </p>
    );
  }
  return null;
}
