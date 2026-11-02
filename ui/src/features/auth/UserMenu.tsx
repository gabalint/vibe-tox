import { Check, ChevronDown, Info, KeyRound, Lock, LogOut, UserRound } from "lucide-react";
import { useState } from "react";
import {
  Header,
  Menu,
  MenuItem,
  MenuSection,
  MenuTrigger,
  Popover,
  Separator,
  VisuallyHidden,
} from "react-aria-components";
import { useNavigate } from "react-router";
import { Button } from "@/components/Button";
import { useToast } from "@/components/Toast";
import type { CommandError } from "@/generated/CommandError";
import type { SessionInfo } from "@/generated/SessionInfo";
import { APPEARANCES, type Appearance, useAppearance } from "@/lib/appearance";
import { useCommandMutation, useCommandQuery } from "@/lib/commands";
import { toUserMessage } from "@/lib/errors";
import type { AuthenticationStatus } from "@/lib/securityKey";
import { ChangePasswordDialog } from "./ChangePasswordDialog";
import { SecurityKeysDialog } from "./SecurityKeysDialog";
import { UnlockPinDialog } from "./UnlockPinDialog";
import styles from "./UserMenu.module.css";

/**
 * The signed-in person's menu: change password, lock, sign out, Appearance (light, dark or the
 * system's setting, applied at once) and "About this app" (the app and web engine versions). Lock and sign-out clear every cached engine result, so nothing
 * from this session is shown to the next person (REQ-IDN-004).
 */
export function UserMenu({ session }: { session: SessionInfo }) {
  const toast = useToast();
  const navigate = useNavigate();
  const [changingPassword, setChangingPassword] = useState(false);
  const [managingKeys, setManagingKeys] = useState(false);
  const [managingPin, setManagingPin] = useState(false);
  const [appearance, chooseAppearance] = useAppearance(session.userId);
  const authentication = useCommandQuery<AuthenticationStatus>("get_authentication_status");
  const lock = useCommandMutation<null>("lock_session", { invalidates: "all" });
  const signOut = useCommandMutation<null>("sign_out", { invalidates: "all" });
  const onError = (error: CommandError) =>
    toast.show({
      tone: "danger",
      title: "The session did not change",
      description: toUserMessage(error),
    });

  return (
    <>
      <MenuTrigger>
        <Button variant="quiet" isPending={lock.isPending || signOut.isPending}>
          <UserRound aria-hidden="true" />
          <VisuallyHidden>Signed in as </VisuallyHidden>
          {session.displayName}
          <ChevronDown aria-hidden="true" />
        </Button>
        <Popover placement="bottom end" className={styles.popover}>
          <Menu className={styles.menu}>
            {authentication.data?.method === "password_totp" && (
              <MenuItem
                className={styles.item}
                textValue="Change password"
                onAction={() => setChangingPassword(true)}
              >
                <KeyRound aria-hidden="true" className={styles.icon} />
                Change password
              </MenuItem>
            )}
            <MenuItem
              className={styles.item}
              textValue="Security keys"
              onAction={() => setManagingKeys(true)}
            >
              <KeyRound aria-hidden="true" className={styles.icon} />
              Security keys
            </MenuItem>
            <MenuItem
              className={styles.item}
              textValue="Unlock PIN"
              onAction={() => setManagingPin(true)}
            >
              <Lock aria-hidden="true" className={styles.icon} />
              Unlock PIN
            </MenuItem>
            <MenuItem
              className={styles.item}
              textValue="Lock now"
              onAction={() => lock.mutate({}, { onError })}
            >
              <Lock aria-hidden="true" className={styles.icon} />
              Lock now
            </MenuItem>
            <MenuItem
              className={styles.item}
              textValue="Sign out"
              onAction={() => signOut.mutate({}, { onError })}
            >
              <LogOut aria-hidden="true" className={styles.icon} />
              Sign out
            </MenuItem>
            <Separator className={styles.separator} />
            <MenuSection
              selectionMode="single"
              disallowEmptySelection
              selectedKeys={[appearance]}
              onSelectionChange={(keys) => {
                const [key] = keys === "all" ? [] : keys;
                if (key !== undefined) chooseAppearance(key as Appearance);
              }}
            >
              <Header className={styles.header}>Appearance</Header>
              {APPEARANCES.map((option) => (
                <MenuItem
                  key={option.id}
                  id={option.id}
                  className={styles.item}
                  textValue={option.label}
                >
                  {({ isSelected }) => (
                    <>
                      <Check
                        aria-hidden="true"
                        className={styles.icon}
                        visibility={isSelected ? "visible" : "hidden"}
                      />
                      {option.label}
                    </>
                  )}
                </MenuItem>
              ))}
            </MenuSection>
            <Separator className={styles.separator} />
            <MenuItem
              className={styles.item}
              textValue="About this app"
              onAction={() => navigate("/about")}
            >
              <Info aria-hidden="true" className={styles.icon} />
              About this app
            </MenuItem>
          </Menu>
        </Popover>
      </MenuTrigger>
      <ChangePasswordDialog isOpen={changingPassword} onClose={() => setChangingPassword(false)} />
      {managingKeys && (
        <SecurityKeysDialog session={session} onClose={() => setManagingKeys(false)} />
      )}
      {managingPin && <UnlockPinDialog onClose={() => setManagingPin(false)} />}
    </>
  );
}
