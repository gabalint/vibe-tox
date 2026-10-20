import type { TotpEnrollment } from "@/generated/TotpEnrollment";
import styles from "./auth.module.css";
import { MfaCodeForm } from "./MfaCodeForm";

interface MfaEnrollmentProps {
  challengeId: string;
  enrollment: TotpEnrollment;
  /** Returns to the username and password step. */
  onCancel: () => void;
  /** Returns to the username and password step after the challenge expired. */
  onExpired: () => void;
}

/**
 * First-sign-in authenticator setup (NFR-19): the engine's QR code and key, confirmed with
 * a code. The SVG is shown through an image, which never runs scripts it might contain.
 */
export function MfaEnrollment({
  challengeId,
  enrollment,
  onCancel,
  onExpired,
}: MfaEnrollmentProps) {
  return (
    <>
      <p>
        Scan this code with the authenticator app on a phone or device you keep with you. Then enter
        the code the app shows.
      </p>
      <img
        className={styles.qr}
        src={`data:image/svg+xml;charset=utf-8,${encodeURIComponent(enrollment.qrSvg)}`}
        alt="QR code for your authenticator app"
      />
      <div className={styles.secret}>
        <span>If you cannot scan the code, enter this key in the app:</span>
        <code className={styles.key}>{groupsOfFour(enrollment.secretBase32)}</code>
      </div>
      <MfaCodeForm challengeId={challengeId} onCancel={onCancel} onExpired={onExpired} />
    </>
  );
}

/** Authenticator apps ignore spaces in a key; groups make it easier to type. */
function groupsOfFour(key: string): string {
  const groups: string[] = [];
  for (let start = 0; start < key.length; start += 4) {
    groups.push(key.slice(start, start + 4));
  }
  return groups.join(" ");
}
