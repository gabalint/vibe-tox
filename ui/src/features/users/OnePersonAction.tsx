import { useState } from "react";
import { Button } from "@/components/Button";
import { ReasonField } from "@/components/ReasonField";
import type { DesignateOnePersonLaboratoryInput } from "@/generated/DesignateOnePersonLaboratoryInput";
import type { SessionInfo } from "@/generated/SessionInfo";
import type { UserSummary } from "@/generated/UserSummary";
import { CommandDialog } from "@/lib/CommandDialog";
import { CURRENT_SESSION, useCommandQuery } from "@/lib/commands";
import { AuthenticationField } from "@/lib/securityKeyAuthenticationField";
import { DESIGNATE_ONE_PERSON_LABORATORY, LIST_USERS } from "./contract";

/**
 * "Set up for one person" (ADR 0015), offered to an account administrator who is the
 * laboratory's only active account. Presentation only: the engine refuses it while another
 * account is active. Afterwards the session, the users and the policy are read again, so the
 * new roles show at once.
 */
export function OnePersonAction({ users }: { users: readonly UserSummary[] | undefined }) {
  const session = useCommandQuery<SessionInfo | null>(CURRENT_SESSION).data;
  const [isOpen, setOpen] = useState(false);
  const active = users?.filter((user) => user.status === "active") ?? [];
  const onlyActive = active.length === 1 && active[0]?.userId === session?.userId;
  if (!onlyActive || !session?.capabilities.includes("manage_users")) return null;
  return (
    <>
      <Button onPress={() => setOpen(true)}>Set up for one person</Button>
      {isOpen && <OnePersonDialog onClose={() => setOpen(false)} />}
    </>
  );
}

function OnePersonDialog({ onClose }: { onClose: () => void }) {
  return (
    <CommandDialog<{ reason: string; password: string }, DesignateOnePersonLaboratoryInput>
      title="Set up for one person"
      submitLabel="Set up for one person"
      command={{
        name: DESIGNATE_ONE_PERSON_LABORATORY,
        invalidates: [CURRENT_SESSION, LIST_USERS, "get_separation_policy"],
      }}
      defaultValues={{ reason: "", password: "" }}
      toInput={({ reason, password }) => ({ reason: reason.trim(), password })}
      successTitle="Your account now has every role"
      onClose={onClose}
    >
      <p>
        For a laboratory you run alone. Your account gets every role, and the laboratory allows one
        person to approve configuration they wrote, release reports whose results they reviewed and
        verify destinations they created. The app asks you to confirm each time, and the audit trail
        marks each one. A quality manager can change this later in Operations settings.
      </p>
      <ReasonField />
      <AuthenticationField />
    </CommandDialog>
  );
}
