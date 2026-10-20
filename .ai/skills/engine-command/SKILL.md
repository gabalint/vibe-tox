---
name: engine-command
description: Implement a state-changing engine command or sensitive query end-to-end (domain function, migration, engine registration, Tauri adapter, TypeScript hook) with authorization, validation, gate rechecks, audit, outbox and PHI-free logging. Use for every new command or query.
---

# Engine command

Every clinical or administrative action follows this one pattern, so that authorization, auditing, logging and transactions are never re-implemented per feature. Read [security, audit and logging](../../../docs/governance/security-audit-logging.md) and the [transaction design](../../../docs/architecture/overview.md#transaction-and-failure-design) first.

## Pipeline

```text
UI hook ─▶ Tauri command (thin) ─▶ Engine method: run(&SPEC) + write ─▶ writer thread: BEGIN IMMEDIATE
                                                                 ├─ ctx.require(capability)       authorize
                                                                 ├─ input → domain types           validate
                                                                 ├─ read gates inside the tx       recheck
                                                                 ├─ apply change (owner's SQL)     change
                                                                 ├─ ctx.audit(event)               audit
                                                                 └─ ctx.enqueue(outbox item)       follow-up
                                                               COMMIT → one central log line
```

The engine pipeline handles the parts below centrally. A command must never do them itself:

- session resolution and lock or idle checks;
- the correlation ID;
- the clock;
- running on the writer thread;
- commit and rollback;
- the completion log line;
- mapping errors to `CommandError`;
- recording an authorization denial in its own transaction.

## Steps

1. **Requirement.** Name the requirement, workflow and invariant IDs, such as `REQ-SPC-002`, `WF-02` or `INV-0x`. Read their acceptance text, and the domain-model rows for the entities you touch.
2. **Schema**, if needed: add a migration to *your* module (`migrations/NNNN_….sql`): STRICT, foreign keys, `CHECK` for enums, `revision` on mutable aggregates, append-only triggers on evidence tables ([coding standards](../../../docs/governance/coding-standards.md#sql-and-migrations)).
3. **Model** (pure): input struct with `#[serde(deny_unknown_fields)]`; parse into domain types (`TryFrom`); state-transition function returning a typed error; unit tests including refused transitions.
4. **Command function** in your domain crate's `commands` module:

   ```rust
   /// Records receipt of a specimen container (REQ-SPC-002).
   pub fn receive_specimen(
       ctx: &mut CommandContext<'_>,
       input: ReceiveSpecimen,
   ) -> Result<SpecimenReceived, SpecimenError> {
       ctx.require(Capability::Accession)?;
       let input = ValidReceipt::try_from(input)?;
       let current = repo::load_for_update(ctx.tx(), input.container_id)?;
       current.ensure_revision(input.expected_revision)?;
       let next = current.receive(&input, ctx.now())?;          // pure transition
       repo::save(ctx.tx(), &next)?;
       ctx.audit(AuditEvent::changed("specimen.receive", next.record_ref(), current.revision, next.revision))?;
       Ok(SpecimenReceived::from(&next))
   }
   ```

   - **Gates.** Every gate the decision depends on is read *inside* this transaction, never from an earlier read (release versus invalidation, NFR-01).
   - **No I/O** in the transaction: no rendering, printing, network or file writes. Enqueue an outbox item, and let a worker do it after commit.
   - **Idempotency.** For replayable inputs (imports, relayed messages), deduplicate on the source identity; the same key with different content is a conflict, not an overwrite.
5. **Engine registration.** Define the command's `CommandSpec` (Tauri name, action name `<module>.<verb>`, required access) and add it to `COMMANDS` in `crates/lis-engine/src/registry.rs`. Add an `Engine` method that runs the domain function through the pipeline (`self.run(&SPEC, …)` with `self.write`, `self.read` or `self.audited_read`). The registry feeds the table-driven authorization test and the shell's capability-file check.
6. **Tauri adapter** (`src-tauri/src/commands/<area>.rs`): one thin async function that takes the typed input, calls the `Engine` method and returns the DTO or `CommandError`. No logic. Register it in the handler list and in the window capability file. These are shared files: use small targeted edits only.
7. **DTO export.** Derive `ts_rs::TS` on input and output DTOs. Decimals and IDs are strings, and timestamps are RFC 3339 strings. Run the crate's tests to regenerate `ui/src/generated/`.
8. **UI hook.** Use `useCommandMutation("receive_specimen", { invalidates: [...] })` or `useCommandQuery`. Never call `invoke` directly.
9. **Tests**, the minimum for each command:
   - success path with the audit event asserted;
   - missing capability refused, with nothing written;
   - each gate's refusal;
   - stale revision causes a conflict;
   - malformed input causes a validation error;
   - replay of an idempotent input has no second effect.

   Safety gates also get a test in both commit orders where invalidation can race ([coding standards](../../../docs/governance/coding-standards.md#tests)).

10. **User manual.** Apply the [user-manual skill](../user-manual/SKILL.md) when a command
    changes user-visible values, defaults, validation, permissions, gates, states or errors.
    Update the affected field/action references in the same change, even if no UI file changes.
    Describe what the screen exposes separately from what the API accepts.

## Queries

Queries run on read connections, call `principal.require(…)` and return DTOs. Sensitive reads (patient or report content, exports, audit) go through the engine's audited-read path, which records an `access` event before returning data. Cross-module reads use the owner's published query functions or `<prefix>_v_*` views.
