# Library policy

Status: **in force**. Owner: the coordinator, acting for the technical maintainer. This is the direct-dependency allow-list. The root [Cargo manifest](../../Cargo.toml), [npm manifest](../../package.json) and their lockfiles define the installed versions; source checks and admission dates below describe the evidence reviewed, not a continuing advisory clearance.

## Admission rule

A dependency is added only when all of these hold. They extend the [dependency-admission control](engineering-standards.md#ai-assisted-development-controls).

1. **Need.** The current milestone needs it, and the standard library or an approved package cannot do the job in about 100 lines of clear code.
2. **Identity.** The package exists on the expected registry under the expected publisher. Check the name and owner, not just that the install succeeds.
3. **Health.** It is actively maintained, has no open advisory, has a license on the [`deny.toml`](../../deny.toml) allow-list, and brings a modest transitive tree.
4. **Pinning.** Rust crates are declared once in the root `[workspace.dependencies]` and used with `{ workspace = true }`. npm packages are saved with an exact version (`npm install --save-exact`). Lockfiles are committed.

**To request a package**, a worker stops and reports `BLOCKED: dependency request` with the need, the alternatives considered (std, approved packages, own code), license, maintenance evidence and footprint. The coordinator decides and records the result here. Adjusting features or patch versions of an approved crate in the root manifest is allowed, with a note in the handoff.

The project's own workspace packages use GPL-3.0-only ([ADR 0017](../architecture/decisions/0017-gplv3-license.md)). `deny.toml` permits that license only through named exceptions for those packages; GPL is not on the external-dependency allow-list. A passing license check does not replace the release license inventory or corresponding-source review ([engineering standards](engineering-standards.md#release-and-maintenance)).

## Rust crates

| Crate | Version | Purpose | Used by |
| --- | --- | --- | --- |
| `rusqlite` (`bundled`, `hooks`) | 0.40.2 | SQLite access with the same SQLite (3.53.2) on every OS; explicit SQL. `hooks` provides the authorizer that guards transaction control | store, domain crates |
| `rust_decimal` | 1.43 | Exact decimal comparison and arithmetic behind lexical decimal types | core |
| `jiff` | 0.2.38 | Instants, offsets and IANA time zones; the one datetime library | core and up |
| `uuid` (`v7`) | 1.28 | Time-ordered record identifiers | core |
| `serde`, `serde_json` | 1.0 | Serialization of DTOs, audit details and outbox payloads | all |
| `thiserror` | 2.0 | Typed error enums | all |
| `ts-rs` | 12.0 | TypeScript bindings generated from Rust DTOs (`ui/src/generated`) | crates that own IPC DTOs |
| `tracing`, `tracing-subscriber`, `tracing-appender` | 0.1 / 0.3 / 0.2 | Structured diagnostic logging to rotating JSON files | store, interfaces, ops, engine, security-key, shell; subscriber setup in the shell only (tests excepted) |
| `sha2`, `hex` | 0.11 / 0.4 | SHA-256 for the audit chain, content-addressed artifacts and record digests | audit, store, report and domain crates |
| `argon2` | 0.6 | Argon2id password hashing | auth |
| `totp-rs` | 6.0 | RFC 6238 TOTP verification | auth |
| `webauthn-rs` | =0.5.5 | Maintained registration/assertion verification: `Passkey` for required verification; `SecurityKey` for the explicit laboratory touch-only opt-in in ADR 0019. Preserve existing credential verification history; sensitive actions require verified assertions. No custom signature verification. | auth |
| `webauthn-rs-proto` | =0.5.5 | Protocol structures shared by the engine and native authenticator; keep the same exact version as the verifier/client | authentication boundary |
| `webauthn-authenticator-rs` | =0.5.5 | Native security-key client; default features disabled, `usb` on macOS/Linux and `win10` on Windows. No browser/loopback server, elevated HID fallback, soft tokens, synced or platform credentials in the shipped application; the `softpasskey` feature is enabled only in auth and engine dev-dependencies for tests. | security-key |
| `url` | =2.5.8 | Parse the fixed native WebAuthn origin using the upstream client's URL type; no network listener. | native authentication |
| `ciborium` | =0.2.2 | Decode bounded attestation data to reconstruct upstream USB credential IDs and validate user-presence/user-verification framing; backup-state rejection belongs to the engine verifier. No custom CBOR parser. | native authentication |
| `async-trait` | =0.1.92 | Implement the upstream asynchronous authenticator trait for cancellable USB calls. | native authentication |
| `subtle` | 2.6.1 | Constant-time comparison for the first-setup recovery-code acknowledgment, using the existing library rather than custom comparison. | engine |
| `openssl` (`vendored`) | =0.10.81 | Select static vendoring of the verifier/native client's existing transitive OpenSSL dependency, avoiding an unmanaged runtime DLL/dylib. No custom cryptography; lockfile pins `openssl-src` 300.6.1+3.6.3 and security patches require rebuilding the app | auth feature anchor; verifier/native client |
| `secrecy`, `zeroize` | 0.10 / 1.9 | Secret wrappers and memory wiping | auth, engine, ops, reporting, security-key, shell |
| `rand` | 0.10 | OS-backed randomness for secrets and salts | auth, ops |
| `keyring` | 4.2 | OS key stores (Credential Manager, Keychain, Secret Service) | engine |
| `tokio` | 1.53 | Async runtime for listeners and workers (already required by Tauri) | store, engine, security-key, shell |
| `serialport` | 4.10 | Serial analyzer connections | interfaces |
| `typst`, `typst-pdf`, `typst-assets` (`fonts`) | 0.15.1 | Embedded report rendering with bundled fonts; pinned and wrapped in `lis-report` | report |
| `tauri`, `tauri-build` | 2.12 / 2.7 | Desktop shell | shell |
| `tauri-plugin-single-instance` | 2.5 | One running instance per database (REQ-OPS-013) | shell |
| `unicode-normalization` | 0.1.25 | NFC comparison keys for identity matching (already a transitive dependency) | identity |
| `csv` | 1.4 | RFC 4180 reading of laboratory result files, with our own size, row, column and field limits around it (already a transitive dependency through Typst). See the [result files contract](../planning/result-files-design-brief.md) | results |

Dev-only: `proptest` 1.12 (property tests), `tempfile` 3.27 (temporary databases and directories), `insta` 1.49 (snapshots of codec and render outputs only, never of UI DOM; declared in the workspace, not yet used by any crate). Exception: `lis-store` may depend on `tempfile` optionally, behind its `testing` feature, which other crates enable only from `[dev-dependencies]`.

### Additional installed crates

| Package | Purpose | Used by |
| --- | --- | --- |
| `tauri-plugin-dialog` 2.8 | Native save/open dialogs initiated by Rust. The UI sends no filesystem path and receives no `dialog:` or `fs:` permission. Its transitive filesystem plugin is not registered or granted. | Desktop shell |
| `keepawake` 0.6, default features off | Sleep prevention while listening (REQ-OPS-013) | Engine |
| `fs4` 1.1, `sync` feature only | Free/total disk space for backup preflight and host health (REQ-OPS-008) | Operations |
| `chacha20poly1305` 0.11 | Authenticated encryption for secrets and backup packages | Authentication, operations |
| `qrcode` 0.14, `svg` feature only | Authenticator enrollment QR code | Authentication |

Windows durable artifact replacement remains an open implementation decision (G2-11). Direct `windows-sys` filesystem calls would require a reviewed exception to the unsafe-code policy; this document does not admit them. Folder backups use the built-in `vtbk1` format. A third-party backup sidecar or write-once bucket backend requires a scoped decision under Q-30/Q-31 and dependency review.

## npm packages

**Runtime:**

| Package | Version | Purpose |
| --- | --- | --- |
| `react`, `react-dom` | 19.3 | UI library |
| `react-aria-components` | 1.22 | Accessible, unstyled primitives under the design system |
| `@tanstack/react-query` | 5.x | Engine state: caching, loading and error states, invalidation |
| `@tanstack/react-table`, `@tanstack/react-virtual` | 9.x / 3.x | `DataGrid` logic and virtualization |
| `react-router` | 8.x | Routing inside the window |
| `react-hook-form` | 7.x | Form state |
| `@tauri-apps/api` | 2.12 | IPC and events, used only in `ui/src/lib/` |
| `lucide-react` | 1.55 | Icons (tree-shaken) |

**Dev:**

| Package | Version | Purpose |
| --- | --- | --- |
| `typescript` | 7.0.2 | Type checking; used with Vite, Vitest, Biome and knip |
| `@types/react`, `@types/react-dom` | 19.3 | React type declarations for type checking |
| `@testing-library/dom` | 10.x | Required peer of `@testing-library/react`; installed through the lockfile, not listed in `package.json` |
| `vite`, `@vitejs/plugin-react` | 8.x / 6.x | Bundler and dev server |
| `@tauri-apps/cli` | 2.12 | Desktop shell build |
| `@biomejs/biome` | 2.5 | Lint and format, the only JS/TS linter and formatter |
| `vitest` | 5.x | Unit and component tests |
| `@testing-library/react`, `@testing-library/user-event` | 16.x / 14.x | Component tests |
| `jsdom` | 30.x | Test DOM |
| `axe-core` | 4.14 | Automated accessibility checks |
| `jscpd` | 5.4 | Duplicate-code detection for TS, CSS and Rust |
| `knip` | 6.x | Unused files, exports and dependencies |

Transitive advisories that are accepted, with their reasons and revisit triggers, are recorded once in [`deny.toml`](../../deny.toml).

### Security-key dependency admission — 2026-10-15

The three `webauthn` packages above are admitted for [ADR 0019](../architecture/decisions/0019-security-key-authentication.md). The registry identifies all three 0.5.5 releases as published by `Firstyear` on 2026-04-30, with MPL-2.0 licenses and Rust 1.88 minimums: [verifier metadata](https://crates.io/api/v1/crates/webauthn-rs/0.5.5), [native-client metadata](https://crates.io/api/v1/crates/webauthn-authenticator-rs/0.5.5), [protocol metadata](https://crates.io/api/v1/crates/webauthn-rs-proto/0.5.5), checked 2026-10-15. MPL-2.0 is permitted by `deny.toml`; retain the dependencies' notices and applicable source obligations in distributed artifacts.

The native client is a pre-1.0 dependency without an established thorough security review; the relying-party library's upstream audit statement does not cover it. Admission permits implementation; the lockfile advisory/license gate, transitive footprint and real-device/OS qualification must still be reported. Enable only the target-specific transports above: the USB path brings CTAP2/OpenSSL and HID support, while the Windows path uses the native Windows API. Do not add Bluetooth, NFC, CLI prompting or software-authenticator features to the shipped application without another review.

The touch-only policy uses the same pinned public verifier APIs, with separately represented optional credentials and unchanged `Passkey` records; it adds no dependency or credential-internals feature. Only the trusted engine selects optional verification; required ceremonies remain strict and a failed PIN never causes a presence-only retry ([authentication research](../research/security-key-authentication.md#laboratory-touch-only-policy)). The client requests personal external hardware, and the engine rejects backup-eligible or backed-up credentials; this is not a manufacturer-attestation catalog or certification of every device. `ctap-hid-fido2` is not admitted: its Windows administrator-only HID access conflicts with unprivileged LIS operation.

Companion registry checks on 2026-10-15 confirmed: [`url` 2.5.8](https://crates.io/api/v1/crates/url/2.5.8), published by Manishearth on 2026-01-05, MIT/Apache-2.0; [`ciborium` 0.2.2](https://crates.io/api/v1/crates/ciborium/0.2.2), published by rjzak on 2024-01-24, Apache-2.0; [`async-trait` 0.1.92](https://crates.io/api/v1/crates/async-trait/0.1.92), published by dtolnay on 2026-08-08, MIT/Apache-2.0; and [`subtle` 2.6.1](https://crates.io/api/v1/crates/subtle/2.6.1), published by isislovecruft on 2024-06-24, BSD-3-Clause. Admission is scoped to the purposes above.

## Tools

| Tool | Purpose |
| --- | --- |
| `cargo-deny` 0.20 | Licenses, advisories, bans and sources (`deny.toml`) |
| `cargo-machete` | Unused Rust dependencies |
| `rustfmt`, `clippy` | Pinned through `rust-toolchain.toml` |
| Python 3.11+ standard library | Change selection, repository metadata/link checks and their regression tests; no Python packages or application runtime dependency. Shares one selector between local work and CI. |
| `actionlint` 1.7.12 | Development-only GitHub Actions syntax/expression checks, using the maintainer's versioned Docker image in CI; no app dependency. License: [MIT](https://github.com/rhysd/actionlint/blob/v1.7.12/LICENSE.txt), checked 2026-10-14; maintained release and interface evidence: [ENG-279](../research/sources-engineering.md#change-sensitive-verification-sources-2026-10-14). |
| Python 3, FFmpeg/ffprobe, cwebp | Local tools for previewing the static website and preparing its media accessibility assets and application stills; not shipped as an LIS or website runtime dependency. See [website maintenance](../development/public-website.md). |

## Rejected or avoided

| Avoid | Reason | Use instead |
| --- | --- | --- |
| ORMs and query macros (`diesel`, `sea-orm`, `sqlx` macros) | Explicit SQL is required; decimal mapping hazards (ENG-80/102) | `rusqlite` with explicit SQL |
| Other decimal or float libraries (`bigdecimal`, `decimal.js`, `big.js`) | One exact decimal type; the UI never computes | `rust_decimal` behind `lis-core` types |
| `chrono`, `time` | One datetime library | `jiff` |
| `anyhow`, `eyre` | Typed errors are required | `thiserror` |
| `log` macros, `env_logger` | One logging facade | `tracing` |
| `lazy_static`, `once_cell`, `parking_lot`, `crossbeam` | The standard library or `tokio` covers them | `std::sync::{LazyLock, OnceLock, Mutex}`, `tokio::sync` |
| HTTP clients (`reqwest` and others) | The local application makes no HTTP calls; optional hosted services ([ADR 0010](../architecture/decisions/0010-optional-cloud-services.md)) need their own scope decision | Not needed |
| Redux, Zustand, MobX, Jotai | Engine state lives in TanStack Query and local state in React | Command hooks, `useState` |
| MUI, Ant, Chakra, Mantine, shadcn/ui, Tailwind, CSS-in-JS | One design system, minimal tooling | `react-aria-components` with CSS Modules and tokens |
| moment, dayjs, date-fns, lodash, ramda, axios | Not needed: `Intl`, built-ins, and no HTTP from the UI | Platform APIs |
| ESLint, Prettier | One tool | Biome |
| Tauri `shell`, `fs`, `http` plugins | Not exposed to the UI (NFR-31) | Engine commands |
