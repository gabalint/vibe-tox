# ADR 0015: Separation of duties is laboratory policy

Status: **accepted** by the project owner, 2026-10-14.

Related: [Q-08](../../planning/open-questions.md) (release policy), [Q-37](../../planning/open-questions.md) (account administration), REQ-RES-005, REQ-OPS-015, REQ-ANA-003, REQ-ANA-009, [security, audit and logging](../../governance/security-audit-logging.md#authorization).

## Context

The product serves physician-office and small independent laboratories, including laboratories operated by one qualified person. Separation controls must follow an explicit laboratory policy while preserving personal attribution. Requiring another account for the same person would falsify that attribution.

The policy covers catalog and result-source configuration, billing-class mappings, technical review/report release and delivery-destination verification. [Workflows](../../requirements/workflows.md), REQ-RES-005 and Q-08 define laboratory responsibility for qualifications and independent review. [ADR 0018](0018-remediation-authority-and-method-continuity.md) assigns application configuration approval to administrators without granting clinical authority.

## Evidence (checked 2026-10-14; US clinical laboratories)

| Source | What it requires | Different person required? |
| --- | --- | --- |
| [42 CFR 493.1251(d)](https://www.ecfr.gov/current/title-42/section-493.1251) | "Procedures and changes in procedures must be approved, signed, and dated by the current laboratory director before use." | No: the director |
| [CMS SOM Appendix C](https://www.cms.gov/regulations-and-guidance/guidance/manuals/downloads/som107ap_c_lab.pdf), D5403 | "Approval of procedures and changes to procedures is the responsibility of the laboratory director. This responsibility cannot be delegated." | No: the director approves even what they wrote |
| [42 CFR 493.1445(a)](https://www.ecfr.gov/current/title-42/section-493.1445), and 493.1407(a) for moderate complexity | "The laboratory director, if qualified, may perform the duties of the technical supervisor, clinical consultant, general supervisor, and testing personnel." | No: one person may hold every role |
| 42 CFR 493.1445(e)(15) | The director specifies in writing "whether supervisory or director review is required prior to reporting patient test results." | No: the director decides |
| CAP GEN.43825 ([2016 public copy](https://www.illumina.com/content/dam/illumina-marketing/documents/services/laboratory-general-checklist.pdf)) | Results are verified by an authorized person before release; "this may or may not require a second person." | Explicitly no |
| CAP DRA.11425 (12/2025 director checklist, [public copy](https://www.dshs.wa.gov/sites/default/files/2026-09/ExhibitA%20CAP%20Director%20Assessment%20Checklist%202025.pdf)) | Approval of new technical policies and procedures may not be delegated. | No |
| [COLA](https://public.powerdms.com/COLAMD/documents/787731), [NYS CLEP General Systems Standards, Dec 2024](https://www.wadsworth.org/sites/default/files/2024-12/General%20Systems%20Standards_Effective%20Dec%202024.pdf), ISO 15189:2022 7.4.1.2 and 8.3.2 (via the [NATA worksheet](https://nata.com.au/files/2023/06/15189-Assessment-Worksheet-2022.pdf)) | The director signs procedures; designated, authorized staff review and release results. | No |
| HIPAA Security Rule, 45 CFR 164 subpart C | Access-authorization policies; no separation-of-duties text. NIST SP 800-53 AC-5 is guidance in which the organization defines the duties. | No |
| Forensic toxicology: [ABFT checklist E-34](https://www.abft.org/wp-content/uploads/2023/04/ABFT_LAP-Checklist_2023-v.Jan-31.pdf) (2023), [ANAB AR 3125](https://anab.qualtraxcloud.com/ShowDocument.aspx?ID=12371) 7.7.1.l | Technical review by a qualified person other than the analyst (ANAB: for a representative sample). | **Yes**, outside the clinical scope |

Narrow CLIA exceptions that do need a second person (general-supervisor review of certain pre-1993 staff, 493.1463(c) and 493.1495(c); cytology rescreening, 493.1274(c)) do not cover toxicology configuration or release. Not verified: the current CAP checklist text and the Joint Commission laboratory manual (paywalled), and the NLCP manual for federal workplace testing. The sources are registered as SR-137 to SR-145 in the [regulatory source register](../../research/sources-regulatory.md).

**Conclusion.** For US clinical laboratories, separation of duties is good practice and laboratory policy, not a legal requirement. The director's procedure-approval responsibility does not itself require a different author and approver.

## Decision

1. **A versioned laboratory policy with three switches**, each saying whether one person may do both steps of a duty (`lis-auth`, `auth_separation_policies`):
   - configuration approval: approve configuration one wrote, which covers catalog versions, All Data Reviewed, result source registrations and billing class mappings;
   - report release: release a report whose reported results one reviewed technically;
   - destination verification: verify a delivery destination one created.

   Until the laboratory sets a version, every duty needs a second person. An administrator changes the policy using fresh authentication, a reason and the current version ([ADR 0018](0018-remediation-authority-and-method-continuity.md)); the change is audited as `auth.set_separation_policy`. Versions are append-only.
2. **Each self-approval is deliberate, attributable and reviewable**, as [AGENTS.md](../../../AGENTS.md#domain-invariants) requires of a policy exception:
   - the owning module checks the policy in force inside the action's transaction, through `SeparationPolicy::authorize_self`, the one home of the rule;
   - the person confirms each self-approval (input field `selfApproval`); without it the command is refused with a field error naming the confirmation;
   - the audit event carries `self_approval: true`, and the audit viewer can list only those events.
3. **A one-person laboratory.** First-run setup offers "one person runs this laboratory". Later, while the actor is the laboratory's one active account, `designate_one_person_laboratory` (`manage_users`, password, reason) does the same. Either way the account gets every role and all three switches are allowed. This is the only case in which anyone grants their own roles; Q-37's other rules are unchanged.
4. **Schema guards follow the policy.** Triggers allow author = approver only while `auth_v_separation_policy` allows it. Destination verification follows the same policy check.
5. **Clinical gates:** release needs fresh authentication and every gate; QC, identity and validity holds are unaffected; an administrative role still confers no clinical authority; the analyst is still not barred from technical review ([M2-R8](../../quality/laboratory-policy-inputs.md)).
6. **Forensic and federally regulated workplace programs**, once supported, must require a second person for technical review whatever the switches say (ABFT E-34, ANAB AR 3125). No such program context exists in the product yet, so nothing is built for it now.

## Alternatives considered

| Alternative | Why not |
| --- | --- |
| Fixed two-person rules for every duty | One-person laboratories cannot operate, and a second account for the same person falsifies attribution |
| No separation rules | Labs with staff lose a control they may want; REQ-RES-005 asks for configured two-person rules |
| Allow self-approval automatically when nobody else holds the capability | Not a laboratory decision; it changes silently with staffing and fails during an absence |
| One switch for every duty | A two-person lab may keep two-person release while its director approves the procedures they wrote, as CLIA expects |
| A typed reason on every self-approval | Daily friction for a one-person lab; the policy version carries the reason, and the per-action confirmation makes each act deliberate |

## Consequences

- A self-approved step has no independent check. The mitigations are an explicit, reasoned policy version, a confirmation per action, the audit marker with its filter, and the laboratory's own periodic review of self-approvals. How often that review happens is the laboratory's decision.
- The laboratory's SOPs and validation must cover the policy it sets. Setting it is not evidence that the arrangement is acceptable to its accreditor or state.
- `lis-catalog`, `lis-analytical` and `lis-billing` depend on `lis-auth` for the policy read, and their schema guards, like `lis-reporting`'s, read the `auth_v_separation_policy` view.

## Revisit when

- forensic or workplace program contexts are added;
- a laboratory, accreditor or state requires independent review of a duty;
- remote review or FIDO2 sign-off changes how a second person is identified.
