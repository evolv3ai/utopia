# Evidence — codebase

Repo root: D:\utopia
Scanned: 2026-11-18
Checks run: all in the disposable sidecar `C:\Users\Owner\.gsd-path\projects\bb0ebb333ad529af\e1cb4cacbe546d13\verify\inspect-codebase` at audited HEAD `3d307f6236904a12ae50e2f31f0d1d1f02f3bc25` (local toolchain: cargo/rustc 1.99.1, node v26.13.1, pnpm 10.14.0, pdftotext 4.06 xpdf; no `psql`; no `UTOPIA_*` env vars set; no `.env`):
- `cargo fmt --all --check` → exit 0, no output
- `cargo test --workspace --no-fail-fast` (no database) → exit 0; 47 test binaries, 1863 passed, 0 failed, 6 ignored; compiled in 2m 36s. Database-backed tests return early without `UTOPIA_DATABASE_URL`, so this count includes tests that did not exercise Postgres (see findings)
- `pnpm install --frozen-lockfile` (in `web/`) → exit 0 (pnpm warned: ignored build script `esbuild`)
- `pnpm test` (in `web/`, vitest) → exit 0; 54 files, 355 tests passed
- `node --test ../scripts/bench/lib.test.mjs` (from `web/`) → exit 0; 15 pass, 0 fail
- `pnpm build` (in `web/`: style guard + `tsc --noEmit` + `vite build`) → exit 0; one JS chunk 1,865.93 kB (560.97 kB gzip), Vite >500 kB chunk warning
- `git status --short` in the sidecar after the runs → clean (build outputs are git-ignored)
- Not run: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build --workspace`, `sqlx migrate run`, `cargo run -p utopia-store --example migrate`, `scripts/smoke.sh`, Docker image build, any Postgres-backed test run — `unverifiable` here (no Postgres started in the sidecar; migration commands are excluded by role)

## Map

- **Stack**: Rust 2021 Cargo workspace (resolver 2), 9 crates, all `version = "0.1.0"`, Apache-2.0 (`Cargo.toml:1-19`). Backend: tokio 1, axum 0.8, tower-http 0.6, sqlx 0.8 (postgres + mysql features, compile-time `sqlx::migrate!`), pgvector 0.4, tantivy 0.26 + tantivy-jieba, reqwest 0.13 (rustls), jsonwebtoken 10, argon2 0.5, aes-gcm 0.10, figment + dotenvy config, oxttl/oxrdfxml RDF parsers (`Cargo.toml:21-105`). No `rust-toolchain` file. Database: PostgreSQL 16 with pgvector (`docker-compose.yml`, image `pgvector/pgvector:pg16`); 113 SQL migrations in `migrations/` numbered 0001–0126 with gaps. Web: React 19, TypeScript 5.7, Vite 6, Tailwind 4, TanStack Router/Query, sigma/graphology, vitest 5.0.0, `packageManager: pnpm@10.2.1` (`web/package.json`). Tooling scripts are Node `.mjs`, plus two Python scripts and shell scripts under `scripts/`. Container: 3-stage `docker/Dockerfile` (node:20-slim → rust:1-slim-bookworm → debian:bookworm-slim with poppler-utils).
- **Entry points**: `utopia-server` binary — `crates/utopia-server/src/main.rs:102` (`#[tokio::main] main`); `utopia` operator CLI binary — `crates/utopia-cli/src/main.rs:88` with subcommands `backup` and `restore` (`main.rs:108-113`); `crates/utopia-store/examples/migrate.rs` (migration runner used by CI); web SPA `web/src/main.tsx` + `web/src/router.tsx`; HTTP router `crates/utopia-server/src/api/mod.rs:102` (`pub fn router`, 162 `.route(` calls in that file, `/health`, `/auth/*`, `/workspaces`, `/alerts`, knowledge-base routes, MCP endpoint in `api/mcp.rs`). Default bind `0.0.0.0:1516`; compose maps Postgres to `127.0.0.1:1517`.
- **Architecture**: Layered crates: `utopia-core` (config, `AppError`, models, secret sealing) ← `utopia-store` (all Postgres access, job queue, 84k lines incl. tests) ← `utopia-server` (axum API, job dispatch, source connectors, 86k lines incl. tests); side crates `utopia-ingest` (file parsing, chunking), `utopia-extract` (LLM extraction/alignment logic), `utopia-reason` (rule/derivation engine, no I/O deps), `utopia-search` (Tantivy index), `utopia-llm` (HTTP model client). On startup the server loads config, runs migrations on a separate short-lived pool (optionally a separate migration role), opens the runtime pool, initialises the credential-sealing key, opens/rebuilds the Tantivy index under `data/index`, resolves the JWT secret, then spawns a Postgres-backed job worker plus timer loops (source sync and inference every 60 s, governance hourly, optional daily vector pruning) before serving HTTP (`main.rs:111-330`). Background work is a string-keyed job dispatch with about 25 job kinds (`main.rs:389-700`: `process_document`, `extract_document`, `sync_source`, `align_types`, `govern`, `materialize_inferences`, …). The server also serves the built web bundle from `UTOPIA_WEB_DIST` and embeds `web/src/docs/*.md` at compile time (`docker/Dockerfile` comment on `docs_corpus.rs`).
- **Conventions**: Code comments are predominantly Chinese, long, and explain why with issue/ADR numbers; user-facing error strings are English with stable `code` values for client-side i18n (`crates/utopia-core/src/error.rs:1-36`). Errors: `thiserror` `AppError`/`AppResult` at API boundaries, `anyhow` internally. Tests: unit tests in sibling `*_tests.rs` files beside the module (server `src/`), integration tests as one sentence-named file per behaviour (`crates/utopia-store/tests/store/` has 141 files such as `a_cycle_is_keyed_by_all_its_facts.rs`); migrations use the same sentence naming (`0126_a_superseded_chunk_is_found_by_its_age.sql`). Web tests are pure-logic vitest files `src/**/*.test.ts` in a node environment (`web/vite.config.ts`); a style guard script gates `pnpm build` (`web/scripts/style-guard.mjs`). Commits: one squash commit per PR, subject is a full sentence describing behaviour and ends with `(#NNNN)` (100 of the last 100 subjects); bodies carry `Signed-off-by` (205 occurrences in the last 100 commit bodies); DCO bot configured in `.github/dco.yml`. CI (`.github/workflows/checks.yml`) runs on push to `main`/`dev` and on PRs: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with Postgres and `UTOPIA_TEST_REQUIRE_DB=1`, `cargo build --workspace`, duplicate-migration-number check plus `sqlx migrate run` twice, and web `pnpm test` / bench scoring test / `pnpm build`.
- **Maturity**: Working product with broad test coverage: 1871 `#[test]`/`#[tokio::test]` attributes in `crates/`, 54 vitest files, all passing locally without a database. No `todo!()`/`unimplemented!()` in `crates/`; 0 `TODO|FIXME|XXX|HACK` matches in `crates/` and `web/src`; one `#[allow(dead_code)]` (`crates/utopia-server/src/blob.rs:19`). Version is still `0.1.0` across the workspace and the compose default image is a release candidate tag (`ghcr.io/deeplethe/utopia:0.1.0-rc9`). No local git tags. `scripts/bench/` is a large measurement harness (corpora, truth files, runners) that is not part of CI except `lib.test.mjs`.
- **Recent activity**: 1039 commits since 2026-08-27; authors by commit count: WaylandYang 640 (+8 as "Wayland Yang"), Maya Kid 130, Szabo Bogdan 45, Rudy Mizrahi Celekli 40, Daniel 35, Royce 29, others fewer. Paths touched in the last 200 commits: `crates/utopia-server` 292, `crates/utopia-store` 218, `web/src` 203, `docs/decisions` 79, `docs/design` 35, `crates/utopia-cli` 20. The last 30 commits (2026-11-07 → 2026-11-18) are small behaviour fixes: source sync (GitLab, Google Drive, RSS, repository sync, checkpoints), value parsing (dates, currency), knowledge-base scoping of lookups, re-push/supersession handling, pagination in the web UI, and per-task model choice (ADR 0067).

