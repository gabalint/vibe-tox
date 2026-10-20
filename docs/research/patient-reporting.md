# Patient reporting: content and disclosure boundaries

Evidence checked **2026-10-14**, for US federal clinical toxicology with nonwaived
testing as the working applicability assumption. This synthesis retains the
reporting sources and limits that inform REQ-RPT-001–010. Source dates, authority and
retrieval limitations are recorded as RPTS-01–12 in the
[regulatory source ledger](sources-regulatory.md#patient-reporting-sources).
State, accreditor and approved laboratory-policy requirements remain unresolved.

## Report content

[42 CFR 493.1291(c)](https://www.ecfr.gov/current/title-42/section-493.1291)
establishes the federal report-content floor. The
[CMS State Operations Manual Appendix C](https://www.cms.gov/Regulations-and-Guidance/Guidance/Manuals/downloads/som107ap_c_lab.pdf),
D5801–D5823, supplies survey interpretation; it is guidance rather than a separate
statute. The following distinctions must survive changes to report templates and
stored report provenance.

| Element | Supported requirement or interpretation | Boundary |
| --- | --- | --- |
| Patient identification | Patient name or unique identifier, and an identification number; CMS accepts name plus accession identification. | Do not invent an external MRN. Birth date does not replace the identification number. |
| Performing location | Name and address of the laboratory that performed testing. | Current contact configuration alone does not establish where historical testing occurred. Referrals require their own attribution and exact-report handling. |
| Report and test dates | Report date; CMS guidance addresses stable final-generation dates and identifiable cumulative test dates. | Label preparation, release, result-recording and testing dates by their actual meaning. Do not infer missing analytical completion dates from software timestamps. |
| Test and specimen | Identify the test performed and specimen source when appropriate. | Requisition information and receipt records are distinct from mandatory printed-report fields. |
| Results | Result, applicable units and/or interpretation. | Preserve exact quantities, comparators, method versions and analytical meaning; the report template must not invent conversions or interpretations. |
| Unacceptable specimen | Relevant condition and disposition information. | Retain supplied condition details and their provenance; routine presentation must not hide a release hold. |
| Corrected reports | Identify corrected results, retain originals and follow correction-notification duties. | A generic correction banner may not identify the affected analyte or context. Display of the prior value is a laboratory design decision, not a universally prescribed field. |

## Surrounding duties

| Area | Evidence and product consequence | Applicability and limits |
| --- | --- | --- |
| Delivery and retrieval | Accurate, timely transmission to authorized recipients and retrievable reports; [493.1291(a), (b), (f), (j)](https://www.ecfr.gov/current/title-42/section-493.1291). | Export or transport acknowledgement alone does not establish successful receipt and processing. |
| Reference and method information | Pertinent reference information available, method/performance/interference information on request, and relevant changes communicated; [493.1291(d), (e)](https://www.ecfr.gov/current/title-42/section-493.1291). | A cutoff, LOD or LOQ is not a reference interval. Necessary assay caveats and report inclusion follow approved laboratory policy, not universal toxicology boilerplate. |
| Urgent results, delays and errors | Immediate urgent alert, urgency-based delay assessment, prompt error notice and correction with originals retained; [493.1291(g), (h), (k)](https://www.ecfr.gov/current/title-42/section-493.1291). | Read-back, escalation and exact time frames require the laboratory's policy. Ordinary delivery work does not supply the whole process. |
| Requisition and receipt | Authorized request, solicitation of required information, accurate transcription and documented receipt time; [493.1241](https://www.ecfr.gov/current/title-42/section-493.1241) and [493.1242](https://www.ecfr.gov/current/title-42/section-493.1242). | These duties do not make every requisition field mandatory on the final PDF. Unknown collection precision must remain explicit. |
| Retention | Original, preliminary, final and corrected reports retained and retrievable for at least two years after reporting, with closure provisions; [493.1105](https://www.ecfr.gov/current/title-42/section-493.1105). | Longer applicable requirements and legal holds may apply. This is not a universal deletion schedule; HIPAA is not a universal six-year medical-record rule. |
| Pertinent interpretation | Director responsibilities under [493.1445(e)(8)](https://www.ecfr.gov/current/title-42/section-493.1445) or [493.1407(e)(8)](https://www.ecfr.gov/current/title-42/section-493.1407). | Complexity and intended use determine applicability. Lab-approved deterministic comments remain distinct from inferred medication adherence or invented clinical conclusions. |
| Patient and representative access | Covered entities provide applicable designated-record-set access under [164.524](https://www.ecfr.gov/current/title-45/section-164.524); the [HHS FAQs](https://www.hhs.gov/hipaa/for-professionals/faq/right-to-access-and-research/index.html) include laboratory records. | Access can extend beyond final PDFs. Identity/authority, request scope, format, timing, extensions and denials need a qualified workflow. A full-host exit export is not a per-patient disclosure. |
| Information blocking | Actor/EHI and knowledge standards under [171.102](https://www.ecfr.gov/current/title-45/section-171.102) and [171.103](https://www.ecfr.gov/current/title-45/section-171.103); [ASTP/ONC guidance](https://healthit.gov/faq/when-would-delay-fulfilling-request-access-exchange-or-use-ehi-be-considered-interference-under/) addresses clinician-review delays. | Absence of a portal alone determines no violation. Do not turn HIPAA's outside response deadline into a blanket waiting period; assess exceptions individually. |
| Part 2 | Determine program and record provenance under [2.12](https://www.ecfr.gov/current/title-42/section-2.12), and applicable disclosure notices under [2.32](https://www.ecfr.gov/current/title-42/section-2.32). | A toxicology result or SUD diagnosis alone does not establish applicability. No universal Part 2 warning belongs on every PDF. The [2024 rule](https://www.hhs.gov/hipaa/part-2/index.html) had a 2026-02-16 compliance date. |

## Current boundaries and evidence still needed

The [requirements](../requirements/functional-requirements.md),
[M3 reporting design](../planning/m3-design-brief.md) and
[domain model](../architecture/domain-model.md) describe the product contracts.
Use the [operator guide](../operations/README.md) and built-in help for implemented
actions. Research is not evidence that a requirement has passed its acceptance
checks.

Source-document custody (DM-07), urgent/delayed-result communication (RPT-06) and
patient/proxy disclosure (RPT-07) remain explicit
[scope decisions](../quality/scope-decisions.md).
Laboratory approval of required method comments/reference information, reporting
vocabulary and any interpretive rules remains Q-22/Q-24/Q-26. Referral and
multi-site attribution need their own qualified contracts.

Preserve exact issued artifacts and versioned content through corrections,
withdrawals and recovery. Partial ordered work is not automatically a preliminary
result. PDF/A or PDF/UA renderer settings alone do not establish accessible
reading order, print quality or conformance; the outstanding validation work
belongs in the [quality baseline](../quality/open-items-g1-g2.md).
No licensed accreditation clauses or state-specific approval are supplied by
this research.
