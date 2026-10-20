# Regulatory landscape and applicability

Status: research baseline checked 2026-10-12 for the [initial scope](../project-scope.md#confirmed-initial-scope); regulatory and laboratory review pending. Nonwaived workflows remain the baseline until the actual menu and complexity are assessed, and state-specific applicability remains open. This is a design input, not a legal opinion, certification, or finding that a laboratory or deployment is compliant.

The [compliance matrix](../requirements/compliance-matrix.md) turns this research into proposed controls and evidence. The [source register](sources-regulatory.md) records authority, dates and access limitations; sources it marks “Source summary” or “Snippet” need primary-text verification before deciding a design or deployment question. Payer coverage conditions and fraud-and-abuse law remain distinct from laboratory-quality rules. “Required” below describes an obligation only when its stated applicability holds; implementation recommendations are explicitly labeled.

## 1. Classify the laboratory, activity, and record before selecting rules

| Activity | Starting regime | Boundary that affects the LIS |
| --- | --- | --- |
| Testing human specimens for diagnosis, prevention, treatment, or health assessment | CLIA, applicable state law, applicable privacy law | Certificate, testing complexity, personnel qualifications, approved procedures, and performing location belong to the laboratory's actual activity. |
| Exclusively forensic testing | CLIA includes a forensic-only exception | The exception is limited to qualifying functions; adding clinical testing changes the analysis. Evidence custody, legal holds, court disclosure, and forensic accreditation need their own assessment. |
| SAMHSA-certified drug testing meeting SAMHSA requirements | Federal workplace program | CLIA has a specific exception for this testing; other testing by that laboratory remains subject to CLIA. |
| DOT-regulated workplace drug testing | 49 CFR Part 40 plus applicable HHS requirements | Requires eligible laboratory certification, program-specific collection, custody, verification, and reporting. A clinical positive is not an MRO-verified employment result. |
| Research with no patient-specific diagnostic/health-assessment results | Potential CLIA research exception | Returning clinical results, FDA-regulated investigations, and study contracts require a fresh determination. |

Sources: [CLIA applicability, 42 CFR 493.3](https://www.ecfr.gov/current/title-42/chapter-IV/subchapter-G/part-493/subpart-A/section-493.3), [CDC CLIA overview](https://www.cdc.gov/clia/), [DOT laboratory participation](https://www.transportation.gov/odapc/labs).

**Design recommendation:** capture a program context on each order/accession, with laboratory entity, performing site, originating jurisdiction, intended use, method/specimen, authorized ordering party, disclosure policy, and applicable policy versions. Do not infer program from a drug name, patient diagnosis, or specimen alone. A laboratory can operate multiple programs, but their cutoffs, reports, permissions, and release criteria must not silently mix.

## 2. CLIA requirements that shape the core clinical workflow

### Preanalytic controls

The laboratory needs authorized test requests and accurate transcription. If oral requests are accepted, it must seek written/electronic authorization within 30 days and retain the authorization or its documented efforts. Requisitions solicit identifiers, requester/contact, requested tests, appropriate source and collection timing, demographics, and test-relevant information. [42 CFR 493.1241](https://www.ecfr.gov/current/title-42/chapter-IV/subchapter-G/part-493/subpart-K/subject-group-ECFR5f8f0b6639946fd/section-493.1241).

CLIA's authorized-person definition depends on state law. Order authority, a laboratory's validated analytical procedure and a payer's coverage policy therefore need separate applicability decisions. **Design recommendation:** distinguish an analytical repeat needed to complete an existing request from an add-on/reflex that expands requested testing; retain the applicable authority for each. A configured reflex rule does not establish that authority by itself. [42 CFR 493.2](https://www.ecfr.gov/current/title-42/chapter-IV/subchapter-G/part-493/subpart-A/section-493.2).

Collection, labeling, transport, storage, processing, rejection, and referral follow written procedures. Receipt date/time must be documented; referrals require a CLIA-certified laboratory or CMS-recognized equivalent. [42 CFR 493.1242](https://www.ecfr.gov/current/title-42/chapter-IV/subchapter-G/part-493/subpart-K/subject-group-ECFR5f8f0b6639946fd/section-493.1242).

**Design recommendation:** preserve the original order payload/document; maintain a reconciliation queue for missing or conflicting identities; use barcode-level specimen/aliquot lineage; record receipt separately from collection; record rejection reason and disposition. Prevent an identity merge from silently transferring released reports to another patient. Requirements that say “solicit” information do not automatically justify blocking every urgent case with incomplete demographics; exception handling needs laboratory-approved SOPs.

### Analytic controls and quality

Performance verification/establishment is required before reporting patient results, with scope varying by unmodified cleared/approved versus modified or other test systems. The LIS should link method versions to laboratory-approved validation evidence rather than declare a method valid because a software test passes. [42 CFR 493.1253](https://www.ecfr.gov/current/title-42/chapter-IV/subchapter-G/part-493/subpart-K/subject-group-ECFRc96daead380f6ed/section-493.1253).

Control procedures monitor analytical accuracy and precision; controls must satisfy applicable acceptance criteria before patient reporting. Failures require documented correction and assessment of affected patient results since the last acceptable run. [42 CFR 493.1256](https://www.ecfr.gov/current/title-42/chapter-IV/subchapter-G/part-493/subpart-K/subject-group-ECFRc96daead380f6ed/section-493.1256), [42 CFR 493.1282](https://www.ecfr.gov/current/title-42/chapter-IV/subchapter-G/part-493/subpart-K/subject-group-ECFRc96daead380f6ed/section-493.1282).

Proficiency testing samples follow routine workload/methods, with restrictions on interlaboratory result communications and referral; ordinary reflex/send-out routing must not inadvertently send PT material elsewhere. Preserve handling and testing records and required analyst/director attestations. [42 CFR 493.801](https://www.ecfr.gov/current/title-42/chapter-IV/subchapter-G/part-493/subpart-H/section-493.801).

**Design recommendation:** link raw instrument artifacts, analysis runs, calibration/QC evaluation, reagent and control lots, method versions, reruns, authorized reviewers, and reportable results. A QC hold cannot be bypassed by a generic administrator permission. Any approved correction must preserve its evidence and affected-result review. Support PT routing safeguards without special analytical treatment or revealing other sites' PT results; detailed enrollment, alternative performance assessment and personnel competency mapping remains a deployment task.

### Postanalytic controls

Reports require patient identification, performing laboratory location, date, test, result and applicable units/interpretation, plus relevant specimen exceptions. Transmission must preserve accuracy through destination. Critical results require immediate alerting; reporting errors require prompt notice and corrected reports while retaining originals. A missed reporting time frame requires an urgency-based determination of whether delay notification is needed. A referring laboratory must not revise the testing laboratory's results or related interpretive information and must retain or produce an exact report duplicate when permitting direct delivery. [42 CFR 493.1291](https://www.ecfr.gov/current/title-42/chapter-IV/subchapter-G/part-493/subpart-K/subject-group-ECFR9482366886d579f/section-493.1291).

**Design recommendation:** treat released reports as immutable versions. Separate clinical release, report rendering, destination authorization, delivery attempts, and acknowledgement. A corrected result creates a new version linked to the prior version and a tracked notification task. Wrong-patient cases require separate review of error notification and access to replacement content. Preserve referral-source interpretation through templates and mappings. Critical-result thresholds, delay assessments and escalation timing require laboratory director approval.

### CLIA program changes and guidance relevant to the LIS (checked 2026-10-12)

- **Remote data review (CMS guidance, enforcement discretion).** QSO-23-15-CLIA was revised on 2025-09-23. Under it, CMS continues to allow personnel to review digital laboratory data, results and images remotely, including from a private residence, under the primary site's certificate, subject to listed conditions. One condition is that the primary laboratory's test reports must indicate the remote site location, using a code if preferred, and the code list must be available on request. Remote review of LC-MS/MS data is common in toxicology, so the LIS should record the review location or code with each remote review and release action, and reports should be able to show it. [QSO-23-15-CLIA revised](https://www.cms.gov/files/document/qso-23-15-clia-revised-2025-09-23.pdf). CMS-3478-P (proposed, §14) would codify "virtual access" and add records of who accessed, the software version and the device used.
- **Fees and personnel.** CMS-3326-F was published 2023-12-28. Its fee provisions took effect 2024-01-27 and its personnel provisions on 2024-12-28. QSO-25-21-CLIA (2025-06-23) announced enforcement discretion on some personnel provisions pending further rulemaking. Personnel-qualification evidence supports REQ-QMS-002 authorization checks. The laboratory decides qualifications; the LIS does not. [CMS-3326-F](https://www.federalregister.gov/documents/2023/12/28/2023-28170).
- **Proficiency testing.** The toxicology regulated analytes in 493.937 are serum acetaminophen, blood alcohol, blood lead, carbamazepine, digoxin, gentamicin, lithium, phenobarbital, phenytoin, salicylate, theophylline, tobramycin, valproic acid and vancomycin. Urine drug screens and confirmations are not regulated analytes. For those tests, 493.1236(c) requires accuracy verification at least twice annually by alternative assessment. The menu therefore determines whether formal PT events or alternative assessments apply (REG-028). [42 CFR 493.937](https://www.ecfr.gov/current/title-42/section-493.937).
- **Paperless certificates.** As of the page update on 2026-10-07, CMS sends CLIA certificates and notices by email and requires online fee payment. Certificate evidence is therefore an electronic document with effective dates, held by the laboratory. The start date of this change was not established. [CMS CLIA page](https://www.cms.gov/medicare/quality/clinical-laboratory-improvement-amendments).
- **Patient access under CLIA.** 493.1291(f) limits release to authorized persons. The exception is 493.1291(l), which, since 2014, has allowed a laboratory to give completed test reports to the patient or the patient's personal representative on request. Patient access timing is discussed in §4 and §11.

## 3. Retention is a policy per record class

These are federal minimums relevant to the baseline, not a universal deletion schedule. State rules, accreditation, contracts, studies, and legal holds may require more.

| Record class | Federal CLIA minimum / trigger |
| --- | --- |
| Test requests and authorizations | At least 2 years. |
| Discontinued test procedure | At least 2 years after discontinuance; preserve use/discontinuance dates. |
| QC, patient test records, instrument printouts and specified analytical records | At least 2 years. |
| Established/verified test-system performance specifications | While the system is used, and never less than 2 years. |
| Proficiency testing and quality assessment records | At least 2 years. |
| Preliminary, final and corrected reports | Retrievable copies for at least 2 years after reporting. |
| Pathology reports, if later in scope | At least 10 years after reporting; pathology slides/blocks/tissues have separate provisions. |

Source: [42 CFR 493.1105](https://www.ecfr.gov/current/title-42/chapter-IV/subchapter-G/part-493/subpart-J/section-493.1105). Closure of a laboratory does not eliminate its retention obligations. This provision does **not** establish a generic two-year physical urine-specimen retention rule.

HIPAA Privacy Rule does not prescribe medical-record retention. HIPAA-required security documentation must be retained six years from creation or last effective date, whichever is later; this is not a rule that every clinical record or every operational log has exactly a six-year lifetime. [HHS medical-record retention FAQ](https://www.hhs.gov/hipaa/for-professionals/faq/does-hipaa-require-covered-entities-to-keep-medical-records-for-any-period/index.html), [HHS Security Rule summary](https://www.hhs.gov/hipaa/for-professionals/security/laws-regulations/index.html).

**Medicare documentation retention is longer than the CLIA minimums.** Providers and suppliers enrolled in Medicare must keep written and electronic documentation about orders, certifications, referrals, prescriptions and requests for payment for 7 years from the date of service. They must also give CMS or its contractors access on request. [42 CFR 424.516(f)](https://www.ecfr.gov/current/title-42/section-424.516). For a laboratory that bills Medicare, the order and authorization record class therefore has a 7-year federal payment trigger as well as the 2-year CLIA minimum above. Other payers, states and contracts can require longer.

**Design recommendation:** store retention category, policy version, triggering date/event, minimum retention date, hold state, and reviewed destruction authority. Separate physical specimen disposition from digital-record deletion. Define how backups age out and how restored data re-obeys holds and previously approved disposal. Preserve patient access and export capability when an external service ends or a deployment closes.

## 4. HIPAA, cloud hosting, and patient rights

HIPAA applicability is based on covered-entity/business-associate status and activity, not the software license. Determine whether the laboratory is a covered entity, which service provider is its business associate, and which subcontractors handle ePHI. Administrative risk analysis, workforce controls, contingency planning, incident handling, and periodic evaluation are organizational duties; application controls alone cannot fulfill them. [HHS Security Rule summary](https://www.hhs.gov/hipaa/for-professionals/security/laws-regulations/index.html).

Technical safeguards include access control, unique identities, emergency access, audit mechanisms, authentication, integrity and transmission protections. Current rules distinguish required and addressable implementation specifications; “addressable” requires a reasoned implementation decision, not omission by default. **Project security recommendation:** require encryption in transit and at rest, MFA for privileged and workforce remote access, least privilege, per-laboratory data separation, supported backups, and auditable emergency access regardless of that distinction. [45 CFR 164.312](https://www.ecfr.gov/current/title-45/subtitle-A/subchapter-C/part-164/subpart-C/section-164.312).

A cloud service maintaining encrypted ePHI can still be a business associate even when it cannot decrypt the data. A BAA and a documented responsibility allocation are required when that relationship applies. Include database, backups, file storage, support, monitoring, messaging, and subprocessors in the data-flow review. “HIPAA hosting” advertising does not approve an architecture. [HHS cloud computing guidance](https://www.hhs.gov/hipaa/for-professionals/special-topics/health-information-technology/cloud-computing/index.html).

**Design recommendation:** a synthetic-data demo or signed installer is independent of approval for any production PHI deployment. Determine production vendor services, eligible regions, contract scope, subprocessors, administrative access, recovery, and export before live data is introduced. Open-source contributors should not receive clinical system access merely because they maintain the code.

HIPAA patient access extends to the designated record set, not just a PDF report. Generally, requests must be acted on within 30 calendar days, with one permitted extension subject to the rule's conditions. Identity verification must not create unreasonable barriers. Separately, laboratories are information-blocking actors (§11), and blanket report embargoes for clinician review can be problematic. A HIPAA "timely access" proposed rule (RIN 0945-AA28) is scheduled for 11/2026, and the 2021 Privacy Rule final rule (RIN 0945-AA00) has been pending OIRA review since 2026-04-02. Neither has been published. HIPAA's outer response deadline does not authorize routine electronic delays. [45 CFR 164.524](https://www.ecfr.gov/current/title-45/subtitle-A/subchapter-C/part-164/subpart-E/section-164.524), [HHS access guidance](https://www.hhs.gov/hipaa/for-professionals/faq/right-to-access-and-research/index.html), [ONC delay FAQ](https://healthit.gov/faq/when-would-delay-fulfilling-request-access-exchange-or-use-ehi-be-considered-interference-under/).

**Design recommendation:** document authorization by recipient, purpose, patient relationship, applicable consent and legal basis; treatment disclosures and patient access have exceptions to some HIPAA minimum-necessary limitations. Keep patient request/amendment workflows separate from result correction. Record legal holds or disclosure denials with the human decision-maker and authority; do not impose a default universal clinician-review embargo.

## 5. 42 CFR Part 2 needs provenance and a separate applicability decision

Part 2 applicability concerns qualifying federally assisted SUD programs and covered records, including records received by other holders under applicable conditions. A toxicology result is not automatically a Part 2 record merely because it measures a controlled substance; an LIS vendor should not independently infer SUD-program status from a positive result. Qualified-service-organization arrangements and data received from a Part 2 program require legal review. [42 CFR 2.12](https://www.ecfr.gov/current/title-42/chapter-I/subchapter-A/part-2/subpart-B/section-2.12).

The 2024 final rule became effective April 16, 2024; compliance was required February 16, 2026. It permits a single future treatment/payment/operations consent and specified HIPAA redisclosures, while retaining stricter limits on proceedings against patients. Physical segregation is expressly not required. The new accounting-right compliance date depends on later HIPAA rulemaking. The 2024 HIPAA reproductive-health privacy rule was largely vacated on 2025-06-18 (*Purl v. HHS*, N.D. Tex.); HHS states that its Notice of Privacy Practices changes for Part 2 survived, with compliance required by 2026-02-16 (search-snippet evidence; the HHS page could not be opened directly). The eCFR still prints vacated text, so counsel should confirm the NPP content against HHS's notice rather than the eCFR. Laboratory NPP and disclosure templates are organizational documents, not LIS configuration. [HHS Part 2 overview](https://www.hhs.gov/hipaa/part-2/index.html), [HHS final-rule fact sheet, updated January 30, 2026](https://www.hhs.gov/hipaa/for-professionals/regulatory-initiatives/fact-sheet-42-cfr-part-2-final-rule/index.html).

Each disclosure made with written patient consent must include the required written notice **and** a copy of consent or a clear explanation of its scope. A generic confidentiality footer alone does not cover both obligations. **Design recommendation:** an enabled report, export or billing route must preserve the approved notice and consent-scope information or route the disclosure to an authorized alternative process. [42 CFR 2.32](https://www.ecfr.gov/current/title-42/chapter-I/subchapter-A/part-2/subpart-C/section-2.32).

**Design recommendation:** keep source organization, program determination, received notices, disclosure authority, consent versions and revocations, purpose, recipient, and downstream notices attached to relevant records. Logical labels support these decisions; they are a proposed engineering mechanism, not a claim that the rule mandates a separate database. An undifferentiated “HIPAA consent” checkbox is insufficient. Counsel must decide the exact treatment of each laboratory/customer relationship, exceptions, revocation, redisclosure, and legal demand.

## 6. Incidents, notification, and evidence

Security incident response and legally reportable breach notification are different workflows. Under HIPAA, an impermissible disclosure is generally presumed a breach unless an exception or documented low-probability assessment applies. Individual notice and business-associate notice generally must occur without unreasonable delay and no later than 60 days after discovery. HHS notice uses a 500-person threshold; media notice uses more than 500 residents of a state/jurisdiction. Smaller breaches have an annual HHS reporting route. Contracts and state laws can impose shorter deadlines. [HHS Breach Notification Rule](https://www.hhs.gov/hipaa/for-professionals/breach-notification/index.html).

Part 2's 2024 rule applies HIPAA-style breach notification to covered Part 2 records. [HHS Part 2 fact sheet](https://www.hhs.gov/hipaa/for-professionals/regulatory-initiatives/fact-sheet-42-cfr-part-2-final-rule/index.html).

**Design recommendation:** track discovery time, affected record classes and people, actor access, containment, preserved evidence, legal assessments, notices and delivery. Use an immediate internal escalation target; do not design “60 days to investigate before telling anyone.” The privacy officer/counsel chooses notices and recipients. No automatic notification to authorities, employers, or patients from an unreviewed security alert. Federal critical-infrastructure, state breach, forensic contract, and specialized incident duties remain applicability research before deployment. The CIRCIA final rule is unpublished (§13).

## 7. Workplace and forensic extensions are separate qualification work

Workplace, forensic and postmortem testing are outside scope ([scope](../project-scope.md#confirmed-initial-scope)); enabling any of them needs separate qualification, not a variant of clinical reports. DOT laboratory participation requires HHS certification or the applicable foreign-laboratory route; an instrumented initial testing facility is not automatically eligible, and approved electronic custody/control arrangements are distinct from ordinary electronic forms ([DOT laboratory participation](https://www.transportation.gov/odapc/labs)). DOT oral-fluid testing requires at least two HHS-certified laboratories ([DOT 40.67](https://www.transportation.gov/odapc/part40/40-67)); the SAMHSA lists of August 2026 and 2026-09-30 name none certified for oral fluid ([SAMHSA August list](https://www.samhsa.gov/sites/default/files/dfw-certified-lab-listing-state-0832026.pdf), [SAMHSA notice, 2026-09-30](https://www.govinfo.gov/content/pkg/FR-2026-09-30/html/2026-20004.htm)). Federal panels change independently of clinical menus: HHS added fentanyl effective 2025-07-07, and matching DOT changes were only proposed ([HHS panels notice](https://www.govinfo.gov/content/pkg/FR-2025-01-16/html/2025-00425.htm), [DOT NPRM](https://www.federalregister.gov/documents/2025/09/02/2025-16720)).

**Design recommendation:** if an extension is ever scoped, model donor identity, seals and transfers, split specimens, MRO review, cancellations, refusals and employer disclosure as separate states (REG-020/021). A laboratory analytical finding must not become an employment decision automatically.

## 8. State and accreditation overlays

New York's Clinical Laboratory Evaluation Program provides a separate permit, survey, testing-category and standards framework, including qualifying out-of-state laboratories accepting New York specimens. California's out-of-state license requirements can apply to diagnostic testing of California-origin specimens. Laboratory location alone therefore cannot identify every applicable state obligation. These are examples, not a fifty-state survey. [New York CLEP clinical laboratories](https://www.wadsworth.org/regulatory/clep/clinical-labs), [California out-of-state licensing](https://www.cdph.ca.gov/Programs/OSPHLD/LFS/Pages/New-OOS-ClinicalLaboratoryLicense.aspx).

Further specimen-origin licensure examples follow. Each needs counsel confirmation for the actual menu and client footprint.

| State | Trigger found in official source | Confidence |
| --- | --- | --- |
| New York | Only NYS-permitted laboratories may test specimens originating from New York; CLEP also has its own test-approval process | Official page opened |
| California | Out-of-state license (moderate/high complexity) or registration for California-origin specimens | Existing source; CDPH page unreachable at the recorded check date |
| Pennsylvania | Act 122-2013 extended the permit requirement to out-of-state laboratories testing specimens collected in Pennsylvania; drugs-of-abuse, blood-lead and alcohol testing have added state approval/PT conditions | Department letter opened; PT conditions from snippets |
| Maryland | License needed to perform tests on specimens acquired from Maryland health care providers; 2026 application has an out-of-state checklist | Statute and application opened |
| Rhode Island | Unlawful to perform services on specimens collected in the state without a license | Statute opened |
| Nevada | Division *may* require out-of-state laboratories to be licensed | Statute opened; discretionary |
| Washington | Medical test site license required in-state; out-of-state applicability not established | Rule opened; unresolved |
| Florida | Clinical laboratory licensure repealed effective 2018-07-01 | Bill opened |

CMS lists only New York and Washington as CLIA-exempt states; New York physician-office laboratories still need CLIA certificates. [CMS Admin Info 26-07](https://www.cms.gov/files/document/admin-info-26-07-clia-2027-original-release-07-01-2026.pdf). Other states were not checked.

State law can also limit direct patient ordering. For example, New York's direct-access testing statute restricts qualifying tests and requires results to go only to the requester ([PHL §576-b](https://www.nysenate.gov/legislation/laws/PBH/576-B)). Some states have reportable results that toxicology menus can trigger. Blood lead is nationally notifiable. Pennsylvania requires reporting of all blood-lead results for people under 16 ([28 Pa. Code §27.34](https://www.pacodeandbulletin.gov/Display/pacode?file=/secure/pacode/data/028/chapter27/s27.34.html)). Washington requires all blood-lead levels, with different deadlines ([WAC 246-101-201](https://app.leg.wa.gov/WAC/default.aspx?cite=246-101-201)). New York requires laboratory blood-lead reporting ([PHL §1370-e](https://www.nysenate.gov/legislation/laws/PBH/1370-E)). The federal certification reference for electronic laboratory reporting is HL7 v2.5.1 ELR Release 1 ([45 CFR 170.205(g)](https://www.ecfr.gov/current/title-45/section-170.205)). The recorded sources establish **no** verified state rule making fentanyl, xylazine or nitazene laboratory results reportable; that question remains open (Maryland's March 2026 list has no drug entries). **Design consequence:** if the menu includes blood lead or other reportable analytes, the deployment needs jurisdiction-aware reportable-result rules, keyed to patient residence and ordering-provider location as the jurisdiction requires. It also needs an ELR route or a controlled manual reporting process with reconciliation. A drug name alone must not trigger public-health reporting.

**Accreditation and standards status.** These are public metadata only; licensed text was not reviewed.

- **ISO 15189.** The ILAC 3-year transition from the 2012 edition to ISO 15189:2022 ended in December 2025, and accreditation to the 2012 edition is no longer recognized under the ILAC arrangement. [ILAC notice](https://ilac.org/latest_ilac_news/iso-151892022-for-medical-labs-published/).
- **CAP.** CAP is a CMS-approved accreditor with deeming authority. It updates its checklists annually, and the 2025 edition was released in December 2025. Public sources show computer-services requirements in the Laboratory General checklist and toxicology in the Chemistry and Toxicology checklist, with forensic drug testing as a separate program. [CAP 2025 checklist release](https://newsroom.cap.org/latest-news/cap-puts-patient-safety-and-regulatory-clarity-at-the-forefront-with-new-laboratory-checklist/s/1a4cb651-9a31-42dd-adf6-eb97c10ab856).
- **COLA.** CMS approval runs to 2030-03-06 and includes toxicology within chemistry. [CMS notice](https://public-inspection.federalregister.gov/2024-04674.pdf).
- **CLSI.** Current documents relevant to LIS design include:
  - AUTO15 autoverification (2019, reaffirmed 2025); AUTO10 is archived.
  - C62 LC-MS methods, 3rd edition (2026-06-30).
  - C52 toxicology and drug testing, 3rd edition (2017, reaffirmed 2021).
  - AUTO11 IT security of IVD systems, 3rd edition (2024).
  - AUTO16 instrument interface, 2nd edition (2026-06-03).
  - AUTO17 semantic interoperability (2023).

  LIS01/LIS02 are archived. [CLSI AUTO15](https://clsi.org/shop/standards/auto15/), [CLSI C62](https://clsi.org/shop/standards/c62/).

The project needs licensed copies before claiming coverage of any of these. CAP checklists are customized by laboratory testing menu, and ISO 15189:2022 concerns medical-laboratory quality and competence. Do not invent checklist clause mappings or reproduce protected standards in this open repository. Obtain authorized current materials and review applicable clauses with the laboratory quality lead. Accreditation can be commercially or contractually necessary and can interact with regulatory pathways; it is not universally mandatory merely because this is an LIS. [CAP checklists](https://www.cap.org/laboratory-quality-solutions/accreditation/accreditation-checklists/), [ISO 15189:2022](https://www.iso.org/standard/76677.html).

## 9. FDA, electronic signatures, and clinical functionality

FDA reports that its May 6, 2024 LDT rule was vacated March 31, 2025 and the regulatory text reverted September 19, 2025. The old staged LDT phaseout timetable must not appear as an active project requirement. This does not settle the regulatory status of every instrument, IVD, software function, or future claim. [FDA LDT status](https://www.fda.gov/medical-devices/in-vitro-diagnostics/laboratory-developed-tests). The reverting final rule is 90 FR 45134, published and effective 2025-09-19 ([FR](https://www.govinfo.gov/content/pkg/FR-2025-09-19/html/2025-18239.htm)). H.R. 8890, the "Enhancing CLIA Act of 2026", was introduced 2026-05-19 and would place LDTs under CLIA. It is a bill, not law ([text](https://www.congress.gov/119/bills/hr8890/BILLS-119hr8890ih.pdf)).

### Software functions: where an LIS can cross into device or CDS territory
Section 520(o)(1) of the FD&C Act excludes several software functions from the device definition:

- administrative support including "laboratory workflow" (paragraph (A));
- functions for "transferring, storing, converting formats, or displaying clinical laboratory test or other device data and results", but only "unless such function is intended to interpret or analyze" those data (paragraph (D));
- clinical decision support that meets all four criteria in paragraph (E).

The electronic-patient-record exclusion (C) depends on ONC-certified health IT, so it is unlikely to cover an LIS. [21 U.S.C. 360j(o)](https://www.govinfo.gov/content/pkg/USCODE-2024-title21/html/USCODE-2024-title21-chap9-subchapV-partA-sec360j.htm).

FDA's Section 3060 guidance (2019, still FDA policy) says LIS/LIMS functions for administrative support, or for transferring, storing, converting or displaying laboratory data, are not devices. It also says that "some LIS and LIMS include software functions that remain device functions, including software functions that analyze medical device data in order to provide a notification or flag (e.g., that a parameter is out of range)" ([guidance](https://www.fda.gov/media/109622/download)).

FDA's current final CDS guidance was issued 2026-01-29. Points relevant to a laboratory:

- A single, discrete, clinically meaningful lab result counts as medical information.
- Software that processes an assay or instrument response to generate a clinical test result does not meet Criterion 1.
- Recommendations must be for a health care professional who can independently review their basis.
- FDA describes enforcement discretion for some functions that give a single clinically appropriate recommendation.
- Medication reconciliation and test reconciliation are listed among its examples.

[FDA CDS guidance](https://www.fda.gov/regulatory-information/search-fda-guidance-documents/clinical-decision-support-software).

**Consequence.** Inventory LIS functions by intended use before release. Storing, transferring and displaying results, workflow queues, and *laboratory-authored* auto-verification rules that release results without adding an interpretation look closest to the exclusions. That is a project reading, not an FDA determination. The following need a documented regulatory assessment before enablement:

- automated flags or alerts derived from results;
- interpretive comments such as "consistent/inconsistent with reported medication";
- metabolite-pattern interpretation;
- patient-facing explanations;
- any AI feature.

Keep such rules authored, versioned and approved by the laboratory. Show their basis to the reviewing professional, and do not ship them as vendor-supplied clinical logic. FDA's AI-enabled device software functions guidance is still a draft (January 2025).

FDA's "Computer Software Assurance for Production and Quality Management System Software" guidance, final 2026-02-03, covers device manufacturers' production and quality-system software. It does not bind a clinical laboratory's LIS. It is a useful risk-based assurance model for the [validation plan](../quality/validation-plan.md), as a project choice. [FDA CSA guidance](https://www.fda.gov/media/188844/download).

### Electronic records and signatures

21 CFR Part 11 applicability is conditional on FDA-required electronic records/submissions and associated signatures under applicable predicate rules. It is not automatically imposed on every clinical LIS simply because it has electronic signatures. Maintain a documented determination by function, record and intended use; validation and trustworthy records remain sensible design objectives even when Part 11 is out of scope. [FDA Part 11 scope guidance, August 2003](https://www.fda.gov/regulatory-information/search-fda-guidance-documents/part-11-electronic-records-electronic-signatures-scope-and-application).

**Design recommendation:** authenticate human approvals and bind their meaning, timestamp and identity to specific record versions. Assess any future diagnostic interpretation, dosing advice, AI interpretation, instrument control, or clinical-decision function before expanding claims. Agent-assisted development does not authorize AI clinical sign-out, changing validated cutoffs, or release of patient results.

## 10. Billing, payment integrity and other unresolved boundaries

Medicare diagnostic-test ordering conditions include treating-practitioner and documentation requirements, with specified exceptions; nonphysician practitioner authority also depends on state scope and Medicare benefit rules. This payment framework does not define every laboratory's clinical ordering authority. [42 CFR 410.32(a), (d)](https://www.ecfr.gov/current/title-42/chapter-IV/subchapter-B/part-410/subpart-B/section-410.32).

The reviewed Novitas LCD L35006, revision effective October 17, 2019, illustrates why payer, date, setting and order source matter. It distinguishes certain reference-laboratory reflex pathways from follow-up after point-of-care testing, allows specified direct-to-definitive pathways and lists specimen-validity testing among noncovered services. Its coverage limits do not waive required analytical or validity controls, and its exceptions are not a national authorization rule. [CMS/Novitas LCD L35006](https://www.cms.gov/medicare-coverage-database/view/lcd.aspx?lcdid=35006&ver=119). L35006 is one of seven active MAC drug-testing LCDs found at the recorded check date (table below; not a verified complete census). Its companion billing article is A56645.

### Medicare order and claim documentation (requirement when billing Medicare)

A test not ordered by the treating physician or practitioner is not reasonable and necessary (410.32(a)). The ordering practitioner keeps the medical-necessity documentation. The entity submitting the claim must keep:

- the documentation it received from the orderer;
- evidence that the claim accurately reflects that information.

On request it must produce:

- the order, with enough information to identify and contact the orderer;
- evidence of accurate order processing and claim submission;
- the diagnostic information the orderer supplied (410.32(d)(2)–(3)).

Laboratory claims also need an ordering practitioner who is identified by NPI and enrolled in, or validly opted out of, Medicare ([42 CFR 424.507](https://www.ecfr.gov/current/title-42/section-424.507); not re-read at the recorded check date). CMS guidance [MLN909221](https://www.cms.gov/files/document/mln909221-complying-documentation-requirements-lab-services.pdf) (May 2026) adds the following:

- A signature on clinical lab fee schedule test orders is not required, but intent to order must be documented in the medical record.
- An unsigned requisition alone does not support intent, and CMS does not accept attestations for unsigned orders or requisitions.
- If a standing order or protocol is used, a copy must accompany the documentation.

### MAC drug-testing coverage policies (payer conditions; vary by MAC, jurisdiction and date of service)

| LCD / billing article | MAC and jurisdiction | Current revision found |
| --- | --- | --- |
| [L35724](https://www.cms.gov/medicare-coverage-database/view/lcd.aspx?lcdid=35724) / A54799 "Urine Drug Testing" | Palmetto GBA, JJ/JM | LCD rev. effective 2025-02-20; article 2026-06-01 |
| [L36029](https://www.cms.gov/medicare-coverage-database/view/lcd.aspx?lcdid=36029&ver=44) / [A56818](https://www.cms.gov/medicare-coverage-database/view/article.aspx?articleid=56818&ver=55) | CGS, J15 | LCD 2026-03-05 (annual review); article 2026-04-16 |
| [L34645](https://www.cms.gov/medicare-coverage-database/view/lcd.aspx?lcdid=34645&ver=51) / A56915 | WPS, J5/J8 | LCD 2025-08-28; article 2026-06-01 |
| [L36668](https://www.cms.gov/medicare-coverage-database/view/lcd.aspx?lcdid=36668&ver=45) / A55001 | Noridian, JE and JF (L36668 revision 13 consolidated JE and JF. Retirement of L36707 on 2025-08-14 is unconfirmed from the fetched text) | LCD 2025-08-14; article 2026-06-01 (not opened) |
| [L39611](https://www.cms.gov/medicare-coverage-database/view/lcd.aspx?lcdid=39611&ver=5) / A59416 | Wellpoint Federal (formerly NGS), J6/JK | LCD 2026-04-01 (name change) |
| L35006 / [A56645](https://www.cms.gov/medicare-coverage-database/view/article.aspx?articleid=56645&ver=48) "Controlled Substance Monitoring and Drugs of Abuse Testing" | Novitas, JH/JL | LCD 2019-10-17; article 2024-10-01 |
| [L36393](https://www.cms.gov/medicare-coverage-database/view/lcd.aspx?lcdid=36393) / A57077 | First Coast, JN | LCD 2019-10-01 |

What follows is read from L35724, with the order wording from CGS A56818. Recorded source summaries report equivalent harmonized text in the CGS, WPS, Noridian and Wellpoint LCDs; the comparison was not independently verified.

- Presumptive (screening) and definitive (identification) testing are distinct services.
- Routine standing orders for all patients in a practice, blanket orders, and the same physician-defined profile used for every patient are not reasonable and necessary.
- Definitive orders should be individualized. The clinician's rationale for the tests and drug classes ordered must be in the medical record.
- Reflex to definitive testing is not covered when the presumptive test was performed at point of care.
- At most one presumptive service is allowed per patient per date of service.
- Specimen validity testing is quality assurance and not separately payable.
- Frequency limits are tied to a documented risk category (chronic opioid therapy) or abstinence stage (substance-use-disorder treatment).
- The CGS and WPS articles say "All tests must be ordered in writing by the treating provider and all drugs/drug classes to be tested must be indicated in the order". They also allow one definitive code per patient per day.

Definitive codes G0480–G0483 are tiered by the number of drug classes tested, including metabolites (1–7, 8–14, 15–21, 22+). G0659 is a further definitive code whose descriptor was not reviewed. Presumptive CPT 80305–80307 are tiered by method complexity. [OIG A-09-21-03006](https://oig.hhs.gov/oas/reports/region9/92103006.pdf). Do not copy AMA CPT descriptors into the repository. Class-counting rules come from the code descriptors and MAC articles, and the billing owner must qualify them.

### Enforcement pattern (illustrative evidence, not findings about any customer)

- **OIG audits.** Medicare paid $704.2 million over five years in at-risk definitive-testing payments at the 22+ class tier, where laboratories "may not have always used presumptive testing to determine the number of drug classes that needed to be tested" (February 2023, [A-09-21-03006](https://oig.hhs.gov/oas/reports/region9/92103006.pdf)). Specimen-validity tests were improperly billed alongside drug tests (2018, [A-09-16-02034](https://oig.hhs.gov/oas/reports/region9/91602034.pdf)).
- **DOJ resolutions.** A $14.5 million settlement (2026-07-15) concerned routine billing of both presumptive 80307 and definitive G0483 on the same sample for a comprehensive panel; the laboratory admitted stated facts for 2018–2023, with no determination of liability for the resolved claims ([DOJ](https://www.justice.gov/opa/pr/labcorp-agrees-pay-145m-resolve-false-claims-act-allegations)). A guilty plea (2026-01-07) involved kickbacks including free services and supplies to a referring practice ([DOJ](https://www.justice.gov/opa/pr/south-carolina-laboratory-pleads-guilty-and-agrees-pay-least-68m-settle-allegations)), and a conviction (2025-02-25) involved percentage-paid marketers and tests doctors did not authorize ([DOJ](https://www.justice.gov/opa/pr/lab-operator-convicted-4m-medicare-fraud-scheme)).
- **CMS Fraud Defense Operations Center.** Reported more than $39 million in payment suspensions to suspect laboratories in January–June 2026 ([fact sheet](https://www.cms.gov/files/document/cpi-fdoc-fact-sheet-updated-first-year.pdf)).

### Fraud-and-abuse laws (requirement; applicability and arrangements are counsel decisions)

- **Anti-Kickback Statute and Stark.** These govern remuneration to and financial relationships with referral sources for federal program business. The EHR-donation safe harbor and exception exclude laboratory companies as donors, so free EHR, portal, interface, supply, collector or phlebotomy arrangements need counsel review. ([42 CFR 1001.952(y)](https://www.ecfr.gov/current/title-42/section-1001.952), [411.357(w)](https://www.ecfr.gov/current/title-42/section-411.357)) OIG's 2014 Special Fraud Alert addresses laboratory payments to referring physicians, such as specimen collection and registry fees ([SFA](https://oig.hhs.gov/documents/special-fraud-alerts/866/OIG_SFA_Laboratory_Payments_06252014.pdf)). The donor exclusion does not end the analysis. OIG and the Stark definition of remuneration both treat items used solely to order from, or receive results of, the furnishing laboratory as having no independent value. §15.5 applies this to the proposed lab-specific client portal.
- **EKRA, 18 U.S.C. §220.** Criminalizes kickbacks for referrals to laboratories, recovery homes and clinical treatment facilities for **any** health care benefit program, including private insurance. Its employee/contractor exception fails where pay varies with referrals, tests or billing ([text](https://www.law.cornell.edu/uscode/text/18/220)). *United States v. Schena* (9th Cir. 2025; certiorari denied 2026-03-23) upheld an EKRA conviction involving percentage-paid marketers ([docket](https://www.supremecourt.gov/search.aspx?filename=/docket/docketfiles/html/public/25-447.html)). The opinion itself was not reviewed. EKRA is directly relevant to labs serving SUD treatment programs and recovery residences.
- **OIG compliance program guidance.** The General Compliance Program Guidance was issued 2023-11-06. No industry-specific guidance for clinical laboratories has been issued (the existing ones cover nursing facilities and Medicare Advantage). OIG lists its 1998 clinical-laboratory guidance in its CPG archive. [OIG compliance guidance](https://oig.hhs.gov/compliance/compliance-guidance/).

### PAMA and No Surprises Act (conditional)

- **PAMA.** CAA 2026 §6226 (2026-02-03) set the most recent private-payer reporting period at 2026-05-01 to 2026-07-31, using data from 2025-01-01 to 2025-06-30. There is no CLFS reduction in 2026; reductions are capped at 15% a year for 2027–2029; a 3-year reporting cycle follows. [CMS CLFS](https://www.cms.gov/medicare/payment/fee-schedules/clinical-laboratory-fee-schedule-clfs). An "applicable laboratory" is determined at NPI level and must meet the majority-of-Medicare-revenue and $12,500 CLFS thresholds ([42 CFR 414.502](https://www.ecfr.gov/current/title-42/section-414.502); its eCFR dates predate CAA 2026). Reporting belongs to the billing system and owner. SALSA's status was not verified.
- **No Surprises Act.** Good-faith-estimate duties for uninsured/self-pay individuals ([45 CFR 149.610](https://www.ecfr.gov/current/title-45/section-149.610)) can reach a laboratory that schedules or is asked about a service directly. HHS's enforcement discretion for co-provider and co-facility estimates (FAQ, 2022-12-02) remains pending future rulemaking, and no superseding rule was found ([FAQ](https://www.cms.gov/files/document/good-faith-estimate-uninsured-self-pay-part-3.pdf)). Laboratory services are "ancillary services" that cannot be balance-billed by notice-and-consent when provided at in-network facilities ([45 CFR 149.420](https://www.ecfr.gov/current/title-45/section-149.420)). Applicability to office-referred outpatient testing needs counsel review.

### Design consequences (project recommendations, not legal advice)

**Design recommendation:** preserve clinical rationale, original order, performed services and amendments; qualify payer/MAC/date rules before billing configuration. The pilot hands off services for an accountable billing decision. It must not automatically bill every measurement/repeat or drop required QC/validity work because separate payment is unavailable. No code tables or universal coverage logic are adopted here. The resulting controls (per-order-version evidence retained under the 7-year Medicare trigger in §3, audited client profiles, point-of-care versus laboratory reflex provenance, versioned drug-class accounting with billing-review flags, frequency and validity visibility, inducement visibility and a claim-review package) are REG-029 to REG-033 in the [compliance matrix](../requirements/compliance-matrix.md). Payer rules never create clinical authority, payer frequency limits never block clinically ordered testing, and the absence of payment never removes required QC or validity work.

Before a production scope is accepted, assign owners for:

- state-by-state licensure, retention and consent;
- minor and representative access;
- payer/MAC policy;
- fraud-and-abuse review;
- OSHA and specimen-shipping safety;
- controlled-reference-material handling;
- public-health reporting;
- FDA software claims;
- forensic evidence requirements.

Payer/MAC policy, fraud-and-abuse, public-health reporting and FDA software have an initial evidence baseline (§8–§10), not exhaustive research; the other regimes are identified only. Do not infer that a DEA drug schedule creates an automatic LIS reporting obligation or that ordinary clinical specimens make the LIS an electronic prescribing system.

## 11. Information blocking and timely electronic access
**Laboratories are information-blocking actors.** 45 CFR 171.102 adopts the 42 U.S.C. 300jj definition of health care provider, and that definition lists "a laboratory". Electronic health information means ePHI that would be in a designated record set; psychotherapy notes and litigation material are excluded. A provider commits information blocking only if it *knows* a practice is unreasonable and likely to interfere with access, exchange or use of EHI. The current exceptions are:

- preventing harm;
- privacy;
- security;
- infeasibility;
- health IT performance;
- protecting care access;
- manner;
- fees;
- licensing;
- TEFCA manner.

[45 CFR 171.102](https://www.ecfr.gov/current/title-45/section-171.102), [42 U.S.C. 300jj](https://www.govinfo.gov/content/pkg/USCODE-2023-title42/html/USCODE-2023-title42-chap6A-subchapXXVIII-sec300jj.htm).

**Disincentives and enforcement.** CMS disincentives exist only for hospitals and critical access hospitals, MIPS eligible clinicians, and Shared Savings Program ACOs and participants; the provision for other providers is reserved. An independent laboratory therefore has no laboratory-specific disincentive today, but it remains an actor that OIG can investigate and refer, and future rulemaking could add one. [45 CFR 171.1001](https://www.ecfr.gov/current/title-45/section-171.1001). HHS announced stepped-up enforcement on 2025-09-03, and OIG and ASTP issued an enforcement alert on 2025-09-04 ([HHS announcement](https://healthit.gov/news/hhs-announces-crackdown-on-health-data-blocking)).

**Rulemaking status.**

- **HTI-5** was proposed 2025-12-29 and has been pending OIRA review as a final rule since 2026-07-28; it was unpublished as of 2026-10-12. It would, among other changes:
  - remove the TEFCA manner exception;
  - narrow an infeasibility condition;
  - add contract-term conditions to the manner exception;
  - include automated and AI access in "access" and "use".
- The unadopted remainder of HTI-2 was withdrawn on 2025-12-29.
- An "HTI-6" API and information-blocking proposal is on the agenda for 11/2026.

Recheck all of these before acceptance. [HTI-5 NPRM](https://www.federalregister.gov/documents/2025/12/29/2025-23896).

**Result timing guidance.** ASTP's FAQs say there is no fixed release deadline, but actors must respond to requests in a timely way. Blanket multi-day delays across routine results do not qualify for the preventing-harm exception, although an individualized determination can. Delaying release because the patient asked is likely not interference. ([FAQ: blanket delays](https://healthit.gov/faq/would-preventing-harm-exception-cover-blanket-several-day-delay-release-laboratory-or-other), [FAQ: patient-requested delay](https://healthit.gov/faq/can-actor-grant-patients-request-delay-release-patients-test-results-eg-laboratory-or-image/))

**Design consequences.**

- Release paths must not impose default blanket embargoes.
- Any delay must be one of the following:
  - an individualized, attributable determination with reason and expiry;
  - a recorded patient preference;
  - a documented exception decision.
- Requests to access or export EHI (reports, results and other designated-record-set content, including billing records where applicable) need a tracked electronic export route in a standard or agreed format.
- Denials or alternative-manner responses need recorded exception rationale.
- Client and interface contracts and fees should be reviewed for interference. Charging or licensing terms for result access belong to counsel.
- The LIS is not ONC-certified health IT. Do not claim certification, and treat "EHI export" here as an access capability, not a certification criterion.

## 12. Accessibility of patient and client digital services
**Section 504 (conditional requirement for recipients of HHS financial assistance).** 45 CFR 84.84 requires web content and mobile apps of recipients to meet WCAG 2.1 Level AA, including content provided through contractual or licensing arrangements. An HHS interim final rule effective 2026-05-07 moved the compliance dates:

- recipients with 15 or more employees: from 2026-05-11 to **2027-05-11**;
- smaller recipients: from 2027-05-10 to **2028-05-10**.

Existing reasonable-modification duties continue meanwhile. OIRA's review list shows a Section 504 final rule (RIN 0945-AA27) received 2026-09-30 and still pending. These dates may change when it publishes (SR-131). [HHS IFR, 91 FR 25496](https://www.govinfo.gov/content/pkg/FR-2026-05-11/html/2026-09266.htm), [45 CFR 84.84](https://www.ecfr.gov/current/title-45/section-84.84). Medicaid funding is federal financial assistance. HHS's 2024 Section 1557 rule states that "Part B payments are considered Federal financial assistance with respect to title VI, title IX, section 504, and the Age Discrimination Act, in addition to section 1557". No reversal of that position was established in the recorded evidence; the review was not exhaustive. [Section 1557 final rule](https://www.govinfo.gov/content/pkg/FR-2024-05-06/html/2024-08711.htm).

**Section 1557.** 45 CFR 92.204 requires accessible information and communication technology. A 2025 court vacated only the gender-identity provisions, and HHS's 2026-06-02 notice says the other provisions "remain in force" ([HHS notice](https://www.govinfo.gov/content/pkg/FR-2026-06-02/html/2026-11015.htm)). 45 CFR 92.210 also requires covered entities to identify and mitigate discrimination risk from patient care decision support tools. That bears on any future interpretive algorithm or AI feature.

**Design consequences.**

- Patient and client portals, report PDFs, result emails and mobile views need conformance evidence at least to WCAG 2.1 AA. The proposed [NFR-09](../requirements/nonfunctional-requirements.md) WCAG 2.2 AA target is stricter and remains appropriate.
- Accessible report rendering, including tagged PDFs or an accessible HTML equivalent, needs an explicit requirement and acceptance.
- Whether a deployment is a "recipient" is decided by counsel. *Litigation note:* a 2026 consent judgment in *Texas v. Kennedy* reportedly vacated only the 504 rule's integration-mandate provisions. This was seen in secondary sources only and is unverified.

## 13. Cybersecurity baseline and incident-reporting watch
- **HHS cybersecurity performance goals.** HHS's healthcare and public-health goals are voluntary: 10 essential and 10 enhanced practices. They are a reasonable project baseline for MFA, asset inventory, vulnerability management, vendor risk, logging and incident planning, and the security design can be mapped to them. [HHS CPGs](https://hhscyber.hhs.gov/cybersecurity-performance-goals.html).
- **HIPAA Security Rule proposal.** It remains proposed. The Unified Agenda lists it as a long-term action with final action 07/2027 ([RIN 0945-AA22](https://www.reginfo.gov/public/do/eAgendaViewRule?pubId=202510&RIN=0945-AA22)).
- **CIRCIA.** CISA has not published the final rule. The 2024 proposal would capture entities above SBA size standards across sectors, and CISA said laboratory operators would be "captured under the size-based criterion". Covered incident and ransom-payment reporting could therefore reach a larger laboratory once a final rule is effective. [CISA CIRCIA](https://www.cisa.gov/circia), [NPRM](https://www.govinfo.gov/content/pkg/FR-2024-04-04/html/2024-06526.htm).
- **CLIA RFI.** A July 2026 CMS CLIA request for information asked about cybersecurity among other topics ([RFI](https://www.federalregister.gov/documents/2026/07/16/2026-14358)). It is not a requirement.
- **Connected instruments.** FDA's section 524B cybersecurity duties apply to manufacturers of instruments the LIS connects to, not to the laboratory.

**Design consequence.** Incident records (§6) should capture what a future CIRCIA report would need without asserting the duty exists today. Before production, security architecture should show its mapping to the HHS goals and to current HIPAA safeguards.

## 14. Active watchlist and decision gates

| Issue | Verified status as of this research | Required next step |
| --- | --- | --- |
| HIPAA Security Rule modernization | Proposed (NPRM 2025-01-06); Unified Agenda lists final action 07/2027 (§15.3). | Recheck final rule and effective/compliance dates before production architecture approval; do not cite proposal details as current mandates. |
| HIPAA access timing and Privacy Rule | 2021 Privacy Rule final (RIN 0945-AA00) at OIRA since 2026-04-02; "timely access" NPRM (RIN 0945-AA28) scheduled 11/2026; 30-day period with one extension still applies. | Recheck before configuring patient-access deadlines. |
| CLIA virtual access/personnel/reporting | CMS-3478-P published 2026-10-08; proposed, comments due 2026-12-07. It would also require reports to show the CLIA number and the laboratory name as on the certificate, and records of virtual access. | Review remote-review workflows against current rules and QSO-23-15 (2025-09-23); track the proposal without claiming it is effective. |
| Information blocking (HTI-5/HTI-6) | HTI-5 final rule pending OIRA review since 2026-07-28; HTI-6 NPRM scheduled 11/2026. | Recheck exceptions before accepting release/export design. |
| Section 504 web/mobile | Compliance dates 2027-05-11 / 2028-05-10 under an interim final rule; a final rule (RIN 0945-AA27) has been under OIRA review since 2026-09-30 and is unpublished (SR-131). | Recheck for a final rule; confirm recipient status per deployment. |
| PAMA reporting | Most recent reporting period 2026-05-01 to 2026-07-31; 3-year cycle next; future dates not published on CMS page. | Billing owner confirms applicable-laboratory status and next period. |
| CIRCIA | Final rule not published. | Recheck before production incident procedures. |
| FDA LDT/software | 2024 rule vacated; reverted 2025-09-19; H.R. 8890 introduced 2026-05-19; CDS guidance revised January 2026; product-function assessment unresolved. | Review current FDA status and intended-use claims at every material scope expansion. |
| CAP/ISO/state overlay | ISO 15189:2012 transition ended December 2025; detailed licensed and jurisdictional gap assessment incomplete. | Lab quality lead and counsel approve deployment-specific matrix. |
| Payer drug-testing policy | Seven MAC LCDs active, several articles revised 2026-04 to 2026-06; 2022 proposal to cap class counts not finalized (secondary sources). | Billing owner rechecks MAC, article version and date-of-service rules before configuration. |
| Lab-furnished client portal and interfaces | Free order/result access is defensible only while it has no independent value to the clinic; EKRA has no matching exception (§15.5). | Counsel reviews the portal feature set and clinic arrangements before any laboratory uses the service, and again whenever a clinic-facing feature is added. |
| Windows 10 on the lab PC | Support ended 2025-10-14. Commercial ESU is paid yearly, for at most three years; consumer ESU excludes commercial use (§15.3). | Inventory the lab PC's OS before onboarding; record ESU or upgrade in the risk analysis. |
| Single-PC operating-model rules | Security Rule NPRM still pending (final targeted 07/2027); Texas SB 1188 storage-location duty applies from 2026-01-01; California HSC 123148 not verified (§15.10). | Recheck before the first state is added and before production architecture approval. |

Sources: [HHS NPRM](https://www.hhs.gov/hipaa/for-professionals/security/hipaa-security-rule-nprm/index.html), [CMS-3478-P proposal](https://www.govinfo.gov/content/pkg/FR-2026-10-08/html/2026-20613.htm), plus the sources cited in §§8–13. These gates apply to future clinical deployment.

## 15. Single-PC distribution and optional hosted services

This section covers what changes because the LIS runs on one laboratory computer ([ADR 0009](../architecture/decisions/0009-single-computer-local-lis.md)) and because optional hosted services, a clinic portal and a later patient portal are proposed ([ADR 0010](../architecture/decisions/0010-optional-cloud-services.md)). Sections 1–14 also apply, with their REG IDs cross-referenced.

**Working interpretation.** The laboratory runs the LIS locally and is usually a HIPAA covered entity, often a physician-office laboratory (POL) or a small independent laboratory. A future service provider might operate separately authorized external services that hold PHI or provide on-site or remote assistance; no service-provider entity or implementation is established by this research.

Labels used throughout:

- **Requirement**: binding when the stated applicability holds.
- **Conditional requirement**: binding only when a stated condition is true.
- **Guidance**: agency interpretation or voluntary framework.
- **Proposal**: proposed, not law.
- **Unknown**: not established at the recorded check date.

Sources are SR-105 onward in the [source register](sources-regulatory.md#single-computer-regulatory-sources). HHS web pages could not be reopened for this section, so HIPAA points rest on the regulation text and the 2013 Omnibus preamble (SR-114). This is research for counsel and the laboratory, not legal advice.

### 15.1 HIPAA role of each hosted service

**Requirement (45 CFR 160.103).** A business associate "creates, receives, maintains, or transmits" PHI for a covered function. The definition expressly includes the following:

- data-transmission services that need "access on a routine basis" to PHI;
- subcontractors of a business associate.

The Omnibus preamble limits the conduit exception to "transmission services ... including any temporary storage of transmitted data incident to such transmission". It also says a data-storage company with access to PHI "qualifies as a business associate, even if the entity does not view the information" (78 FR 5572, SR-114). HHS's cloud guidance, already cited in §4 (SR-014), applies the same point to encrypted no-view storage.

| Activity | PHI handled | Likely role (project reading for counsel) | Basis and caveat |
| --- | --- | --- | --- |
| Publishing the open-source code and installers | None | Not a business associate | The publisher does not create, receive, maintain or transmit PHI. The answer changes once the publisher or a service provider touches a lab's data, including through telemetry or crash reports. |
| Encrypted cloud backup, including "zero-knowledge" (service provider never holds the key) | Persistent ciphertext of the whole LIS database | **Business associate**; requires a BAA | It is persistent storage, not transient transmission (78 FR 5572; SR-014). Lack of the key affects the breach analysis (§15.2), not the role. |
| Clinic portal (released reports, pending orders) | Plaintext PHI, routinely accessed by the service | **Business associate** | Maintains and transmits PHI for the lab's treatment disclosures. |
| Report delivery relay (secure email, fax, Direct) | Queued and stored messages, delivery logs | **Business associate**, not a conduit | Queuing, retry, logs and copies go beyond "temporary storage ... incident to" transmission. Fax and Direct (HISP) vendors behind it are **subcontractors**. |
| HL7 interface hub to clinic EHRs | Orders and results, mapped and transformed | **Business associate** (transmission with routine access) | Paragraph (3)(i) of the definition. Each clinic's EHR vendor is the clinic's business associate, not the service provider's. |
| Remote support of the lab PC | Screen and database access during sessions | **Business associate** when support can reach PHI | Treat it as a business associate. A specific HHS FAQ on incidental access was not reopened. Session logging is a 164.312(b) audit matter. |
| On-site setup, analyzer interfaces and validation | PHI visible on site; possible copies | **Business associate**, or **workforce** at the lab's election | The Omnibus preamble says an on-site contractor with "more than incidental access" may be treated as workforce or as a business associate (78 FR 5574). Validation should use synthetic data where possible. |
| Cloud host (IaaS/PaaS), email and SMS providers, remote-access relay, monitoring | Varies | **Subcontractors** where they store or can access PHI | A downstream BAA is needed (164.502(e)(1)(ii), 164.504(e)(5)). Whether a remote-access relay carrying end-to-end encrypted sessions is a mere conduit is a question for counsel. |

**Consequences (project recommendations).**

1. Use one master BAA per laboratory, with a schedule listing each service and its data, and a public subcontractor list.
2. Telemetry, crash reports and support bundles from the application must be PHI-free by default. Otherwise the software's publisher quietly becomes a business associate of every installation.
3. A laboratory that self-hosts its own backup and uses no hosted service needs no BAA with a service provider.

### 15.2 Breach and incident allocation

- **Requirement.** The business associate notifies the covered entity "without unreasonable delay and in no case later than 60 calendar days after discovery" (164.410). Discovery includes what would have been known "by exercising reasonable diligence".
- **Requirement.** The BAA must require reporting of "any security incident" (164.314(a)(2)(i)(C)).
- **Allocation.** Notifying individuals, HHS and the media stays with the laboratory unless the BAA delegates it (§6, REG-017).
- **Requirement / design.** PHI encrypted consistently with HHS guidance is not "unsecured" (164.402). Loss of zero-knowledge backup ciphertext, with the keys uncompromised, is then generally not a notifiable breach. Loss of the portal or relay plaintext stores would be.
- **Consequence.** Key custody is a breach-scope decision. Document who holds backup keys, how recovery works when the lab loses its key, and whether the service provider offers key escrow. Escrow by the service provider removes the zero-knowledge benefit.
- **Consequence.** BAAs should set an incident-notice clock much shorter than 60 days. Keep a per-lab incident record, because one service-provider incident can involve many covered entities and states.
- **Unknown.** State breach laws also bind the service provider as data holder. Service-provider notice duties were not verified state by state at the recorded check date.

### 15.3 Security Rule on a single PC

**Current requirements that the single-PC model makes prominent** (eCFR source summary, SR-117):

- **Contingency plan, 164.308(a)(7).** The data backup plan ("retrievable exact copies"), disaster recovery plan and emergency-mode operation plan are **required**. Testing and revision, and applications and data criticality analysis, are **addressable**.
  - A hosted backup service can implement part of the backup plan.
  - The lab still owns the disaster recovery plan, the emergency-mode plan and any test of restoring onto replacement hardware.
- **Device and media controls, 164.310(d).** Disposal and media re-use are **required**: remove ePHI before re-use, and dispose of media properly. Accountability and backup-before-move are **addressable**. Replacing or retiring the lab PC, swapping its disk or returning a leased machine are regulated events.
- **Encryption at rest and in transit, 164.312(a)(2)(iv) and (e)(2)(ii).** These are **addressable**. 164.306(d) requires the lab to implement them or document why not and adopt an equivalent. On a single PC, full-disk encryption also turns a stolen PC into a likely non-notifiable event (§15.2).
- **Audit controls, 164.312(b).** These remain **required** (REG-012). Operating-system audit settings on the lab PC are outside the application's control and belong in the onboarding checklist.

**HIPAA Security Rule NPRM status (proposal).** The only Federal Register document under RIN 0945-AA22 is the NPRM, 90 FR 898, published 2025-01-06. The Unified Agenda lists it as a long-term action with final action targeted for 07/2027, and it was not on OIRA's review list on 2026-10-12 (SR-116, SR-131; §13, REG-026/REG-043). If finalized as proposed, the rule would, among other things:

- remove the required/addressable distinction;
- require MFA and encryption at rest and in transit, with limited exceptions;
- require a written technology asset inventory and network map, reviewed every 12 months;
- require restoration of critical systems and data within 72 hours;
- require a compliance audit at least every 12 months;
- require vulnerability scans every 6 months and penetration tests every 12 months;
- require written verification from business associates every 12 months, consisting of a written analysis plus a certification;
- require business associates to report activation of their contingency plan within 24 hours.

**Design consequence.** Set the project's recovery target at or below 72 hours, from an off-PC backup (lab-owned or hosted) to a replacement PC, now. Doing so costs little and avoids redesign if the rule is finalized. Do not present it as a current mandate.

**Guidance.**

- HHS 405(d) HICP "Technical Volume 1: Cybersecurity Practices for Small Healthcare Organizations" (2023 edition, SR-118) is the best-matched voluntary baseline for a POL. HHS's voluntary performance goals (§13, SR-100) apply as before.
- NIST SP 800-63-4 (final July 2025, SR-121) is a voluntary reference for authentication levels. No HIPAA text adopting it was found.

**Windows 10 (risk-analysis fact).**

- Microsoft ended Windows 10 support on 2025-10-14 (SR-119).
- Organizational ESU provides per-device coverage for at most three years; the laboratory must qualify its eligibility and current coverage.
- The consumer ESU, which runs to 2027-10-12, "can't be used in commercial scenarios" and excludes domain-joined and MDM-managed devices. A lab PC is a commercial device.
- An unpatched Windows 10 lab PC is therefore a documented risk-analysis finding. Onboarding should record OS version, ESU status, TPM 2.0 and disk encryption.
- Not checked: Windows 10 LTSC/IoT editions, which have different dates. Recheck at onboarding if a lab uses them.
- The desktop scope covers Windows, macOS and Linux ([ADR 0012](../architecture/decisions/0012-rust-tauri-desktop-application.md)). By the same reasoning, a macOS release Apple no longer patches, or a Linux release past its support period, would be a risk-analysis finding (project inference; servicing dates not checked here).

### 15.4 CLIA with one PC as the laboratory's record system

The CMS State Operations Manual Appendix C (Rev. 236, issued 2026-01-23; interpretive text Rev. 233, effective 2025-09-12; SR-122) is public **guidance** that surveyors use. Its passages that matter here:

- **Result transmission, 493.1291(a), D5801.** The regulation requires "adequate manual or electronic system(s)" so that results reach the "final report destination" accurately, reliably and on time. It expressly covers results "electronically reported to network or interfaced systems".
  - The guidance says "routine checks should be conducted to verify that transmissions are being accurately and reliably conveyed to the final report destination".
  - It says a laboratory "may contract with another entity to assist in the delivery of patient reports". If that entity is a business associate, the contract must meet 164.504(e).
  - **Consequence:** the portal, the relay and each EHR interface are in CLIA scope as the lab's delivery chain. The product needs per-destination delivery evidence and a lab-run periodic check that shows the received content matches what was sent (REG-007, REQ-INT-002).
- **Retention and retrieval, 493.1105, D3027 and onward.** The rule applies to LIS records. The lab sets and documents its own evaluation of storage and retrieval. Reports must be retrievable as "the exact information as sent".
  - Surveyor probes ask "What is the laboratory's procedure for record retrieval when the information system is not functioning due to a security breach, change in system, or extended downtime?"
  - **Consequence:** with one PC, the backup and restore path *is* the CLIA retrieval path. The two-year report-copy duty (REG-009) must survive PC loss, the lab leaving a service provider, and the provider closing. A lab-held export independent of hosted services is needed.
- **Downtime procedure, 493.1251(b)(14).** LIS procedures must name whom to notify "if the LIS goes down". The lab must have "a plan if a test system becomes inoperable for either a short or extended time". The product should ship a downtime and recovery runbook template for lab approval.
- **Software changes, 493.1253 probe.** "How does the laboratory determine if a new or revised LIS program (whether purchased or developed in-house) performs acceptably before it is integrated into routine operation?" LIS calculations "must be verified ... prior to initial calculation of patient results."
  - **Consequence:** updates must never install automatically into production. Each release needs notes, a hash and verification material so that the lab can accept it before use.
- **Maintenance, 493.1254 / D5429.** The laboratory "must perform and document maintenance as specified by the manufacturer for the LIS computer". For open-source software the project is the de facto "manufacturer" of the instructions. Publish maintenance and function-check instructions, such as backup verification, disk-capacity checks and clock checks.
  - Appendix C under 493.1101 also says capacity maintained "by deletion of data ... should be scheduled and documented".
- **Confidentiality, 493.1231.** D5201 gives entering a user identification and password as an example of protecting LIS access. It does not prescribe that combination exclusively or establish a categorical shared-OS-login finding. Separately, 493.1283(a)(4) requires testing-personnel identity in test records (SR-154). The project's per-person LIS authentication and workstation controls address attribution and access; the laboratory must assess its actual OS-account arrangement. [Security-key research, checked 2026-10-14](security-key-authentication.md).
- **Patient access, 493.1291(l) (requirement as written; permissive).** The lab "may" give completed reports to the patient or personal representative that, "using the laboratory's authentication process", can be identified as the patient's. The patient portal's identity proofing is therefore the lab's authentication process for CLIA purposes (§15.8).

**POL certificate and complexity (requirement; applicability depends on menu).**

- FDA categorizes commercial tests. A test system that does not appear on the categorization lists "is considered to be a test of high complexity" until reviewed (42 CFR 493.17(c)(4); FDA states uncategorized tests "default to high complexity", SR-123, SR-124).
- An LC-MS/MS laboratory-developed method is therefore high complexity. A POL running it needs a certificate of compliance or accreditation for high complexity, with matching personnel (REG-038).
- Commercial immunoassay screens are categorized product by product: some are waived, many are moderate on analyzers. Check each in FDA's CLIA database. No example entry was opened at the recorded check date.
- How CMS treats *modified* cleared tests was not re-verified here.
- COLA's CMS approval covers toxicology (SR-098). That COLA is the dominant POL accreditor was not verified.

**Consequence.** Product configuration must record the certificate type and complexity per test (REQ-QMS-006). A waived-only POL needs far less of the analytic module than an LC-MS/MS lab.

### 15.5 Fraud and abuse: laboratory-furnished clinic portals

The applicability question is whether a laboratory furnishes portal or interface access to its referral sources (ordering clinics). Anything of value the laboratory furnishes to a referral source falls under the Anti-Kickback Statute (all federal programs). It falls under Stark when a referring physician has a financial relationship with the lab, and under EKRA (all payers, including commercial) for laboratories (§10, REG-032). Findings:

| Authority | Verified content | Classification |
| --- | --- | --- |
| OIG Special Fraud Alert on clinical laboratory services (1994; reprinted 59 FR 65372, 1994-12-19; SR-105) | Lists "Provision of computers or fax machines, unless such equipment is integral to, and exclusively used for, performance of the outside laboratory's work" among suspect inducements | Guidance |
| OIG, 1991 safe-harbor preamble, as restated in OIG's 1997 letter (SR-106) and AO 12-20 | A lab-furnished computer usable only to print the lab's results has "no independent value"; a general-purpose PC "could constitute an illegal inducement" | Guidance |
| OIG EHR safe-harbor final rule (78 FR 79202, 2013-12-27; effective 2014-03-27; SR-107) | Laboratory companies removed as protected EHR donors. Exclusion "does not affect our position concerning the provision of free access to certain limited-use interfaces"; "the substance, not the form, of an arrangement" governs | Requirement (safe-harbor scope) and guidance |
| 42 CFR 1001.952(y)(1)(i) and 411.357(w)(1) (SR-109, SR-110) | EHR donor must be "other than a laboratory company" / "not a laboratory company"; recipients otherwise pay 15% of cost | Requirement (no protection for labs) |
| OIG Advisory Opinion 12-20 (issued 2012-12-12; SR-108) | A hospital's free interface for community physicians to send lab/diagnostic orders and receive results was not remuneration. The interface was certified to "serve no purpose other than to transmit the orders and results". Physicians "remain responsible for all aspects" of their own EHR, hardware and connectivity. OIG called it a "contemporary analog" of the limited-use computer | Guidance; binds only the requestor |
| Stark definition of remuneration, 42 CFR 411.351 (SR-111) | Excludes items "in fact, used solely" for ordering tests or "communicating the results of tests ... for the entity furnishing" them | Requirement (definition) |
| Cybersecurity donations, 1001.952(jj) / 411.357(bb) (effective 2021-01-19; SR-109, SR-110) | No donor-type exclusion found, so laboratories are not excluded. Eligibility must not "directly take into account the volume or value of referrals"; written description required; (bb) defines technology as "any software or other types of information technology" | Requirement (protection available if met) |
| EKRA, 18 U.S.C. 220(b) (SR-112) | Exceptions cover discounts, bona fide employment, specified personal-services contracts, coinsurance waivers and a few others. **No EHR, interface, portal or cybersecurity exception**, and AKS safe harbors are not generally imported | Requirement |

**What this means for the design (issues for counsel, not conclusions).**

1. **Keep the portal a limited-use ordering and result tool of the furnishing lab.** Clinic-facing functions stay limited to that lab's orders, requisitions and labels, results and reports, supply requests and order messages (REQ-ORD-011). General scheduling or practice management, e-prescribing, chart or EHR functions, non-lab document storage, analytics on the clinic's business, other laboratories' results, clinic billing services and unrelated patient engagement give independent value to clinics and should not be furnished free. If such features exist, clinics should contract and pay for them directly at fair market value. This is a counsel decision.
2. **One lab per portal context.** A multi-lab ordering hub given free to a clinic is not "used solely" for the furnishing entity (411.351). If a clinic uses several labs on the platform, each lab's grant should cover only that lab's ordering and results. Any cross-lab convenience needs a separate counsel-approved arrangement.
3. **Clinic-side costs.** In AO 12-20 the physicians paid for their own EHR, hardware and connectivity. Paying a clinic's EHR-vendor interface or connection fees, or furnishing clinic hardware not limited to the lab's work, is a higher-risk pattern; counsel decides who pays interface fees before an EHR interface arrangement is established. Labels and printers limited to the lab's specimens resemble the Stark specimen-collection exclusion, but the AKS analysis still applies.
4. **No selection by referral value.** Do not let the lab offer, enable or withhold portal or interface access based on a clinic's expected order volume or payer mix. The software should not provide such targeting reports (REQ-OPS-005). The lab's onboarding criteria for clinics are a compliance record.
5. **Referral-related terms.** Counsel must assess any external-service agreement or arrangement that could tie compensation to referrals, tests or billing (Q-32). The software must not implement referral-value access controls or a referral marketplace. Commercial terms are outside this repository’s LIS requirements.
6. **Cybersecurity help.** Help from a lab to a clinic, such as securing the clinic's portal access, is protectable only within 1001.952(jj)/411.357(bb). It needs written terms and selection that does not depend on referrals. EKRA has no parallel exception.
7. **POLs.** If the laboratory belongs to the referring practice, portal use by its own physicians is not a gift to an outside referral source. Stark's in-office ancillary services exception (42 CFR 411.355(b); not re-read at the recorded check date) governs self-referral. As soon as the POL accepts outside clinics' specimens, items 1–6 apply.
8. **Records.** Use REG-032's provisioned-service record to list, per clinic, the access, interfaces, devices and support furnished, with dates and approver. This gives counsel evidence that the "limited use" facts remain true.

Enforcement context: see the cited cases and their limits in §10.

### 15.6 FDA boundary for LIS, middleware and portals (guidance; statutory exclusion)

**Reconfirmed.** Section 520(o)(1)(D) excludes functions "solely intended to transfer, store, convert formats, or display" device data and clinical laboratory results, "unless" intended to "interpret or analyze" them.

- FDA's MDDS guidance (2022-09-28; SR-125) says such functions "are not devices and thus are not subject to FDA regulatory requirements applicable to devices".
- FDA's *Policy for Device Software Functions and Mobile Medical Applications* (2022-09-28; SR-126) lists, among non-devices, functions "for transferring, storing, converting formats, or displaying clinical laboratory test or other device data and results".
- The same guidance excludes from Non-Device-MDDS "software functions intended to generate alarms or alerts or prioritize patient-related information on multi-patient displays". FDA describes these as device functions "because these functions involve analysis or interpretation".

**Consequences.**

- The LIS, interface hub, relay and a display-only portal stay closest to the exclusion.
- The following need the §9 / REG-024 assessment before release, whether in the clinic portal or the future patient portal:
  - automated critical-value alerting;
  - worklists ranking patients by result;
  - "out of range" or "inconsistent with prescribed medication" flags generated by the software;
  - patient-facing explanations.
- The non-MDDS definition also requires not "controlling or altering the functions or parameters of any connected medical devices". Bidirectional analyzer interfaces that send orders or worklists should be assessed so that they do not change instrument parameters.

### 15.7 Secure delivery, interoperability and public-health reporting

- **Fax and email (requirement and guidance).** The Omnibus preamble lets covered entities send unencrypted email to an individual who has been warned of the risk and still prefers it. The entity is then "not responsible for unauthorized access ... while in transmission" (78 FR 5634; SR-114). This is a patient-access route, not a basis for unencrypted email to clinics.
  - HHS FAQs on fax and email for treatment (FAQs 482 and 570) remain unverified (SR-135).
  - Fax remains lawful with reasonable safeguards. Cloud-fax and Direct providers are subcontractors (§15.1).
- **Direct messaging (guidance, unknown).** No federal rule requiring a laboratory to use Direct, or requiring DirectTrust accreditation, was found. Certification criteria bind certified health IT developers, not laboratories. DirectTrust's accreditation page returned 403 (SR-136), so program details are unverified. Treat accreditation of the chosen HISP as a vendor-selection criterion.
- **TEFCA (voluntary).** ASTP's page (updated 2026-09-03; SR-132) describes participants and subparticipants connected through QHINs; laboratories are not named. A single-PC lab would join, if at all, as a subparticipant through an HIE or vendor. That is a possible future integration, not a duty.
- **CMS Interoperability Framework (voluntary).** The CMS page, modified 2026-08-06 (SR-133), describes a voluntary pledge program ("a voluntary blueprint") with no laboratory participant category. Early adopters reached general availability in July 2026, and FHIR API criteria were "in effect as of July 4th, 2026". Criterion 13 expects "labs in LOINC". It remains a market expectation, not a laboratory obligation. LOINC coding on outbound results is the practical consequence.
- **Electronic laboratory reporting.** This is unchanged (§8, REG-041). On a single PC, ELR is another outbound interface. If it is routed through a hosted interface hub, the hub's service provider is a business associate for that flow too. Public-health disclosures themselves are permitted.

### 15.8 Patient portal (later phase)

- **Access timing (requirement; proposal pending).** The 30-day right-of-access period with one extension still applies (§4, REG-015). The 2021 Privacy Rule final rule (RIN 0945-AA00) has been under OIRA review since 2026-04-02 and is unpublished (SR-131). Its 2021 proposal would have shortened the period to 15 calendar days. Recheck before the patient portal is designed.
- **Information blocking (requirement).** Labs are actors (§11, REG-016/036), and a patient portal is an access channel. HTI-5 has been at OIRA since 2026-07-28 and is unpublished (SR-131). No blanket delays.
- **CLIA 493.1291(l) and identity (requirement and guidance).** Direct release uses "the laboratory's authentication process" (§15.4). NIST SP 800-63-4 (July 2025) is a voluntary reference for proofing and authentication levels. Proofing must not become an unreasonable barrier to access (§4).
- **State release rules (unknown).** California Health and Safety Code §123148 governs electronic release of certain results to patients and may treat some result types differently. Its official text returned 403 at the recorded check date, so its current list and any delay periods are **not verified**. Toxicology relevance must be checked before patient release in California. New York direct-access limits are unchanged (REG-042).
- **Accessibility.** Unchanged (§12, REG-039). A Section 504 final rule is now at OIRA (§14).

### 15.9 Open-source distribution and validation

- **No certification implied (project position).**
  - The LIS is not FDA-cleared, and under §15.6 it should not need to be.
  - It is not ONC-certified (§11).
  - It does not make a laboratory CLIA-compliant.
  - An open-source license's warranty disclaimer allocates contract risk only. It does not move any CLIA or HIPAA duty off the laboratory.
- **Validation ownership (requirement).** CLIA places verification of the LIS and of each update on the laboratory (§15.4; director duties in 42 CFR 493.1407/493.1445, not re-read here). External specialists may assist with planning, executing and documenting testing. Acceptance must be signed by the laboratory director or designee.
- **Project responsibility.** Publishing maintenance and function-check instructions makes the project the de facto "manufacturer" whose instructions surveyors expect labs to follow (§15.4). A service provider that creates, receives, maintains or transmits PHI for the laboratory is a business associate (§15.1).
- **Service provider's own status (unknown, probably none).** A consultant that performs no testing is not a "laboratory" and needs no CLIA certificate (493.2 and 493.3 define laboratories by examining specimens). No federal licensure of LIS validation consultants was found; state licensing of laboratory consultants was not checked, and contract liability remains a risk.

### 15.10 State overlays that change with hosted services

| Jurisdiction | Rule (status) | Effect on the single-PC model |
| --- | --- | --- |
| Texas | SB 1188 (89th Leg.), Health & Safety Code ch. 183. Effective 2025-09-01; the storage duty in §183.002 applies to storage on or after **2026-01-01**. Covered entities, as defined in §181.001, must keep EHRs they control "physically maintained in the United States or a territory", including records stored by "an entity that provides cloud computing services". AG civil penalties up to $250,000 per violation. Also requires immediate parent access to minors' records, with exceptions (SR-127). **Requirement** | Hosted backup, portal and relay data for Texas labs must stay on US infrastructure, including subcontractors and support access paths. Minor-access rules affect any later patient portal. |
| Texas | HB 300 (2011; effective 2012-09-01): training for covered-entity employees within 60 days of hire and every two years, with a signed attendance statement; notice of electronic disclosure (§181.154) (SR-128). Texas defines covered entity more broadly than HIPAA. **Requirement as enacted**; current amended text and a service provider's coverage not verified | A service provider's staff handling Texas PHI likely need Texas-specific training records. Confirm the current §181.101 timing. |
| Florida | §408.051(3): a health care provider "that utilizes certified electronic health record technology" must keep offsite or cloud-stored patient information in the continental US, its territories or Canada (SR-129). **Conditional requirement** (certified EHR users) | Applies to Florida clinics, and to labs only if they use certified EHR technology. A US-only hosting policy satisfies it. |
| California | HSC §123148 (electronic result release). **Unknown**: text not accessible at the recorded check date (SR-130) | Recheck before patient release in California. |
| All states | Breach-notification statutes and service-provider notice duties. **Unknown** in detail for a service provider | Track by patient residence in the incident record (§15.2). |
| Medicaid and payer contracts | Offshore-PHI restrictions in state Medicaid or managed-care contracts. **Unknown**: none verified at the recorded check date | Treat as contract terms the lab may pass down. A US-only data and support policy covers the likely cases. |

**Consequence (project recommendation).** Adopt a US-only policy for data location *and* administrative or support access for all PHI-bearing hosted services and subcontractors. This one policy covers Texas and Florida and the likely contract terms.

### 15.11 Candidate controls

The [compliance matrix](../requirements/compliance-matrix.md) is the canonical record
of proposed controls, requirement traces, evidence and accountable human owners:

| Control | Current trace |
| --- | --- |
| Service-provider role, BAAs, data inventory and PHI-free telemetry | REG-044 |
| Limited-use laboratory portal/interface boundary and referral safeguards | REG-045/046 |
| Delivery-chain verification and software-update acceptance | REG-047 |
| Contingency, recovery and media disposal | REG-048 |
| State storage-location overlays | REG-049 |
| Later patient-portal applicability | REG-050 |

These controls retain their conditional applicability and unresolved laboratory/counsel
review. They do not authorize external-service implementation or deployment.
