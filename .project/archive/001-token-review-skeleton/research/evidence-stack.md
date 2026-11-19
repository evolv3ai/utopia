# Evidence — stack

<!-- Written by one researcher role. Consumed by the decider. -->

Dimension: stack
Questions assigned: Do token read routes behave on a running build of this checkout as ADR 0014 decision 7 says? The owner's deployed build is unknown (Delegated, driver D004: "Not sure"); the program verifies its own build.; Which local Postgres 16 + pgvector instance can Verify use on this machine (the compose service on 127.0.0.1:1517, or another)?; Are migration numbers 0081–0089, 0107, 0108, 0112, 0124 and ADR numbers 0055–0059, 0068 reserved upstream, and how should new numbers be chosen to avoid colliding with `deeplethe/utopia`?

Method note: no server was built or run and no container or database was started, created, pulled, or stopped. Local evidence is static (source, tests, read-only `git`). Upstream evidence was read on 2026-11-18 from the public repository `deeplethe/utopia` (`git ls-remote`, GitHub REST tree listings, the pull-request and releases pages). Local HEAD is `3d307f6236904a12ae50e2f31f0d1d1f02f3bc25`.

## Finding: Every published release, including the compose default `0.1.0-rc9`, predates all token read routes

- **Claim**: Tag `v0.1.0-rc9` points at commit `888761c` (2026-10-11). The `SessionOrToken` extractor was introduced by `15921f0` on 2026-10-16 (#1244), which is not an ancestor of `888761c`; at `888761c` the export handler takes `AuthUser` (session only) and no file under `crates/utopia-server/src` mentions `SessionOrToken`. The upstream repository has only the tags `v0.1.0-rc1` … `v0.1.0-rc9`, all marked pre-release, and `release.yml` publishes images only on `v*` tags (or a manual dispatch with a named tag). HEAD is 186 commits past `888761c`.
- **Source**: `git merge-base --is-ancestor 15921f0 888761c` → exit 1; `git grep -c SessionOrToken 888761c -- crates/utopia-server/src` → no matches; `git grep -n 'AuthUser\|SessionOrToken' 888761c -- crates/utopia-server/src/api/export_routes.rs` → lines 24, 41 `AuthUser`; `git log -S'pub struct SessionOrToken' -- crates/utopia-server/src/auth.rs`; `git rev-list --count 888761c..HEAD` → 186; `git ls-remote https://github.com/deeplethe/utopia` (tags); https://github.com/deeplethe/utopia/releases; `docker-compose.yml:49`; `.github/workflows/release.yml:15-17,181-189`
- **Confidence**: medium
- **Why it matters here**: A deployment started from the documented compose default answers 401 to a personal token on `/kbs/{id}/export` and every other graph read. That matches the spec's "401 even on /export" and reconciles it with the charter's ground-truth correction: both hold, for different builds. Program criteria 1–2 can only be demonstrated on a build of this checkout or later, not on any published image.
- **Limits**: The commit ordering and the source at `888761c` are high confidence. The level is medium because "no newer image exists" is unconfirmed: the container registry's tag list was not read, and a manually dispatched image is possible.

## Finding: At HEAD, eleven read routes plus three document reads and the base list accept a personal token through one extractor

- **Claim**: These handlers take `SessionOrToken` and gate through `require_reader_kb` (owner's role at least Viewer via `access::require_kb`, then `reader.reaches(kb_id)`): `GET /kbs/{id}/entities`, `GET /kbs/{id}/entities/{entity_id}`, `GET /kbs/{id}/entities/{entity_id}/history`, `GET /kbs/{id}/facts/{fact_id}/evidence`, `GET /kbs/{id}/graph/overview`, `GET /kbs/{id}/graph/neighborhood`, `GET /kbs/{id}/ontology`, `GET /kbs/{id}/ontology/entity-types/{type_id}/entities`, `GET /documents/{id}/extractions`, `GET /kbs/{id}/export`, `POST /kbs/{id}/search`, and also `GET /documents/{id}/content`, `GET /documents/{id}/versions`, `GET /kbs/{id}/sources/{source_id}/documents/by-external-id`. `GET /workspaces/{id}/kbs` takes any token and filters the list with `reaches`. No scope check is made on these reads (read is the lowest scope, so a write token also qualifies).
- **Source**: `crates/utopia-server/src/auth.rs:175-253`; `crates/utopia-server/src/api/documents_routes.rs:84-95,98-100,197-199,222-224,450-452`; `graph_routes.rs:72-78,106-112,143-149,177-183,524-529,632-634`; `ontology_routes.rs:27-33,90-92`; `export_routes.rs:38-44`; `search_routes.rs:66-76`; `kbs.rs:68-78`; route table `crates/utopia-server/src/api/mod.rs:144,183,295,345,406,425-426,432-448,485,498`; `docs/decisions/0014-identity-from-the-person-scope-from-the-token.md:88-130`
- **Confidence**: high
- **Why it matters here**: These are the reads the program "builds on rather than adding". The pattern for opening a Review read to a token already exists and is one helper call (`require_reader_kb`), so Review reads can follow it without a new authorization model.
- **Limits**: Based on static reading of the handlers and router; nothing was run.

## Finding: Token refusals on those routes have fixed statuses, and only the write refusals carry a stable code

- **Claim**: A token that is unknown, revoked, or expired fails in `tokens::authenticate` with `AppError::Unauthorized` (the revocation and expiry tests are in the SQL `WHERE`, re-run on every request, and each success updates `last_used_at`). A token whose `kb_ids` exclude the base gets `AppError::NotFound` on reads. An owner without a role on the base gets `Forbidden` (or `NotFound` on a restricted base) from `access::require_kb`. Write-capable token routes call `require_write` first and return `CodedForbidden` with `token_read_only` or `token_kb_out_of_scope`; a token creating a fetching source gets `token_source_kind`. Read refusals carry no token-specific code.
- **Source**: `crates/utopia-store/src/tokens.rs:155-176`; `crates/utopia-server/src/auth.rs:194-253`; `crates/utopia-server/src/api/documents_routes.rs:89-93`; `crates/utopia-store/src/access.rs:83-93`; assertions in `crates/utopia-server/src/api/token_setup_tests.rs:290,338,454,475,485,490` and `token_read_tests.rs:172,190`
- **Confidence**: high
- **Why it matters here**: Success criterion 5 asks for "a stable error code" on refused Review writes and invisibility for an out-of-scope base. The existing codes `token_read_only` and `token_kb_out_of_scope` and the 404-on-read rule are the precedent; a refusal for "owner is below editor" currently has no code (plain 403).
- **Limits**: The level applies to the code paths. The HTTP status that each `AppError` variant maps to was taken from the test assertions (401, 403, 404), not from reading `error.rs`.

## Finding: Review routes are session-only at HEAD, so a personal token gets 401 there

- **Claim**: In `review_routes.rs` every handler takes `AuthUser` except `decide_alignment_kind_word`, which takes `SessionOrToken`, calls `require_write(Some(kb_id))`, then `require_kb(..., Role::Editor)`. `AuthUser` decodes every bearer as a JWT, so a `utp_pat_` token fails with `Unauthorized` before any role check.
- **Source**: `crates/utopia-server/src/api/review_routes.rs:76-82,186,247,283,353,486,560,584,608,656,717,732,1370-1379,1434`; `crates/utopia-server/src/auth.rs:156-173` and the doc comment at `175-187`
- **Confidence**: high
- **Why it matters here**: Confirms the charter's ground truth and gives the one in-file example of a token-accepting Review write (scope check before role check, token id in the audit detail).

## Finding: The token read behaviour is covered by tests that need a database, and none of them has been run in this pipeline

- **Claim**: `token_read_tests.rs` sends a real bearer token through the full router for the eleven read paths and asserts 200 for a viewer's read token (`a_read_token_of_a_base_viewer_gets_200_on_every_graph_read`), 404 for a token limited to another base (`a_token_limited_to_another_base_gets_404_on_every_graph_read`), and an audit row `kb.exported` with the owner as actor and `token_id` in the detail. `token_setup_tests.rs` covers `token_read_only`, `token_kb_out_of_scope`, the 401 of a dead token, and base listing. Each fixture starts with `test_db::url()` and returns early when `UTOPIA_DATABASE_URL` is unset; the inspect run had no database, so these tests passed without executing. The read fixture calls `utopia_store::db::migrate` itself; the store suite does not.
- **Source**: `crates/utopia-server/src/api/token_read_tests.rs:28-33,92-114,163-229`; `crates/utopia-server/src/api/token_setup_tests.rs:217,428-490`; `crates/utopia-server/src/api/mod.rs:683-687`; `.project/research/evidence-codebase.md` (Checks run: "no database"; Blocked areas)
- **Confidence**: high
- **Why it matters here**: These tests are the cheapest proof of ADR 0014 decision 7 for this checkout: with a database, `cargo test -p utopia-server token_read` exercises the routes in-process without starting the server. Until that runs, "token reads work on this build" rests on static reading only. They are also the template for the program's own token tests on Review routes.

## Finding: No Postgres 16 with pgvector is running or installed on this machine; the compose service is not up

- **Claim**: Nothing listens on port 1517. Port 5432 is held by a native Windows service `postgresql-x64-17` (PostgreSQL 17, running, automatic start), whose install tree contains no pgvector files (no `vector.control`, no `vector.dll`). Docker Desktop is installed (client 29.9.2) but its engine is stopped (`com.docker.service` Stopped, `docker-desktop` WSL distribution Stopped), so `docker ps`/`docker images` fail from Windows. A separate Docker engine 29.1.3 runs inside the WSL `Ubuntu` distribution with zero containers and no `pgvector` or `postgres` image. `psql`, `pg_isready`, and `sqlx` are not on PATH (`psql.exe` exists at `C:\Program Files\PostgreSQL\17\bin\`). No `UTOPIA_*`, `PG*`, or `DATABASE_URL` variable is set.
- **Source**: `Get-NetTCPConnection -State Listen` filtered to ports 1516/1517/5432/5433 → only 5432 (PID 7636); `Get-Service *postgres*,*docker*`; `docker --version`; `docker ps -a` / `docker images` → "failed to connect to the docker API at npipe:////./pipe/dockerDesktopLinuxEngine"; `wsl -l -v`; `wsl -d Ubuntu -e sh -c 'docker info; docker ps -a; docker images'`; glob `C:\Program Files\PostgreSQL\**\vector*` (two unrelated pgAdmin Python files only); glob `C:\Program Files\PostgreSQL\*/bin/{psql,pg_isready,postgres}.exe`; `psql --version`, `sqlx --version` → command not found
- **Confidence**: medium
- **Why it matters here**: Delegated constraint D006 requires a migrated Postgres 16 + pgvector for Verify. None exists here today, so every database-backed Verify is blocked until someone starts one. Starting Docker Desktop or pulling `pgvector/pgvector:pg16` is an environment change outside this role.
- **Limits**: What is running and installed is high confidence. The level is medium because "PostgreSQL 17 has no pgvector" rests on a file search of the install tree; `pg_available_extensions` was not queried because no connection was made.

## Finding: The existing local PostgreSQL 17 cannot stand in for the compose service as it is

- **Claim**: The first migration runs `CREATE EXTENSION IF NOT EXISTS vector;`, so the migration step fails on a server without pgvector. The compose file moved its host port to 1517 for this exact case, noting that a locally installed Postgres on 5432 "most likely has no pgvector". CI and compose both pin `pgvector/pgvector:pg16`, with user, password, and database all `utopia`.
- **Source**: `migrations/0001_core.sql:3`; `docker-compose.yml:4,9-11,20-27`; `.github/workflows/checks.yml:53-63,124-134`; `CONTRIBUTING.md:83-87`
- **Confidence**: high
- **Why it matters here**: Answers "the compose service, or another": the only instance the repository documents and CI matches is the compose `db` service on `127.0.0.1:1517`. Using the native PostgreSQL 17 would need pgvector installed into it and would also differ from CI by a major version.

## Finding: Database-backed Verify is a two-step sequence against one database, and the default suite must not share it with the delivery regressions

- **Claim**: The documented sequence is (1) `docker compose up -d db`; (2) with `UTOPIA_DATABASE_URL=postgres://utopia:utopia@localhost:1517/utopia`, run `cargo run -p utopia-store --example migrate`; (3) with the same URL and `UTOPIA_TEST_REQUIRE_DB=1`, run the tests. The migrate example only rolls forward and prints one line; without step 2 the store tests fail on missing relations (#869). With `UTOPIA_TEST_REQUIRE_DB` set and no URL, `test_db::url()` panics instead of skipping. The `#[ignore]`d `human_phrase_materialization_delivery` tests start a real queue worker and need a dedicated idle database; they are not part of the normal suite. CI additionally sets `UTOPIA_TEST_REQUIRE_PDFTOTEXT=1` and installs Poppler.
- **Source**: `crates/utopia-store/examples/migrate.rs:1-27`; `crates/utopia-store/src/test_db.rs:10-31`; `.github/workflows/checks.yml:75-91`; `CONTRIBUTING.md:49-51,66-105`
- **Confidence**: high
- **Why it matters here**: This is the exact setup a task Verify must name. Tests insert under fresh UUIDs and delete their own organisation afterwards (`token_read_tests.rs:154-160`), so one shared migrated database serves repeated runs; whether parallel isolated worktrees can share it safely was not established.

## Finding: A new migration in a task worktree changes the shared database's schema for every other worktree

- **Claim**: Migrations are embedded at compile time and applied by number to whichever database the URL names; sqlx indexes applied migrations by version. A database migrated by one worktree that contains a new migration then holds a version unknown to a worktree at the base commit.
- **Source**: `.project/research/evidence-codebase.md` finding "Migration numbers are non-contiguous and embedded at compile time" (`crates/utopia-store/src/db.rs`, `sqlx::migrate!`); `.github/workflows/checks.yml:119-121,153-157`
- **Confidence**: medium
- **Why it matters here**: The pipeline verifies each task at its recorded base plus only that task's patch. If program tasks add migrations, one shared Postgres instance may not serve concurrent or sequential isolated Verifies without a per-task database name; this is a plan-level constraint, not a decision made here.
- **Limits**: The embedding and by-version indexing are sourced; the failure of an older binary against a newer-migrated database is sqlx's documented default behaviour but was not reproduced here.

## Finding: Upstream `dev` is exactly the local HEAD, and upstream `main` has the same migrations directory

- **Claim**: `deeplethe/utopia` `refs/heads/dev` is `3d307f6236904a12ae50e2f31f0d1d1f02f3bc25`, identical to local HEAD. Upstream `main` (`a0a800f`, the remote default) is a different commit that is not in the local object store, but its `migrations` tree id equals local `HEAD:migrations` (`1f50b03f…`) and its `docs/decisions` listing introduces no file name absent from local HEAD. Other upstream branches: `assets`, `dependabot/cargo/zip-9.0.5`, `docs/0064-cut-4-design`.
- **Source**: `git ls-remote https://github.com/deeplethe/utopia` (1225 refs); `git rev-parse HEAD:migrations HEAD:docs/decisions`; https://api.github.com/repos/deeplethe/utopia/git/trees/a0a800f5d15648a25afcc4dd20da22e6d286f3ab; `git ls-tree --name-only 765d4eec422da553b814abc98f7e622dcc61a48e:decisions` compared with HEAD (no extra names)
- **Confidence**: high
- **Why it matters here**: The fork is not behind upstream on either numbered directory today, so the local gap list is the upstream gap list. The highest merged numbers upstream are migration 0126 and ADR 0075.

## Finding: None of the missing numbers is held by an upstream branch or open pull request

- **Claim**: Of the 22 open upstream pull requests (#1075, #1279, #1423, #1431, #1434–#1441, #1445, #1454, #1456, #1457, #1485, #1493, #1496, #1503, #1504, #1517), none contains a migration numbered 0081–0089, 0107, 0108, 0112, or 0124, and none contains an ADR numbered 0055–0059 or 0068. Twenty-one of them carry a `migrations` tree identical to some earlier state of `dev` (every file name is in local HEAD). The local history, across all refs, contains no commit that ever added, renamed, or deleted a file at any of those numbers, and no tracked file under `docs`, `crates`, or `web/src` references ADR 0055–0059 or 0068.
- **Source**: https://github.com/deeplethe/utopia/pulls?q=is%3Apr+is%3Aopen ("Open 22"); per-PR root trees at `https://api.github.com/repos/deeplethe/utopia/git/trees/<head sha>` for all 22 head SHAs taken from `git ls-remote`; `git ls-tree --name-only <migrations tree>` compared with `HEAD:migrations` for the seven distinct trees present locally; decisions trees `34db55e7…` (#1075), `a4c15afb…` (#1279), `56c1035d…` (#1493), `2fc4815c…` (#1517) read through the same API; `git log --all --name-status -- 'migrations/008[1-9]_*' 'migrations/0107_*' 'migrations/0108_*' 'migrations/0112_*' 'migrations/0124_*'` → empty; `git log --all --name-status -- 'docs/decisions/005[5-9]-*' 'docs/decisions/0068-*'` → empty
- **Confidence**: high
- **Why it matters here**: The gaps are not live reservations. They are also not free in any documented sense: no rule says a skipped number may be reused, and `docs/decisions/README.md` says ADR numbers increase "in creation order, no gaps", which the tree already violates. Nothing upstream tracks a reservation list.
- **Limits**: The level applies to open pull requests and branches as of 2026-11-18; no source says whether closed or draft-elsewhere work once used those numbers.

## Finding: One open upstream pull request already claims migration 0127

- **Claim**: Open pull request #1496 ("Purging a document finds the rows that cascade by chunk", opened 2026-11-14, base `dev`) adds `migrations/0127_document_purge_finds_chunk_dependents.sql`. Its tree has no `0126_` file, so it was numbered past a number then held by another in-flight change (0126 merged on 2026-11-16 in #1492).
- **Source**: https://api.github.com/repos/deeplethe/utopia/git/trees/7494ffad57747eaf8fc91e5d4110fbfcd9fe1e92 (113 entries, highest `0127_document_purge_finds_chunk_dependents.sql`, no 0124 or 0126); `git log --diff-filter=A -- 'migrations/0126_*'` → `0002406` 2026-11-16 (#1492)
- **Confidence**: medium
- **Why it matters here**: "Next number after 0126" is already taken upstream. A program migration numbered 0127 would collide with #1496 if both reach `deeplethe/utopia`. It also shows how the gaps arise: authors number past in-flight work, and a number is lost when that work is renumbered or dropped.
- **Limits**: The presence of the `0127_` file in the pull request is high confidence. The level is medium because the reading of why 0126 was skipped is inferred from dates.

## Finding: The repository's numbering rules are "latest on the target branch plus one", enforced only for exact duplicates

- **Claim**: `CONTRIBUTING.md` tells contributors to "check the latest number on `main` before opening a PR". CI's `migrations` job fails on any duplicated four-digit prefix (`ls migrations/ | cut -d_ -f1 | sort | uniq -d`) and then runs `sqlx migrate run` twice on a fresh database; it does not check contiguity or ordering against already-deployed databases. ADR file names are `NNNN-short-english-title.md`, "increasing in creation order, no gaps"; no CI step checks ADR numbers. Open pull request #1279 carries its ADR without a number (`use-decays-the-ledger-does-not.md`, titled "ADR (unnumbered, on hold)").
- **Source**: `CONTRIBUTING.md:114`; `.github/workflows/checks.yml:119-157`; `docs/decisions/README.md:11`; https://github.com/deeplethe/utopia/pulls?q=is%3Apr+is%3Aopen; https://api.github.com/repos/deeplethe/utopia/git/trees/a4c15afbaf3453dd39dc15a01039e835ea88be69
- **Confidence**: high
- **Why it matters here**: The upstream-compatible choice follows from these rules and the findings above: take a number above every merged number and every number claimed by an open upstream pull request at the time of writing (today that is 0128 or higher for migrations and 0076 or higher for ADRs), re-check `git ls-remote` and the open pull requests before each landing, and expect to renumber at upstream submission. Because a fork database that has applied a migration under one number must be able to replay it under a new number, CI's second `migrate run` ("迁移改号后要能在已经跑过旧号的库上安全重放") names the property a renumbered migration must keep: idempotent SQL.

## Finding: The fork has no branch or tag that reserves numbers either

- **Claim**: The fork's remote has four heads (`dev` and three `fix/*` branches) and no tags; local HEAD is the fork's `dev`. No local or fork ref contains a migration or ADR at the missing numbers.
- **Source**: `git ls-remote --heads origin`; `git ls-remote --tags origin` → empty; `git tag -l` → empty; the two `git log --all` path queries cited above → empty
- **Confidence**: high
- **Why it matters here**: The program is the only prospective author of new numbers on the fork, so collisions can only come from upstream.

## Assigned questions — answers

- Do token read routes behave on a running build of this checkout as ADR 0014 decision 7 says? The owner's deployed build is unknown (Delegated, driver D004: "Not sure"); the program verifies its own build. → Statically, yes: at HEAD `3d307f6` the eleven reads of decision 7 (entities search, entity detail, entity history, fact evidence, graph overview, neighbourhood, ontology, class instances, document extractions, export, chunk search), plus document content, versions, by-external-id and the base list, take `SessionOrToken`; any live token of an owner with at least Viewer role is accepted, a token whose `kb_ids` exclude the base gets 404, a dead token gets 401, and export writes `kb.exported` with the token id (`auth.rs:175-253`, `documents_routes.rs:84-95`, `token_read_tests.rs:92-229`). Not proven on a running build: no server was started and the database-backed tests that assert these statuses have not executed in this pipeline (no database). Those routes were opened between 2026-10-16 and 2026-10-25 (#1244, #1263, #1264, #1280, #1265, #1266, #1273), after `v0.1.0-rc9` (`888761c`, 2026-10-11), the newest published tag and the compose default; any deployment on rc9 or earlier answers 401 to a token on all of them, which is consistent with the spec's report. Review routes remain session-only at HEAD except `decide_alignment_kind_word`.
- Which local Postgres 16 + pgvector instance can Verify use on this machine (the compose service on 127.0.0.1:1517, or another)? → None is available now. The documented instance is the compose `db` service (`pgvector/pgvector:pg16`, `postgres://utopia:utopia@localhost:1517/utopia`), but nothing listens on 1517, the Docker Desktop engine is stopped, and no pgvector image is present in either Docker engine on the machine. The only running Postgres is native PostgreSQL 17 on 5432 with no pgvector files, which fails the first migration (`CREATE EXTENSION vector`). Setup once an instance exists: `cargo run -p utopia-store --example migrate` with `UTOPIA_DATABASE_URL` set, then the tests with the same `UTOPIA_DATABASE_URL` and `UTOPIA_TEST_REQUIRE_DB=1` (`checks.yml:79-91`, `CONTRIBUTING.md:83-88`, `test_db.rs:10-31`). Missing here: a running Docker engine, the `pgvector/pgvector:pg16` image, and the `db` container; `psql` and `sqlx` are not on PATH but the two-step setup needs neither.
- Are migration numbers 0081–0089, 0107, 0108, 0112, 0124 and ADR numbers 0055–0059, 0068 reserved upstream, and how should new numbers be chosen to avoid colliding with `deeplethe/utopia`? → Not reserved by anything visible: upstream `dev` equals local HEAD, upstream `main` has the same migrations tree, and none of the 22 open upstream pull requests or the other upstream branches contains a file at any of those numbers; local history never contained them either. No document says skipped numbers may be reused, so reuse has no supporting source. The stated rule is latest number on the target branch plus one, with CI rejecting only exact duplicates (`CONTRIBUTING.md:114`, `checks.yml:138-145`, `docs/decisions/README.md:11`). Migration 0127 is already claimed by open pull request #1496, so the lowest number not in use or in flight today is migration 0128 and ADR 0076; the claim must be re-checked against upstream `dev` and open pull requests before each landing and again before any upstream submission, where renumbering should be expected.

## Dead ends

- Running the token read tests or the server to observe the statuses — excluded by role (no build, no test, no database start); the behaviour is asserted by tests that have not executed here.
- `ghcr.io/deeplethe/utopia` image tag list — not read; whether an image newer than `0.1.0-rc9` was published by manual dispatch is unknown. The owner's deployed build stays unknown.
- Mirroring upstream pull-request heads into a scratch repository with `git fetch` to diff them — refused by the pipeline guard hook; replaced by per-commit tree listings from the GitHub REST API, which give file names but not file contents.
- `https://api.github.com/repos/deeplethe/utopia/pulls?state=open&per_page=100` and the `contents/migrations` listing — both exceed the reader's page limit and were truncated; the HTML pull-request list and git tree endpoints were used instead.
- Closed or merged-then-reverted upstream pull requests as the origin of the gaps — not searched (1184 pull-request refs); the cause of each gap is unsourced beyond the dates showing that ADR 0054 and 0060 were both added on 2026-09-25 and that #1496 numbered past an in-flight 0126.
- Web search for upstream issues discussing reserved ADR or migration numbers — returned nothing about this repository.
- Querying the native PostgreSQL 17 for `pg_available_extensions` — not done (no database connection was made); absence of pgvector rests on the install tree containing no `vector*` extension files.
- Whether WSL's Docker engine publishes `127.0.0.1:1517` to Windows-side `cargo` — not testable without starting a container.
- The fork's GitHub pages — not opened on the web; fork refs were read through the existing `origin` remote only.
