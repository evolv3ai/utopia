---
id: T007
title: Bump the backup manifest schema version to the number of migration files so a backup made by this build restores
wave: 3
deps: []
status: done
agent: build_T007
base: d68e4bbe4d905207d7c2215cceb76910c3e971d2
worktree: null
task_branch: null
files:
  - crates/utopia-cli/src/main.rs
---

# T007 — Bump the backup manifest schema version to the number of migration files so a backup made by this build restores

## Context

Patch task for final-review finding P001 (`.project/review/PATCH-FINDINGS.md`, reviewed HEAD `c7db84941a3f8bea4f41365c78fd9b9e83434608`), carried verbatim: "**Source**: `.project/review/final-gap-1.md`"; "**Locator**: Risk: project Verify"; "**Evidence**: Project Verify failed at the reviewed commit."; "**Fix direction**: Resolve the recorded command failure before shipping." The source gap review records "Observed: Exit 101; exact stdout and stderr are in the command/commit ledger entry." and the ledger entry (`.project/build/verify-ledger.jsonl`, last line, field `execution`) records the failure verbatim:

```
---- tests::schema_version_policy_compares_against_current stdout ----

thread 'tests::schema_version_policy_compares_against_current' (16536) panicked at crates\utopia-cli\src\main.rs:1121:9:
assertion `left == right` failed: CURRENT_SCHEMA_VERSION must match the count of applied migrations; bump it when you add a migration
  left: 113
 right: 114
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    tests::schema_version_policy_compares_against_current

test result: FAILED. 14 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
```

Cause: T003 added `migrations/0128_a_token_agent_action_is_kept_in_its_own_ledger.sql`, the 114th `.sql` file in `migrations/`, and no task bumped `const CURRENT_SCHEMA_VERSION: u32 = 113;` at `crates/utopia-cli/src/main.rs:85`. The constant is the number of migration files, not the highest number (the Chinese comment above it says so; the numbering has gaps). The same defect makes 11 tests of `crates/utopia-cli/tests/restore.rs` fail with "manifest schema_version 114 is newer than this binary's 113" (orchestrator diagnostic run without fail-fast at the same commit): that fixture writes a manifest whose `schema_version` is the counted number of files in `migrations/` (`restore.rs:84-96`), and `run_restore` refuses a manifest newer than the constant (`main.rs:534`).

## Approach

- The planner searched the whole tree (Rust, web, docs, scripts, fixtures) for every site that encodes the schema version or the migration count. Exactly one hand-written number exists: the constant at `crates/utopia-cli/src/main.rs:85`. Every other site derives its value and needs no change: `main.rs:377`, `:1052`, `:1071`, `:1182` use the constant; the guard test at `main.rs:1088-1126` and the fixture at `crates/utopia-cli/tests/restore.rs:84-96` count the `.sql` files in `migrations/`; `.github/PULL_REQUEST_TEMPLATE.md:23` states the rule without a number. The web app and the docs hold no copy.
- The right value is the number of `*.sql` files in `migrations/` at your base (114 at `c7db84941a3f8bea4f41365c78fd9b9e83434608`). Count them; do not derive the value from the highest file number (0128).
- Keep the constant a hand-written literal. Its doc comment says "bumping is a deliberate decision, not a side effect of a code change"; do not replace it with a computed value, a build script or a macro, and do not weaken or change the guard test or the restore gate (`manifest.schema_version > CURRENT_SCHEMA_VERSION` refuses, `<` warns).
- Do not change `crates/utopia-cli/tests/restore.rs`, any migration, or any file outside `files`. If you find another hand-written copy of the number that the planner's search missed, stop with `NEEDS-ORCHESTRATOR` naming the path; do not edit outside `files`.
- If you touch the comment above the constant, keep it in Chinese and keep its meaning (count, not highest number).
- Host preconditions of the Verify (the orchestrator provides them; they are not product defects and not yours to fix): the compose database container `utopia-db-1` runs and is published on `127.0.0.1:1517` (user and password `utopia`); `pg_dump` and `pg_restore` of PostgreSQL 16 are on `PATH` (the restore test `real_postgres_backup_and_restore_require_force_and_preserve_files_on_sql_failure` calls them against the version 16 server; the orchestrator puts `C:/Users/Owner/.gsd-path/tools/pg16/pgsql/bin` first on `PATH` for Verify runs). The Verify uses its own database `m001_t007`.
- Not in scope: the upstream test `crates/utopia-store/tests/intake_runs_while_model_work_holds_every_slot.rs` fails with "deadline has elapsed" only in a database where the `utopia-server` tests already ran; it failed the same way at the milestone's base. PLAN.md's project Verify now runs the store crate on its own database (Delegated, driver D065). Do not touch that test.
- Conventions (bind this task): explanatory code comments in Chinese; user-facing strings in English; one behaviour-sentence commit subject with a DCO `Signed-off-by`; this task adds no migration and no ADR and changes no public API or data model; nothing is sent to `deeplethe/utopia` or a deployed instance.

## Interface contract

- None

## Intent coverage

- None

## Acceptance criteria

