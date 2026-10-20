import type { TextFormat } from "@/components/FormFields";
import { TextInput } from "@/components/FormFields";
import { ReasonField } from "@/components/ReasonField";
import type { LaboratoryProfileView } from "@/generated/LaboratoryProfileView";
import type { SetLaboratoryProfile } from "@/generated/SetLaboratoryProfile";
import { CommandDialog } from "@/lib/CommandDialog";
import { optionalText } from "@/lib/choices";
import { GET_LABORATORY_PROFILE } from "@/lib/commands";
import { NPI_FORMAT } from "../formats";

const CLIA_FORMAT: TextFormat = {
  pattern: /^\d{2}D\d{7}$/,
  message: "Enter the CLIA number as two digits, D, seven digits.",
  description: "Two digits, D, seven digits",
};

/**
 * Form values. The address lines are named `addressLines.0` to `addressLines.2`, so the
 * engine's `addressLines[0]` errors land on the matching line.
 */
interface ProfileValues {
  name: string;
  addressLines: [string, string, string];
  cliaId: string;
  director: string;
  phone: string;
  timeZone: string;
  billingNpi: string;
  taxId: string;
  reason: string;
}

function toValues(profile: LaboratoryProfileView | null): ProfileValues {
  const lines = profile?.addressLines ?? [];
  return {
    name: profile?.name ?? "",
    addressLines: [lines[0] ?? "", lines[1] ?? "", lines[2] ?? ""],
    cliaId: profile?.cliaId ?? "",
    director: profile?.director ?? "",
    phone: profile?.phone ?? "",
    timeZone: profile?.timeZone ?? "",
    billingNpi: profile?.billingNpi ?? "",
    taxId: profile?.taxId ?? "",
    reason: "",
  };
}

function toInput(
  profile: LaboratoryProfileView | null,
  values: ProfileValues,
): SetLaboratoryProfile {
  return {
    expectedVersion: profile?.version ?? null,
    name: values.name.trim(),
    addressLines: values.addressLines.map((line) => line.trim()).filter((line) => line !== ""),
    cliaId: values.cliaId.trim(),
    director: values.director.trim(),
    phone: values.phone.trim(),
    timeZone: values.timeZone.trim(),
    billingNpi: optionalText(values.billingNpi),
    taxId: optionalText(values.taxId),
    ...(profile ? { reason: values.reason.trim() } : {}),
  };
}

interface ProfileDialogProps {
  /** The current version, or `null` to set the first one. */
  profile: LaboratoryProfileView | null;
  onClose: () => void;
}

/**
 * Sets a new version of the laboratory profile (REQ-RPT-001), prefilled from the current one.
 * Every version after the first needs a reason. Saving refreshes the profile, so every screen
 * switches to the new time zone.
 */
export function ProfileDialog({ profile, onClose }: ProfileDialogProps) {
  const title = profile ? "Change laboratory profile" : "Set laboratory profile";
  return (
    <CommandDialog<ProfileValues, SetLaboratoryProfile, LaboratoryProfileView>
      title={title}
      submitLabel="Save profile"
      command={{ name: "set_laboratory_profile", invalidates: [GET_LABORATORY_PROFILE] }}
      defaultValues={toValues(profile)}
      toInput={(values) => toInput(profile, values)}
      successTitle="Laboratory profile saved"
      errorTitle="Not saved"
      onClose={onClose}
    >
      <p>Reports print these details. Times on every screen show in this time zone.</p>
      <TextInput name="name" label="Laboratory name" isRequired autoFocus />
      <TextInput name="addressLines.0" label="Address line 1" isRequired />
      <TextInput name="addressLines.1" label="Address line 2" />
      <TextInput name="addressLines.2" label="Address line 3" />
      <TextInput name="cliaId" label="CLIA number" format={CLIA_FORMAT} isRequired />
      <TextInput name="director" label="Laboratory director" isRequired />
      <TextInput name="phone" label="Phone" isRequired />
      <TextInput
        name="timeZone"
        label="Time zone"
        description="IANA name, such as America/Chicago"
        isRequired
      />
      <TextInput name="billingNpi" label="Billing NPI" format={NPI_FORMAT} />
      <TextInput name="taxId" label="Tax ID" description="Nine digits, optionally as NN-NNNNNNN" />
      {profile && <ReasonField />}
    </CommandDialog>
  );
}
