import { Controller, useFormContext } from "react-hook-form";
import { CheckboxGroup } from "@/components/CheckboxGroup";
import type { RoleCode } from "@/generated/RoleCode";
import type { RoleInfo } from "@/generated/RoleInfo";
import { capabilityLabel } from "./capabilityLabels";
import styles from "./users.module.css";

/**
 * The roles to grant, as `roles`, inside a react-hook-form `FormProvider`. Only the roles the
 * engine marks `assignable` for the signed-in person are offered; held roles they cannot change
 * stay as they are and are listed. Each role shows what it allows in laboratory terms; the
 * engine maps roles to capabilities and checks every change.
 */
export function RoleChecklist({ roles }: { roles: readonly RoleInfo[] }) {
  const { control } = useFormContext<{ roles: RoleCode[] }>();
  const offered = roles.filter((role) => role.assignable);
  return (
    <Controller
      control={control}
      name="roles"
      render={({ field }) => {
        const kept = roles.filter((role) => !role.assignable && field.value.includes(role.code));
        return (
          <>
            {offered.length === 0 ? (
              <p className={styles.hint}>You cannot assign any role.</p>
            ) : (
              <CheckboxGroup
                legend="Roles"
                layout="columns"
                options={offered.map((role) => ({
                  id: role.code,
                  label: role.label,
                  details: [role.description, `Allows: ${allowed(role.capabilities)}`],
                }))}
                value={field.value}
                onChange={(codes) =>
                  field.onChange(
                    roles.filter((role) => codes.includes(role.code)).map((role) => role.code),
                  )
                }
              />
            )}
            {kept.length > 0 && (
              <p className={styles.hint}>
                Also holds {kept.map((role) => role.label).join(", ")}, which you cannot change.
              </p>
            )}
          </>
        );
      }}
    />
  );
}

function allowed(capabilities: readonly string[]): string {
  return capabilities.length === 0 ? "nothing" : capabilities.map(capabilityLabel).join(", ");
}
