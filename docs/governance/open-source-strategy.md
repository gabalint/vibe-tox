# Open-source project governance

Status: **GPL-3.0-only; remaining governance decisions open.**
[ADR 0017](../architecture/decisions/0017-gplv3-license.md) records the license;
the [development disclaimer](development-disclaimer.md) states the development
and clinical-validation limits without adding license restrictions.

The repository [gabalint/vibe-tox](https://github.com/gabalint/vibe-tox) is public,
and the presentation website is published from `website/`. Supported software
releases, signing and security-response commitments remain open
([Q-19](../planning/open-questions.md#decision-and-discovery-queue)).

## License choice

| Material | Current terms | Maintainer responsibility |
| --- | --- | --- |
| Original repository code, documentation, synthetic fixtures, website and presentation assets | GPL-3.0-only unless identified otherwise | Preserve the unmodified [LICENSE](../../LICENSE), consistent SPDX metadata and contributor attribution |
| Third-party dependencies, fonts, media and other incorporated material | Their respective terms and notices | Check compatibility and retain required notices/source obligations for the actual distributed artifact |
| Contributions | GPL-3.0-only, from people with the right to grant that license | Follow [contribution guidance](../../CONTRIBUTING.md); no copyright assignment, separate DCO or CLA is adopted |
| Separately developed future hosted-service code | Undecided, Q-11 | Obtain a scope and license decision before development; reuse of covered project material remains subject to its license |

The [license decision and primary
sources](../architecture/decisions/0017-gplv3-license.md#consequences-and-limits)
explain commercial use, private modification, conveying obligations and the
version-3-only grant. Ownership administration, trademarks and any separate
contribution agreement remain Q-11.

Before distributing covered binaries, verify the complete dependency/content
inventory, applicable notices, compatibility and matching corresponding source.
The [library policy](library-policy.md) and [packaging release
criteria](../development/packaging.md#release-readiness) define the checks. A
passing dependency allow-list does not certify the complete release package.

## Local safety and portability

Audit, local backup, restore, complete export, manual report delivery and security
fixes work without a hosted service (INV-16; [scope](../project-scope.md#product-decision)).
Qualified write-once storage and off-host protection for every installation remain
Q-30/Q-31 and [G2-37](../quality/open-items-g1-g2.md).

## Governance and responsibility

| Responsibility | Current status and decision owner |
| --- | --- |
| Maintenance, review and release authority | Name accountable maintainers and their merge, dependency, release and support responsibilities (project owner; Q-12/Q-19) |
| Security reporting | Follow the [security policy](../../SECURITY.md); a private reporting route and response commitments remain unresolved |
| Release identity and support | Decide publisher, signing route, release channel and support window before promising supported releases (Q-19/Q-34) |
| Laboratory acceptance | The laboratory approves its configuration, methods, workflows and each release for its intended use; software publication supplies no clinical certification |
| Contribution and trademark administration | Resolve ownership administration, conduct/review expectations and official-build identification without inventing legal entities or agreements (Q-11) |

Clinical defaults remain governed by laboratory policy and review. Community
contributions, external-service interests or a maintainer's release decision do
not supply laboratory authorization (RISK-33).

## What can be shared

When publication is authorized, share original code and documentation, approved
synthetic fixtures, redistributable interface schemas, reproducible verification
protocols and release evidence. Preserve source attribution and content licenses.
Do not include patient information, credentials, operational keys, restricted
manuals, purchased accreditation text, unlicensed billing terminology or
proprietary vendor binaries without redistribution rights.

Keep source-document and fixture provenance review in each contribution. A
release's corresponding source and validation evidence must be usable without
private working notes or the local review archive.

## Maintenance and external services

The current maintenance scope is the local application: security/dependency
updates, host qualification, analyzer compatibility, migration guidance and
release evidence. No supported-release schedule or service level has been
established (Q-12/Q-34).

Optional hosted services remain separate proposals under
[ADR 0010](../architecture/decisions/0010-optional-cloud-services.md). Their
operator, agreements, infrastructure and operational duties need explicit
decisions before implementation. Contributing to this repository does not
establish a hosted-service operation. Issues, discussions and support bundles
must use synthetic or appropriately redacted material and contain no PHI.