## Finding: A local `cargo test` without a database is green but skips the Postgres-backed tests

- **Claim**: Database-backed tests return early (pass) when `UTOPIA_DATABASE_URL` is unset; they fail instead only when `UTOPIA_TEST_REQUIRE_DB` is set. `test_db::url()` is called at 408 sites, and `UTOPIA_DATABASE_URL`/`test_db` is referenced in 223 `.rs` files.
- **Source**: `crates/utopia-store/src/test_db.rs:13-31`; `grep -rnoE "test_db::url\(\)" crates --include=*.rs | wc -l` → 408; `cargo test --workspace --no-fail-fast` → 1863 passed with no database present; `.github/workflows/checks.yml` test job sets `UTOPIA_TEST_REQUIRE_DB: "1"`
- **Confidence**: high
- **Why it matters here**: A task Verify that runs `cargo test` without a migrated Postgres and `UTOPIA_TEST_REQUIRE_DB=1` reports success without executing store or most server integration behaviour.

## Finding: The test database must be migrated before tests run

- **Claim**: Most database tests do not run migrations themselves; CI migrates first with `cargo run -p utopia-store --example migrate` and then runs `cargo test --workspace` against the same database.
- **Source**: `crates/utopia-store/src/test_db.rs:10-11`; `.github/workflows/checks.yml` test job step "迁移测试库" and its comment citing #869
- **Confidence**: high
- **Why it matters here**: Database-backed verification needs a two-step setup; running tests against an empty database fails with missing relations rather than skipping.

