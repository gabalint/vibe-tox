# Security and privacy design

Status: local security design with separately proposed external-service controls. Legal applicability is maintained in the [compliance matrix](../requirements/compliance-matrix.md). Implementation and remaining qualification limits are recorded in the [implementation plan](../planning/implementation-plan.md#status) and [release gates](../quality/open-items-g1-g2.md). Design controls are not proof of HIPAA compliance or a claim that every control is explicitly mandated by law.

The design covers the local application ([ADR 0009](decisions/0009-single-computer-local-lis.md)) and the separately proposed hosted services ([ADR 0010](decisions/0010-optional-cloud-services.md)). Local attribution uses personal application accounts under [ADR 0019](decisions/0019-security-key-authentication.md); OIDC is a proposal for future portal identity. The privileged host actor is the OS administrator or root. [Desktop application controls](#desktop-application-controls) protect the embedded UI, OS key stores hold secrets, and each OS needs a qualified disk-encryption baseline.

## Trust and data model

Protect patient/donor identifiers, orders, medication/history information, specimen metadata, results, reports, audit records, partner payloads, backups, keys and credentials. Program provenance and special disclosure restrictions travel with records and derived exports; the Part 2 applicability assessment must distinguish covered provenance from an arbitrary toxicology result. Lab client organizations are authorization scopes within one laboratory's installation, not separate legal tenants.

**Who holds what.** Each laboratory is the HIPAA covered entity and runs its own installation on a host computer it controls. A service provider becomes the lab's business associate only for the external services the lab uses, and for remote support or on-site work that reaches PHI ([§15.1](../research/regulatory-landscape.md#151-hipaa-role-of-each-hosted-service)). Publishing the free software does not make its publisher a business associate, provided telemetry carries no PHI.

| Actor | Reach | Main controls |
| --- | --- | --- |
| Lab OS administrator or root (any OS) | **The privileged actor.** Can read or copy the database file, close or kill the app, change the clock, alter local files and the app binaries, and stop backups and anchoring | Full-disk encryption (BitLocker, FileVault or LUKS); per-person OS admin accounts, no shared root password; hash-chained audit anchored off the host ([ADR 0008](decisions/0008-tamper-evident-audit-and-immutable-recovery.md)); immutable off-host copies; residual-privilege statement approved by the lab |
| OS account that runs the app | The database, artifacts, listeners and the app's key-store entries | A standard (non-administrator) OS account; data-directory permissions limited to it and administrators. Anyone using that OS session can reach the app window, but must sign in to the app to act |
| App administrator | App configuration and users | An app role, not release authority; changes audited; FIDO2 is the production target (current alternatives below) |
| Lab staff | Per-role app functions | Per-person app accounts with offline MFA; fresh authentication for release |
| Other local processes and users on the host or LAN | No UI port to reach. Processes in the same OS session can read the app's memory and files; other OS users cannot, by OS permissions. LAN hosts reach configured analyzer ports; an authenticated workstation listener remains proposed | [Desktop application controls](#desktop-application-controls); OS account separation |
| Analyzers and lab-run interfaces | Inbound data on serial and LAN ports | Restricted adapter identities, bounded parsing, quarantine; never exposed beyond the lab LAN |
| Hosted services (optional) | Only the data each enabled external service needs; never clinical state (INV-14) | BAA; per-lab database; device certificates; outbound-only link; US-only |
| Support staff (project or service provider) | PHI only during lab-initiated remote sessions or on site | [Support sessions](#telemetry-support-and-remote-sessions); US-only access; session records |
| Clinic portal users | The furnishing lab's ordering and released results for their clinic | Portal OIDC login with MFA, per-lab and per-clinic authorization (REQ-ORD-011) |

**Proposed cloud data boundary.** No cloud service is implemented. [ADR 0010](decisions/0010-optional-cloud-services.md) (item 3) lists the only data classes a service may hold; unreleased results, QC, worklists and the full patient index never leave the host (REQ-SVC-003).

**Synthetic data.** Demo, development and automated tests use synthetic records. Copying a lab's data to development is not a debugging workflow. Support extracts require purpose limitation, the lab's explicit authority, a BAA, secure handling and retention. Ordinary observability contains opaque correlation IDs and operational facts. Correlation IDs can still be linkable and need appropriate handling.

### Business-associate scope per service

Project reading for counsel, from [§15.1](../research/regulatory-landscape.md#151-hipaa-role-of-each-hosted-service) (REG-044, REQ-SVC-008):

| Activity | PHI handled | Likely role |
| --- | --- | --- |
| Publishing code and installers; PHI-free telemetry | None | Not a business associate |
| Managed backup, including zero-knowledge | Ciphertext of the whole database | Business associate. Lack of the key affects breach analysis, not the role |
| Client portal | Released reports, pending orders | Business associate |
| Delivery relay | Queued messages, delivery logs | Business associate, not a conduit. Fax and Direct providers are subcontractors |
| EHR interface endpoint | Orders and results in flight | Business associate |
| Audit anchoring | Digests only | Outside PHI scope, subject to privacy-officer confirmation |
| Remote support | Screen and database access during sessions | Business associate |
| On-site setup, analyzer interfaces, validation | PHI visible on site | Business associate, or workforce at the lab's election |

### Key custody

| Key or credential | Held by | Where | Loss or compromise |
| --- | --- | --- | --- |
| Disk-encryption recovery key (BitLocker, FileVault or LUKS) | Laboratory | Off the host, in two lab-controlled places; never only in a personal Microsoft or Apple account ([ENG-106](../research/sources-engineering.md#single-computer-technology-sources)) | Lost: a locked disk is unrecoverable, so restore from backup |
| Backup repository key | Laboratory by default | The OS key store on the host (Windows Credential Manager/DPAPI, macOS Keychain, Linux Secret Service), plus the recovery kit off the host. Escrow with a service provider only by the lab's explicit choice (Q-31) | Lost and not escrowed: the backup is lost. Compromised: backup copies become unsecured PHI ([§15.2](../research/regulatory-landscape.md#152-breach-and-incident-allocation)) |
| Device certificate private key | The host computer | OS key store, hardware-backed and non-exportable where available (TPM on Windows and Linux, Secure Enclave on macOS); otherwise software-protected by the OS key store | Revoked at the hosted gateway; re-enrollment issues a new one |
| Personal FIDO2 keys and TOTP seeds | Each staff member | Hardware key; TOTP seed encrypted on the host | Revoke and re-enroll; leaver drill |
| Break-glass credential | Laboratory | Sealed, offline | Use alerts immediately; rotate after use |
| Update signing: per-OS code-signing keys (Authenticode, Apple Developer ID, Linux package keys), the Minisign updater key and the TUF-style manifest root | Release publisher | HSM or cloud signing for code where the platform allows; Minisign and manifest root keys offline ([ENG-111/113](../research/sources-engineering.md#single-computer-technology-sources), [ENG-132](../research/sources-engineering.md#desktop-application-stack-sources-2026-10-12)) | A compromise affects every install; the root key rotates through the manifest chain |
| Per-lab cloud encryption keys | Service provider | KMS or equivalent, per tenant | Scoped to one lab |
| Cloud administrative credentials | Provider staff | Phishing-resistant MFA, separate from ordinary accounts | Incident across tenants |

## Threat/control/verification matrix

| Threat | Proposed controls | Evidence required |
| --- | --- | --- |
| Cross-client or cross-site record access | Engine-side (and, in the cloud, server-side) subject/action/resource/context checks; relationship-scoped queries and exports; deny by default | Negative tests for list/search/detail/bulk/export/background paths and shared patient identities |
| Stolen staff account or session on a shared bench computer | Per-person app accounts; offline MFA (FIDO2 or TOTP); FIDO2 for release and administration; fresh authentication for release; lab-set idle lock; revocation (REQ-IDN-004, NFR-19) | Session, idle-lock and reauthentication tests; enrollment inventory showing FIDO2 on release and admin roles; leaver drill |
| Unauthorized result release | Separate technical/release/admin permissions, qualification scope, all gates rechecked inside the `BEGIN IMMEDIATE` release transaction ([overview](overview.md#transaction-and-failure-design)) | Both-commit-order, permission-revocation, failed-QC and unauthorized-release scenarios |
| Malicious or malformed partner, analyzer or cloud-relayed payload | Restricted adapter identities, bounded parsing, quarantine, validation, original byte retention | Malformed input corpus, replay/conflict tests, parser resource limits |
| Host computer theft or loss | Full-disk encryption (BitLocker, FileVault or LUKS) with an off-host recovery key; device-certificate revocation; restore from the recovery kit and an off-host copy (REG-048) | Theft drill: revoke, restore onto a replacement computer, confirm the old certificate is refused |
| Ransomware on the host | Off-host immutable or offline copies that the host's credentials cannot delete; OS reinstalled before restore; analyzer and MLLP ports never on the internet | Restore from an immutable copy with host credentials assumed compromised; attempted deletion and retention-shortening with host credentials fails (NFR-20) |
| Lab administrator tampering with records or audit | Hash-chained audit and release manifests; digest anchors leave the host on a schedule; anchor-lag alert; optional OS file-access auditing of the data directory (Windows object-access auditing, macOS or Linux audit subsystems; inference) ([ADR 0008](decisions/0008-tamper-evident-audit-and-immutable-recovery.md)) | Simulate row edits, chain recomputation, deleted segments, replaced report files and stopped anchoring; verification detects them beyond the anchored window; residual-privilege statement approved by the lab |
| Malicious, rolled-back or silent update | Per-OS signed packages (Authenticode; Developer ID with notarization; signed Linux packages); Minisign-signed updater artifacts; signed, expiring update manifest with an offline root key and monotonic versions (TUF concepts); no automatic install; lab approval recorded; pre-upgrade snapshot (REQ-OPS-006, NFR-24) | Reject an unsigned package, an expired manifest, a rollback to an older version and a freeze attack; confirm no install without recorded approval |
| Build supply-chain compromise | Pinned dependencies and build inputs; SBOM meeting the CISA 2026 minimum elements; Sigstore-signed artifacts with SLSA v1.2 provenance (proposed Build L2 for the pilot, L3 as a goal); vulnerability and license review; protected release credentials (NFR-21) | Repeatable release evidence per OS; provenance verification; dependency exception record. [ENG-45–47](../research/sources-engineering.md) |
| OS web engine changes under the app (WebView2 Evergreen, WKWebView or WebKitGTK update; a vulnerable engine) | Clinical logic, gates and authorization only in the Rust engine; the UI only presents and collects input; webview version in the session audit context; strict CSP and no remote content limit exposure; optional WebView2 Fixed Version runtime, which the project must then patch ([ADR 0012](decisions/0012-rust-tauri-desktop-application.md#consequences)) | UI smoke check after OS updates; per-OS test matrix including webview versions; review of engine advisories in release notes |
| Device impersonation at the cloud | Per-device mTLS certificate with a key in the OS key store, hardware-backed where available; one-time enrollment code; one current device per lab; revocation on re-enrollment (REQ-SVC-001, WF-19) | Enrollment and revocation drill; a revoked or second device is refused; a copied certificate file without the key-store key fails; where no hardware backing exists, the residual risk is recorded |
| Tenant cross-access in the cloud | One database per lab; per-lab keys and object prefixes; PHI-free control plane; authorization per lab for global clinic logins (REQ-SVC-002, NFR-27) | Negative isolation tests on API, workers, exports, backups, logs and support tools; tenant threat model |
| Stale cloud copy of a corrected or withdrawn report | Correction and withdrawal notices go through the outbox to every destination and cache; withdrawn copies marked on arrival; portal shows last-sync time (REQ-RPT-013) | Correction-propagation scenario including an offline host; portal access after withdrawal shows the withdrawn status |
| Script injection in the app window, or the UI calling engine functions it should not (XSS through payload text, a malicious dependency, remote content) | [Desktop application controls](#desktop-application-controls) (NFR-31): capability allow-list, strict CSP, no remote content, input validation and authorization in Rust | Injected-markup tests with hostile payload text; capability inventory review; attempts to call unlisted commands or load remote URLs fail; port scan shows no UI listener |
| LAN listener attack, if LAN workstations are enabled (Q-28) | Authenticated TLS on named interfaces only; per-request authorization; no unauthenticated endpoints | Unauthenticated, wrong-certificate and replay tests; unauthenticated-endpoint inventory |
| Telemetry or support bundle leaking PHI | PHI-free by default; allowlisted fields; redaction; PHI-bearing bundles only under a BAA with the lab's approval (REQ-SVC-003, NFR-32) | Inspection of representative telemetry, crash reports and bundles from synthetic runs |
| Accidental PHI in logs, URLs, analytics or AI tools | Payload minimization, structured redaction, no third-party tracking, no PHI in prompts, secret scanning | Egress inventory and representative log/trace/error inspections |
| Misused remote support session | Lab-initiated, attended, time-limited sessions; US-only staff; session records; no standing remote access | Session log review; attempt to connect without lab initiation fails |
| Report sent to the wrong recipient or signed-link disclosure | Current recipient authorization at retrieval and dispatch; destination verification before first PHI delivery to a new fax number or Direct address (REQ-RPT-012); authenticated revocable access | Revocation, wrong-person correction, changed-recipient and new-destination tests |
| Interface engine or hosted review service becomes an unmanaged PHI store | Include it in the data-flow and asset inventory; agreement review; MFA and least-privilege administration; patch source; retention, backup and audit alignment ([ADR 0007](decisions/0007-integration-edge-and-interface-engines.md)) | Data-flow map review, patch/version record, restore test of the engine store, end-to-end receipt reconciliation |
| Improper disposal of the host disk or backup media | Disposal runbook: final verified backup, certificate revocation, wipe or destroy, record ([deployment](deployment.md#host-retirement-and-disk-disposal)) | Disposal records for retired hosts, disks and rotated media |
| Retention or hold error | Record-class policy, legal holds, approved disposition, derived-copy inventory including cloud copies | Expiry/hold/restore tests including backups and cloud copies |

OWASP ASVS supplies a useful verification vocabulary; the proposed reference is ASVS 5.0.0, with a documented Level 2 applicability map plus risk-driven additions during detailed design. The official page identifies 5.0.0 as stable at this check; referencing a subset does not establish a passed ASVS level. NIST SSDF 1.1 supplies the documented development-process baseline; SSDF 1.2 is only an initial public draft (December 2025) [ENG-44](../research/sources-engineering.md). Neither is a regulatory certification. [ENG-10: OWASP ASVS](https://owasp.org/projects/asvs), [ENG-11: NIST SSDF 1.1](https://csrc.nist.gov/pubs/sp/800/218/final)

### Alignment targets from proposed and voluntary federal guidance

The HIPAA Security Rule NPRM remains **proposed**. The 2026 Unified Agenda lists it as a long-term action with an estimated final action in July 2027. HHS's healthcare Cybersecurity Performance Goals are **voluntary**. Neither is cited as a current mandate. The project uses them as design targets so that a final rule does not force rework. HHS 405(d) HICP Technical Volume 1 for small organizations is the best-matched voluntary baseline for an office lab ([§15.3](../research/regulatory-landscape.md#153-security-rule-on-a-single-pc)). [SR-019](../research/sources-regulatory.md), [ENG-40–42](../research/sources-engineering.md)

| Proposed / voluntary item | Design response |
| --- | --- |
| MFA on all technology assets; CPG MFA, unique credentials, separate privileged accounts, revoke departing users | Per-person app accounts with offline MFA; FIDO2 for release and administration; per-person OS administrator accounts; leaver revocation drill (NFR-19, REQ-IDN-004) |
| Encrypt all ePHI at rest and in transit; CPG strong encryption | Full-disk encryption on the host (BitLocker, FileVault or LUKS); built-in encrypted `vtbk1` backups; TLS on the LAN listener, the cloud link and MLLP tunnels; per-tenant encryption in the cloud |
| Annual technology asset inventory and network map; CPG asset inventory | Per-install inventory covering the host computer, backup targets, analyzers, printers, LAN workstations, enabled external services and support access, updated with each release and reviewed at least annually (NFR-22) |
| Restore critical systems within 72 hours; backups ≤48 hours old; separate backup controls | 72-hour replacement-computer restore as a project target (REG-048, NFR-30); off-host immutable copies (NFR-20) |
| Patching within 15/30 days for critical/high; six-monthly scans; annual penetration test; CPG vulnerability mitigation | Proposed release targets once maintainers are staffed (NFR-21); expedited, lab-approved security updates; OS support status in health checks (NFR-25); public vulnerability-disclosure route before public release (Q-19) |
| Network segmentation; CPG segmentation and centralized logs | No UI port; analyzer and MLLP ports on the lab LAN only; outbound-only cloud link; no inbound internet ports. Logs stay on the host, with PHI-free operational signals to the service provider only if the external service is enabled |
| Annual compliance audit and business-associate verification | Laboratory activities, with verification of the service provider for external services in use. The product supplies exportable control evidence (access reviews, audit-chain verification, restore records, update approvals) |
| CPG essentials owned by the organization: email security, basic training, incident planning, vendor/supplier requirements | Outside the software. The lab owns them for its installation; the service provider owns them for hosted services. The product contributes the supplier inventory (NFR-22), incident-record fields and security documentation |

The rows above cover all ten CPG essential goals and the relevant enhanced goals: asset inventory, testing, segmentation, centralized logs, configuration management and third-party vulnerability disclosure. This satisfies the REG-043 mapping request, pending security-officer review. [SR-100](../research/sources-regulatory.md)

### Regulated-function intended-use inventory (design obligation)

Before any of the following functions is enabled, it is listed with its intended use, inputs, outputs, audience, author/approver and whether a professional can independently review its basis:

- result-derived flags and notifications
- autoverification
- validity classification
- medication-consistency annotations
- other interpretive comments
- patient-facing explanations
- any future AI feature

Counsel and quality assess each entry against the FD&C Act software exclusions and FDA's Section 3060 and January 2026 CDS guidance. Interpretive rules and algorithms also get the Section 1557 §92.210 protected-characteristic input review. These are Q-26 and REG-024/040; see [regulatory landscape §9](../research/regulatory-landscape.md#9-fda-electronic-signatures-and-clinical-functionality).

The architecture supports this obligation in three ways:

- Rules are laboratory-authored, versioned configuration with no shipped clinical content.
- Every derived output shows the rule version and inputs to the reviewer.
- A function cannot be enabled without a recorded assessment reference.

The inventory records project readings, not FDA determinations. [SR-077–079](../research/sources-regulatory.md)

## Identity and authorization

**Accepted application authority, 2026-10-14 ([ADR 0018](decisions/0018-remediation-authority-and-method-continuity.md)).** Administrator approves and activates versioned application configuration, with recorded actor, version, activation and audit history. Configurable separation of duties remains enforced. Formal laboratory clinical approval may be maintained externally; this permission does not resolve unrelated source-document retention. Administrator alone cannot originate orders, technically review results or release reports. Those capabilities and engine checks remain separate.

**Local accounts on the host.** The LIS has its own per-person accounts, independent of the OS login, because bench computers are often shared. People sign in at the app's own sign-in screen; the app keeps receiving analyzer data while it shows that screen with nobody signed in, and received results wait for review (REQ-IDN-004, [technology research §8](../research/single-pc-technology.md#8-local-authentication-on-a-shared-bench-pc)). There is no OIDC provider and no internet dependency for local sign-in:

- **Factors — accepted 2026-10-15 ([ADR 0019](decisions/0019-security-key-authentication.md)).** First-administrator signup offers a personal security key with authenticator-verified PIN/biometric activation or the existing password/TOTP route. Key signup requires no LIS password or TOTP enrollment. Existing accounts can migrate after fresh authentication; password/TOTP credentials are retired only after a new key verifies. Key-only accounts reject password fallback. Additional staff accounts currently use temporary-password onboarding before their own migration. The native client uses USB on macOS/Linux and Windows WebAuthn on Windows; normal operation must not require elevation. Physical key/OS qualification remains G2-09/Q-42.
- **Release and administration.** A key account uses a fresh signed assertion with required user verification for the exact typed action retained in Rust, even when touch-only sign-in is allowed. Private, one-use confirmation receipts bind account, session, purpose and credential/account revisions and never return to the UI. The existing password/TOTP alternative still confirms actions with a fresh password; it does not satisfy the role-wide phishing-resistant target in NFR-19. That production-policy/qualification gap remains explicit in G2-09.
- **Laboratory key policy — accepted 2026-10-15.** A versioned, default-off setting permits touch-only sign-in/unlock for compatible newly enrolled keys. Changing it requires `manage_users` and fresh normal confirmation; enabling it requires a recorded reason even for the first policy version. The transaction audits the change, ends other laboratory sessions and invalidates outstanding action proofs while retaining the confirmed caller. In-flight key ceremonies recheck their policy snapshot. First-administrator signup and replacement-key recovery retain required verification. Touch-only authentication proves possession and presence, is single-factor, and increases exposure to lost or unattended keys; the laboratory owns the risk decision.
- **Personal unlock PIN — accepted 2026-10-15.** Optional and off by default; six ASCII digits unlock only the same user’s existing locked session. Five consecutive wrong/malformed submissions end it. Counters persist per session; successful PIN unlock resets them without extending the 12-hour absolute lifetime. PIN setup/change/removal requires fresh normal authentication and ends other sessions; administrative reset/key recovery removes it. Store only the encrypted Argon2id verifier under the installation key with user/purpose binding. A database-only copy cannot expose the verifier, but host/key compromise and shoulder surfing remain risks. PINs never replace initial sign-in, user switching, recovery or sensitive-action confirmation. See [ADR 0019](decisions/0019-security-key-authentication.md).
- **Fast user switching** at the app's sign-in screen, without an OS sign-out (which would stop listeners). Key accounts use their own selected key for sign-in and unlock, under the laboratory key policy above. Locked, ended or replaced sessions invalidate pending ceremonies. A keyboard-wedge badge identifies but never authenticates. Validate hardware PIN/UV caching before claiming a new PIN entry for each confirmation.
- **Credential recovery.** Every key enrollment issues a new high-entropy, hashed, one-use recovery code. Recovery verifies a replacement key before atomically consuming the code, revoking previous keys and ending sessions. It grants no session or roles; the person signs in with the replacement key. Cancellation leaves the existing credential/code intact. The installation encryption recovery kit remains separate. First signup retains a short-lived session permit until the user acknowledges saving the code; recovery never receives that permit.
- **Idle lock.** The configured idle timeout locks the current session and clears working content; the same user resumes with their normal credential or enabled personal PIN. Absolute expiry instead requires full sign-in; the engine enforces it, not the UI. Listeners keep running.
- **OS biometrics** (Windows Hello, Touch ID, Linux fprintd) are not used for attribution on a shared OS login, because credentials belong to the OS profile (inference, to be tested; [ENG-126](../research/sources-engineering.md#single-computer-technology-sources) for Windows). They are an alternative only where each person has their own OS account.
- **Relying-party ID.** The trusted engine uses `<workstation UUID>.vibe-tox.invalid` with its HTTPS origin, derived from the installation’s persisted workstation ID. No network request resolves this name. Restoring the database preserves the identity. LAN authentication remains outside this implementation and must qualify its RP/origin behavior separately (Q-28).
- **Attribution.** Every command records the app user, session and workstation.

Unique user identification is a required HIPAA implementation specification ([ENG-127](../research/sources-engineering.md#single-computer-technology-sources)). CLIA requires testing-personnel identity in test records; CMS survey guidance gives a user ID/password as an example of LIS confidentiality protection. It does not establish a categorical shared-OS-login violation or require a password in addition to a key ([§15.4](../research/regulatory-landscape.md#154-clia-with-one-pc-as-the-laboratorys-record-system), SR-122/154). The dedicated workstation and OS login are complementary safeguards; the current LIS design still authenticates the individual operator.

**Cloud identities.** OIDC applies only to the cloud services. Clinic users have one global portal login with MFA, authorized per lab and per clinic; a clinic ordering from two labs sees two separate lab-scoped contexts (REQ-ORD-011). Provider staff use separate administrative identities with phishing-resistant MFA. Apply current OAuth security guidance to the portal: authorization-code flow with PKCE, exact redirect validation, issuer/audience/expiry/nonce validation and key rotation, and a backend-mediated session that keeps tokens out of browser storage. [ENG-18: IETF RFC 9700](https://www.rfc-editor.org/rfc/rfc9700.html)

**Authorization.** Authentication establishes identity; application policy determines patient/client access and permitted clinical actions. Define separate accessioning, analyst, technical reviewer, authorized releaser, quality manager, billing, client clinician, auditor, integration service and app administrator capabilities. OS administration is not an app role and confers no clinical authority. Local credentialing and laboratory policy determine role assignment. A universal two-person release rule is not asserted; record whether policy requires independent reviewers and prevent self-approval where it does. The versioned separation-of-duties policy implements this ([ADR 0015](decisions/0015-separation-of-duties-policy.md)). No generic administrative edit of clinical state. Grant revocation participates in the release gate checks.

Remote review and release record the approved ReviewLocation code and session context (user, time, workstation). A location not on the approved list is denied (REG-037, SR-071).

Permission checks cover list queries, create operations, nested records, bulk actions, exports and workers. Object-level checks alone are insufficient for list and create paths. In the cloud, the database per lab is the tenant boundary; within a lab's database the portal still enforces per-clinic scope.

Use NIST SP 800-63B-4 as vocabulary [ENG-43](../research/sources-engineering.md):

- **Phishing-resistant authentication target.** FIDO2 for release and administration remains the production target; ADR 0019 retains an explicit password/TOTP alternative in the local application. Offering key authentication does not establish role-wide enforcement or hardware qualification. Future remote administration requires its own approved policy.
- **Passkeys.** Synced passkeys are acceptable up to AAL2, mainly for portal users.
- **Recovery administrators.** Non-exportable hardware-bound keys (AAL3-style), separate from their ordinary accounts.
- **SMS and voice.** As a project choice, not used for workforce authentication.
- **Sessions.** Idle and absolute limits are laboratory-approved values. NIST suggests AAL2 inactivity of no more than 1 hour and AAL3 no more than 15 minutes (both SHOULD).

**Emergency access.** Proposed; not implemented. A sealed, offline break-glass account serves when no administrator can sign in. Its use has a limited purpose and time bound, a recorded reason, an immediate alert and after-the-fact review within an approved period. It cannot silently bypass QC or confer result-release qualification.

### Desktop application controls

Desktop UI security boundary (NFR-31; [ADR 0012](decisions/0012-rust-tauri-desktop-application.md), [desktop stack research](../research/desktop-application-stack.md#one-process-what-it-means)). The application opens no UI server port; capability and content-security controls govern its embedded UI.

- **No UI port.** The window talks to the engine through in-process Tauri commands only; no local web server, no localhost listener.
- **Capability allow-list.** Tauri capabilities grant the window only the named engine commands it needs; no shell, filesystem or HTTP plugins exposed to the UI unless a reviewed need exists.
- **Content security policy.** A strict CSP: only bundled scripts and styles, no inline script, no `eval`, no remote origins. No remote content is ever loaded into the window, and navigation away from the bundled UI is blocked.
- **No `dangerous*` settings.** Tauri options named `dangerous…` stay off, and the release checklist verifies it.
- **The engine decides.** Every command validates its input and authorizes the signed-in person in Rust. The UI is treated as untrusted: it presents and collects input but never decides a gate, calculation or permission.
- **Payload text is data.** Analyzer, HL7 and portal text is rendered as text, never as HTML, so a hostile payload cannot script the window.
- **LAN listener, only if Q-28 is enabled.** Authenticated TLS on named interfaces, never all interfaces by default, with no unauthenticated endpoints.
- **OS accounts.** The app runs under a standard OS account; the data directory admits only that account and administrators.
- Never expose analyzer TCP listeners or MLLP beyond the lab LAN.

### Device certificates and the cloud link

Proposed (REQ-SVC-001, WF-19; [technology research §6](../research/single-pc-technology.md#6-local-to-cloud-connectivity)):

- The host connects outbound only, while the app is open, over HTTPS with a per-device X.509 client certificate. The key is generated non-exportably in hardware where the OS offers it (TPM on Windows through the Microsoft Platform Crypto Provider, Secure Enclave on macOS, TPM2 on Linux where configured), otherwise held in the OS key store ([ENG-121/122](../research/sources-engineering.md#single-computer-technology-sources)). Which key types each platform's hardware supports for TLS client authentication is a spike item.
- Enrollment uses a one-time code issued to a lab administrator, in the style of the RFC 8628 device grant.
- The cloud accepts traffic only from the lab's currently enrolled certificate. Re-enrollment after a restore revokes the old one, which fences a stolen or recovered old host.
- If an OAuth layer is added, tokens are sender-constrained (RFC 8705 mTLS-bound or RFC 9449 DPoP).
- Messages carry idempotency keys; a reused key with a different payload is an error ([ENG-123](../research/sources-engineering.md#single-computer-technology-sources)).

### Report access after issuance

Preserve who was authorized at issuance as historical evidence, then evaluate present authority for every report retrieval, export and delivery. Patient merges do not unite client permissions; withdrawals, corrected identity and changed disclosure restrictions update current access without erasing earlier artifacts. Sensitive reads use current authoritative access/restriction state.

The portal is a cache of released versions. Correction, withdrawal and access-revocation notices travel through the outbox to the portal and every other destination, and withdrawn copies are marked on arrival (REQ-RPT-013, REQ-RPT-014). While the host is offline, the portal cannot learn of a withdrawal; it shows the lab's last-sync time, and the lab's procedure covers direct notification for urgent corrections. Cached lists must not expose stale patient associations, snippets or download links after revocation reaches the cloud.

Use application-mediated authenticated retrieval where immediate revocation is required. A bearer object URL remains usable until its effective expiry unless the storage layer enforces revocation; short lifetime alone does not supply a current authorization check. Audit permitted historical access and make superseded/withdrawn status explicit. External copies already delivered by fax, Direct or EHR cannot be recalled; track notification and partner reconciliation separately, with minimal content for wrong-person recipients.

## Audit, secrets, and retention

Record actor/service identity, action, record scope, purpose/reason where required, old/new revision references, event and receipt timestamps, workstation, correlation ID, and outcome. Minimize duplicate clinical contents in audit events. Clinical changes and their essential audit records commit together in the same `BEGIN IMMEDIATE` transaction. If essential audit cannot be persisted, sensitive state changes fail safely; define and validate equivalent behavior for sensitive reads. Separate diagnostic logs, security events, clinical audit history, and physical custody records.

The release/audit transaction establishes application consistency, not protection against the lab's OS administrator or root, who can edit the database file directly. [ADR 0008](decisions/0008-tamper-evident-audit-and-immutable-recovery.md) proposes the mechanism:

- Audit events, release manifests and interface receipts carry per-stream sequence numbers and predecessor hashes.
- Digest-only anchors of sealed segments leave the host on a schedule: to a hosted write-once store over the outbound link, or directly to a lab-owned write-once bucket or offline media (Q-30). Anchors hold no PHI.
- Scheduled verification alerts on gaps, forks and anchor lag. An anchor outage queues anchors and raises alerts but does not block release unless lab policy says so.
- The SQLite host does not use pgaudit. OS file-access auditing on the data directory (Windows object-access auditing, macOS or Linux audit subsystems) can add a record of direct file access, with events kept off the host; this is an inference to test, and it is easily disabled by the same administrator.

Test altered database rows, substituted artifact files, stopped anchoring, and deletion of both live and backup evidence. A hash stored beside editable content is not independent integrity evidence. The laboratory approves the residual privilege and evidence-loss window before a real-data pilot. Every OS administrator (and anyone with root) on the host is named in that residual-privilege statement.

Keep TLS on the LAN listener, the cloud link and MLLP tunnels. Encrypt the host disk and every backup. Inventory key custody and rotation ([key custody](#key-custody)) and validate restoration after key rotation. Application secrets live outside the repository and installers. Scope credentials per installation and integration; revoke them without deleting historical attribution. Encryption never substitutes for authorization or a required business associate agreement.

Implement a configurable retention schedule by record class, jurisdiction, program, contract and legal hold. Do not apply "HIPAA six years" as a universal clinical-record retention period. Disposition must account for artifacts, revisions, messages, exports, cloud copies and backup aging. Recovery must reapply holds and disposition policy without silently reintroducing records into ordinary access. The compliance matrix supplies the research baseline; the laboratory approves the actual schedule.

### Telemetry, support and remote sessions

- **Telemetry.** Off or PHI-free by default: version, health-check states, counts and timings, opaque install ID. No patient, specimen, order, result or free-text fields; no file paths that embed names (NFR-32, REQ-SVC-003). This keeps the software publisher outside a business-associate role ([who holds what](#trust-and-data-model)).
- **Crash reports and support bundles.** Redacted by default and shown to the lab before sending. A PHI-bearing bundle requires a BAA, the lab's explicit approval per bundle, purpose limitation and deletion after use.
- **Remote support sessions.** Lab-initiated and attended, time-limited, with no standing remote access or unattended agent. Support staff are US-based (REQ-SVC-006, REG-049). Each session is recorded with who, when, why and what was accessed (164.312(b)). Whether a remote-access relay is a conduit or a subcontractor is a question for counsel.

### Host retirement and media disposal

Disposal and media re-use are required under 164.310(d) ([§15.3](../research/regulatory-landscape.md#153-security-rule-on-a-single-pc)). Retiring or replacing the host, returning a leased machine, swapping a disk and retiring backup media follow the [deployment runbook](deployment.md#host-retirement-and-disk-disposal), with a disposal record (REG-048).

## Incident and production readiness

Maintain an incident process for access misuse, wrong-recipient delivery, specimen/result integrity errors, host loss or theft, ransomware, outages, lost custody records and compromised dependencies or updates. Preserve evidence and activate the appropriate privacy, laboratory and operational owners. External notifications depend on the applicable rule and facts; do not hardcode one deadline for every incident. Incident records capture the fields a future CIRCIA report would need without asserting a current duty (REG-043, SR-101): discovery and impact times, affected systems and data classes, suspected vector, ransom demand or payment, and containment.

**Allocation between lab and service provider** ([§15.2](../research/regulatory-landscape.md#152-breach-and-incident-allocation)). The service provider notifies the lab of security incidents in its services well inside the 60-day maximum; the BAA sets a shorter clock. Notifying individuals, HHS and the media stays with the lab unless the BAA delegates it. A single cloud incident can affect many labs, so the provider keeps a per-lab incident record. Loss of hardware protected by full-disk encryption or zero-knowledge backup ciphertext, with keys uncompromised, is generally not a notifiable breach; loss of the portal or relay plaintext would be.

Before a restricted real-data shadow evaluation: establish data-flow/threat assessment, the host baseline and onboarding record, applicable BAAs, access matrix, security verification, restore evidence onto a replacement computer, disclosure/consent policy where applicable, incident playbook and an approved evaluation protocol. The protocol defines the incumbent clinical authority and prevents evaluation outputs from guiding care. Operational patient reporting additionally requires validated intended use, qualified staff, clinical SOPs and explicit laboratory acceptance of the installed version. The [stage gates](../quality/validation-plan.md) record evidence and responsible humans. No real data is authorized by this design.
