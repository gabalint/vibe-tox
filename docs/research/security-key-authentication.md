# Security key authentication for the dedicated LIS workstation

Sources checked **2026-10-14** and **2026-10-15**. Scope: US clinical toxicology; local, offline-capable application; synthetic data. [ADR 0019](../architecture/decisions/0019-security-key-authentication.md) records the accepted credential policy and its implementation boundaries; this document retains the regulatory and technical evidence behind it. Platform credentials such as Windows Hello remain outside this scope. Hardware qualification and laboratory operating policy remain [Q-42](../planning/open-questions.md); source inspection and synthetic tests do not establish support for a particular device or host.

## Laboratory touch-only policy

The default is PIN/biometric verification; ADR 0019 defines the explicit laboratory opt-in
for touch-only own-account enrollment, sign-in and key unlock, and the actions that keep
required verification.

Touch-only proves possession and presence, not that the person touching the key is its owner.
The additional risk is a lost, borrowed or unattended key used by someone who can reach the
workstation. An individual locked OS account is relevant protection but does not automatically
turn a shared desktop session into a second LIS authentication factor. This is the project's
risk assessment, not a laboratory compliance determination. [WebAuthn, ENG-266](https://www.w3.org/TR/webauthn-3/#test-of-user-presence),
[NIST, ENG-43/127](https://pages.nist.gov/800-63-4/sp800-63b.html#single-factor-cryptographic-authentication).

Current US federal HIPAA technical safeguards do not prescribe a hardware-key PIN; the
proposed broader MFA rule remains a proposal. Existing identity, access,
risk-assessment and audit obligations remain. State, accreditation and contract requirements
still need laboratory-specific review. [SR-013](https://www.ecfr.gov/current/title-45/section-164.312),
[SR-117](https://www.ecfr.gov/current/title-45/section-164.306),
[HHS rulemaking status](https://www.hhs.gov/hipaa/for-professionals/security/hipaa-security-rule-nprm/factsheet/index.html).

The optional `SecurityKey` credential type is represented separately from `Passkey`
credentials, whose registered verification requirement is preserved; configured keys may
still ask for a PIN. Physical-key behavior on each supported OS remains a qualification task.

## Password/TOTP alternative and recovery boundary

The password/TOTP route remains a distinct authentication option. Its common-password
blocklisting and sole-user forgotten-credential recovery need qualification alongside
the security-key route. The installation recovery kit restores encrypted installation
data; it does not replace a forgotten account credential. See the
[operator workflow](../operations/installation-and-first-start.md#password-and-authenticator-alternative).
REQ-IDN-004 and NFR-19 are project authentication targets, not prescribed federal
password lengths or authentication mechanisms.

## Personal session-unlock PIN

The optional personal PIN unlocks the same person's existing authenticated session. It is
separate from a security key's activation PIN and does not replace initial authentication
or fresh confirmation of sensitive actions. Its accepted limits (six digits, five failures,
predictable-pattern rejection, 12-hour absolute lifetime) are in ADR 0019; they are product
settings, not regulatory limits. The [domain](../../crates/lis-auth/src/unlock_pin.rs),
[engine](../../crates/lis-engine/src/commands/unlock_pin.rs) and
[operator guide](../operations/installation-and-first-start.md#optional-personal-unlock-pin)
are the implemented behavior references.

**US federal assessment, checked 2026-10-15:** current HIPAA does not categorically
prohibit this arrangement. Individual authentication, authorized access, audit controls
and a documented assessment of inactivity handling and workstation risks still apply.
This is a conditional design assessment, not a compliance determination for a laboratory;
state/accreditor/contract requirements remain unresolved. [SR-013, technical safeguards](https://www.ecfr.gov/current/title-45/section-164.312),
[SR-117, flexibility and addressable safeguards](https://www.ecfr.gov/current/title-45/section-164.306).

NIST permits reduced-factor reauthentication within a bounded AAL2 session, but that does
not automatically qualify a six-digit application PIN. Its shorter activation-secret
provisions concern access to an authenticator's stored cryptographic key. A simple local
PIN comparison is not thereby MFA, a hardware-protected activation secret, or evidence of
NIST assurance-level conformance. [ENG-43/127, reauthentication](https://pages.nist.gov/800-63-4/sp800-63b/aal/),
[activation secrets](https://pages.nist.gov/800-63-4/sp800-63b/authenticators/).

The intended protection is against casual use of an unattended session on a controlled
workstation. The verifier is encrypted under the installation key, so a database copy
without that key does not expose it for offline guessing; a salted hash alone would not
make six digits resistant to offline guessing. Shoulder surfing, shared PINs and compromise
of the running host or its installation key remain risks; this is not hardware retry protection.

## Implemented key route

ADR 0019 describes first-administrator key signup, staff migration from password/TOTP,
engine-retained sensitive actions, recovery codes and revocation. The
[operator guide](../operations/installation-and-first-start.md#sign-in-with-a-personal-security-key)
is the canonical current workflow. Main code references: [setup UI](../../ui/src/features/auth/SetupScreen.tsx),
[engine enrollment](../../crates/lis-engine/src/commands/security_keys/registration.rs),
[verifier](../../crates/lis-auth/src/key_verifier.rs),
[action receipt](../../crates/lis-auth/src/key_proofs.rs),
[recovery](../../crates/lis-auth/src/key_recovery.rs) and the
[native boundary](../../crates/lis-security-key/src/lib.rs), admitted under the
[library policy](../governance/library-policy.md#security-key-dependency-admission--2026-10-15).

Hardware qualification, operating-system coverage, fresh-PIN behavior and restore rehearsal remain required. No attestation catalog certifies the key manufacturer. Backup eligibility/state rejection does not prove every physical-device property. The installation recovery kit is distinct from the personal account recovery code.

## What the rules establish

| Authority | Status and relevant finding | Consequence for this application |
| --- | --- | --- |
| HIPAA [45 CFR 164.312](https://www.ecfr.gov/current/title-45/section-164.312), REG-011 / SR-013 | Current federal regulation: unique user identification, emergency access, audit controls and person/entity authentication. It does not prescribe a numeric PIN length or require a separate application password. Automatic logoff is addressable. | Maintain individual LIS identities, authorization and auditable access. A prior OS login alone does not establish who is currently acting in a shared desktop. |
| HIPAA [164.306](https://www.ecfr.gov/current/title-45/section-164.306), [164.308](https://www.ecfr.gov/current/title-45/section-164.308), [164.310](https://www.ecfr.gov/current/title-45/section-164.310), SR-117 | Current federal regulation: risk-based safeguards, access management, contingency planning and workstation protection. Addressable measures require assessment and a documented disposition; they are not simply optional. | Include the dedicated host in the risk assessment and operating procedures. Physical security complements authentication. |
| [HHS OCR authentication guidance](https://www.hhs.gov/hipaa/for-professionals/security/guidance/cybersecurity-newsletter-june-2023/index.html), SR-153 | June 2023 guidance recognizes PIN/password knowledge factors and possessed tokens. Password plus PIN alone repeats one factor. | A key plus its verified PIN is a reasonable design direction; two prompts alone do not establish MFA. |
| [CLIA 493.1283(a)(4)](https://www.ecfr.gov/current/title-42/section-493.1283), SR-154; [CMS SOM Appendix C](https://www.cms.gov/Regulations-and-Guidance/Guidance/Manuals/downloads/som107ap_c_lab.pdf), SR-122 | Regulation requires testing-personnel identity in test records. D5201 survey guidance gives a user ID/password as an example of protecting LIS confidentiality. | Preserve attribution. The example does not mandate a password in addition to a key or establish a categorical shared-OS-login violation. |
| [HIPAA Security Rule NPRM](https://www.hhs.gov/hipaa/for-professionals/security/hipaa-security-rule-nprm/index.html), SR-116; [Unified Agenda](https://mobile.reginfo.gov/public/do/eAgendaViewRule?RIN=0945-AA22&pubId=202510), SR-068 | Proposal, not an effective MFA mandate. The agenda lists a July 2027 final-action target, not a compliance deadline. | Build strong authentication now without representing proposed rules as current law. |

State, accreditation and contractual requirements remain Q-01/Q-13. FDA Part 11 is conditional on the regulated records/function (SR-034); controlled-substance prescription signing rules are not the baseline for toxicology testing. These findings do not certify laboratory compliance.

**The secured workstation matters.** HHS explicitly distinguishes the risks of in-person access, remote access and privileged accounts, and calls for authentication appropriate to each. That supports simplifying local use after assessing the actual host and operators; it is not blanket approval of any short PIN. No effective federal mandate requires a specific combination such as a 12-character password plus TOTP. [HHS guidance, SR-153](https://www.hhs.gov/hipaa/for-professionals/security/guidance/cybersecurity-newsletter-june-2023/index.html), [current technical safeguards, SR-013](https://www.ecfr.gov/current/title-45/section-164.312)

## Authentication factors and PIN boundaries

NIST SP 800-63B-4 distinguishes an authenticator's activation secret from a verifier-checked password. Its activation-secret minimum is four characters, with six recommended and at most ten consecutive failures. Passwords used with MFA have an eight-character minimum; single-factor passwords require fifteen. A key requiring PIN or biometric activation can supply two factors. These are technical benchmarks, not HIPAA PIN rules; the guideline addresses network authentication and does not establish this native application's AAL. [ENG-43, §§3.1.1, 3.1.7, 3.2.10](https://pages.nist.gov/800-63-4/sp800-63b.html)

| Current route | Authentication property | Qualification boundary |
| --- | --- | --- |
| Personal FIDO2 key + PIN verified by that key | Possession and knowledge; no separate application password needed when verification is required and checked | Preferred; propose six or more digits for qualified keys |
| Personal FIDO2 key + fingerprint verified on the key | Possession and biometric verification | Optional; assess gloves, accessibility and PIN fallback during bench trials |
| LIS password + TOTP | Current software route; no security-key purchase | Retain as an enrollment alternative where supported policy allows it; it does not satisfy the proposed FIDO2 target for release/administration |
| Personal short PIN for session unlock | An application secret; host ownership does not give it hardware retry protection | Accepted only within an existing session, with the encrypted verifier and retry boundary above; not a replacement initial sign-in factor |
| Security-key touch only | User presence is distinct from user verification; single-factor authentication | Accepted only under the explicit laboratory opt-in for sign-in/unlock; cannot confirm sensitive actions or be described as MFA |

WebAuthn separates presence (`UP`) from verification (`UV`). The verifier must validate the signed assertion, challenge, relying-party identity and required flags; a UI claiming that the key was touched is insufficient. [ENG-266, WebAuthn](https://www.w3.org/TR/webauthn-3/)

The six-digit policy needs hardware qualification: minimum-PIN enforcement/reporting is optional in CTAP, and verification does not reveal the PIN length. CTAP also permits retained PIN/UV authorization tokens, so a new assertion with `UV` does not prove a new PIN entry. Test token lifetime and fresh interaction for each supported platform/key combination. [ENG-267, CTAP §§6.5, 7.4, 12.5](https://fidoalliance.org/specs/fido-v2.3-ps-20260226/fido-client-to-authenticator-protocol-v2.3-ps-20260226.html)

Password-policy changes need a review of sign-in, unlock, action confirmation, recovery and blocklisting together. Windows Hello remains outside current scope: Microsoft documents both TPM protection and permitted software key storage, so a Hello label alone would not prove hardware backing or individual LIS identity. [ENG-288](https://learn.microsoft.com/en-us/windows/security/identity-protection/hello-for-business/faq)

## Enrollment and recovery qualification

Direct key signup is available for the first administrator. Additional staff still use the
password/TOTP enrollment path before migrating their own account to a key. Do not describe
the proposed direct-key enrollment of additional staff as implemented.

Qualification must cover interrupted signup, the issued personal recovery code, spare-key
custody, credential migration, lost or revoked keys and restoration from an older backup.
Migration must not leave a hidden password bypass, and restored credentials must be reconciled
against revocations. Personal account recovery and the installation recovery kit have distinct
purposes. OS-account identity integration remains outside the current scope.

## Operator design and qualification requirements

ADR 0019's identity, safety and failure table defines the required behavior. Qualification
and laboratory policy still need to settle:

- **Account selection.** Keep explicit account selection before key use; username-free credential discovery adds complexity without a demonstrated need.
- **Idle lock.** The 15-minute idle default remains a proposal pending laboratory approval. Locking differs from signing out, sleeping or closing the application; analyzer intake continues only while the application runs and the host stays awake.
- **Spare keys and PINs.** Enroll a second personal key stored apart from the everyday key. A FIDO PIN belongs to the authenticator and may affect its other credentials; do not change an existing personal key's PIN policy silently.
- **One-person laboratory.** Define an offline, attributable recovery procedure and protected recovery material before requiring keys. Recovery must not confer clinical qualifications or override QC/identity holds.
- **Replacement workstation or restore.** Preserve the installation's relying-party identity and credential public keys, reconcile revocations since the backup before normal access, and test recovery with the spare key and without internet.

The baseline supports both individual OS accounts and a controlled shared bench desktop. In either case, every clinical action retains the individual LIS principal. OS login can become an integrated identity source only through a separately verified binding and session design. On a shared OS account, platform biometrics do not by themselves identify different LIS operators. Host controls (standard-user runtime account, full-disk encryption, physical access restriction, patched software, restricted remote access) reduce risk but do not protect a running LIS from every malicious process or host administrator; the [host baseline](../architecture/deployment.md) and Q-29 are authoritative.

## Integration with the current application

Implementation and remaining hardware qualification are tracked in [G2-09](../quality/open-items-g1-g2.md). Related requirements: REQ-IDN-003/004, NFR-08/19/31; validation scenarios VAL-33/35/53.

- **Platform client.** Microsoft recommends its native Windows WebAuthn API ([ENG-268](https://learn.microsoft.com/en-us/windows/security/identity-protection/hello-for-business/webauthn-apis)); the `ctap-hid-fido2` alternative documents administrator-only Windows HID access and is not admitted ([ENG-269](https://docs.rs/crate/ctap-hid-fido2/3.5.11/source/README.md)). Do not elevate the LIS for device access.
- **Trusted verification.** Native CTAP clients supply relying-party and client data themselves, so browser origin isolation and phishing resistance cannot be assumed from USB transport alone. Platform or synced credentials need separate qualification, including native credential-selection enforcement.
- **Secrets.** Direct CTAP may require transient native PIN collection, which must be cleared and excluded from logs, audit and persisted UI state.
- **Rollout.** Prove enrollment, offline sign-in, unlock, action confirmation, revocation, recovery and restore under standard OS privileges on each supported OS.

The key signs an authentication assertion; it is not a patient-report signing certificate or evidence of the operator's clinical qualifications.

## OpenSSL applicability review — 2026-10-15

The synthetic implementation locks `openssl` 0.10.81, `openssl-sys` 0.9.117 and vendored `openssl-src` 300.6.1+3.6.3. This embeds **OpenSSL 3.6.3**, which predates the August and September security updates; the September 29 release is 3.6.5. The published stack is retained for synthetic implementation, with a patched, reviewed build required before production qualification ([G2-09](../quality/open-items-g1-g2.md)). No advisory suppression or claim that 3.6.3 is fully patched follows from this review. [OpenSSL release notes, ENG-286](https://openssl-library.org/news/openssl-3.6-notes/)

The following is a scoped **static reachability inference**, based on the locked package sources, not an exploit test or a general vulnerability clearance. The [September 29 advisory](https://openssl-library.org/news/secadv/20260929.txt) and [CRLDP fix](https://github.com/openssl/openssl/commit/8e0efc7549b7ff8246d40e585e3fd604f728473f) provide the upstream conditions.

| Advisory area | Observed application path and inference |
| --- | --- |
| DTLS, QUIC, TLS context replacement, CMP | The verifier and admitted USB CTAP client use cryptographic primitives, not these protocol implementations. No affected call path was found. |
| SM2 and non-NIST signing timing | Production verification handles public signatures; the CTAP PIN channel uses P-256 ECDH, AES-256-CBC, SHA-256/HMAC/HKDF. No SM2 or non-NIST private signing path was found. Test-only software authenticators do not ship as the key transport. |
| X.509 concurrent extension-cache use-after-free, CVE-2026-84783 | Upstream identifies OpenSSL 4.0 as affected, and explicitly excludes 3.6. |
| Relative CRLDP allocation, CVE-2026-35189 | Registration can parse unsolicited attestation certificates even though it requests no attestation. Both admitted verifier APIs use no attestation CA list in this implementation and skip CA-chain verification; its remaining OpenSSL certificate getters do not populate the affected extension cache. No reachable affected path was demonstrated. Enabling attestation trust-chain policy would require a new review. |

Reproducible trace (ENG-287): the published `webauthn-rs`/core 0.5.5 metadata identifies commit `d2c10d53ca5ef033d37ee6462e936e9eb72ad98c`. [`webauthn-rs/src/lib.rs:652`](https://github.com/kanidm/webauthn-rs/blob/d2c10d53ca5ef033d37ee6462e936e9eb72ad98c/webauthn-rs/src/lib.rs#L652) passes `None` as the attestation CA list; [`core.rs:670`](https://github.com/kanidm/webauthn-rs/blob/d2c10d53ca5ef033d37ee6462e936e9eb72ad98c/webauthn-rs-core/src/core.rs#L670) gates chain verification on that list. Core `attestation.rs:314,495,941` serializes certificates for extension checks with Rust `x509_parser`, while `crypto.rs:66` obtains their public keys. OpenSSL 3.6.3's [`x_x509.c:38`](https://github.com/openssl/openssl/blob/openssl-3.6.3/crypto/x509/x_x509.c#L38) decode callback does not build the extension cache; [`x509_cmp.c:117,378,385`](https://github.com/openssl/openssl/blob/openssl-3.6.3/crypto/x509/x509_cmp.c#L378) returns the subject/public key without calling it. The affected cache implementation is `crypto/x509/v3_purp.c:438`. The published `webauthn-authenticator-rs` 0.5.5 `src/crypto.rs:40,50,70,116` supplies the P-256/CBC trace; `src/ctap2/pin_uv.rs:289,355` supplies HMAC. These package files were inspected in the local Cargo source cache.

This review covers the authentication paths above. It does not qualify every OpenSSL entry point, every future dependency feature, hostile-certificate resource limits, supported hardware, or OS packaging. Recheck the upstream advisory list and actual linked version when updating the stack, and rerun signed-ceremony and native integration tests after a crypto update. The current source analysis does not remove the production patch gate.

## Verification limits

This analysis rests on source inspection, not a fresh test run. Native hardware/OS qualification and real restore rehearsal remain unverified. Before production qualification, Q-42 must record qualified keys/platforms, the supported OS-account arrangement and laboratory-approved recovery controls.
