# Laboratory operator guide

These guides describe how laboratory staff operate the implemented application. It is
development software built on synthetic data; see the
[development disclaimer](../governance/development-disclaimer.md). A laboratory must approve its
intended use, procedures, configuration and each release before clinical use.

The application, its data and every function in these guides run on the laboratory's own
computer without internet or a hosted service ([project scope](../project-scope.md#product-boundaries)).
Analyzer listeners run only while the application is open, including at its sign-in screen.
Whether off-host tamper-evidence anchoring works in every installation remains
[Q-30](../planning/open-questions.md#decision-and-discovery-queue). The application does not make
an administrator a clinical authority.

## Roles and separation of duties

The application uses per-person accounts. Do not share an application account. One person may hold
more than one role only when the laboratory has assessed and accepted that arrangement.

| Role | What the role does in the application | Important boundary |
| --- | --- | --- |
| Administrator | Manages accounts, approves and activates configuration under the configured separation-of-duties rule, runs backups and restore tests, handles the recovery kit, and creates exit exports | Has no clinical authority merely by being administrator |
| Quality manager | Records quality decisions and closes a recovery reconciliation | Assigns or removes clinical roles after the first quality manager exists |
| Accessioner | Enters orders as supplied, receives specimens and may deliver released reports | Does not review or release results unless separately assigned |
| Analyst | Performs analytical work | Does not gain technical-review or release authority |
| Technical reviewer | Performs technical result review | Does not gain report-release authority |
| Authorized releaser | Releases, corrects and delivers reports | Release still requires the applicable gates and fresh authentication |
| Billing | Works on billing records and handoffs | Does not gain clinical authority |
| Auditor | Reads the audit trail | Read access does not permit changing records |

Current account-separation rules are recorded in [Q-37](../planning/open-questions.md#decision-and-discovery-queue):

- The first administrator may appoint the first quality manager while none exists.
- After that appointment, adding or removing a clinical role requires quality authority.
- Resetting the credentials of a person with a clinical role requires both account-administration
  and quality authority.
- Nobody can change their own roles or reset their own sign-in through administration.
- The last active administrator and last active quality manager cannot be disabled.
- Account creation, role changes and credential resets require the acting person's password or security key and
  are recorded.

The laboratory still must decide whether administrator and quality-manager duties are held by
different people and who reviews sensitive account events. That unresolved local decision is also
part of [Q-37](../planning/open-questions.md#decision-and-discovery-queue).

### Separation of duties and one-person laboratories

By default the application needs a second person to:

- approve configuration someone wrote: test, panel, method and validity-policy versions, All Data
  Reviewed, result source registrations and billing class mappings;
- release a report whose results someone reviewed technically;
- verify a delivery destination someone created.

For US clinical laboratories this is laboratory policy, not a legal requirement; the regulatory
evidence is in [ADR 0015](../architecture/decisions/0015-separation-of-duties-policy.md).

- **Changing the policy.** An administrator opens **Operations settings → Separation of duties →
  Change separation of duties**, sets the three switches, and enters a reason and their password or security key.
  Each change is a new, audited version.
- **Each self-approval is confirmed.** Even when the policy allows it, the application asks the
  person to confirm that no second person reviewed the step, every time ("I confirm that no second
  person reviewed this"), and marks the audit event. An auditor lists these events with **Audit
  trail → Self-approvals only**. Decide in your SOP who reviews them and how often.
- **One person runs the laboratory.** At first-run setup, answer "Who will use this laboratory?"
  with **Only me: I run this laboratory alone**. Later, while yours is the laboratory's only active
  account, use **Users → Set up for one person**, with a reason and your password or security key. Either way your account gets every role and all three switches allow you
  to work alone. When someone joins, an administrator can require a second person again.
- **Exploring the demo alone.** Keeping **Load Standard Catalog and demo data** checked during
  demo account creation applies the same one-person arrangement to that demo account. After
  successful authentication, the app prepares catalog drafts and synthetic records.
  Self-approval confirmation and catalog review still apply. See the
  [first-run demo walkthrough](installation-and-first-start.md#explore-with-the-bundled-dataset).
- **Your obligations.** Your SOPs and validation must cover the policy you set. Setting it in the
  application is not evidence that your accreditor or state accepts the arrangement.

## Guide index

Within the signed-in application, **Page help** opens guidance for the current screen.
**Browse all topics** searches the bundled guides without leaving your work. Help works
offline and is updated with the application. Clinical procedures remain the laboratory's
approved procedures; see [how built-in help is maintained](../development/contextual-help.md).

For order and accession entry, **Entering a new order** and **Receiving specimens** in
Page help cover test/panel search, reviewing chosen work, required source evidence and
collection/material entry. Their canonical references are [new-order.json](../help/new-order.json)
and [receive-specimens.json](../help/receive-specimens.json).

| Guide | Use it for |
| --- | --- |
| [Installation and first start](installation-and-first-start.md) | Preparing the computer, trying the bundled demo, first-account fields and dataset choice, retry/reset, MFA, quality-manager appointment and the laboratory profile |
| [Backups and recovery kit](backups-and-recovery-kit.md) | Targets, schedules, verified copies, restore tests, key custody and backup health |
| [Restore and recovery](restore-and-recovery.md) | Restoring to a replacement computer and reconciling the lost interval |
| [Daily operation](daily-operation.md) | Keeping listeners available, closing safely, work queues, health, clock issues and unclean stops |
| [Result files](result-files.md) | Loading LC-MS/MS result files: vendor export settings, file templates, value words, the watched folder, measured-not-ordered results and refused files |
| [Updates](updates.md) | Recording approval, making a pre-update backup and accepting the running version |
| [Exit export](exit-export.md) | Producing and independently checking a complete portable copy |
| [Billing evidence and handoff](billing-evidence-and-handoff.md) | Draft manual procedure for billing evidence, corrections, rejected handoffs and payer record requests (REQ-BIL-006) |

The defaults and unresolved laboratory choices used throughout these guides come from the
[laboratory policy inputs](../quality/laboratory-policy-inputs.md#operations) and the canonical
[decision queue](../planning/open-questions.md).
