# Local build storage

Cargo normally writes build artifacts to the checkout's `target/` directory. Keep a warm cache and reuse it; relocating artifacts is optional and does not change application-data storage.

For Tauri builds, set an **absolute** target directory from the repository root:

```bash
CARGO_TARGET_DIR="$PWD/target" npm run tauri -- build --debug --bundles app -- --locked
```

The Tauri CLI runs Cargo from `src-tauri`, so a relative `CARGO_TARGET_DIR=target/example` can put the build under `src-tauri/target/example` while packaging scripts expect the repository's `target/example`. Use an absolute path consistently in builds and artifact collection. In a coordinated multi-agent wave, use the assigned worker cache as required by the [implementation plan](../planning/implementation-plan.md#fleet-and-coordination).

If build storage is on an external drive, configure its path in the local shell or launch environment, keep that drive mounted throughout the build, and avoid committing workstation-specific paths. GUI-launched tools need the same environment as shell-launched tools. A per-session `TMPDIR` may redirect temporary files when needed; changing global shell startup files is not required by this project.

Moving an existing Cargo cache can leave absolute paths in generated build-script output. If a later build identifies such a stale path, clean only the affected package with `cargo clean -p <package>` using the intended absolute target directory, then rebuild. Do not routinely clean the whole workspace or create a fresh target directory to work around build locks.

Keep each worker cache associated with one source checkout. A copied snapshot and the live checkout must not share workspace artifacts that may describe different source inputs. If a cache is seeded from another checkout, refresh the affected workspace packages while preserving external dependencies, and confirm that new regression tests appear in the workspace test list before accepting the gate.
