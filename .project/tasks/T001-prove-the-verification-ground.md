---
id: T001
title: Prove database-backed verification and per-run database isolation on the compose Postgres
wave: 1
deps: []
status: done
agent: build_T001
base: 5e101654fc91deff9eb3f5a8e7a8bc4983960cfc
worktree: null
task_branch: null
files:
  - scripts/prove-db-isolation.sh
  - crates/utopia-store/tests/a_second_migration_run_changes_no_schema.rs
---

# T001 — Prove database-backed verification and per-run database isolation on the compose Postgres

## Context

Every later task of this program counts as verified only against a migrated Postgres 16 + pgvector with `UTOPIA_TEST_REQUIRE_DB=1`; a plain `cargo test` without a database returns early and proves nothing (`crates/utopia-store/src/test_db.rs`). The settled Verification stack is the compose `db` service (`pgvector/pgvector:pg16`), already running as container `utopia-db-1` and published on `127.0.0.1:1517` (user `utopia`, password `utopia`, default database `utopia`), with one database per Verify run because migrations are embedded per build (`sqlx::migrate!("../../migrations")` in `crates/utopia-store/src/db.rs`) and a database migrated by one build breaks a build with another migration set. None of this was ever executed from this machine (Windows, Git Bash, cargo). This task proves it: the existing token read and token setup tests pass there (the program's proof that ADR 0014 decision 7 holds on this build), and two builds with different migration sets each migrate and test their own database on the one instance.

## Approach

- Deliver one re-runnable script, `scripts/prove-db-isolation.sh <prefix>`, and one Rust test file. The script is the proof; it must fail loudly (`set -euo pipefail`, non-zero exit) on any step that does not hold.
- The database server is the running container `utopia-db-1`. Create databases through it, e.g. `docker exec utopia-db-1 psql -U utopia -d utopia -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS <name> WITH (FORCE)" -c "CREATE DATABASE <name>"`. Do not use `docker compose exec`: a Verify runs in a sidecar worktree whose directory name is not the compose project name. Let the container name, host and port be overridable by environment variables with these defaults. Under Git Bash, guard against path conversion of arguments if it bites (`MSYS_NO_PATHCONV=1`).
- Connection string for a database `<name>`: `postgres://utopia:utopia@127.0.0.1:1517/<name>`. The migration runner is `cargo run -p utopia-store --example migrate` with `UTOPIA_DATABASE_URL` set (`crates/utopia-store/examples/migrate.rs`).
- The existing tests to run are the modules `token_read_tests` (8 tests) and `token_setup_tests` (9 tests) in `crates/utopia-server/src/api/`, with `cargo test -p utopia-server -- token_read_tests token_setup_tests` and `UTOPIA_TEST_REQUIRE_DB=1`. A cargo filter that matches nothing exits 0, so the script must check the test names or counts in the output, not only the exit code.
- "Two builds with different migration sets": build A is the tree as checked out; build B is the same tree plus one extra probe migration with a version above every real one (a harmless idempotent statement). How build B is produced is yours to choose (a temporary file in `migrations/` removed by an exit trap, or a temporary copy of the tree), but the crate has no `build.rs`, so make sure the store crate is really recompiled with the extra migration, and leave `git status --porcelain` empty when the script ends, also on failure.
- The Rust test is a top-level integration test (its own binary, like `crates/utopia-store/tests/a_due_base_is_queued_once.rs`), because it reads schema state that is not owned by one base: see the rule in the header of `crates/utopia-store/tests/store/main.rs`. It uses `utopia_store::test_db::url()` and `utopia_store::db::migrate`.
- While working, run `UTOPIA_TEST_REQUIRE_DB=1 cargo test --workspace` once against a database of your own and record in the Log which tests fail on this machine, if any. This is information for the ship gate (the full database-backed suite has never run here); it is not this task's evidence and must not be added to the Verify.
- The `#[ignore]`d delivery tests stay out.
- Conventions (bind this task): explanatory code comments in Chinese; tests and migrations named as sentences; user-facing error strings in English with a stable `code`; an ADR precedes any public-API or data-model change (ADR 0076, T002) — this task changes neither; new migration numbers start at 0128 and ADR numbers at 0076, re-checked against upstream `deeplethe/utopia` `dev` and its open pull requests before landing — this task adds no numbered file to the tree; migration SQL idempotent; one behaviour-sentence commit subject with a DCO `Signed-off-by`; nothing is sent to `deeplethe/utopia` or a deployed instance.

## Interface contract

- None

## Intent coverage

- SC1

## Acceptance criteria

1. `bash scripts/prove-db-isolation.sh m001_t001`, run from the repository root of a fresh worktree, exits 0 and prints one line `db-isolation: ok`. It exits non-zero when any step below does not hold.
2. The script drops and creates the databases `m001_t001_a` and `m001_t001_b` on the compose instance, so a rerun starts clean.
3. Build A migrates `m001_t001_a` with the migration runner, and with `UTOPIA_TEST_REQUIRE_DB=1` all 8 tests of `token_read_tests` and all 9 tests of `token_setup_tests` report `ok` against it; the script fails if fewer ran.
4. E1 (held-out; ruling: running the migration runner a second time against the same per-run database succeeds and changes no schema): the script runs the migration runner a second time against `m001_t001_a` and it succeeds, and the test `a_second_migration_run_changes_no_schema` in `crates/utopia-store/tests/a_second_migration_run_changes_no_schema.rs` passes against it. That test records the schema (tables, columns, indexes, constraints) and the rows of `_sqlx_migrations`, calls `utopia_store::db::migrate` again, and asserts both are equal afterwards.
5. Build B, whose migration set holds one migration that build A lacks, migrates `m001_t001_b` and passes the 8 `token_read_tests` against it. The script asserts that `_sqlx_migrations` of `m001_t001_b` holds the extra version and `_sqlx_migrations` of `m001_t001_a` does not.
6. After the script ends, with success or failure, `git status --porcelain` in the worktree is empty.
7. The Log names the outcome of one full `cargo test --workspace` run with `UTOPIA_TEST_REQUIRE_DB=1` on this machine (pass, or the failing tests).

## Verify

```bash
test -s crates/utopia-store/tests/a_second_migration_run_changes_no_schema.rs && bash scripts/prove-db-isolation.sh m001_t001
```

Heavy: yes

## Log

- 2026-11-18 — created by planner
- 2026-11-18 — build_T001 preflight: clean (all named paths exist or are declared; SC1 exists; Interface contract None; `token_read_tests` has 8 tests, `token_setup_tests` 9; `utopia-db-1` publishes `127.0.0.1:1517`). Note: at dispatch `git status --porcelain` already showed ` M .project/tasks/T001-prove-the-verification-ground.md` (orchestrator frontmatter), so the script asserts the worktree status is unchanged from its start rather than literally empty.
- 2026-11-18 — build_T001 implementation: wrote `scripts/prove-db-isolation.sh <prefix>` (drops/creates `<prefix>_a`, `<prefix>_b` through `docker exec utopia-db-1 psql`; build A migrates `_a`, runs `token_read_tests` + `token_setup_tests` with `UTOPIA_TEST_REQUIRE_DB=1` and counts `ok` lines per module (min 8 / 9, none not-ok), migrates a second time, runs the new test; build B adds temporary `migrations/9999_a_probe_migration_exists_only_in_build_b.sql` (`SELECT 1;`), touches `crates/utopia-store/src/db.rs` (mtime only) to force the recompile, migrates `_b`, runs `token_read_tests`, asserts version 9999 is in `_b` and not in `_a` and `_b` has exactly one more row; exit trap removes the probe, touches `db.rs` again and fails if `git status --porcelain` differs from the start; container, host, port, user, password, admin database overridable by `UTOPIA_PROVE_DB_*`). Wrote `crates/utopia-store/tests/a_second_migration_run_changes_no_schema.rs` (snapshots tables, columns, indexes, constraints from `pg_catalog` and all `_sqlx_migrations` rows, calls `utopia_store::db::migrate`, asserts both equal). Not done, outside `files`: the header of `crates/utopia-store/tests/store/main.rs` asks for one line per top-level test binary saying why; that file is not in this task's `files`.
- 2026-11-18 — build_T001 full-suite information (not Verify evidence): `UTOPIA_TEST_REQUIRE_DB=1 cargo test --workspace --no-fail-fast` against migrated database `m001_t001_full` (left in place on `utopia-db-1`): exit 101, 2 tests failed, everything else passed (utopia-server 822 passed / 2 ignored; store lib 102 passed / 1 ignored; `store` binary 361 passed; the new `a_second_migration_run_changes_no_schema` 1 passed). Failures: (1) `utopia-cli --test restore` `real_postgres_backup_and_restore_require_force_and_preserve_files_on_sql_failure` — panics `pg_dump is required for database-backed CLI tests; install PostgreSQL client tools` (no `pg_dump` on this machine's PATH); (2) `utopia-store --test intake_runs_while_model_work_holds_every_slot` — `Error: deadline has elapsed` after 10 s, reproduced on a second run alone.
- 2026-11-18 — build_T001 Verify: NOT RUN. BLOCKED: the project's pre-tool guard (`.gsd-path/guard_hook.py`) refuses the Verify command from this coder's shell: `bash reads commands from a file or stdin the guard cannot inspect; run the commands directly or use bash -c '<commands>'`. `bash scripts/prove-db-isolation.sh m001_t001` (and `bash -n`) are therefore unavailable to the coder, and the coder did not route around the guard through another shell. The script has never been executed, not even syntax-checked; only the Rust test has run (passed, in the full-suite run above, after a prior migrate of that database). Needed: the orchestrator runs the Verify through its own runtime, or the guard/Verify form is changed, then redispatch with the result if it fails.
- 2026-11-19 — orchestrator Verify (isolate gsd-path-task/T001): pass, exit 0; output tail:
  ```
  test api::token_read_tests::a_read_token_of_a_base_viewer_gets_200_on_every_graph_read ... ok
  test api::token_read_tests::a_search_by_source_and_external_id_prefix_docs_finds_only_docs_a_md ... ok
  test api::token_read_tests::an_external_id_prefix_docs_underscore_finds_docs_a_md_but_not_docsxa_md ... ok
  
  test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 816 filtered out; finished in 3.22s
  
  token_read_tests: 8 ok
  --- the probe migration is in m001_t001_b and not in m001_t001_a
  m001_t001_a: 113 migrations, m001_t001_b: 114 migrations
  db-isolation: ok
  NOTICE:  database "m001_t001_a" does not exist, skipping
  NOTICE:  database "m001_t001_b" does not exist, skipping
     Compiling utopia-store v0.1.0 (C:\Users\Owner\.gsd-path\projects\bb0ebb333ad529af\e1cb4cacbe546d13\task\T001\crates\utopia-store)
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.83s
       Running `target\debug\examples\migrate.exe`
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s
       Running `target\debug\examples\migrate.exe`
     Compiling utopia-store v0.1.0 (C:\Users\Owner\.gsd-path\projects\bb0ebb333ad529af\e1cb4cacbe546d13\task\T001\crates\utopia-store)
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.91s
       Running `target\debug\examples\migrate.exe`
  ```
