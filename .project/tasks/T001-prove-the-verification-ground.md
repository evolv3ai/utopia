---
id: T001
title: Prove database-backed verification and per-run database isolation on the compose Postgres
wave: 1
deps: []
status: pending
agent: null
base: null
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
