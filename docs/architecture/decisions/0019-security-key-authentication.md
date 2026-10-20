# ADR 0019: Personal security keys and session-unlock PINs

Status: **Accepted product direction by the project owner, 2026-10-15.** The project owner authorized implementation after reviewing [the authentication research](../../research/security-key-authentication.md). Implementation and native hardware qualification are separate evidence; acceptance of this decision does not certify support for a particular key or operating system. Owner: project owner and technical maintainer; the laboratory owns its operating policy.

## Context

The LIS runs on one laboratory-controlled computer and preserves individual attribution when staff share its OS session. Security keys provide a personal authentication route alongside password/TOTP. Routine unlock, sensitive-action confirmation and recovery need explicit boundaries so convenience does not bypass the required authority or verification.

Current federal HIPAA requirements specify identity verification, unique users, authorized access, audit controls and emergency access, with safeguards selected through risk assessment. They do not prescribe this particular password length or password-plus-TOTP combination. CLIA requires confidentiality and testing-personnel attribution. The HIPAA MFA rulemaking remains a proposal; the July 2027 agenda entry is a final-action target, not an effective or compliance date. The [research and source ledger](../../research/sources-regulatory.md#dedicated-workstation-authentication-review) distinguish regulation, guidance and technical benchmarks.

## Decision

Offer **Security key** and **Password + authenticator app** as first-administrator enrollment routes. Additional staff currently follow temporary-password/TOTP setup and then migrate their own account; direct key signup for those accounts remains outside the supported direct-signup route. First-administrator key registration requires verification on that key using its PIN or a supported biometric. It replaces the routine LIS password and authenticator-code workflow for that account. Enrollment must not first require a password/TOTP account merely to reach key settings.

**Laboratory touch-only setting, accepted by the project owner, 2026-10-15.**
The versioned security policy may explicitly allow personal keys without configured PIN or
biometrics for sign-in and same-user key unlock. The default is off. An administrator needs
fresh normal authentication to change this choice; enabling it requires a recorded reason,
including the first policy version. The change ends other laboratory sessions and invalidates
pending proofs and ceremonies while retaining the freshly confirmed caller's session.

When enabled, subsequent own-account enrollment can accept a signed presence-only credential.
Existing credentials retain their registered verification requirements; enabling the setting
does not rewrite their metadata. Configured PINs, biometric verification and device-enforced
restrictions may still require verification. A failed PIN never falls back to touch-only.
First-administrator signup and replacement-key recovery retain required verification. Policy
version and values are rechecked after hardware interaction and inside completion transactions.

Sensitive actions, including report release, credential changes, personal unlock-PIN settings
and this laboratory setting, still require a verified key assertion for key accounts.
A touch-only key cannot authorize them without configuring its PIN/biometrics or selecting
another registered key that can verify the user. Password accounts retain their existing
fresh-password confirmation. The optional route is single-factor authentication: someone
holding the key who can reach the workstation can act as its owner. Laboratory approval of
that physical-access risk remains necessary; the option makes no MFA or compliance claim.

Apply the selected credential consistently to sign-in, same-user unlock and fresh confirmation for sensitive actions. A key account must not encounter an undisclosed mandatory password later. A successful, fully authorized first enrollment can open the authenticated session directly. Ordinary work reuses that session; release and account/credential administration still require fresh confirmation and current authority.

Keep the existing password/TOTP route available. Its continued availability is not evidence that the existing FIDO2 requirements for release and administration are fully met. Record any remaining difference from REQ-IDN-004/NFR-19 explicitly in implementation status and verification; do not relabel TOTP or password-only confirmation as phishing-resistant authentication.

**Personal unlock PIN, accepted by the project owner, 2026-10-15.** This is a personal PIN to unlock the LIS, independent of a security-key activation PIN. Offer an optional six-digit PIN for the same user's locked, unexpired session. Normal sign-in remains necessary after restart, sign-out, switching users or the existing 12-hour absolute limit. Five consecutive incorrect or malformed PIN submissions end that session; reopening the prompt or using a password unlock does not reset the counter. A successful PIN unlock resets consecutive failures without extending the absolute lifetime. The PIN cannot confirm sensitive actions or change its own configuration.

PIN setup, change and removal require an unlocked session and fresh normal authentication. Store an Argon2id verifier encrypted under the installation data key, with user- and purpose-bound authenticated encryption; a copied database alone must not expose the short-PIN verifier. Administrative credential reset and offline key recovery remove the PIN. Configuration changes invalidate pending credential proofs and end other sessions. The six-digit/five-attempt settings and predictable-pattern rejection are product choices, not HIPAA or NIST mandates. The [research](../../research/security-key-authentication.md#personal-session-unlock-pin) records the assessment and residual host risks.

Password-only initial sign-in, Windows Hello, synced passkeys and an implicit OS-login bypass remain outside this decision. Device credentials require separate identity binding, native integration and recovery qualification before acceptance.

## Identity, safety and failure behavior

| Boundary | Required behavior |
| --- | --- |
| Individual identity | Bind every registered credential to its LIS account. Retain roles, engine-side authorization, session ownership and audit attribution independently of the OS account. A credential does not confer clinical qualifications or report-release authority. |
| Verification | Verify registration and signed assertions in the engine with a fixed relying-party identity, fresh challenges and user presence. Require user verification by default and for every sensitive-action proof. Only the explicit laboratory policy permits presence-only own enrollment, sign-in and key unlock; preserve existing credential requirements and recheck policy before commit. Do not claim a new PIN entry solely from the assertion's verification flag. |
| Secrets and host privileges | Keep the key PIN out of persistent state, audit and diagnostics. Use an OS-owned prompt where supported. Native device access must work under the normal unprivileged application account. No UI listener or local web server is authorized by this decision. |
| Enrollment | An interrupted or cancelled enrollment must leave no active account without a usable credential. Subsequent enrollment or replacement requires appropriate authorization and verified assignment to the individual. Repeated or stale completion must not duplicate credentials or bypass account changes. |
| Sensitive action | Freeze the exact action and input in the engine before key confirmation. The engine dispatches that retained operation after verification; the UI cannot swap its action or parameters. Consume the private confirmation receipt once, recheck current permissions and clinical gates, and record the outcome. Do not send the receipt to the UI. Wait for hardware outside the database writer transaction so analyzer intake continues. |
| Lock and user switch | Preserve listener operation while the application is open and the host awake. Clear exposed working content and pending authentication at session boundaries. A new operator receives their own session. |
| Recovery and revocation | Provide attributable credential recovery without granting clinical authority or overriding clinical holds. Account for a sole operator, spare keys, lost keys and revocations after a backup. The installation recovery kit restores encryption access and must not be described as a sign-in recovery credential unless an implemented, tested procedure explicitly gives it that role. |
| Offline use | Enrollment, sign-in, unlock and required confirmation must work without an identity provider or internet connection. Database restore must preserve the intended relying-party identity and reconcile credential revocations before normal operation. |

## Acceptance evidence and revisit triggers

Before reporting the feature complete, verify real account enrollment, cancellation, duplicate/stale assertions, offline sign-in, unlock, action confirmation, revocation, recovery and database restore. Test under standard OS privileges on each claimed supported platform and qualified key. Preserve per-person role checks, authorization on clinical-role credential resets and synthetic-data audit evidence. Update operator guidance and every affected bundled-help field/action in the same change.

The verifier and native client are the `webauthn-rs` packages admitted in the [library policy](../../governance/library-policy.md), which records their versions and API use. Required verification and the optional touch-only route keep separately represented stored credentials. No custom signature verification or credential-internals feature is introduced. Use USB on macOS/Linux and the native Windows WebAuthn API on Windows. The native client requests external hardware; the verifier rejects backup-eligible or backed-up credentials. These checks do not establish manufacturer-attestation certification. The native client remains pre-1.0 and its thorough security review is unproven; qualification evidence must disclose that limitation.

Use typed reauthentication internally while preserving existing password JSON input compatibility. Key confirmation dispatches the engine-retained action rather than returning a reusable authorization token. Reuse existing accounts, authorization, sessions and audit integration; do not replace those boundaries or write a new cryptographic verifier.

The implementation uses a rotating one-use offline recovery code for replacement-key registration; it never opens a session or grants clinical authority. Each enrollment replaces the previous recovery code. The installation recovery kit is separate. Supported key/firmware combinations, enforceable PIN policy, platform coverage and laboratory qualification of sole-user recovery remain tracked in [Q-42](../../planning/open-questions.md). The six-or-more-digit PIN preference is a qualification target, not a universal HIPAA rule or a capability to claim for every key. Dependency admission preserves the unprivileged native boundary; it does not replace lockfile advisory/license checks or hardware validation.

Revisit this decision when the laboratory's actual sharing or remote-access model changes, when a binding federal/state/accreditor requirement changes, when supported native authentication APIs change, or when recovery/hardware testing shows that the chosen workflow cannot meet the required identity and availability safeguards.

## Build qualification boundary

The published native/verifier stack statically links the locked vendored OpenSSL source so the installed app does not rely on an undeclared system OpenSSL library. The currently compatible published source bundles OpenSSL 3.6.3 and predates upstream August/September security releases. The [scoped advisory review](../../research/security-key-authentication.md) found no demonstrated affected call path in this implementation; that is an inference about current APIs, not a blanket clearance or an advisory suppression. Upgrade to a patched compatible source before production (G2-09), and repeat relevant crypto/platform checks. Current code and automated synthetic-data checks do not establish production readiness.