## Finding: Migration numbers are non-contiguous and embedded at compile time

- **Claim**: `migrations/` holds 113 files numbered 0001–0126; numbers 0081–0089, 0107, 0108, 0112, and 0124 are absent. Migrations are embedded with `sqlx::migrate!("../../migrations")` and run by the server at every start. CI rejects duplicate version prefixes and replays the migration set twice.
- **Source**: `ls migrations | cut -d_ -f1` gap scan; `crates/utopia-store/src/db.rs` (`migrate`); `crates/utopia-server/src/main.rs:116-124`; `.github/workflows/checks.yml` migrations job
- **Confidence**: high
- **Why it matters here**: A new migration takes the next number after 0126; whether the missing numbers are reserved or were removed is not determinable from the tree. Any migration change requires a server rebuild.

## Finding: The backend build depends on files under `web/`

- **Claim**: `utopia-server` compiles `web/src/docs/` content into the binary via `include_str!` in `docs_corpus.rs`, so the Rust build reads files outside `crates/`.
- **Source**: `docker/Dockerfile` (`COPY web/src/docs/ web/src/docs/` and its comment); `crates/utopia-server/src/docs_corpus.rs`; `web/src/docs/ingest.md`, `web/src/docs/mcp.md`
- **Confidence**: high
- **Why it matters here**: A task scoped to `web/` that renames or removes these files breaks `cargo build`; file-scope separation between web and backend tasks is not clean here.

## Finding: `web/tests/*.test.mjs` are not run by `pnpm test` or CI

- **Claim**: Seven browser tests in `web/tests/` use `node:test` with an externally installed `playwright-core` and a Chromium path supplied by env vars; vitest includes only `src/**/*.test.ts`, and no workflow references `web/tests`.
- **Source**: `web/tests/chat-view.test.mjs:1-11` (header comment: "separate from pnpm test (Vitest) and CI"); `web/vite.config.ts` `test.include`; `grep -rn "web/tests" .github web/package.json` → no matches
- **Confidence**: high
- **Why it matters here**: These tests are on-demand only; their pass/fail state at HEAD was not established by any check run here.

## Finding: The local toolchain differs from CI's

- **Claim**: CI uses Node 20 and the pnpm version from `packageManager` (10.2.1) on Ubuntu with Poppler `pdftotext`; this machine has Node 26.13.1, pnpm 10.14.0, Windows, and xpdf `pdftotext` 4.06. All checks run here still passed.
- **Source**: `.github/workflows/checks.yml` web job (`node-version: 20`), `web/package.json` `packageManager`; `node --version`, `pnpm --version`, `pdftotext -v` in the sidecar
- **Confidence**: high
- **Why it matters here**: Local green is evidence for this environment, not for CI's; the PDF fallback test (`crates/utopia-ingest/tests/pdf_gbk.rs:101`) only becomes mandatory when `UTOPIA_TEST_REQUIRE_PDFTOTEXT` is set.

## Finding: `origin` is a fork; the manifests and compose file name another repository

