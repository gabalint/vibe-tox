# Security policy

The application is in development. There are no supported software releases, operated production services, or published response commitments yet. Code-level security rules are in [security, audit and logging standards](docs/governance/security-audit-logging.md).

## Reporting a vulnerability

Report vulnerabilities privately through GitHub: open the repository's **Security** tab and choose [Report a vulnerability](https://github.com/gabalint/vibe-tox/security/advisories/new). Do not open a public issue for a vulnerability.

Never include credentials, patient information, real laboratory exports or real requisitions in a report, an issue or a pull request, and do not publish exploit details affecting a live system. Use synthetic data in a reproduction. If a report cannot be made without sensitive material, say so in the private report and wait for an authorized handling environment to be arranged.

Responsible response roles and response-time commitments are not yet established (Q-19 in the [open questions](docs/planning/open-questions.md#decision-and-discovery-queue)); reports are reviewed on a best-effort basis.

## Scope

The application runs on a laboratory-controlled computer. Its security boundaries include per-person application accounts and permissions, imported files and analyzer traffic, the desktop renderer's access to the engine, recovery integrity, and protection against accidental disclosure in logs and exports. Please describe the affected boundary.

The computer's owner or operating-system administrator can inspect and change the process and its files. The application does not claim to prevent that authority or make a locally editable database tamper-proof. This limitation does not remove the need to enforce application permissions or safely handle external input. Host security and physical access remain laboratory responsibilities.

Discuss non-sensitive design issues through [GitHub issues](https://github.com/gabalint/vibe-tox/issues). Responsible maintainers and a coordinated disclosure process must be established before the first supported software release.

The proposed security architecture is in [security and privacy](docs/architecture/security-and-privacy.md), and operational release requirements are in the [validation plan](docs/quality/validation-plan.md).
