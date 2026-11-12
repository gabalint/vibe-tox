import { Check, ChevronDown, Copy, Info, KeyRound, Lock, LogOut, UserRound } from "lucide-react";
import { useEffect, useRef, useState } from "react";
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
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { useToast } from "@/components/Toast";
import type { AppInfo } from "@/generated/AppInfo";
import type { CommandError } from "@/generated/CommandError";
import type { SessionInfo } from "@/generated/SessionInfo";
import { APPEARANCES, type Appearance, useAppearance } from "@/lib/appearance";
import { useCommandMutation, useCommandQuery } from "@/lib/commands";
import { toUserMessage } from "@/lib/errors";
import type { AuthenticationStatus } from "@/lib/securityKey";
import { unsavedFormNames } from "@/lib/unsavedForms";
import { ChangePasswordDialog } from "./ChangePasswordDialog";
import { SecurityKeysDialog } from "./SecurityKeysDialog";
import { UnlockPinDialog } from "./UnlockPinDialog";
import styles from "./UserMenu.module.css";

/**
 * The signed-in person's menu: change password, lock, sign out, Appearance (light, dark or the
 * system's setting, applied at once), "About this app" (the app and web engine versions) and, last, the
 * running version, which copies itself (tickets #4584, #4745). Lock and sign-out clear every cached engine result, so nothing
 * from this session is shown to the next person (REQ-IDN-004). Sign out asks first while a page form has
 * unsaved input (ticket #4585).
 */
export function UserMenu({ session }: { session: SessionInfo }) {
  const toast = useToast();
  const navigate = useNavigate();
  const [changingPassword, setChangingPassword] = useState(false);
  const [managingKeys, setManagingKeys] = useState(false);
  const [managingPin, setManagingPin] = useState(false);
  const [leavingForms, setLeavingForms] = useState<string[]>([]);
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
  const signOutNow = () => {
    setLeavingForms([]);
    signOut.mutate({}, { onError });
  };

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
              onAction={() => {
                const forms = unsavedFormNames();
                if (forms.length > 0) setLeavingForms(forms);
                else signOutNow();
              }}
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
            <VersionItem />
          </Menu>
        </Popover>
      </MenuTrigger>
      <ChangePasswordDialog isOpen={changingPassword} onClose={() => setChangingPassword(false)} />
      {managingKeys && (
        <SecurityKeysDialog session={session} onClose={() => setManagingKeys(false)} />
      )}
      {managingPin && <UnlockPinDialog onClose={() => setManagingPin(false)} />}
      <ConfirmDialog
        title="Leave without saving?"
        isOpen={leavingForms.length > 0}
        onOpenChange={(isOpen) => {
          if (!isOpen) setLeavingForms([]);
        }}
        confirmLabel="Sign out without saving"
        onConfirm={signOutNow}
      >
        <p>
          You have unsaved changes in {leavingForms.join(", ")}. If you sign out, what you typed is
          lost.
        </p>
      </ConfirmDialog>
    </>
  );
}

/** How long "Copied" or "Not copied" shows after the version row is chosen. */
const COPY_RESULT_SHOWN_MS = 2000;

/**
 * The running version, from the same `app_info` query as About this app. A menu holds only items,
 * so the whole row copies the version; a small copy icon right after the version, in the same
 * text style, is the hint (ticket #4745). The menu stays open to show "Copied" briefly.
 */
function VersionItem() {
  const info = useCommandQuery<AppInfo>("app_info");
  const [copy, setCopy] = useState<"idle" | "Copied" | "Not copied">("idle");
  const timer = useRef<number | undefined>(undefined);
  useEffect(() => () => window.clearTimeout(timer.current), []);
  const version = info.data?.appVersion;
  if (version === undefined) return null;
  const show = (result: "Copied" | "Not copied") => {
    setCopy(result);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setCopy("idle"), COPY_RESULT_SHOWN_MS);
  };
  return (
    <>
      <Separator className={styles.separator} />
      <MenuItem
        className={styles.item}
        textValue={`Version ${version}`}
        aria-label={
          copy === "idle" ? `Copy version ${version}` : `Version ${version} ${copy.toLowerCase()}`
        }
        shouldCloseOnSelect={false}
        onAction={() =>
          void navigator.clipboard.writeText(version).then(
            () => show("Copied"),
            () => show("Not copied"),
          )
        }
      >
        <span className={styles.version}>
          Version {version}
          {copy === "Copied" ? (
            <Check aria-hidden="true" className={styles.copyIcon} />
          ) : (
            <Copy aria-hidden="true" className={styles.copyIcon} />
          )}
          {copy !== "idle" && copy}
        </span>
      </MenuItem>
    </>
  );
}