- **Claim**: `origin` is `https://github.com/evolv3ai/utopia` with default branch `dev` and no `main` branch on the remote (heads: `dev` and three `fix/*` branches), while `Cargo.toml` `repository` and the compose default image point at `deeplethe/utopia`. CI triggers on pushes to `main` and `dev`.
- **Source**: `git remote -v`; `git ls-remote --heads origin`; `git branch -a` (`origin/HEAD -> origin/dev`); `Cargo.toml:18`; `docker-compose.yml` app image; `.github/workflows/checks.yml` `on.push.branches`
- **Confidence**: high
- **Why it matters here**: The pipeline integrates onto `origin`'s default branch (`dev`) of the fork; nothing in the tree says how or whether work flows to the `deeplethe` repository.

## Finding: Pipeline control files are excluded locally, not tracked

- **Claim**: `AGENTS.md`, `WORKFLOW.md`, and `.gsd-path/` are ignored through `.git/info/exclude` in the primary checkout; `.claude/` is ignored by the tracked `.gitignore`. None of them exist in the tracked tree at HEAD, and the sidecar worktree does not contain them.
- **Source**: `git check-ignore -v AGENTS.md WORKFLOW.md .gsd-path .claude` in `D:\utopia` → `.git/info/exclude:11-13`, `.gitignore:18`; `ls -la` of the sidecar root
- **Confidence**: high
- **Why it matters here**: These files are per-clone state; a fresh clone or linked worktree lacks them, and they cannot be committed without changing the ignore rules.

## Finding: `docs/` is mostly git-ignored by design

- **Claim**: `.gitignore` ignores `/docs/*` except `docs/decisions/`, `docs/design/`, and `docs/pipeline.md`; 85 files are tracked there (70 ADRs incl. README, 14 design documents, `pipeline.md`). ADR numbering reaches 0075 with gaps (for example no 0068).
- **Source**: `.gitignore` (`/docs/*` block); `git ls-files docs`
- **Confidence**: high
- **Why it matters here**: Any new document placed elsewhere under `docs/` is silently untracked; `.project/` is not ignored and appears as untracked in the primary checkout.

## Finding: `utopia-cli` exists as a workspace crate alongside a proposal file describing it

- **Claim**: `.roadmap-proposals/` contains one tracked proposal for backup and restore commands as a separate `utopia-cli` crate; that crate is present in the workspace (2196 lines, `backup`/`restore` subcommands, 28 tests passing) and 20 of the last 200 commits' file touches are in it. The Dockerfile builds and ships only `utopia-server`.
- **Source**: `.roadmap-proposals/` (first 40 lines read); `crates/utopia-cli/src/main.rs:108-113`; `cargo test` output (15 unit + 13 `tests/restore.rs`); `docker/Dockerfile` (`cargo build --release -p utopia-server`, single `COPY --from=build`)
- **Confidence**: high for presence and tests; medium for "not shipped in the image" (release workflow read only to line 80)
- **Why it matters here**: The CLI is implemented and tested but its distribution path is not visible in the container build.

## Finding: Two crates carry most of the code, in a few very large files

- **Claim**: `utopia-server` (158 `.rs` files, ~86k lines) and `utopia-store` (224 `.rs` files, ~84k lines) hold about 170k of roughly 203k Rust lines; individual files exceed 3000 lines (`utopia-store/src/resolution.rs` 3371, `reasoning.rs` 3285, `server/src/api/mcp_tests.rs` 3848). All server modules are private `mod` declarations in the binary crate — there is no `lib.rs` for `utopia-server`.
- **Source**: `find crates/<c> -name '*.rs' | wc -l` and `wc -l` per crate; `crates/utopia-server/src/main.rs:1-53`
- **Confidence**: high
- **Why it matters here**: Server code cannot be imported by another crate or by `tests/` integration tests; server tests live inside `src/`. Parallel tasks touching the same large files will collide on file scope.

## Finding: Six tests are permanently ignored and several more need live external services

