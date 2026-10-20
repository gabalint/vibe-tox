import { Link } from "react-router";
import { AuthGate } from "@/features/auth/AuthGate";
import { ListenerStatus } from "@/features/listeners/ListenerStatus";
import { AppShell } from "./AppShell";
import { GALLERY_PATH } from "./navigation";

/** The app frame for the signed-in person, or the setup, sign-in or lock screen (REQ-IDN-004). */
export function SessionRoot() {
  return (
    <AuthGate
      aside={
        <>
          <ListenerStatus />
          {import.meta.env.DEV && (
            <Link to={GALLERY_PATH}>Component gallery (development build)</Link>
          )}
        </>
      }
    >
      {(session) => <AppShell session={session} />}
    </AuthGate>
  );
}
