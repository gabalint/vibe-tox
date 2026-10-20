export type { AuthenticationStatus } from "@/generated/AuthenticationStatus";
export type { KeyRecoveryResult as KeyEnrollmentResult } from "@/generated/KeyRecoveryResult";
export type { SecurityKeyCredentialView as SecurityKeyCredential } from "@/generated/SecurityKeyCredentialView";
export type { SecurityKeyStatus } from "@/generated/SecurityKeyStatus";

/** Ephemeral selection is scoped to the individual session account and cleared with its cache. */
export const securityKeySelection = (userId: string) => ["security-key-choice", userId];