- **Claim**: Six tests carry `#[ignore]` (live Ark OCR API, live HTTPS RSS, and four that need a dedicated idle database). Query-engine tests for MySQL, Trino, and ClickHouse skip unless `UTOPIA_TEST_MYSQL_URL`, `UTOPIA_TEST_TRINO_URL`, or `UTOPIA_TEST_CLICKHOUSE_URL` are set; CI sets none of them.
- **Source**: `grep -rn "#\[ignore" crates --include=*.rs` (6 hits, listed with reasons in `cargo test` output); `crates/utopia-server/src/query_engine/mysql.rs:422-431`, `trino.rs:212`, `clickhouse.rs:593-703`; `.github/workflows/checks.yml` test job env
- **Confidence**: high
- **Why it matters here**: Lakehouse/warehouse connectors (MySQL, Trino, ClickHouse, Snowflake, Databricks under `query_engine/`) have no live-service coverage in CI; changes there are verified only by mocked or unit tests.

## Finding: The web bundle is a single ~1.87 MB JavaScript chunk

- **Claim**: `pnpm build` emits one `index-*.js` of 1,865.93 kB (560.97 kB gzip) and Vite prints its >500 kB chunk warning; there is no code splitting configured.
- **Source**: `pnpm build` output in the sidecar; `web/vite.config.ts` (no `build` options)
- **Confidence**: high
- **Why it matters here**: Observation only; the build exits 0 with the warning.

## Apparent intent

- Inference: the project is a self-hosted knowledge-base server that ingests documents and external sources, extracts a typed, time-aware fact graph under an ontology with human review and governance, and answers questions with citations through chat and MCP — based on the job kinds in `crates/utopia-server/src/main.rs:389-700`, the migration names, and the module list (`ontology_*`, `phrase_alignment`, `time_resolution`, `governance`, `api/chat.rs`, `api/mcp.rs`).
- Inference: current work is hardening rather than new architecture — based on the last 30 commit subjects, which are narrow behaviour corrections with issue numbers, and on the absence of TODO/stub markers.
- Inference: the project is approaching but has not cut a 0.1.0 release — based on workspace version `0.1.0`, the compose pin `0.1.0-rc9`, and a multi-arch release workflow keyed on `v*` tags.
- Inference: this checkout is a downstream fork of `deeplethe/utopia` used for local work — based on the `origin` URL versus the `repository` field and image name. Whether the fork intends to track or diverge from upstream is not visible.

## Open questions for define

- Is this milestone's work intended for the fork only (`evolv3ai/utopia`, default branch `dev`), or should it be shaped to go upstream to `deeplethe/utopia` (DCO sign-off, issue-first/ADR-first rules, PR to `dev`)?
- Should task and project Verify run against a real Postgres (migrated, `UTOPIA_TEST_REQUIRE_DB=1`) so database tests actually execute, and if so which database instance may the pipeline use?
- Are the upstream conventions binding for this milestone: Chinese explanatory comments, sentence-named tests and migrations, one behaviour-sentence commit subject per PR, an ADR before data-model or public-API changes?
- Are the missing migration numbers (0081–0089, 0107, 0108, 0112, 0124) reserved by in-flight upstream branches, and how should a new migration number be chosen to avoid colliding with upstream?
- Which area is this milestone about — server/store behaviour, the web UI, the `utopia` CLI, connectors, or the bench harness?
- Must local verification match CI's toolchain (Node 20, pnpm 10.2.1, Linux, Poppler), or is the Windows toolchain on this machine acceptable evidence?

## Blocked areas

- Postgres-backed test behaviour — no database was started in the sidecar, so the store integration tests and DB-dependent server tests were compiled and invoked but returned early; their real pass/fail state at HEAD is `unverifiable` here.
- `cargo clippy --workspace --all-targets -- -D warnings` and `cargo build --workspace` — not run; lint status at HEAD is `unverifiable` here.
- Migration replay (`sqlx migrate run` twice) and `scripts/smoke.sh` / Docker image boot — not run (migration commands excluded by role; no container started).
- `web/tests/*.test.mjs` browser tests — need an external `playwright-core` install and a Chromium path; not run.
- Live-service tests (Ark OCR, MySQL, Trino, ClickHouse, public RSS) — require credentials or services not available.
- `.github/workflows/release.yml` beyond line 80 and `scripts/bench/` internals — not read in full; statements about them are limited to what is cited above.
- Upstream repository state (`deeplethe/utopia` branches, tags, open PRs) — not inspected; only the fork's `origin` was queried.
