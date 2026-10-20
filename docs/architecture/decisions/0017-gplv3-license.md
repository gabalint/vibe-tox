# ADR 0017: GPL version 3 and the development disclaimer

Status: **Accepted by the project owner, 2026-10-14.** Decision owner: project owner.
The project uses **GPL-3.0-only** and the informational development disclaimer.
This resolves the license-choice portion of Q-11 and supports NFR-13's license
records. Ownership administration, trademarks and contribution governance remain
open.

## Scope and authority

The license covers the project's own source, documentation, synthetic fixtures,
website and original presentation assets unless identified otherwise. Third-party
material retains its own terms and notices.

Choosing an open-source license does not require publishing private development
work or authorize a production release; repository publication follows
[AGENTS.md](../../../AGENTS.md#current-mandate).

## Decision

| Element | Accepted rule |
| --- | --- |
| License | Use the unmodified GNU General Public License, version 3, with SPDX identifier `GPL-3.0-only`. The example “or later” notice in the application appendix does not expand the project's grant. |
| Canonical text and metadata | Keep the official GPLv3 text in root [LICENSE](../../../LICENSE), with consistent workspace, npm and Tauri metadata and project-crate license policy. |
| Attribution | Use “Vibe Toxicology LIS contributors” for the respective rights holders. Preserve applicable third-party terms, attribution and notices. |
| Development disclaimer | State that the project is developed with substantial AI assistance and reasonable effort in research, planning, analysis, review and testing. Publish the [canonical disclaimer](../../governance/development-disclaimer.md) and concise summaries in the README and presentation. |
| Disclaimer boundary | Keep the disclaimer informational, with GPLv3 sections 15–17 warranty/liability references. It does not modify the license or add noncommercial, no-resale, clinical-use or other restrictions. |

## Consequences and limits

GPLv3 allows laboratories to use and modify the application internally. Others
may redistribute compliant versions, including commercial distribution. When
conveying the covered program or a covered derivative, provide the license,
required notices and corresponding source through an allowed GPLv3 method.
Source availability is an obligation to recipients under those terms; private
modifications do not require public release.

Separate independent works merely aggregated with the application do not
automatically become GPL-covered. Mere network interaction without transferring
a copy does not itself trigger the GPL's conveying obligations. See GPLv3
sections 0–6 and the GNU FAQ in the
[source ledger](../../research/sources-engineering.md#gplv3-license-sources).

The license covers software and other covered project material, not patient data
merely processed by the application. It supplies no clinical approval or
regulatory certification. Warranty and liability provisions in sections 15–17
remain subject to applicable law and written agreements; no immunity is promised.

The collective contributor attribution is not a legal entity, copyright
assignment or claim that all AI-generated material is copyrightable. No separate
DCO, CLA or trademark policy is adopted. This decision does not certify ownership
of every contribution or compatibility of every future release artifact. Review
those rights and the complete dependency/content inventory before release.

Separately developed future cloud-service code remains Q-11 and outside the
implementation mandate. Reuse of covered project code must respect its applicable
license; communication over a protocol alone does not decide whether another
program is a covered derivative.

## Evidence and release review

Primary license text and publisher guidance were checked on 2026-10-14 and are
recorded as ENG-270–ENG-274 in the
[engineering source ledger](../../research/sources-engineering.md#gplv3-license-sources).
GPLv3 was published on 2007-06-29. These sources establish license terms and
publisher explanations, not a US/state-law enforceability opinion.

The [open-source strategy](../../governance/open-source-strategy.md) defines
current distribution responsibilities. Each release needs its own license,
notice, dependency-compatibility and corresponding-source checks. Revisit this
decision before distributing a binary with unresolved rights or source-delivery
obligations, adding separately licensed code, or when the project owner changes
the license.
