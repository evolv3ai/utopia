# Lessons

- 001-token-review-skeleton — A task that adds a migration file must also bump `CURRENT_SCHEMA_VERSION` in `crates/utopia-cli/src/main.rs` (it is the count of migration files) and run `cargo test -p utopia-cli` in its Verify; the first project Verify failed on this and cost a patch wave.
- 001-token-review-skeleton — On this Windows host a Verify must build into its own empty Rust build directory outside the worktree (`CARGO_TARGET_DIR`): a directory shared between worktrees served stale artifacts, and build output inside a verify sidecar blocks its cleanup.
- 001-token-review-skeleton — The project Verify needs `pg_dump` and `pg_restore` 16 on PATH and must run the `utopia-store` tests on their own freshly migrated database; `intake_runs_while_model_work_holds_every_slot` times out behind the jobs the server tests leave queued.
