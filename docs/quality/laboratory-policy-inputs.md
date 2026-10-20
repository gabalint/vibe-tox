# Laboratory policy and qualification inputs

These stable input IDs identify decisions and acceptance evidence still needed from a laboratory. Implemented defaults are software behavior, not approved clinical policy. [Open questions](../planning/open-questions.md) name decision owners; [G1/G2](open-items-g1-g2.md) tracks readiness. IDs remain stable where an accepted ADR governs the implementation choice.

## Order to result

The [order-to-result contract](../architecture/order-to-result-contract.md) defines the technical boundaries.

| ID | Laboratory input | Current behavior and acceptance boundary |
| --- | --- | --- |
| M2-R1 | Order-completeness/authority versus payer evidence (Q-21) | Staff classify supplied evidence. Clinical clarification holds remain separate from financial deficiencies. The laboratory must approve its evidence and follow-up procedure. |
| M2-R2 | Accession/container number formats and label content | Default accession `A` + year + sequence and container `C` + sequence; label truncation is recorded. Qualify actual stock, layout, scanner and patient-identity presentation. |
| M2-R3 | Validity categories and actions per matrix (Q-24) | No clinical validity policy ships. Required validity review needs an approved policy and evidence. |
| M2-R4 | Rounding for compare-after-rounding methods (Q-03) | Cases needing an unsupported rounding rule are not classifiable. Supply/validate rules before enablement. |
| M2-R5 | ASTM correction semantics (Q-04/Q-25) | A changed resend without an approved revision indicator remains a conflict for attributable review. Qualify actual analyzer/version/profile behavior. |
| M2-R6 | Performed-but-unordered findings (Q-08/Q-41) | ASTM uses the Not ordered queue; files retain usable measurements without a result line. Only an accepted add-on and reviewer action can use them. Notification to the orderer remains laboratory policy. |
| M2-R7 | QC acceptance and control ranges (Q-08) | Attributable assessment of configured evidence; no unrestricted waiver or administrator QC bypass. |
| M2-R8 | Analyst, technical reviewer and releaser responsibilities (Q-08) | Roles and attribution remain distinct. ADR 0015 governs configurable self-approval/release; the laboratory must approve its staffing/oversight policy. |
| M2-R9 | Reflex trigger policy | Supplied authority and attributable manual decisions; no unapproved automatic clinical rule. |
| M2-R10 | Stability windows for add-ons | Laboratory SOP and attributable stability confirmation remain required where applicable; software must not invent a window. |
| M2-R11 | Volume units and measured/estimated status | Optional exact values; arithmetic only for identical unit text; unknown volume does not automatically deplete material. |
| M2-R12 | Rejected-specimen follow-up | Confirm the laboratory's replacement/recollection and correction procedure; do not treat a rejected specimen as silently accepted. |
| M2-R13 | Collection time-zone evidence | Preserve supplied civil time, zone/offset and precision. Ambiguous times need explicit resolution; collection corrections preserve history. Qualify the lab's source and correction procedure. |
| M2-R14 | Instrument readiness evidence | Run creation requires current service/readiness information; no status evidence does not imply readiness. |

## Reporting and handoff

The [reporting contract](../architecture/reporting-contract.md) defines immutable reports, release and delivery evidence. Authentication follows ADR 0019; laboratory method/release policies remain Q-08/Q-21/Q-24.

