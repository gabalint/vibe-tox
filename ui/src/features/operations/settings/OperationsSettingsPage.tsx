import { EmptyState } from "@/components/EmptyState";
import { Page, PageHeader } from "@/components/Page";
import type { OperationsSettings } from "@/generated/OperationsSettings";
import { useHasCapability } from "@/lib/commands";
import styles from "@/styles/layout.module.css";
import {
  APPROVE_CONFIGURATION,
  GET_OPERATIONS_SETTINGS,
  MANAGE_CONFIGURATION,
  MANAGE_QUALITY,
  MANAGE_USERS,
  SET_OPERATIONS_SETTINGS,
  SETTINGS_QUERIES,
} from "../contract";
import { SecurityPolicySection } from "./SecurityPolicySection";
import { SeparationPolicySection } from "./SeparationPolicySection";
import { SettingsSection } from "./SettingsSection";
import { OPERATIONS_FIELDS } from "./settingFields";

/**
 * Operations settings and the security policy (REQ-OPS-007, REQ-OPS-008, NFR-19; design brief
 * §8). Both are versioned laboratory decisions (L2 to L6, L11): the defaults apply until the
 * laboratory sets values, and every later version needs a reason. Operations settings need
 * `manage_configuration`, the security policy `manage_users`, and the separation-of-duties
 * policy (ADR 0018) configuration approval to change or quality authority to read.
 */
export function OperationsSettingsPage() {
  const canConfigure = useHasCapability(MANAGE_CONFIGURATION);
  const canManageUsers = useHasCapability(MANAGE_USERS);
  const canManageQuality = useHasCapability(MANAGE_QUALITY);
  const canApprove = useHasCapability(APPROVE_CONFIGURATION);
  return (
    <Page>
      <PageHeader
        title="Operations settings"
        description="Backup interval and retention, health check thresholds, the sign-in security policy, and separation of duties."
      />
      <div className={styles.stack}>
        {canConfigure && (
          <SettingsSection<OperationsSettings>
            heading="Operations settings"
            query={GET_OPERATIONS_SETTINGS}
            title="Change operations settings"
            command={{ name: SET_OPERATIONS_SETTINGS, invalidates: SETTINGS_QUERIES }}
            fields={OPERATIONS_FIELDS}
            nested
            successTitle="Operations settings saved"
          />
        )}
        {canManageUsers && <SecurityPolicySection />}
        {(canApprove || canManageQuality) && <SeparationPolicySection />}
        {!canConfigure && !canManageUsers && !canManageQuality && !canApprove && (
          <EmptyState
            title="No settings to show"
            description="Operations settings need manage configuration; the security policy needs manage users; separation of duties needs configuration approval or quality authority."
          />
        )}
      </div>
    </Page>
  );
}
