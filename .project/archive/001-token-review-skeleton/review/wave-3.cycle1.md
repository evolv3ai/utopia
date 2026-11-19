# Review — wave 3, cycle 1

Wave verdict: pass
Cycle: 1
Depth: verify-only
Tasks reviewed: 1

Written by the build orchestrator from the isolated Verify evidence it already holds; no independent reviewer ran. Evidence: the recorded orchestrator Verify of T007 at base `d68e4bbe4d905207d7c2215cceb76910c3e971d2` (`D:\utopia\.git\gsd-path\dispatch\gsd-path-M001\T007\manual-2026-11-19T173828+0000\verify.json`, exit 0, sidecar `gsd-path-verify/task-t007-verify`, its own empty Rust build directory, `pg_dump`/`pg_restore` 16.11 first on PATH) and the landing commit `aab411c133886dac208d39fc7aee57cb32250dbe`.

## T007 — Bump the backup manifest schema version to the number of migration files so a backup made by this build restores: pass

- ✅ T007/AC1 — `git show aab411c -- crates/utopia-cli/src/main.rs`: the one changed line is `const CURRENT_SCHEMA_VERSION: u32 = 114;` (was `113`); `ls migrations/*.sql | wc -l` at the landing is 114. The Verify's first clause compares the literal with that count and passed.
- ✅ T007/AC2 — `grep -rhoE 'CURRENT_SCHEMA_VERSION: u32 = [0-9]+' crates --include='*.rs' | sort -u` gives the single line `CURRENT_SCHEMA_VERSION: u32 = 114` (the Verify's first clause, passed). The planner's and the coder's searches (task Context and Log) found no other hand-written copy of the schema version or the migration count; the restore fixture and the guard test count the files in `migrations/`.
- ✅ T007/AC3 — recorded: `test tests::schema_version_policy_compares_against_current ... ok` and `test result: ok. 15 passed; 0 failed; 0 ignored` for `cargo test -p utopia-cli --bin utopia`, with `UTOPIA_TEST_REQUIRE_DB=1` on the freshly migrated database `m001_t007`. The diff does not touch the test.
- ✅ T007/AC4 — recorded: `test real_postgres_backup_and_restore_require_force_and_preserve_files_on_sql_failure ... ok` and `test result: ok. 13 passed; 0 failed; 0 ignored` for `cargo test -p utopia-cli --test restore`; the recorded output holds no "is newer than this binary's" text; the Verify's `pg_dump --version` and `pg_restore --version` version-16 checks passed.
- ✅ T007/AC5 — `git show --stat aab411c` lists `crates/utopia-cli/src/main.rs` (1 insertion, 1 deletion) and the task file only; `crates/utopia-cli/tests/restore.rs`, the guard test and the restore gate are not in the diff.

Warnings (non-blocking):
- The interface contract is `None`; nothing to check there.
- The landing commit carries no DCO `Signed-off-by` trailer, as in waves 1 and 2.

## Summary for orchestrator

- blocked findings: none
- repeat offenders: none
- warnings worth a human eye: the missing DCO sign-off on pipeline landing commits (already reported in waves 1 and 2).