| ID | Laboratory input | Current behavior and acceptance boundary |
| --- | --- | --- |
| M3-L1 | Report unit/grouping | One report per specimen; an item spanning accessions blocks release. |
| M3-L2 | Report number | Accession number plus specimen-type code; validate local numbering and recipient expectations. |
| M3-L3 | Partial reports | Component-level partial release with explicit acknowledgement; define recipient handling and outstanding-component follow-up. |
| M3-L4 | Self-release | ADR 0015 policy governs whether a reviewer can release their own results; allowed actions still need confirmation. Lab policy/oversight remains to be accepted. |
| M3-L5 | Issue time/candidate lifetime | Candidate preparation time in the laboratory zone; fixed 60-minute release lifetime. |
| M3-L6 | Release authentication and key qualification | Fresh action confirmation is implemented for supported authentication routes. Physical-key/OS qualification and production authentication policy remain G2-09/Q-42. |
| M3-L7 | Printed vocabulary | Exact lexical values and source qualitative codes; quantitative cutoff comparison does not invent a clinical positive/negative interpretation. Approve the complete report presentation. |
| M3-L8 | Validity consequences | Approved policy actions determine hold/recollect, qualifier or category comment behavior; no shipped clinical thresholds. |
| M3-L9 | Container holds | Quality/excursion affect release; retention/legal holds protect custody. Confirm the laboratory's distinction and procedures. |
| M3-L10 | Destination verification | A destination needs verification under the current authority/separation policy. Qualify verification method and recipient identity. |
| M3-L11 | Routing | Active location destinations, otherwise client destinations, plus required prior recipients for follow-up. Validate routing per client. |
| M3-L12 | Withdrawal | Terminal report-level withdrawal with minimal notice to previous destinations; approve the recipient/disclosure procedure. |
| M3-L13 | Impact during delivery | Open impact pauses delivery attempts until attributable quality assessment; define escalation and correction handling. |
| M3-L14 | Billing derivation/handoff (Q-16/Q-21) | Collection civil date, final-report prerequisites, result-neutral class derivation and explicit failed/validity/unordered dispositions. Receiving-system and payer policy still need acceptance. |
| M3-L15 | Laboratory profile | Authorized configuration records versioned profile changes and reasons; verify CLIA, signatory, time-zone and billing identity fields. |
| M3-L16 | Impact ownership | Quality staff assess impact; authorized signatories issue corrections. Assign named owners and response procedures. |

## Operations

The [operations contract](../architecture/operations-contract.md) describes implemented safeguards. It does not qualify a host, backup target or laboratory recovery procedure.

| ID | Laboratory input | Current behavior and acceptance boundary |
| --- | --- | --- |
| M4-L1 | Backup/export authority | Administrator role includes the relevant capabilities; sensitive operations also require fresh normal authentication. Assign custody and least-privilege responsibilities. |
| M4-L2 | Backup interval/retention | Default 60-minute full copies; 6 recent, 7 daily and 4 weekly sets per target; keep manual/pre-update sets. Warning age must exceed interval. Independent seals run at least every 15 minutes. The 15-minute recovery point (NFR-30) is unmet; incremental backups remain proposed in [ADR 0014](../architecture/decisions/0014-incremental-backups.md). |
| M4-L3 | Offline rotation | Removable target rotation; default warning after 7 days without an offline copy. |
| M4-L4 | Restore testing | Before operational acceptance and on an approved recurring schedule; default age warning is 90 days. |
| M4-L5 | Recovery-kit custody | Protected copies off the host; default storage-confirmation age is 90 days. Assign authorized custodians. |
| M4-L6 | Anchors and thresholds (Q-30) | Available folder targets receive anchors; off-host lag defaults to 30-minute warning/24-hour incident. Validate independent custody and inability of a compromised host to erase all evidence. |
| M4-L7 | Recovery reconciliation | Release, delivery, billing handoff and label printing wait for complete attributable quality closure with fresh authentication. Validate the lost-interval runbook. |
| M4-L8 | Number floors | Restore adds 1,000 accession numbers/5,000 container codes; the lab can raise floors further. Confirm coverage of issued physical labels. |
| M4-L9 | Unaccepted application version | Banner/work item; no current release block. Decide operational enforcement and acceptance ownership. |
| M4-L10 | Exit-export custody | Plaintext complete package; laboratory protects/encrypts destination media and governs disclosure. |
| M4-L11 | Session/lockout policy | Default idle timeout 15 minutes and five failures/15-minute lockout; normal session limit and authentication policy follow ADR 0019. Approve configured values and physical access controls. |
| M4-L12 | Web-engine change | Warning pending a smoke-check attestation; qualification is required for each supported platform/runtime. |
| M4-L13 | Restore destination | Restore requires an installation without users. Rebuilding a live host follows the documented move-aside/replacement runbook, never an in-app overwrite of live user data. |

Engineering qualification remains in [G1/G2](open-items-g1-g2.md) and the [validation plan](validation-plan.md). An implementation default does not resolve an input.
