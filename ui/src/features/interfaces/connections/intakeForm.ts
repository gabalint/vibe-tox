// The form values of an HL7 Orders connection: how its input folder is read and what it
// accepts. The engine checks every value again.
import type { IntakeSettings } from "@/generated/IntakeSettings";
import type { PartnerConnection } from "@/generated/PartnerConnection";
import type { PartnerProtocol } from "@/generated/PartnerProtocol";
import type { UnknownPatient } from "@/generated/UnknownPatient";

export interface IntakeValues {
  checkEverySeconds: string;
  acceptFrom: string;
  unknownPatient: UnknownPatient;
  /** Shown only: an unknown test code always moves the file to Failed. */
  unknownTest: string;
  remoteProcessedFolder: string;
  remoteFailedFolder: string;
}

/** The local Processed and Failed folders, shown; the engine keeps them behind handles. */
export interface MovedToValues {
  processedFolder: string;
  processedFolderChoice: string;
  failedFolder: string;
  failedFolderChoice: string;
}

export function toIntakeValues(intake: IntakeSettings | null | undefined): IntakeValues {
  return {
    checkEverySeconds: String(intake?.checkEverySeconds ?? 30),
    acceptFrom: intake?.acceptFrom ?? "",
    unknownPatient: intake?.unknownPatient ?? "hold",
    unknownTest: "reject",
    remoteProcessedFolder: intake?.remoteProcessedFolder ?? "",
    remoteFailedFolder: intake?.remoteFailedFolder ?? "",
  };
}

/** A folder connection's local Processed and Failed folders; empty over SFTP. */
export function toMovedToValues(connection: PartnerConnection | null): MovedToValues {
  const local = connection?.settings.protocol === "folder";
  return {
    processedFolder: local ? (connection?.processedFolder ?? "") : "",
    processedFolderChoice: "",
    failedFolder: local ? (connection?.failedFolder ?? "") : "",
    failedFolderChoice: "",
  };
}

/** The intake settings as entered; the server folders only for SFTP. */
export function toIntake(values: IntakeValues, protocol: PartnerProtocol): IntakeSettings {
  const settings: IntakeSettings = {
    checkEverySeconds: Number(values.checkEverySeconds),
    acceptFrom: values.acceptFrom.trim(),
    unknownPatient: values.unknownPatient,
  };
  return protocol === "sftp"
    ? {
        ...settings,
        remoteProcessedFolder: values.remoteProcessedFolder.trim(),
        remoteFailedFolder: values.remoteFailedFolder.trim(),
      }
    : settings;
}
