import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import { type ActionItem, RecordActions } from "@/components/RowActions";
import { StatusBadge } from "@/components/StatusBadge";
import type { UserSummary } from "@/generated/UserSummary";
import type { OpenRecord } from "@/lib/openRecord";
import { Authenticator, authenticationLabel, Lockout, statusLabel } from "./userColumns";

interface UserPanelProps {
  user: UserSummary;
  roleNames: (user: UserSummary) => string;
  /** The actions offered on this account; the first is the main one, unless destructive. */
  actions: readonly ActionItem[];
  /** The account is the signed-in person's own. */
  own: boolean;
  record: OpenRecord;
}

/** One account in view mode: roles, status and sign-in state, and the changes offered. */
export function UserPanel({ user, roleNames, actions, own, record }: UserPanelProps) {
  return (
    <DetailPanel
      identifier={user.displayName}
      status={<StatusBadge tone="neutral">{statusLabel(user)}</StatusBadge>}
      actions={actions.length > 0 && <RecordActions items={actions} />}
      onClose={record.close}
      onPrevious={record.previous}
      onNext={record.next}
    >
      {own && <p>Ask someone else to change your own roles or reset your sign-in.</p>}
      <Facts
        items={[
          { label: "Username", value: user.username },
          { label: "Roles", value: roleNames(user) },
          { label: "Sign-in method", value: authenticationLabel(user) },
          { label: "Authenticator", value: <Authenticator user={user} /> },
          { label: "Sign-in lockout", value: <Lockout user={user} /> },
        ]}
      />
    </DetailPanel>
  );
}