1. `crates/utopia-cli/src/main.rs` declares `const CURRENT_SCHEMA_VERSION: u32 = <n>;` where `<n>` is the number of `*.sql` files in `migrations/` (114 at the reviewed commit), as a literal.
2. No other Rust source under `crates/` declares a `CURRENT_SCHEMA_VERSION: u32` literal with a different value, and no hand-written copy of the schema version or the migration count exists elsewhere in the tree.
3. With `UTOPIA_TEST_REQUIRE_DB=1` against a freshly migrated database, every unit test of the `utopia` binary passes, including `tests::schema_version_policy_compares_against_current`, unmodified.
4. With `UTOPIA_TEST_REQUIRE_DB=1` against the same database and `pg_dump`/`pg_restore` 16 on `PATH`, every test of `cargo test -p utopia-cli --test restore` passes, including `real_postgres_backup_and_restore_require_force_and_preserve_files_on_sql_failure`, and the output holds no "is newer than this binary's" refusal.
5. The diff changes only `crates/utopia-cli/src/main.rs`; the guard test, the restore gate and `crates/utopia-cli/tests/restore.rs` are unchanged.

## Verify

```bash
n=$(ls migrations/*.sql | wc -l | tr -d '[:space:]') && test "$(grep -rhoE 'CURRENT_SCHEMA_VERSION: u32 = [0-9]+' crates --include='*.rs' | sort -u)" = "CURRENT_SCHEMA_VERSION: u32 = $n" && grep -q "^const CURRENT_SCHEMA_VERSION: u32 = $n;" crates/utopia-cli/src/main.rs && pg_dump --version | grep -q ' 16\.' && pg_restore --version | grep -q ' 16\.' && docker exec utopia-db-1 psql -U utopia -d utopia -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS m001_t007 WITH (FORCE)" -c "CREATE DATABASE m001_t007" && export UTOPIA_DATABASE_URL=postgres://utopia:utopia@127.0.0.1:1517/m001_t007 && cargo run -p utopia-store --example migrate && { out=$( { UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-cli --bin utopia && UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-cli --test restore; } 2>&1); rc=$?; printf '%s\n' "$out"; [ "$rc" -eq 0 ]; } && for t in schema_version_policy_compares_against_current real_postgres_backup_and_restore_require_force_and_preserve_files_on_sql_failure; do printf '%s\n' "$out" | grep -Eq "$t \.\.\. ok" || { echo "MISSING $t"; exit 1; }; done && if printf '%s\n' "$out" | grep -q "is newer than this binary's"; then echo "SCHEMA VERSION SKEW"; exit 1; fi
```

Heavy: yes

## Log

- 2026-11-19 — created by planner (patch mode, finding P001)
- 2026-11-19 — build_T007: preflight clean at base d68e4bbe (114 `*.sql` files in `migrations/`; one `CURRENT_SCHEMA_VERSION: u32` literal under `crates/`, at `crates/utopia-cli/src/main.rs:85`; no other hand-written copy found). Implementation: changed the literal `113` to `114` on that line; comment, guard test, restore gate and `tests/restore.rs` untouched.
- 2026-11-19 — build_T007: Verify ran as one piece in Git Bash from `D:\utopia`, text as written, preceded by `export PATH="/c/Users/Owner/.gsd-path/tools/pg16/pgsql/bin:$PATH"` and followed by `; echo "VERIFY_EXIT=$?"`; the guard hook did not refuse it. Result: pass, exit 0. Database `m001_t007` created and migrated; `cargo test -p utopia-cli --bin utopia` 15 passed, 0 failed (`schema_version_policy_compares_against_current ... ok`); `cargo test -p utopia-cli --test restore` 13 passed, 0 failed (`real_postgres_backup_and_restore_require_force_and_preserve_files_on_sql_failure ... ok`); no "is newer than this binary's" in the output.
- 2026-11-19 — orchestrator Verify (sidecar gsd-path-verify/task-t007-verify): pass, exit 0; output tail:
  ```
     Compiling chardetng v1.0.0
     Compiling base64 v0.23.1
     Compiling sha2 v0.11.0
     Compiling utopia-reason v0.1.0 (C:\Users\Owner\.gsd-path\projects\bb0ebb333ad529af\e1cb4cacbe546d13\verify\task-t007-verify\crates\utopia-reason)
     Compiling cron v0.12.1
     Compiling strum v0.27.2
     Compiling pdf-extract v0.12.0
     Compiling utopia-ingest v0.1.0 (C:\Users\Owner\.gsd-path\projects\bb0ebb333ad529af\e1cb4cacbe546d13\verify\task-t007-verify\crates\utopia-ingest)
     Compiling tokio-rustls v0.26.5
     Compiling rustls-platform-verifier v0.6.2
     Compiling hyper-rustls v0.27.9
     Compiling reqwest v0.13.1
     Compiling utopia-llm v0.1.0 (C:\Users\Owner\.gsd-path\projects\bb0ebb333ad529af\e1cb4cacbe546d13\verify\task-t007-verify\crates\utopia-llm)
     Compiling utopia-extract v0.1.0 (C:\Users\Owner\.gsd-path\projects\bb0ebb333ad529af\e1cb4cacbe546d13\verify\task-t007-verify\crates\utopia-extract)
     Compiling sqlx v0.8.6
     Compiling pgvector v0.4.2
     Compiling utopia-core v0.1.0 (C:\Users\Owner\.gsd-path\projects\bb0ebb333ad529af\e1cb4cacbe546d13\verify\task-t007-verify\crates\utopia-core)
     Compiling utopia-store v0.1.0 (C:\Users\Owner\.gsd-path\projects\bb0ebb333ad529af\e1cb4cacbe546d13\verify\task-t007-verify\crates\utopia-store)
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 18s
       Running `C:/Users/Owner/.gsd-path/cargo-target/finish-T007\debug\examples\migrate.exe`
  ```
