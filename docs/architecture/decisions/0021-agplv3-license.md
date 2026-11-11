# ADR 0021: AGPL version 3, copyright and trademark notice

Status: **Accepted by the project owner, 2026-11-11.** Decision owner: project owner.
Supersedes ADR 0017 (GPL-3.0-only, accepted 2026-10-14), which remains in Git history.
The project uses **AGPL-3.0-only**, the copyright and trademark notice below and the
informational development disclaimer. This resolves the license-choice portion of Q-11
and supports NFR-13's license records. Contribution governance remains open.

## Scope and authority

OpenBGA Tox is an open-source project of BGASoft Inc. The license covers the project's own
source, documentation, synthetic fixtures, website and original media unless identified
otherwise. Third-party material retains its own terms and notices.

Builds and source already published under GPL-3.0-only stay available under that license
to the people who received them; this decision does not withdraw it. Every build and
release from this decision onward is distributed under AGPL-3.0-only.

Choosing an open-source license does not authorize a production release, signing or
clinical use; publication follows [AGENTS.md](../../../AGENTS.md#current-mandate).

## Decision

| Element | Accepted rule |
| --- | --- |
| License | Use the unmodified GNU Affero General Public License, version 3, with SPDX identifier `AGPL-3.0-only`. The example “or later” notice in the license's appendix does not expand the project's grant. |
| Canonical text and metadata | Keep the official AGPLv3 text, unedited, in root [LICENSE](../../../LICENSE), with consistent workspace, npm and Tauri metadata and project-crate license policy (`deny.toml` exceptions). `scripts/check_repository_metadata.py` checks that they agree. |
| Copyright | `Copyright (C) 2026 BGASoft Inc. and OpenBGA contributors.` in the [README](../../../README.md#license-and-development-disclaimer), [NOTICE](../../../NOTICE) and the installer metadata. Preserve applicable third-party terms, attribution and notices. |
| Names and logos | OpenBGA and the OpenBGA product names and logos are trademarks of BGASoft Inc. The license does not grant permission to use them (AGPLv3 section 7(e)), except as needed to describe the origin of the software. |
| Development disclaimer | Publish the [canonical disclaimer](../../governance/development-disclaimer.md) and concise summaries in the README and on the website. It stays informational, with AGPLv3 sections 15–17 warranty/liability references, and adds no noncommercial, no-resale, clinical-use or other restriction. |

## Consequences and limits

Laboratories may use and modify the application internally, and others may redistribute
compliant versions, including commercially. Conveying the program or a covered derivative
requires the license, required notices and corresponding source through an allowed method
(sections 4–6). Private modifications that are neither conveyed nor offered to remote users
do not require publication.

Section 13 adds a network-source requirement: anyone who modifies the program and lets
users interact with the modified version remotely through a computer network must offer
those users its corresponding source from a network server at no charge. The current
application runs on one laboratory computer, but a modified version reached over a network,
such as a future clinic portal or report-delivery service built on covered code, carries
that obligation. Section 13 also permits combining covered code with GPLv3 code; the GPLv3
part keeps its own terms.

Separate independent works merely aggregated with the application do not become covered.
The license covers software and other covered project material, not patient data processed
by the application. It supplies no clinical approval or regulatory certification. Warranty
and liability provisions remain subject to applicable law and written agreements.

The trademark term limits use of the names and logos only; it does not restrict the
software rights the license grants. A written trademark-use policy remains Q-11. No separate DCO or CLA is adopted, and this decision does not certify ownership of
every contribution or compatibility of every future release artifact.

Separately developed future cloud-service code remains Q-11 and outside the implementation
mandate; reuse of covered project code in it carries the AGPL terms, including section 13.

## Evidence and release review

The license text was downloaded from the publisher on 2026-11-11 and copied unchanged into
`LICENSE`. Sources are ENG-308–ENG-310 in the
[engineering source ledger](../../research/sources-engineering.md#license-sources), with the
GNU FAQ (ENG-271) and Apache License (ENG-273) entries that apply equally to AGPLv3. They
establish license terms and publisher explanations, not a US or state-law enforceability
opinion.

The [open-source strategy](../../governance/open-source-strategy.md) defines current
distribution responsibilities. Each release needs its own license, notice,
dependency-compatibility and corresponding-source checks. Revisit this decision before
offering a network service built on covered code, adding separately licensed code, or when
the project owner changes the license.
