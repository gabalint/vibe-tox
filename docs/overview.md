# Vibe Toxicology LIS at a glance

Status: updated 2026-10-20 from the [project scope](project-scope.md), the
[implementation plan](planning/implementation-plan.md#status) and the
[operator guide](operations/README.md). This is the canonical short introduction to the
product; keep the status line with any copy taken from it.

## In one line

A free, open-source laboratory information system for small US clinical toxicology laboratories,
licensed under [GPL-3.0-only](../LICENSE), running on a single computer the laboratory owns.

## In 30 seconds

Vibe Toxicology LIS covers a test from order to delivered report: orders, specimens and labels,
analyzer results, quality control, review, release, delivery and billing. It is a desktop
application for Windows, macOS and Linux that keeps all data on the lab's own computer, works
without internet, and lets the lab back up, restore or export everything at no cost. Optional
external services, such as a portal for ordering clinics, remain proposals outside the current
application scope.

> **Status.** Built and tested on synthetic data only. It is not yet clinically validated, and is
> not ready to report patient results.

This project is vibe coded with substantial AI assistance; see the
[development disclaimer](governance/development-disclaimer.md).

## The two-minute overview

### Who it is for

US clinical toxicology laboratories small enough to run on one computer: physician-office labs and
small independent labs. They need full traceability from order to report, without running servers
or depending on an internet connection.

### How work flows through it

| Step | What the application does |
| --- | --- |
| **Order** | Records tests, diagnoses and supporting evidence exactly as the clinic supplied them, never filling them in |
| **Accession** | Receives specimens, records collection and prints barcode labels |
| **Bench** | Builds worklists and runs; receives results from ASTM and Olympus AU400/AU640 analyzers and from LC-MS/MS result files; records quality control |
| **Review** | Puts every result through technical review; a failed QC or an identity question holds it |
| **Release** | Rechecks every hold at the moment an authorized person releases the PDF report; a correction keeps the earlier version |
| **Deliver** | Logs each fax, email, mail, courier or pickup with the evidence available, never a bare "delivered" |
| **Bill** | Derives billable services from the work performed, not from positive results, and prepares the billing handoff |

### Core principles

- **One computer, works offline.** The lab's computer is the system of record. Analyzers keep
  sending results while the application is open, even with nobody signed in.
- **The lab owns its data.** Encrypted backups to the lab's own disk, NAS or USB drive, restore
  tests and a complete export in documented formats are all free.
- **Safety is built in.** Quality-control and identity holds block release, and released reports
  are versioned.
- **Every action is attributed.** Personal accounts with two-factor sign-in or a personal
  security key, roles, configurable
  separation of duties (including a one-person mode) and a tamper-evident audit trail of every
  change and every view of sensitive data.
- **A head start.** A standard catalog for urine and oral fluid loads as drafts for the lab to
  review and approve, and a demo laboratory lets people try it safely.

### Everything runs locally

The application works without any hosted service. Optional future hosted services may support
ordering clinics, off-site backup, report delivery and EHR interfaces, with a patient portal
later. These services are not built. They would only relay requests and deliver copies; the
local LIS remains responsible for clinical records.

### Where it stands (October 2026)

The order-to-report workflow and the backup, restore and export tools are built and tested,
and a development build runs on macOS. Still to come: validation with a partner
laboratory and signed installers. Extra workstations on the lab network and optional hosted
services need their own scope decisions. Each laboratory must validate the application and
approve its own procedures before clinical use.

## Before publishing this text

| Item | Why it matters | Record |
| --- | --- | --- |
| License and trademark | GPL-3.0-only. Ownership administration, trademark policy and future cloud-service licensing remain open. | [ADR 0017](architecture/decisions/0017-gplv3-license.md), [Q-11](planning/open-questions.md#decision-and-discovery-queue) |
| Publication | The repository and presentation website are public; signed releases and security-response commitments remain unresolved. | [Q-19](planning/open-questions.md#decision-and-discovery-queue), [website maintenance](development/public-website.md) |
| Optional hosted services | Service boundaries remain proposals. | [ADR 0010](architecture/decisions/0010-optional-cloud-services.md), [scope](project-scope.md#product-boundaries) |
| Off-host tamper-evidence anchoring | Anchoring to a lab-owned target or offline media is implemented; whether off-host anchoring works in every installation without a hosted service is open. | [Q-30](planning/open-questions.md#decision-and-discovery-queue) |
| Status line | No document certifies compliance, accreditation, clinical validity or production readiness. | [AGENTS.md](../AGENTS.md#sources-of-authority) |

Update this page when a milestone changes what the application does, or when one of these
decisions is made.
