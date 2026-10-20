# Operations IPC contract index

The [local operations contract](../architecture/operations-contract.md#ownership-and-interfaces) defines authority, native-dialog and recovery boundaries. Exact supported inputs and responses are the engine command signatures and generated types in `ui/src/generated/`; UI feature contracts import those types directly.

| Area | Engine owner | Contract |
| --- | --- | --- |
| Backups, kits, restore, reconciliation and exit export | `crates/lis-engine/src/commands/backup/` | [Backup and recovery](../architecture/operations-contract.md#backup-contract) |
| Audit anchors | `crates/lis-engine/src/commands/anchors.rs` | [Seals, classification and lag](../architecture/operations-contract.md#audit-seals-and-anchors) |
| Host health, runs, settings, updates and key recovery | `crates/lis-engine/src/commands/operations/` | [Host lifecycle](../architecture/operations-contract.md#host-health-and-application-lifecycle), [update records](../architecture/operations-contract.md#update-records) |
| Startup state and close/quit | `src-tauri/src/` | [Startup and native lifecycle](../architecture/operations-contract.md#host-health-and-application-lifecycle) |

Sensitive operation confirmation uses the generated reauthentication type ([ADR 0019](../architecture/decisions/0019-security-key-authentication.md)). Review changed command contracts with their callers, authorization registration and focused tests.
