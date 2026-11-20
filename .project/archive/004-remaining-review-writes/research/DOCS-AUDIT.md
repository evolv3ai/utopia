# Docs Audit

Repo root: D:\utopia
Audited: 2026-11-20
Audited HEAD: d5d2356d3f9e3fc09123da6fd20a51349d5627dc
Alignment mode: no

## Summary

| Verdict | Count |
|---------|-------|
| verified | 261 |
| stale | 31 |
| aspirational | 3 |
| unverifiable | 2 |
| descriptive docs (no testable claims) | 3 |

Worst drift: README.md's Roadmap still lists business rules, OIDC SSO and backup/restore commands as unbuilt (and two design files repeat "proposed and not built") while `attribute_rules`, `oidc_routes.rs` and the `utopia-cli` backup/restore crate are in the code. New at this HEAD: docs/design/access-and-audit.md and docs/design/chat-and-mcp.md still describe token access as it was before ADR 0076 ("every other route is session-only", "an external agent can therefore only propose"), while ten Review routes and six MCP tools now serve a token; web/src/docs/mcp.md itself carries no drift.

Method note: delta re-audit against the prior audit (Audited HEAD 6c52ce8). All checks ran in the disposable verify sidecar at the audited HEAD; the primary worktree ran no project command and no remote was contacted. 16 rows are marked `unchanged: ` and carried under the delta rule (verified, not a command claim, and neither the document nor any path or directory prefix the evidence names is in the changed set). Every other row was re-verified at d5d2356, in one of three ways. (1) web/src/docs/mcp.md and docs/decisions/0076 were rewritten in M003, so their claims were extracted afresh and each was checked by reading the code (review_ops.rs, review_tools.rs, review_routes.rs, token_agent_actions.rs, execution_gate.rs, governance.rs, resolution.rs, auth.rs, error.rs, mcp.rs, api/mod.rs, migration 0129 and the web pages), with line numbers as they are now. (2) Rows of other documents whose evidence names a changed file (crates/utopia-cli/src/main.rs, crates/utopia-core/src/models.rs, api/mod.rs, web/src/api.ts, ADR 0076, the migrations directory) were looked up again and their line numbers and counts corrected where the code moved. (3) For the remaining rows the document and every file the evidence names are byte-identical between the two heads (`git diff --name-only --no-renames 6c52ce8 d5d2356` lists 39 paths outside `.project/`, the same set as the changed-set file, with no deletion or rename), so the recorded file:line evidence was confirmed to hold; these rows lost the `unchanged: ` prefix only because their evidence names a directory that contains a changed file. The absence greps (`STALL_NUDGE`, `on_model_select`, `AUTO_CLOSE_MIN_CONFIDENCE`, `doc_cache`, `judge_direction`, action tables, `feishu`, SPARQL routes, 0049 references, the pipeline drop codes) were run again. Two commands were executed: `cargo fmt --all --check` (exit 0) and `node --test scripts/bench/lib.test.mjs` (15 pass, 0 fail). `cargo test`, `cargo clippy`, `pnpm build`, Docker, the DB-backed review and store tests and live-model bench runs were not executed, so tests cited as evidence are cited by name and line, not by result. For decision records the audited claim is the record's own status line, except 0076, whose decisions were checked individually. In ADR 0076 the paragraphs that still call merge, manual merge, revert and batch closed to tokens (the Problem section, decision 12's first paragraph and earlier revisions, the quoted status history) are dated history, each superseded in place by a 2026-11-19 gated-write revision; they were read as history and not extracted as present-tense claims. Suppressed: the claim "0127 is claimed by upstream #1496" by the accept-drift ruling on queue item 38, and the two branch-protection claims in CONTRIBUTING.md and CONTRIBUTING.zh-CN.md by the rulings on items 13 and 14. Queue items 1 to 12 and 15 to 34 persist with their numbers; four new items, 39 to 42, were opened. The pre-tool guard hook refused nothing during this audit.

## Doc: .github/PULL_REQUEST_TEMPLATE.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "`CURRENT_SCHEMA_VERSION` in `crates/utopia-cli/src/main.rs` equals the number of files in `migrations/`" | structure | verified | crates/utopia-cli/src/main.rs:85 `CURRENT_SCHEMA_VERSION: u32 = 115`; `ls migrations` → 115 files (0129 added, constant raised with it) |
| "For changes under `web/`: `pnpm build` and `pnpm test` pass" | command | verified | web/package.json scripts `build`, `test` exist; .github/workflows/checks.yml `web` job runs both (static; not executed) |
| "UI strings are in both `web/src/i18n/en.ts` and `zh.ts`" | structure | verified | web/src/i18n/ contains en.ts, zh.ts, index.ts |

## Doc: .roadmap-proposals/backup-restore.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Both halves are implemented" (`utopia backup` and `utopia restore` in a separate `utopia-cli` crate) | feature | verified | crates/utopia-cli/src/main.rs (backup manifest write at :377, restore version gate at :534), crates/utopia-cli/tests/restore.rs; workspace member in Cargo.toml |
| "`restore` will refuse a manifest whose `schema_version` is greater than the running server's" | feature | verified | crates/utopia-cli/src/main.rs:534 `if manifest.schema_version > CURRENT_SCHEMA_VERSION`, :541 older-version warning branch |
| "The sealing key is left out of the archive by default" | feature | verified | crates/utopia-cli/src/main.rs:26-30 `--include-secret-key` doc comment "Off by default"; :432-435 |
| "`schema_version`: pulled from the running migrations" | feature | stale | crates/utopia-cli/src/main.rs:80-85: a hand-maintained constant `CURRENT_SCHEMA_VERSION`, "Kept as a constant — bumping is a deliberate decision" |
| "the operator can pipe `utopia backup --output -` to `aws s3 cp - s3://…` today" | command | aspirational | No `"-"`/stdout output handling in crates/utopia-cli/src/main.rs (grep for `"-"` and stdout finds only `Stdio::null()` at :329); same doc says backup writes a single `*.tar.gz` file and refuses to overwrite |
| "CI provides PostgreSQL 16 clients" | config | verified | unchanged: .github/workflows/checks.yml `test` job installs `postgresql-client-16` |

## Doc: CONTRIBUTING.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "CI runs exactly this" (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `pnpm install --frozen-lockfile && pnpm build`) | command | verified | .github/workflows/checks.yml:41,43,84,182 run these exact commands (lint/test/web jobs). `cargo fmt --all --check` executed in sidecar at d5d2356 → exit 0, no diff; clippy, test and pnpm build not executed |
| "that is how the `migrations` job in CI runs the whole `utopia-store` suite" | config | stale | .github/workflows/checks.yml: `UTOPIA_TEST_REQUIRE_DB` is set on the `test` job; the `migrations` job only runs `sqlx migrate run` twice and its comment says the DB-backed tests no longer run there |
| "`test_db::url()` reads `UTOPIA_DATABASE_URL`. With `UTOPIA_TEST_REQUIRE_DB=1` also set, a missing database is a failure" | config | verified | crates/utopia-store/src/test_db.rs references both variables |
| "export UTOPIA_DATABASE_URL=postgres://utopia:utopia@localhost:1517/utopia" (1517 is the host-side port) | config | verified | unchanged: docker-compose.yml:27 `${UTOPIA_DB_BIND:-127.0.0.1:1517}:5432`; .env.example same URL |
| "`cargo test --locked -p utopia-store --test human_phrase_materialization_delivery -- --ignored --skip crash_child ...`" and the `busy_defers_without_retaining_connections` lib test | command | verified | crates/utopia-store/tests/human_phrase_materialization_delivery.rs (contains `crash_child`); crates/utopia-store/src/materialize_delivery_tests.rs `busy_defers_without_retaining_connections` (static; not executed) |
| "CI runs `cargo test --workspace`, `pnpm test` and `scripts/bench/lib.test.mjs`, and nothing else" | config | verified | .github/workflows/checks.yml test + web jobs; `node --test scripts/bench/lib.test.mjs` executed in sidecar at d5d2356 → tests 15, pass 15, fail 0 |
| "`docker-compose.yml` defaults `UTOPIA_IMAGE` to a fixed tag ... the pin has been on one of them since rc8" | config | verified | unchanged: docker-compose.yml:49 `${UTOPIA_IMAGE:-ghcr.io/deeplethe/utopia:0.1.0-rc9}` |
| "Every workflow declares its own `permissions:`" | config | verified | unchanged: grep `permissions:` → checks.yml, release.yml, star-history.yml each have one block |
| "Source installs need Poppler ... the Docker image includes both" (`poppler-utils poppler-data`) | config | verified | docker/Dockerfile:27 installs `poppler-utils poppler-data`; crates/utopia-ingest/src/parsers.rs uses `pdftotext` |

## Doc: CONTRIBUTING.zh-CN.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "`cargo fmt --all --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace` / `pnpm install --frozen-lockfile && pnpm build`" (CI 跑的就是这些) | command | verified | .github/workflows/checks.yml:41,43,84,182; `cargo fmt --all --check` executed in sidecar at d5d2356 → exit 0, no diff; the listed commands are the ones CI runs (others static) |
| "`cargo run -p utopia-store --example migrate` # 空库先迁移；CI 的 backend job 也是这么做的" | command | verified | crates/utopia-store/examples/migrate.rs; checks.yml `test` job step runs the same command (static; not executed) |

## Doc: README.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "The [walkthrough](docs/walkthrough.md) runs this end to end with `curl`" | structure | aspirational | docs/ contains only decisions/, design/, pipeline.md; `git log --all -- docs/walkthrough.md` → no commits; .gitignore ignores `/docs/*` except decisions, design and pipeline.md, so the file cannot be tracked as written |
| "One Rust binary and one Postgres. Full-text search is embedded in the binary, vectors go in pgvector, and the job queue is a table" | structure | verified | Cargo.toml deps `tantivy`, `pgvector`; docker-compose.yml has only `db` (pgvector/pgvector:pg16) and `app`; crates/utopia-store/src/jobs.rs |
| "Upload PDF, DOCX, PPTX, XLSX, XLS, ODS, CSV, TSV, Markdown, HTML or plain text" | feature | verified | crates/utopia-ingest/src/lib.rs:347-350 dispatch on xlsx/xls/ods/pptx/csv/tsv; Cargo.toml `pdf-extract`, `calamine`, `htmd`, `chardetng` |
| "Web pages, RSS, GitHub, Jira, Notion, WebDAV and S3-compatible buckets sync on a schedule" | integration | verified | crates/utopia-core/src/models.rs:306 `SourceKind` has Url, Rss, GithubIssues, GithubRepo, JiraIssues, Notion, Webdav, S3 (and also Gitlab*, AzureBlob, Gcs, GoogleDrive, which the README does not list) |
| "Scans and images are read by a MinerU service or a Volcengine Ark vision model" | integration | verified | crates/utopia-core/src/models.rs:542-550 (MinerU layout service setting); crates/utopia-server/src/readers/ark_ocr.rs |
| "Full-text on Tantivy, vectors on pgvector, fused with RRF" | feature | verified | crates/utopia-search/src/lib.rs and crates/utopia-server/src/retrieval.rs reference RRF; Cargo.toml `tantivy`, `pgvector` |
| "An MCP server for every knowledge base" | feature | verified | crates/utopia-server/src/api/mod.rs:240 `.route("/kbs/{id}/mcp", post(mcp::handle))` |
| "Five ship inside the binary: schema.org, W3C Org, PROV-O, FOAF and IOF Core" | feature | verified | crates/utopia-server/src/ontology_packs.rs:42-78 five `include_bytes!` of crates/utopia-server/packs/*.gz |
| "Derivation is off by default, since a wrong axiom derives wrong facts" | config | stale | migrations/0050_a_new_base_starts_with_its_engines_on.sql `ALTER COLUMN materialize_inferences SET DEFAULT TRUE`; crates/utopia-core/src/models.rs:1248-1251 comment says default on (0050) |
| "Mount a database on a base (Postgres, MySQL and the engines that speak its protocol, ClickHouse, Trino for Iceberg / Delta Lake / Hive, Databricks, Snowflake)" | integration | verified | crates/utopia-server/src/query_engine/ (mod.rs:48 `"trino"`, clickhouse.rs, snowflake/databricks modules per mod.rs:7); migrations 0021, 0025, 0106 |
| "Each knowledge base has its own members and roles: owner, admin, editor and viewer" | feature | verified | crates/utopia-core/src/models.rs:55 `enum Role { Viewer, Editor, Admin, Owner }` |
| "The first account registered becomes the system administrator" | feature | verified | crates/utopia-store/src/accounts.rs:18,58 first user creates the organisation and becomes owner + system admin |
| "Requirements: Docker (local development also needs Rust 1.85+, Node 20+, pnpm)" | config | unverifiable | No `rust-version` in Cargo.toml or any crate and no rust-toolchain file; CI uses `dtolnay/rust-toolchain@stable`, Dockerfile `rust:1-slim-bookworm`. Node 20 matches checks.yml and docker/Dockerfile:2. Confirming the 1.85 floor needs a build on 1.85 |
| "The prebuilt image is published for `linux/amd64` and `linux/arm64`" | config | verified | unchanged: .github/workflows/release.yml:36-40 matrix platforms linux/amd64, linux/arm64 |
| "docker compose --profile app up -d" / "Open http://localhost:1516" | command | verified | docker-compose.yml `app` service has `profiles: ["app"]`, ports `1516:1516` (static; not executed) |
| "docker compose -f docker-compose.yml -f docker-compose.build.yml --profile app up -d --build" | command | verified | docker-compose.build.yml and docker/Dockerfile exist (static; not executed) |
| "cargo run -p utopia-server" (backend on :1516) / "cd web && pnpm install && pnpm dev" (frontend on :5173, proxying /api) | command | verified | crates/utopia-core/src/config.rs:60 `bind_addr: "0.0.0.0:1516"`; web/vite.config.ts:11-14 port 5173, `/api` proxy to 127.0.0.1:1516 (static; not executed) |
| "[ ] **Business rules**: rules written by people over an entity's attribute facts ... (#277)" | status | stale | migrations/0028_a_rule_reads_attributes.sql `CREATE TABLE attribute_rules`; commit 0b18c7c "A rule reads attributes and concludes a type (0021 / #277)"; docs/decisions/0021 status Implemented |
| "[ ] **Agent memory over MCP**: episode writes, the retrieve endpoint, and the MCP server" | status | stale | The MCP server and the `remember` tool exist (crates/utopia-server/src/api/mcp.rs; commit 57680e9 "An agent can record over MCP"); at least the "MCP server" part is built |
| "[ ] **Enterprise**: OIDC SSO, backup and restore commands, benchmarks at 100k documents" | status | stale | crates/utopia-server/src/api/oidc_routes.rs, migrations 0056 and 0103, docs/decisions/0069 Implemented; crates/utopia-cli backup/restore (.roadmap-proposals/backup-restore.md: "Both halves are implemented") |
| "Pin a specific version with `UTOPIA_IMAGE` in production" | config | verified | unchanged: docker-compose.yml:49 |

## Doc: README.zh-CN.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "docker compose --profile app up -d" / "cargo run -p utopia-server" / "cd web && pnpm install && pnpm dev" | command | verified | Same files as README.md: docker-compose.yml profiles/ports, config.rs:60, web/vite.config.ts:11-14 (static; not executed) |
| "[ ] **问数与映射添加数据湖仓支持**：Iceberg / Delta Lake，以及 Databricks、Snowflake、MaxCompute 的映射探索与 Ontology2SQL 支持" | status | stale | crates/utopia-server/src/query_engine/ has trino, databricks and snowflake engines (migration 0021); README.md says the same three "are in" and lists only MaxCompute as open — the two READMEs contradict each other |
| "[ ] **精确到时刻**：在年 / 月 / 日之外加一档 `instant` 精度" | status | stale | migrations/0033_the_world_axis_reaches_the_second.sql; docs/decisions/0024 status "Implemented (migration 0033)"; README.md has no such roadmap item |
| "[ ] **企业化**：OIDC SSO、备份恢复命令、10 万文档级别的性能基准" | status | stale | crates/utopia-server/src/api/oidc_routes.rs; crates/utopia-cli backup/restore |
| "[ ] **业务规则**：由人写下、作用于实体属性事实的规则" | status | stale | migrations/0028_a_rule_reads_attributes.sql; docs/decisions/0021 Implemented |
| Use-case row links the walkthrough (`docs/walkthrough.md`) | structure | aspirational | README.zh-CN.md:60; docs/walkthrough.md absent at HEAD and in all git history |

## Doc: SECURITY.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "By default the port binds to loopback (`127.0.0.1:1517`)" / default password `utopia` | config | verified | unchanged: docker-compose.yml:10 `${UTOPIA_DB_PASSWORD:-utopia}`, :27 `${UTOPIA_DB_BIND:-127.0.0.1:1517}:5432` |
| "Credentials sealed at rest ... AES-256-GCM ... `UTOPIA_SECRET_KEY`, or `secret.key` generated in the data directory" | feature | verified | crates/utopia-core/src/secrets.rs uses `Aes256Gcm`; crates/utopia-core/src/config.rs references `secret.key`; Cargo.toml `aes-gcm` |
| "`Secure` on session cookies behind TLS — decided from `X-Forwarded-Proto` ... `UTOPIA_COOKIE_SECURE=true`" | config | verified | crates/utopia-server/src/auth.rs and crates/utopia-core/src/config.rs reference both |
| "Optional least-privilege runtime role — set `UTOPIA_APP_DB_PASSWORD` and `UTOPIA_MIGRATION_URL`" | config | verified | docker-compose.yml:19,62; docker/init-app-role.sh; migrations/0010_least_privilege_role.sql |
| "Data sources reach only granted workspaces" | feature | verified | migrations/0014_data_source_grants.sql:22 `CREATE TABLE data_source_grants` |
| "Accounts are deactivated, not deleted — `users.deactivated_at`" | feature | verified | migrations/0001_core.sql contains `deactivated_at` |
| "Passwords hashed with argon2" | feature | verified | crates/utopia-server/Cargo.toml and crates/utopia-server/src/api/auth_routes.rs use argon2 |

## Doc: SECURITY.zh-CN.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "数据库默认口令是 `utopia`。默认配置下端口只绑回环（`127.0.0.1:1517`）" | config | verified | unchanged: docker-compose.yml:10,27 |
| "按 `X-Forwarded-Proto` 判定 ... `UTOPIA_COOKIE_SECURE`" | config | verified | crates/utopia-server/src/auth.rs, crates/utopia-core/src/config.rs |
| "口令哈希用 argon2" | feature | verified | crates/utopia-server/src/api/auth_routes.rs |

## Doc: crates/utopia-server/packs/README.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "由 `src/ontology_packs.rs` 用 `include_bytes!` 内嵌进二进制" (five pack files listed) | structure | verified | crates/utopia-server/src/ontology_packs.rs:42-78; all five .gz files present in crates/utopia-server/packs/ |
| "未压缩合计 1.7 MB，压缩后 316 KB" | structure | verified | unchanged: `gzip -l packs/*.gz` → 313,173 compressed / 1,749,220 uncompressed bytes |
| "由 `src/scenario_packs.rs` 内嵌 ... 跑 `cargo test -p utopia-server scenario_packs`" | command | verified | crates/utopia-server/src/scenario_packs.rs; packs/scenarios/ has three JSON packs (static; test not executed) |

## Doc: crates/utopia-server/testdata/README.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "`cargo test -p utopia-server live_agent_plan_reads_scans -- --ignored --nocapture`" | command | verified | crates/utopia-server/src/readers/ark_ocr.rs:462 `live_agent_plan_reads_scans` (static; needs `ARK_API_KEY`, not executed) |
| "`ARK_OCR_TEST_PDF` ... `ARK_OCR_TEST_OUTPUT` ... `ARK_OCR_TEST_BASE_URL` and `ARK_OCR_TEST_MODEL`" | config | verified | crates/utopia-server/src/readers/ark_ocr.rs:464,467,530,553 |
| "`ark-ocr-pages.pdf` is a synthetic three-page PDF" | structure | verified | crates/utopia-server/testdata/ark-ocr-pages.pdf exists (page count not opened) |

## Doc: docs/decisions/0001-ontology-import-and-governance.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: In progress · P0–P2c, the P3 budget and P4a built; P5 delivered by 0002" | status | verified | crates/utopia-server/src/owl_import.rs, migrations/0004_ontology.sql, crates/utopia-reason/ (sampled; individual phases not each traced) |

## Doc: docs/decisions/0002-reasoning-engine.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: In progress · R0, R1 and R2 built · open: R3, incremental maintenance" | status | verified | crates/utopia-reason/src/derive.rs:172 `pub fn derive`; migrations/0013_reasoning.sql `fact_derivations`; no incremental-maintenance code found |

## Doc: docs/decisions/0003-ontology-growth-loop.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (#44)" | status | verified | unchanged: commit dd1f7b9 "Adopting a predicate moves the facts that were waiting for it" (#44) |

## Doc: docs/decisions/0004-language-and-localization.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented · revised 2026-11-03 (#1468)" | status | verified | commit d026704 (#1468); web/src/i18n/en.ts, zh.ts |

## Doc: docs/decisions/0005-alert-center.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented · open: `document.no_text_layer` is unwired" | status | stale | crates/utopia-store/src/alerts.rs:65 `DOCUMENT_NEEDS_READER = "document.needs_reader"` with a comment that this is the `document.no_text_layer` 0005 left empty; wired at crates/utopia-server/src/pipeline.rs:96 and tested in pipeline_tests.rs:745 |

## Doc: docs/decisions/0006-ontology-scale-and-the-prompt.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Superseded by 0044 · was built" | status | verified | migrations/0063_extraction_reads_only_the_open_graph.sql; docs/decisions/README.md index "Superseded / fully (by 0044)" |

## Doc: docs/decisions/0007-who-decides-what-becomes-a-relation.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (#112)" | status | verified | unchanged: commit b1b2bb8 (#112) |

## Doc: docs/decisions/0008-ontology-packs-as-cold-start.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented" | status | verified | crates/utopia-server/src/ontology_packs.rs:42-78 five embedded packs |

## Doc: docs/decisions/0009-no-type-is-a-type.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented · open: `metric` and `dimension` retire under 0036" | status | verified | migrations/0024_types_resolve_themselves.sql; `metric`/`dimension` are still the accepted mapping kinds at crates/utopia-server/src/api/mapping_routes.rs:264 |

## Doc: docs/decisions/0010-no-relation-is-no-relation.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented" | status | verified | crates/utopia-store/tests/store/no_predicate_still_shows.rs |

## Doc: docs/decisions/0011-a-mapping-is-not-a-fact.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (#126, #140, #148)" | status | verified | commits 97da5ea (#126), bc8620f (#140), 3726191 (#148); migrations/0006_semantic_layer.sql `concept_mappings` |

## Doc: docs/decisions/0012-the-ontology-is-a-contract-not-a-suggestion.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (#190, #196)" | status | verified | migrations/0019_a_signature_holds_on_every_path.sql (PR numbers not found as commit subjects; migration is the evidence) |

## Doc: docs/decisions/0013-a-source-should-hand-over-its-history.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented for GitHub, Jira, Notion and GitLab (#134, #135, #213, #1327) ... open: Feishu, Confluence" | status | verified | commits e32c158, 10c9648, 0f3ff5a, 4ed8a59, 63b8fd3; `SourceKind` has Github*, Jira, Notion, Gitlab*; no `feishu` in crates/ or web/src (grep repeated at d5d2356) |

## Doc: docs/decisions/0014-identity-from-the-person-scope-from-the-token.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented · 2026-10-16 · #180, #318, #1243, #1262, #1271" | status | verified | commits b5643ff (#180), 57680e9 (#318); migrations/0017_personal_tokens.sql; crates/utopia-server/src/api/token_routes.rs |

## Doc: docs/decisions/0015-recording-a-sentence-is-not-asserting-a-fact.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (#180) · open: `remember` over MCP" | status | stale | `"remember"` is served in crates/utopia-server/src/api/mcp.rs; commit 57680e9 "An agent can record over MCP, and the card says which agent" (#318); web/src/docs/mcp.md lists `remember` as an MCP tool |

## Doc: docs/decisions/0016-close-the-open-seams-before-cutting-new-ones.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: In progress · A, B1–B3, C1 and C2 done · open: B4, C3–C5, D" | status | verified | Sampled: A1 migrations/0018_a_fact_awaiting_a_nod.sql; C2 crates/utopia-core/src/models.rs:1252 "(0016 C2)"; C1 scripts/bench/truth/*.wikidata.json; B4 (R3) has no code |

## Doc: docs/decisions/0017-a-contradiction-points-upstream.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (#238, #243)" | status | verified | commits 52809eb, 417b49b; migrations/0020_a_contradiction_points_upstream.sql |

## Doc: docs/decisions/0018-the-lakehouse-is-one-protocol-away.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (#239) · open: Databricks and Snowflake against a real cluster (#241, #242), MaxCompute" | status | verified | commit f18c560; migrations/0021_lakehouse_engines.sql; crates/utopia-server/src/query_engine/ (real-cluster runs not checkable here) |

## Doc: docs/decisions/0019-the-second-clock-can-be-rewound.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (#317, #337, #549)" | status | verified | commits 35be42d, f3d05f3; migrations/0027_entities_rewind_too.sql; crates/utopia-store/src/record_axis.rs |

## Doc: docs/decisions/0020-an-auditor-reads-it-without-us.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented · revised 2026-10-23 (#1345) · open: SPARQL (#308)" | status | verified | crates/utopia-server/src/api/export_routes.rs:30 (`turtle` or `jsonld`), crates/utopia-store/src/export.rs; no SPARQL endpoint in api/mod.rs |

## Doc: docs/decisions/0021-a-rule-reads-attributes-and-concludes-a-type.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (#359)" | status | verified | commit 0b18c7c; migrations/0028_a_rule_reads_attributes.sql |

## Doc: docs/decisions/0022-an-unknown-date-is-not-an-open-one.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented in five cuts (#394, #393, #679) · revised 2026-10-03 (#1054)" | status | verified | migrations/0030_an_unknown_date_is_not_an_open_one.sql; commits 573fda3 (#679), b4b1123 (#1054) |

## Doc: docs/decisions/0023-rss-observations-are-not-documents.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (#326)" | status | verified | commit 28db351; migrations/0032_rss_full_content.sql |

## Doc: docs/decisions/0024-the-world-axis-reaches-the-second.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (migration 0033)" | status | verified | migrations/0033_the_world_axis_reaches_the_second.sql |

## Doc: docs/decisions/0025-governance-reads-the-ledger-before-it-decides.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented · cuts 1–4 (#434, #437)" | status | verified | commits 1e061e7, d391b99; migrations/0035_governance_reads_the_ledger.sql |

## Doc: docs/decisions/0026-a-decision-records-why.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (migration 0038)" | status | verified | migrations/0038_a_decision_records_why.sql |

## Doc: docs/decisions/0027-an-automatic-merge-is-gated-by-what-it-can-undo.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented" | status | verified | migrations/0037_the_fuse_trips_on_the_second_revert.sql; crates/utopia-store/src/execution_gate.rs |

## Doc: docs/decisions/0028-the-adjudicator-looks-before-it-asks.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented" | status | verified | crates/utopia-extract/src/governor.rs:4 cites 0028; migrations/0036_the_agent_asks_before_it_guesses.sql |

## Doc: docs/decisions/0029-a-rule-may-say-or-once.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (migration 0039)" | status | verified | migrations/0039_a_rule_can_say_or.sql |

## Doc: docs/decisions/0030-a-rule-may-read-what-a-rule-concluded.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: In progress · cuts 1–2 built (migration 0040)" | status | verified | migrations/0040_a_premise_may_be_derived.sql |

## Doc: docs/decisions/0031-an-event-holds-at-the-moment-it-names.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (#486, #593) · open: the UI cut" | status | verified | unchanged: commit 2838296 "A relation says whether it happens or holds" (#593) |

## Doc: docs/decisions/0032-a-rule-computes-what-it-concludes.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: In progress · cuts 1–2 built (migration 0041)" | status | verified | migrations/0041_a_rule_computes_what_it_concludes.sql |

## Doc: docs/decisions/0033-rss-source-summaries-are-source-scoped.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (#462, #417)" | status | verified | commit 90f7c15 (#462); crates/utopia-store/src/rss_full_content.rs |

## Doc: docs/decisions/0034-an-action-is-a-declared-call.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Proposed · no code" | status | verified | No `actions`/`action_params`/`action_grants`/`action_runs` tables in migrations/ and no such symbols in crates/ |

## Doc: docs/decisions/0035-a-vector-index-is-built-by-a-job.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (#512, #514)" | status | verified | crates/utopia-store/src/vector_index.rs (PR numbers not found as commit subjects; module is the evidence) |

## Doc: docs/decisions/0036-exploration-aligns-a-schema-to-the-ontology.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: In progress · conversion expressions (decision 3) and decision 7 built · open: #554, #556" | status | verified | migrations/0044, 0045 (schema document leaves extraction), 0047; `metric`/`dimension` still accepted at crates/utopia-server/src/api/mapping_routes.rs:264 (open part) |

## Doc: docs/decisions/0037-a-relation-carries-its-own-attributes.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: In progress · cuts 1 and 1b built (#598, #600)" | status | verified | commits 0e3123b, 1333049; migrations/0049_a_relation_declares_its_qualifiers.sql |

## Doc: docs/decisions/0038-the-interface-has-a-light-side.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (#599)" | status | verified | commit 2767877; web/src/theme.ts |

## Doc: docs/decisions/0039-a-chunk-is-what-extraction-sees.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: In progress · cut 1 built · open: cut 2, an external parser" | status | verified | crates/utopia-ingest/src/chunker.rs:44-45 cites 0039, `BUDGET_TOKENS = 300` |

## Doc: docs/decisions/0040-a-chunk-says-where-its-words-came-from.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented · cuts 1–3 ... (migrations 0058–0060)" | status | verified | migrations/0058, 0059, 0060 present |

## Doc: docs/decisions/0041-a-name-is-a-claim-about-an-entity.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: In progress · cut 1 built (#670), cut 2 channel 2 built 2026-09-23, revised 2026-09-25 (#889)" | status | verified | commits f680576, 2f90d8b; migrations/0055_a_name_is_a_fact.sql, 0080_a_name_has_a_vector.sql |

## Doc: docs/decisions/0042-the-chat-loop-is-a-runner-with-hooks.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (#548) · revised 2026-09-26 (#937) · revised 2026-09-27 (#973)" | status | verified | commits 52186fb "The chat loop runs on rig", e5b5681; `rig-core` in crates/utopia-server/Cargo.toml; crates/utopia-server/src/api/agent.rs |

## Doc: docs/decisions/0043-every-review-queue-is-governed.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Accepted 2026-09-14 · no code on `dev` (#699 closed)" | status | verified | The only `0043` in crates/*/src is crates/utopia-store/src/documents.rs:1512, which names the sha index of migrations/0043_a_blob_is_looked_up_by_its_hash.sql, not this record; no commit with subject "(#699)" in history; 0076 builds its own ledger instead of 0043's (absence evidence only) |

## Doc: docs/decisions/0044-the-ontology-is-a-view-over-what-documents-say.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: In progress · accepted 2026-09-17 · cut 1 built (#731), cut 2 mostly built (#751, #876, #878, #882), cut 5 is 0061, cut 6 built (#885)" | status | verified | unchanged: commits 60df635, f760c76, 60cd5d3, 5fc36e7, 58b4bf9, ef6495c; migrations 0061-0068, 0074 (bench percentages in the status line not reproduced) |

## Doc: docs/decisions/0045-a-time-mention-is-resolved-against-its-document.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: In progress · accepted 2026-09-17 · cuts 1–3 built (#740, #761) · open: cut 4" | status | verified | commit 24a85df (#740); migrations/0064_a_time_mention_is_resolved_against_its_document.sql; crates/utopia-server/src/time_resolution.rs (note: the commit carrying "(#761)" is "Fix GBK-encoded PDF text extraction") |

## Doc: docs/decisions/0046-the-app-surface-is-mcp.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Accepted 2026-09-19 · a refusal: no app center, no component runtime, no sandbox" | status | verified | crates/utopia-server/src/api/mod.rs has the MCP route and no app-center/runtime routes |

## Doc: docs/decisions/0047-a-rule-may-conclude-a-relation.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented 2026-09-25 (#861, migration 0071)" | status | verified | commit 535aac0; migrations/0071_a_rule_joins_two_entities.sql |

## Doc: docs/decisions/0048-provenance-references-stay-inside-the-knowledge-base.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (#832, migration 0070)" | status | verified | commit ea0557b; migrations/0070_provenance_never_points_across_a_knowledge_base.sql |

## Doc: docs/decisions/0049-expression-declarations-are-checked-when-a-rule-is-written.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Proposed" | status | verified | No reference to 0049 in crates/*/src (grep); docs/design/rules.md lists unit/datatype checks at write time under "Proposed and not built" (absence evidence only) |

## Doc: docs/decisions/0050-an-action-attempt-keeps-its-identity-and-uncertain-outcome.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Proposed · documentation only" | status | verified | No `action_attempt`/action tables or symbols in crates/ or migrations/ |

## Doc: docs/decisions/0051-a-human-phrase-decision-carries-its-materialization-work.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented · 2026-09-23 (#876) · revised 2026-10-12 (#1137)" | status | verified | commits 60cd5d3, b7f756e; crates/utopia-store/src/materialize.rs; migrations/0123 |

## Doc: docs/decisions/0052-document-content-is-a-read-contract.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Proposed" | status | stale | crates/utopia-server/src/api/documents_routes.rs:219 cites (0052) for shipped behaviour; routes `/documents/{id}/content` (api/mod.rs:427) and `.../documents/by-external-id` (:486) exist; docs/design/access-and-audit.md cites [0052] under "What it does today". How much of the record is built was not traced |

## Doc: docs/decisions/0053-a-phrase-decision-records-the-inputs-it-considered.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented 2026-09-23 (#878, migration 0072) · revised 2026-09-26 (migration 0092) · revised 2026-09-27 (#975, migration 0098) · revised 2026-10-17 (#1295, migration 0114)" | status | verified | unchanged: migrations 0072, 0092, 0098, 0114 present; commits 5fc36e7, 7ceead2, fea5171 |

## Doc: docs/decisions/0054-a-source-may-push-statements-in-the-open-contract.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented · cut 1 (#884) · revised 2026-10-23 (#1375)" | status | verified | commits 22c21ea, ead46cb; crates/utopia-server/src/api/mod.rs:509 `/sources/{source_id}/statements`; `SourceKind::Statements` |

## Doc: docs/decisions/0060-a-rule-definition-has-a-history.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented 2026-09-25 (#913, migration 0076)" | status | verified | commit a656cb0; migrations/0076_a_rule_definition_has_a_history.sql |

## Doc: docs/decisions/0061-the-ontology-is-proposed-from-the-open-graph-and-judged-by-its-questions.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: In progress · accepted 2026-09-26 · cuts 1 and 1.1 built (#947, #950), cut 3 slices 1–2 (#955, #962)" | status | verified | commits b07af8f, 4f9458c, 6f74a08, 6ef840a; migrations 0077, 0078; crates/utopia-server/src/ontology_agent.rs |

## Doc: docs/decisions/0062-the-export-says-what-is-contested.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented 2026-09-26 (#922, migration 0093)" | status | verified | commit 06768aa; migrations/0093_an_axiom_that_moved_closes_its_findings.sql |

## Doc: docs/decisions/0063-stopping-a-chat-ends-the-generation.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented 2026-09-27 (#969, migration 0095)" | status | verified | commit d9c3aab; migrations/0095_a_stopped_chat_keeps_its_partial_answer.sql |

## Doc: docs/decisions/0064-extraction-reads-time-as-it-goes.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: In progress · accepted 2026-09-28 · cut 1 built (#990, the bench) · cut 2 built (#991) · cut 3 built (#995, migration 0100)" | status | verified | commits 841b619, e6c7647, ea1fde8; migrations/0100; scripts/bench/timewords.mjs |

## Doc: docs/decisions/0065-readers-choose-their-provider.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented · 2026-09-30 · migration 0102 · open: none" | status | verified | migrations/0102_readers_choose_their_provider.sql; crates/utopia-server/src/readers/ |

## Doc: docs/decisions/0066-another-app-may-trade-a-fresh-id-token-for-a-session.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented · 2026-10-02 · #1010, migration 0103" | status | verified | migrations/0103 `CREATE TABLE oidc_exchanges`; api/mod.rs:127 `/auth/oidc/exchange` |

## Doc: docs/decisions/0067-each-task-chooses-its-model.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Accepted · 2026-10-19 · #470 · open: cuts 1–4" | status | verified | Record landed in commit ccd4203 "ADR 0067: each task chooses its model (#1239)"; no `on_model_select` or per-task model symbols in crates/*/src |

## Doc: docs/decisions/0069-the-identity-provider-vouches-for-listed-domains.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented · 2026-10-16 · #1237, #1258, #1362" | status | verified | commits 2bce328, ff52e45, 8ea0612; crates/utopia-server/src/api/oidc_routes.rs |

## Doc: docs/decisions/0070-a-role-word-speaks-while-its-chunk-is-live.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: In progress · 2026-10-25 · decision 1 built (migration 0121), decisions 2 and 3 built (`record_axis::role_word_speaks_given` ...)" | status | verified | migrations/0121_a_role_word_is_recorded_against_its_chunk.sql; `role_word_speaks_given` in crates/utopia-store/src/record_axis.rs |

## Doc: docs/decisions/0071-a-source-may-declare-its-names-identities.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented · 2026-10-25 · #1333, #1390, #1425, migration 0125" | status | verified | commits 50904e9, 00f1172, 3b09f9f; migrations/0125 present |

## Doc: docs/decisions/0072-a-source-checkpoint-ends-at-a-timestamp-boundary.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Implemented 2026-11-10 (#1359, #1397, migration 0119)" | status | verified | commits 6431a5a, 3d307f6; migrations/0119_a_source_checkpoint_ends_at_a_timestamp_boundary.sql |

## Doc: docs/decisions/0073-a-scenario-pack-is-a-vocabulary-with-its-questions.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: In progress · decisions 1 to 3 and the first three packs built (`scenario_packs.rs`, `packs/scenarios/`, `CreateKbReq.scenario_packs`, `GET /scenario-packs`)" | status | verified | crates/utopia-server/src/scenario_packs.rs; packs/scenarios/ has three JSON files; `scenario_packs` in api/kbs.rs; `scenario-packs` route in api/mod.rs |

## Doc: docs/decisions/0074-a-google-drive-source-is-connected-by-a-person.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented · 2026-10-23 · migration 0120" | status | verified | migrations/0120; crates/utopia-server/src/google_drive.rs, api/google_drive_routes.rs ("tried by hand against a real Drive" not checkable) |

## Doc: docs/decisions/0075-a-ticket-source-says-where-its-reading-stopped.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented · 2026-10-28 · no migration (the column came with 0119)" | status | verified | migrations/0119 present; crates/utopia-server/src/api/sources_checkpoint_tests.rs |

## Doc: docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: In progress · accepted 2026-11-18 · the keep slice built 2026-11-19 (migration 0128): a personal token reads Duplicates and Overview and keeps one duplicate pair through the policy function, over the token API and the MCP tools `review_list`, `review_item` and `review_keep`, with the `token_agent_actions` ledger, the precedent exclusion, and "owner via token name" in Agent History and Activity" | status | verified | migrations/0128_a_token_agent_action_is_kept_in_its_own_ledger.sql; crates/utopia-server/src/api/review_ops.rs:112-125 (list), :201-286 (item), :347-353 (summary), :361-388 (keep, through `agent_policy` at :370); crates/utopia-server/src/api/review_tools.rs:31-33; crates/utopia-store/src/token_agent_actions.rs:141-271; crates/utopia-store/src/governance.rs:85; web/src/pages/TokenAgentRow.tsx:11, web/src/pages/Review.tsx:2370, web/src/pages/KbSettings.tsx:21, :658-659 (static; DB-backed tests not executed) |
| "the read slice built 2026-11-19 (no migration): a personal token lists all thirteen Review sections with a `total`, reads one listed item of any section and both proof routes" | status | verified | crates/utopia-server/src/api/review_ops.rs:89-101 `TOKEN_SECTIONS` (eleven queue keys), :130-142 `history`, :347-353 `summary`, :201-286 `item`, :147-168 both proofs; crates/utopia-server/src/api/review_routes.rs:216-221 (`total` in the list body); crates/utopia-server/src/api/graph_routes.rs:540-542, :562-564 (`SessionOrToken`); tests review_token_reads_tests.rs:198, :994 (not executed) |
| "the gated-write slice built 2026-11-19 (migration 0129): a write token of an editor merges a duplicate pair or two entities behind the merge gate, reverts a merge a token agent made and batches up to 50 duplicate pairs with a unit revert, over the token API and the MCP tools `review_merge`, `review_revert` and `review_batch`; a held or propose-only call changes nothing and waits in the ledger as a proposal that only a signed-in editor answers; finalized token writes are limited to 200 per rolling hour and a per-token breaker makes a token propose only" | status | verified | migrations/0129_a_token_agent_action_keeps_its_reason_its_batch_its_merge_and_its_later_state.sql:30-36, :56-58; crates/utopia-server/src/api/review_ops.rs:489-506 `merge_pair`, :687-705 `merge_entities`, :720-737 `revert_merge`, :554-612 `batch`, :664-682 `revert_batch`, :536 `TOKEN_BATCH_MAX: usize = 50`, :290 `TOKEN_WRITES_PER_HOUR: i64 = 200`, :317-344 `guard`; crates/utopia-server/src/api/review_tools.rs:34-36; crates/utopia-store/src/token_agent_actions.rs:434-495 `land_proposal` (ledger row and `agent.proposal` audit row, nothing else written); crates/utopia-server/src/api/review_routes.rs:921-923, :959-961 (the two answer handlers take `AuthUser`); `ls migrations` → 115 files, highest 0129 (static; DB-backed tests not executed) |
| "open: this record has opened the Keep, Merge, Revert merge and Batch rows of the table to tokens, and the alignment kind-word decision was already open to write tokens under ADR 0014 decision 6; every other row stays closed to tokens (mapping decision, the consistency check, and every propose-only operation, for which the proposal store holds nothing yet: it holds merge and revert proposals only)" | status | verified | Token branches in crates/utopia-server/src/api/review_routes.rs: :360-374 (decision route: `keep`, `merge`), :1022-1034 (batch), :1170-1192 (unit revert), :1277-1300 (revert), :1353-1365 (manual merge); :2091-2097 `decide_alignment_kind_word` takes `SessionOrToken` and calls `require_write`. `AuthUser` only at :252-254 (close), :313-315 (conflict), :439-441 (governor answer), :1209-1211, :1233-1235 (confirm, reject), :1475-1477 (mapping), :1528-1530, :1734-1736 (violation, defect), :1679-1681, :1863-1865 (consistency check, inference), :1800-1802 (nod), :1935-1937, :1973-1975 (alignment rule, phrase), :2155-2157 (errata). The proposal store's operations are the three `answer_proposal` knows (:866-869: `review.merge`, `merge.manual`, `merge.revert`) |
| "The handlers behind `GET /api/v1/kbs/{kb_id}/review?queue=duplicates`, `GET /api/v1/kbs/{kb_id}/review/summary` and `POST /api/v1/kbs/{kb_id}/review/{review_id}` (action `keep`) take `SessionOrToken` instead of `AuthUser`" (decision 1) | feature | verified | crates/utopia-server/src/api/review_routes.rs:85-87 (`list`), :353-355 (`decide`), :1448-1450 (`summary`) take `caller: SessionOrToken`; routes at crates/utopia-server/src/api/mod.rs:534, :536, :565; the session branch runs the earlier code after the `caller.pat` early return (:92, :360, :1454) |
| "The same extractor change now also covers the Decisions history (`GET /api/v1/kbs/{kb_id}/review/history`) and the two proof routes ... and the list handler serves a token all eleven queue sections" (decision 1, read-slice revision) | feature | verified | crates/utopia-server/src/api/review_routes.rs:1424-1426 (`history` takes `SessionOrToken`); crates/utopia-server/src/api/graph_routes.rs:540-542 `derived_proof`, :562-564 `blocked_proof`; routes at crates/utopia-server/src/api/mod.rs:535, :459, :464; crates/utopia-server/src/api/review_ops.rs:89-101, :121-123 (eleven sections for a token). The revision's closing "No write handler changed" describes the read slice only; decision 14 (docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md:364-372) records the four handlers the next slice changed |
| "`review_list` takes a `section` parameter and applies the existing 1 to 200 page clamp; `review_item` returns one item's evidence; `review_keep` keeps one duplicate pair ... The write tool is listed and callable only with scope `write` and an owner at least Editor" (decision 2) | feature | verified | crates/utopia-server/src/api/review_tools.rs:31-33, :378-399 (`listed` drops every write tool unless `can_write`), :498-559; crates/utopia-server/src/api/review_routes.rs:134 `clamp(1, 200)`; crates/utopia-server/src/api/mcp.rs:107-110 (`can_write` = write scope and owner at least Editor); a caller without the right that calls it anyway is refused inside crates/utopia-server/src/api/review_ops.rs:69-84 |
| "`review_list` now serves all thirteen sections: the eleven queue sections with `limit` and `offset`, `decisions` with the history route's own `page` and `per`, and `overview` with no paging argument. `review_item` takes `section` and `id`; `section` left out means `duplicates` ... A name outside the thirteen still answers `unknown_queue`" (decision 2, read-slice revision) | feature | verified | crates/utopia-server/src/api/review_tools.rs:504-533 (`overview` → summary, `decisions` → history with `page`/`per`, anything else → list with `limit`/`offset`), :535-545 (`section` defaults to `duplicates`); crates/utopia-server/src/api/review_ops.rs:104-106, :121-123, :242-244 (`unknown_queue`); tests review_mcp_reads_tests.rs:193, :843, :964 (not executed) |
| "There are now six: `review_merge`, `review_revert` and `review_batch` were added, one tool per operation family, listed and callable under the rule `review_keep` follows (scope `write`, owner at least Editor). Each calls the same operation function as its route (decision 5), and no tool answers a proposal" (decision 2, gated-write revision) | feature | verified | crates/utopia-server/src/api/review_tools.rs:31-36 (six names), :39-46 (`serves`, `writes`), :378-399, :560-656 (each write tool calls `review_ops::merge_pair` / `merge_entities` / `revert_merge` / `revert_batch` / `batch`, the functions the REST token branches call at crates/utopia-server/src/api/review_routes.rs:364-365, :1025, :1190, :1297, :1356); a tool name not in `serves` gets JSON-RPC -32601 at crates/utopia-server/src/api/mcp.rs:234-247; test review_mcp_reads_tests.rs:1040, review_proposal_answer_tests.rs:1075 (not executed) |
| "The table is compiled in. It is not a per-base setting" / "The policy function lives with the `execution_gate` module ... answers `applied`, `held` or `proposed`" (decisions 3 to 5) | structure | verified | crates/utopia-store/src/execution_gate.rs:149-171 `AgentOperation` (nineteen rows), :186-190 `AgentOutcome`, :206-221 `agent_ruling` (a `match` with no wildcard arm), :226-235 `agent_policy` |
| "The Merge, Revert merge and Batch rows now are [reachable by a token], through `review_ops::merge_pair`, `merge_entities`, `revert_merge`, `batch` and `revert_batch`. The gate is asked inside the store functions those call (`hold_for_token_merge`, then `agent_policy`, before the merge transaction opens) ... The session branch of each handler is unchanged and still calls no gate" (decision 5, gated-write revision) | feature | verified | crates/utopia-server/src/api/review_ops.rs:489, :687, :720, :554, :664; crates/utopia-store/src/token_agent_actions.rs:587-588 (`hold_for_token_merge`, then `held_back` → `agent_policy` at :416) before `pool.begin()` at :621, and :765-766 before :796; crates/utopia-store/src/execution_gate.rs:279-318. Session branches in crates/utopia-server/src/api/review_routes.rs:375-424, :1035-1100, :1301-1331, :1366-1405 call `resolution::decide_review`, `decide_reviews`, `revert_merge` and `merge_entities` and no gate; tests review_session_unchanged_tests.rs:233, :492 and review_token_merge_tests.rs:1855 (not executed) |
| "A read needs an owner at least Viewer and a token that reaches the base; a token whose `kb_ids` exclude the base gets 404" with its revision: "That is the token API's answer only. The MCP endpoint checks the token's base list itself, before any tool runs, and answers such a token with HTTP 403 ... A dead or revoked token gets 401 on both" (decision 6) | feature | verified | Token API: crates/utopia-server/src/api/review_ops.rs:52-64 turns an owner below Viewer and an excluded base into `NotFound`. MCP: crates/utopia-server/src/api/mcp.rs:96-98 returns `AppError::Forbidden` inside `authorize`, called first in `handle` (:177); crates/utopia-server/src/error.rs:33-35 maps NotFound → 404, Unauthorized → 401, Forbidden → 403; crates/utopia-store/src/tokens.rs:157-170 (unknown, revoked or expired → `Unauthorized`); test review_token_reads_tests.rs:759 (not executed) |
| Codes table: `token_read_only`, `token_kb_out_of_scope`, `token_owner_below_editor`, `already_decided`, `token_operation_not_open`; "scope is checked first" (decision 6) | feature | verified | crates/utopia-server/src/auth.rs:247-268 (`require_token_write`: scope, then `kb_ids`); crates/utopia-server/src/api/review_ops.rs:41-47, :69-84; crates/utopia-store/src/token_agent_actions.rs:273-278 (`CodedConflict` `already_decided`); crates/utopia-server/src/error.rs:36-43 |
| "The ledger is `token_agent_actions` ... base, operation, target kind and id, owner, token, token name snapshot, outcome, request payload, time ... told apart by a `source` field" (decision 7) | structure | verified | migrations/0128_a_token_agent_action_is_kept_in_its_own_ledger.sql (columns kb_id, operation, target_kind, target_id, owner_id, token_id, token_name, outcome, payload, created_at); crates/utopia-store/src/token_agent_actions.rs:1420-1436 `"source": "token"`, :1477-1547 `agent_history` (governor rows taken unchanged from `governance::list`); crates/utopia-server/src/api/review_routes.rs:199-205 |
| "Migration 0129 adds them in one file, all nullable: `gate_reason`, `batch_id`, `merge_id`, `target_key`, `state`, `state_at` and `state_by` ... the partial unique index `token_agent_actions_open_proposal_idx` on `(kb_id, operation, target_key)` over rows with outcome `held` or `proposed` and no `state` ... `undone` is read from the merge, not stored ... The column's check constraint admits `undone`, but no statement writes it" (decision 7, gated-write revision) | structure | verified | migrations/0129_a_token_agent_action_keeps_its_reason_its_batch_its_merge_and_its_later_state.sql:30-36 (seven `ADD COLUMN IF NOT EXISTS`, none `NOT NULL`), :47-49 (check admits `undone`), :56-58 (the index); crates/utopia-store/src/token_agent_actions.rs:83-92 (`undone` computed from `entity_merges.reverted_at`), :346-380 (`ON CONFLICT ... DO NOTHING` on that index), :598, :777, :946 (the three target keys), :1088-1092 (`answer` stores only `approved`, `declined`, `superseded`); `grep "'undone'"` over crates outside tests → only crates/utopia-store/src/token_agent_actions.rs:88; :1437-1451 (the five history keys inserted only when they have a value) |
| "Every token Review write adds `detail.token_id` and a `detail.token_name` snapshot ... The stored HTTP `User-Agent` is returned to readers marked as declared ... A failed snapshot or audit write fails the action" (decision 8) | feature | verified | crates/utopia-store/src/token_agent_actions.rs:196-205, :317-327 (name read in the transaction; a missing name is an error), :251-267 and :385-404 (`token_id`, `token_name` in the audit detail, written with `record_on` inside the transaction), :269 (commit after it); crates/utopia-store/src/audit.rs:189-199 `declared_client`; web/src/pages/TokenAgentRow.tsx:11; test review_mcp_tests.rs:1040 (not executed) |
| "The governor's precedent queries and `settled_by_people` exclude audit rows whose detail carries a token id and review rows marked by the new nullable `resolution_reviews.decided_token`. `after_human_decision` is not called for a token decision ... `review/agent/{decision_id}`, stays session-only" (decision 9) | feature | verified | crates/utopia-store/src/governance.rs:85 `NOT_BY_TOKEN`, used at :101, :114, :128, :842; `decided_token IS NULL` at :177, :610; migrations/0128 `ADD COLUMN IF NOT EXISTS decided_token UUID`; crates/utopia-server/src/api/review_ops.rs:361-388, :489-506, :687-705 make no `after_human_decision` call; crates/utopia-server/src/api/review_routes.rs:439-441 `agent_answer` takes `AuthUser`; tests crates/utopia-store/tests/a_token_keep_is_never_human_precedent.rs:842, review_token_api_tests.rs:568 (not executed) |
| "the session cap stays 500" (decision 10) | config | verified | crates/utopia-server/src/api/review_routes.rs:1007 `BATCH_MAX: usize = 500`, :1036 |
| "The constants are `review_ops::TOKEN_BATCH_MAX` (50) and `review_ops::TOKEN_WRITES_PER_HOUR` (200). One function, `review_ops::guard`, runs before every token call that would finalize (`keep`, `merge_pair`, `merge_entities`, `revert_merge`), after the write checks and before the store function" with its two bullets (429 `agent_rate_limited` at 200 or more applied rows in the rolling hour; the breaker on `governance::FUSE_WINDOW_DAYS` (7) and `governance::FUSE_REVERTS` (2), keeps still finalize) (decision 10, gated-write revision) | config | verified | crates/utopia-server/src/api/review_ops.rs:536, :290, :317-344 (`finalized >= TOKEN_WRITES_PER_HOUR` → `rate_limited()` :330-332; breaker asked only for `Merge` and `RevertMerge`, `Ok(false)` otherwise :333-343), called at :375 (keep), :497, :695, :728, each after `authorize_write`; :293-301 `AppError::RateLimited` with `agent_rate_limited`; crates/utopia-server/src/error.rs:45-48 (429); crates/utopia-store/src/token_agent_actions.rs:1170-1185 (`outcome = 'applied'` rows of that token and base since the given time), :1200-1225 (reverted merges this token made, no applied token revert naming them or a person's `merge.revert` audit row), :423-425 (`breaker` → proposed); crates/utopia-store/src/governance.rs:1088-1089; tests review_token_guard_tests.rs:370, :531, :698 (not executed) |
| "A merge revert restores graph state only" with its revision: "every `facts` row of the base, every `entities` row except `updated_at`, and every `fact_evidence` row are equal before the merge and after the revert; for `typed_fact_sources`, every row present before the merge is present after the revert ... The existing revert was not changed" (decision 11) | feature | verified | The comparison is the one the test makes: crates/utopia-store/tests/a_token_merge_is_gated_recorded_and_undone.rs:366-371 (`to_jsonb(e) - 'updated_at'`, four state parts), :421-447 (three parts equal, `typed_fact_sources` may only gain the copied link), :1727, :1795-1796 (not executed); `git diff 6c52ce8 d5d2356 -- crates/utopia-store/src/resolution.rs` has no hunk between new lines 2600 and 2789, where `revert_merge` (:2654) sits; the tool description says the same at crates/utopia-server/src/api/review_tools.rs:271-276 |
| "Open to a write token of an editor now: keep, merge and manual merge behind the gate, revert of a token agent's merge, batch on duplicate pairs and its unit revert. Still closed to every token: answering a proposal, its own included (the `agent-proposals` routes take a session only); the governor's decision route; every propose-only row of the table ...; the mapping decision and the consistency check" (decision 12, gated-write revision) | status | verified | Open: crates/utopia-server/src/api/review_routes.rs:360-374, :1022-1034, :1170-1192, :1277-1300, :1353-1365. Closed: :921-923, :959-961 (answer routes, `AuthUser`), :439-441 (governor), :1475-1477 (mapping), :1679-1681 (consistency check), and the other `AuthUser` handlers listed in the status row above. The decision's first paragraph and its two earlier revisions (docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md:279-301) still list merge, manual merge, revert and batch as closed; each is kept as dated history and is superseded by this revision at :303-316, which names them as built |
| "`total` is a new field; `counts` is unchanged ... for Alignment (the badge omits proposed implication rules), Low confidence and Agent History the badge named after the section is not the number of listed rows" / "The Low confidence list is unchanged, and its total counts the listed rows, derived facts included" (decision 13) | feature | verified | crates/utopia-server/src/api/review_routes.rs:137-221 (`total` computed per queue beside `counts`); crates/utopia-store/src/review.rs:80 (`lowconf` badge keeps `derived_by_rule IS NULL`), :88-89 (`agent` badge counts `status = 'proposed'` only), :101-102 (`alignment` badge unions phrase and type bindings, no `implication_rules`); crates/utopia-store/src/alignment_queue.rs:235 `total`; crates/utopia-store/src/graph.rs:2105-2107; crates/utopia-store/src/token_agent_actions.rs:1463-1468; tests review_totals_tests.rs:217, :395 (not executed) |
| "Decisions keeps `page` and `per` and its `{events, total}`" / "The Alignment list has an identifier tie-break. It is ordered by `decided_at` and then by the item's own identifier" (decision 13) | feature | verified | crates/utopia-server/src/api/review_ops.rs:130-142; crates/utopia-server/src/api/review_tools.rs:511-521; crates/utopia-store/src/alignment_queue.rs:162-223 (`OrderKey = (decided_at, id or kind_word, kind index)`, sorted before `skip`/`take`); tests review_totals_tests.rs:513, review_token_reads_tests.rs:640 (not executed) |
| "`review_item` identifies an item by `section` and `id` ... an Alignment kind word has none and is named by its word, compared exactly as stored ... There is one not-found answer ... `overview` lists no items and gives the same answer" / "An Axioms item carries a `proof` key" / "`mappings` is refused for a token with `unknown_queue`. The list route serves it to a session" (decision 13) | feature | verified | crates/utopia-server/src/api/review_ops.rs:176-183 `is_the_item` (exact `kind_word` compare, no trim or case fold), :209-211 (`overview` → `NotFound`), :242-247, :269-271 (one `NotFound`), :274-284 (`proof` only for `violations`, null unless `derived_contradiction`), :89-101 (`mappings` not among token sections); crates/utopia-server/src/api/review_routes.rs:171-174 (`mappings` arm for a session); tests review_mcp_reads_tests.rs:518, :616, :843, :964 (not executed) |
| "reading "Awaiting your nod" gives a token no way to nod" (decision 13) | feature | verified | crates/utopia-server/src/api/review_routes.rs:1800-1802 `decide_pending` takes `AuthUser` (route at crates/utopia-server/src/api/mod.rs:562-563); no Review tool nods (crates/utopia-server/src/api/review_tools.rs:31-36); test review_token_reads_tests.rs:1049 (not executed). The same bullet's "No Review write is added ... no MCP write tool was added" speaks of the read slice, under the heading "Added 2026-11-19"; decision 14 records the writes the next slice added |
| "Four existing handlers change extractor in place and keep their session branch: the decision route ..., the manual merge ..., the revert ... and the batch ... Three routes are new: the unit revert `POST /api/v1/kbs/{kb_id}/review/agent-batches/{batch_id}/revert` and the two proposal answer routes under `/api/v1/kbs/{kb_id}/review/agent-proposals`" (decision 14) | structure | verified | crates/utopia-server/src/api/review_routes.rs:353-355 `decide`, :1346-1348 `manual_merge`, :1270-1272 `revert_merge`, :1015-1017 `batch` (each `caller: SessionOrToken`, session code after the `caller.pat` return); :1163-1165 `agent_batch_revert`, :921-923, :959-961; routes at crates/utopia-server/src/api/mod.rs:565, :630-631, :622-623, :537, :553-554, :548-549, :544-545 |
| "The token merge gate is `hold_for_merge` (identity source, functional contradiction, live derivation, entity named in an answer) followed by two holds only a token merge gets, with the reason codes `identity`, `contradiction`, `derived`, `answered`, `kept_by_person` and `reverted_merge` ... Both are looked up by the two entity ids ... The product's own deciders keep calling `hold_for_merge` and never see the two new holds" (decision 14) | feature | verified | crates/utopia-store/src/execution_gate.rs:239-267 (`hold_for_merge`: identity first, then `hold` at :133-144 in the order contradiction, derived, answered), :279-318 (`hold_for_token_merge`: the latest decided review of the two entities in either order is `kept` with `decided_by` set and `decided_token` null → `KeptByPerson`; any reverted merge between them in either direction → `RevertedMerge`), :73-82 (the six codes); crates/utopia-store/src/token_agent_actions.rs:587, :765 (queue merge and manual merge both call it with entity ids); tests crates/utopia-store/tests/a_token_merge_is_gated_recorded_and_undone.rs:1416, review_token_merge_tests.rs:1389 (not executed) |
| "A merge the gate holds, and a call that may only propose, answer HTTP 200 with outcome `held` or `proposed`, a stable `reason` and an `explanation` sentence ... the ledger row holds the full request and an `agent.proposal` audit row is written with it. The propose-only reasons are `merge_by_person`, `merge_by_governor` and `breaker`" (decision 14) | feature | verified | crates/utopia-server/src/api/review_ops.rs:464-480 `merge_answer` (an `Ok` body with `outcome`, `reason`, `explanation`), :434-457; crates/utopia-store/src/token_agent_actions.rs:434-470 (one transaction: ledger row with the payload, then `agent.proposal` through `record_on`; no graph statement), :423-425 (`breaker`), :934-939 (`merge_by_person` when `merged_by` is set, else `merge_by_governor`) |
| "One open proposal per base, operation and target ... A queue merge's target is its review id; a manual merge's is the unordered pair of entity ids ...; a revert's is the merge id" / "A repeat, or a second call at the same moment, writes no row and returns the open proposal's id with `"code": "already_proposed"`" (decision 14) | feature | verified | crates/utopia-store/src/token_agent_actions.rs:598 (review id), :777 (`source.min(target)` and `source.max(target)` joined by a pipe character), :946 (merge id), :346-380 and :471-488 (a conflicting insert writes nothing and the open row is read back with `existing: true`); crates/utopia-server/src/api/review_ops.rs:475-477; test review_token_merge_tests.rs:1580 (not executed) |
| "The merge, the review row, the ledger row and the audit row commit in one transaction. The steps after the commit can fail; the answer is then still `applied`, with `follow_up_error`" (decision 14) | feature | verified | crates/utopia-store/src/token_agent_actions.rs:621-683 (one `tx`: `merge_entities_in`, the review `UPDATE`, `insert_row`, `record_on`, commit), :685-690 and :505-517 (`follow_up` after the commit returns its error as a string); crates/utopia-server/src/api/review_ops.rs:467-471; crates/utopia-store/src/resolution.rs:2564 `merge_follow_up` |
| "A merge the ledger shows a token agent made on that base, through whichever token, and that is not reverted, is reverted outright. A merge a person made (one made by approving a proposal included) or the governor made is a proposal ... An unparseable body on the token revert is 422 `invalid_body`" (decision 14) | feature | verified | crates/utopia-store/src/token_agent_actions.rs:920-922 (already reverted → `already_decided`), :925-939 (`by_token_agent` = an applied `review.merge` or `merge.manual` row with that `merge_id`, any token; an approved proposal's row is not `applied`, so its merge counts as the person's); crates/utopia-server/src/api/review_routes.rs:1279-1294; tests review_token_revert_tests.rs:789, :909 (not executed) |
| "A live token merge counts in no governor precedent read. A merge made by a token and reverted by a token is not counted as a reverted merge either ... A person's revert of a token merge counts as that person's revert" (decision 14) | feature | verified | crates/utopia-store/src/governance.rs:85, :128 (`merge.revert` audit rows with a `token_id` excluded), :177 (`decided_token IS NULL`), :186-200 (a reverted merge is not counted when the ledger has both an applied merge row and an applied `merge.revert` row for it); tests review_token_guard_tests.rs:1008, crates/utopia-store/tests/a_token_merge_is_gated_recorded_and_undone.rs:2421 (not executed) |
| "A token batch takes 1 to 50 ids and one action, `merge` or `keep` ... Zero or more than 50 ids is 422 `batch_size`, and an unknown action is the session batch's own uncoded 422; both refuse the whole batch" (decision 14) | feature | verified | crates/utopia-server/src/api/review_ops.rs:562-567 (`batch_size`), :568-571 `AppError::Validation("action must be merge or keep")`, the sentence the session batch gives at crates/utopia-store/src/resolution.rs:1962; crates/utopia-server/src/error.rs:50 maps `Validation` to 422 and sets no `code` (:66-69); both returns come before the `batch_id` is issued at :572; test review_token_batch_tests.rs:546 (not executed). web/src/docs/mcp.md:393, :437 say the same |
| "The unit revert reverts each merge the batch applied, best effort, one outcome per merge; an already reverted merge is that item's `already_decided`, keeps and proposals are left. Its callers are a write token of an Editor ... and a signed-in Editor ...; there is no Review tab button. A batch that made no merge answers 200 with no outcomes; 404 is only for a batch id no ledger row of the base carries" (decision 14) | feature | verified | crates/utopia-server/src/api/review_ops.rs:664-682; crates/utopia-server/src/api/review_routes.rs:1163-1207 (token branch, then the session branch through `revert_as_person` :1108-1151); crates/utopia-store/src/token_agent_actions.rs:1134-1160 (`None` when no row carries the id, else only applied merge rows, possibly empty); `grep -rn "agent-batches" web/src` → no file outside the documentation (web/src/api.ts names only the `agent-proposals` routes at :2876, :2890); test review_token_batch_tests.rs:739 (not executed) |
| "Single (`…/agent-proposals/{proposal_id}`) and bulk (`…/agent-proposals`, 1 to 500 ids) need a signed-in Editor; every personal token gets 401, and no MCP tool answers a proposal ... Approval runs the person's own merge or revert with the person as actor and no gate, and its audit row names the proposal (`agent_proposal`) ... Also built: bulk per-item `not_found`; 422 `unknown_action` ...; 403 for a session below Editor; an uncoded 409 while a running governor round holds the pair" (decision 14) | feature | verified | crates/utopia-server/src/api/review_routes.rs:921-937, :959-994 (`AuthUser`, `require_kb(..., Role::Editor)`), :966-972 with `BATCH_MAX` :1007, :599-608 (`unknown_action`), :655-838 (`decide_review`, `merge_entities`, `revert_merge` with the user as actor; `agent_proposal` in the audit detail at :722, :789, :833), :862-864 (`already_decided`), :876-879 (`Gone` → `superseded`), :943 (`not_found`), :620-626 with crates/utopia-store/src/token_agent_actions.rs:1112-1120 (a governor `Conflict` is not "gone", so it is returned as the uncoded 409 of crates/utopia-store/src/resolution.rs:2056); crates/utopia-server/src/api/mcp.rs:234-247; tests review_proposal_answer_tests.rs:764, :863, :962, :1075 (not executed) |
| "A Duplicates item gains `agent_proposal` and a merges-list row gains `agent` and `agent_proposal`, each serialized only when it has a value ... The Merges section shows a token agent's merge as "owner via token name" from `agent`" (decision 14) | feature | verified | crates/utopia-core/src/models.rs:1083-1084, :1123-1128 (`skip_serializing_if = "Option::is_none"`); crates/utopia-store/src/resolution.rs:1829-1844, :2810-2815; web/src/pages/AgentProposal.tsx:31-44 `MergeActor`, web/src/pages/Review.tsx:516; test review_proposal_answer_tests.rs:1671 (not executed) |
| "A refused tool call carries the REST code when the route has one (`not_found` for a 404). Three cases carry a message only: a pair held by a running governor round, a malformed `review_merge` or `review_revert` call ..., and an unknown `review_batch` action" (decision 14) | feature | verified | crates/utopia-server/src/api/review_tools.rs:466-480 with crates/utopia-server/src/api/review_ops.rs:514-532 (code and message from the rendered REST error; 404 → `not_found`; no `code` key without a code), crates/utopia-server/src/api/review_tools.rs:584-596, :621-633 (a text error for the malformed forms), crates/utopia-server/src/api/review_ops.rs:568-571 and crates/utopia-store/src/token_agent_actions.rs:497-502 (both uncoded) |
| "On every token write route a write token whose base list excludes the base is refused 403 `token_kb_out_of_scope`, between the scope check and the role check" (decision 14) | feature | verified | crates/utopia-server/src/auth.rs:247-268 (scope, then base list), called first in crates/utopia-server/src/api/review_ops.rs:73, before the Editor check at :74-83; every token write enters through `authorize_write` (:368, :496, :561, :670, :694, :727) |
| Known limits: "A person's approval is not one transaction with its merge" / "A token revert is not one transaction with its ledger and audit rows" / "Answers to one proposal, and unit reverts of one batch, are serialised within one server process only" (decision 14) | feature | verified | crates/utopia-server/src/api/review_routes.rs:850-918 (the person's path commits, then `record_approval`, then `token_agent_actions::answer`), :590 `ANSWERING` (a process-local `tokio::sync::Mutex`); crates/utopia-store/src/token_agent_actions.rs:962-1009 (`resolution::revert_merge` commits its own transaction, the two rows follow with three attempts); crates/utopia-server/src/api/review_ops.rs:620 `UNIT_REVERTING` (static) |
| "`save_agent_proposals`, `open_proposal`, `open_proposals` and `reject_proposal` in the store's ontology module ... `adopt` in `question_routes` (`POST /kbs/{id}/ontology/proposals/adopt`) ... `decide_proposal`, only rejects and answers `adoption_endpoint` ... `unadopt_types` ... behind `type_resolution_undo` ... `adopt_iri_onto_key`" | structure | verified | crates/utopia-store/src/ontology.rs:1979, 2003, 2021, 1963, 1561; crates/utopia-server/src/api/ontology_routes.rs:616-625, 808, 280; crates/utopia-server/src/ontology_agent.rs:733, 1047, 1216; crates/utopia-server/src/api/mod.rs:392 (adopt route), :335 (`type_resolution_undo`); crates/utopia-store/src/resolution.rs:2882 `unadopt_types` (symbols located; the "no statement moves a row out of adopted" finding not re-derived) |
| "This record is 0076 and the ledger's migration is 0128" | structure | stale | docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md:520 names one migration for the ledger. `ls migrations` → 115 files, highest 0129: migrations/0129_a_token_agent_action_keeps_its_reason_its_batch_its_merge_and_its_later_state.sql alters `token_agent_actions`, and the record's own status line (:3, "migration 0129") and decision 7 (:166-187) attribute it to this record. 0128 is still the migration that creates the ledger; what moved on is that the section accounts for this record's numbers and leaves 0129 out (the upstream check of that sentence is under the accept-drift ruling on queue item 38 and is not audited) |
| "The gaps are not reused (0055 to 0059 and 0068 here; 0081 to 0089, 0107, 0108, 0112 and 0124 among migrations)" | structure | verified | `ls docs/decisions` → 70 records and README.md (0054 → 0060, 0067 → 0069); `ls migrations` → 115 files, none numbered 0081 to 0089, 0107, 0108, 0112, 0124 or 0127; 0129 follows 0128 |
| "`scripts/check-upstream-numbers.sh <adr-number> <migration-number>` re-checks both ... Exit 0 is free, 1 names the colliding file, 2 means upstream could not be read" | command | verified | scripts/check-upstream-numbers.sh:3-5 (usage and exit codes), :23-41 `exit 2`, :157 `exit 1`, :160 `exit 0`; reads only `deeplethe/utopia` (:14-17) (static; the file is not in the changed set; not executed because it calls the upstream forge) |
| "The precedent exclusion has not been exercised end to end." | status | stale | Tests now exercise it against a real database, at the store and through the HTTP routes: crates/utopia-store/tests/a_token_keep_is_never_human_precedent.rs:842 `a_token_keep_changes_no_precedent_no_settlement_and_queues_no_govern_round` (precedents, ledger search, `decided_before`, `settled_by_people` and the gate read the same before and after a token keep, :746-838), crates/utopia-store/tests/a_token_merge_is_gated_recorded_and_undone.rs:2421, review_token_guard_tests.rs:1008 `a_token_merge_and_a_token_reverted_token_merge_move_no_governor_read_and_a_persons_revert_of_one_counts_as_any_revert`, review_token_api_tests.rs:568, review_token_merge_tests.rs:1855 (present; not executed here). No test was found that runs a full govern round with a model after a token write, so whether the question is closed is the owner's call (docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md:588-590) |
| "Decisions 2, 5, 7, 10, 11 and 12 carry the dated revisions for this slice, and decision 14 is new" (Status history) | structure | verified | docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md:68, :121, :166, :212, :250, :303 (each "Revision 2026-11-19 (the gated-write slice ...)"), :358 (decision 14, "Added 2026-11-19"); the quoted earlier `open:` text at :612-618 matches the prior audit's status row |

## Doc: docs/decisions/README.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "File names are `NNNN-short-english-title.md`, four digits, increasing in creation order, no gaps" | structure | stale | docs/decisions/ has no 0055, 0056, 0057, 0058, 0059 or 0068 (0054 → 0060, 0067 → 0069); docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md:522-524 itself states these gaps exist and are not reused, contradicting "no gaps" |
| "The directory is flat" | structure | verified | docs/decisions/ contains only files: 70 records + README.md |
| Index: every record listed with the first word of its own status line | status | verified | 70 index rows (README.md:35-164, 0076 at :91 as In progress) vs 70 record files (`ls docs/decisions` → 71 entries with README.md); 0076's status line, rewritten again in M003, still starts `In progress` (0076:3); the other 69 records are byte-identical to the prior baseline, where each first status word matched its index row |
| "`/docs/*` is git-ignored except `/docs/decisions/`, `/docs/design/` and `/docs/pipeline.md`" | config | verified | .gitignore: `/docs/*`, `!/docs/decisions/`, `!/docs/design/`, `!/docs/pipeline.md` |
| "that page explains how things flow ... with five mermaid diagrams" | structure | verified | `grep -c '```mermaid' docs/pipeline.md` → 5 |
| "A status line is one line. It starts with one of five words" | structure | verified | First words across the 70 records: Proposed, Accepted, In progress, Implemented, Superseded (0072 carries the word without the `Status` label); 0076:3 starts `In progress` |

## Doc: docs/design/README.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| Domains table: twelve design files, one per domain | structure | verified | docs/design/ has the twelve named files plus prior-work.md and README.md |
| "`proposed`: ... 0044 and 0045 sit in PRs #710 and #724" | status | stale | docs/decisions/0044 and 0045 status lines are "In progress" with cuts built (#731, #740 merged; migrations 0061-0064); the index lists both as In progress |
| "pipeline.md still describes the typed path of 2026-09-02 and waits for a rewrite around the open graph" | status | verified | See docs/pipeline.md section below: its drop codes and write-time direction check are no longer in the code |
| "the typed extraction path is deleted together with its per-base switch (#736) ... Extraction writes nothing bound to an ontology any more" | feature | verified | migrations/0063_extraction_reads_only_the_open_graph.sql; crates/utopia-server/src/extraction_open.rs is the extraction path |

## Doc: docs/design/access-and-audit.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "a base has the roles Viewer, Editor and Admin (`access::kb_role`, `require_kb`)" | feature | stale | crates/utopia-core/src/models.rs:55 `enum Role { Viewer, Editor, Admin, Owner }`; README.md lists "owner, admin, editor and viewer". `access::kb_role` exists (crates/utopia-store/src/access.rs:12) |
| "`personal_tokens` (prefix `utp_pat_` ...) ... writes an audit row `mcp.tool_called`" | feature | verified | migrations/0017_personal_tokens.sql; `utp_pat_` in crates/utopia-core/src/models.rs:1837; crates/utopia-server/src/api/mcp.rs:226,278 |
| "`POST /auth/oidc/exchange` ... `UTOPIA_OIDC_EXCHANGE_AUDIENCES` ... spends it once (`oidc_exchanges`)" | feature | verified | api/mod.rs:127; api/oidc_exchange_tests.rs:88; migrations/0103:7 |
| "`GET /kbs/{id}/sources/{sid}/documents/by-external-id?external_id=…`" | feature | verified | crates/utopia-server/src/api/mod.rs:486 |
| "sealed at rest (`secrets::seal`), listed in `SOURCE_SECRET_KEYS`" | feature | verified | crates/utopia-core/src/secrets.rs; crates/utopia-core/src/models.rs:264 |
| "`GET /kbs/{id}/export?format=turtle\|jsonld` streams the base as RDF" | feature | verified | crates/utopia-server/src/api/export_routes.rs:30 |
| "Proposed and not built: OIDC SSO, backup and restore [0016 E, #712]" | status | stale | crates/utopia-server/src/api/oidc_routes.rs, migrations/0056 (SSO subject link), docs/decisions/0069 Implemented; crates/utopia-cli backup/restore with tests |
| "apart from MCP, every other route is session-only [0014 d6, d7]" | feature | stale | docs/design/access-and-audit.md:20-25 lists the routes that take `SessionOrToken` and names no Review route except "deciding a kind word". Ten more take it: crates/utopia-server/src/api/review_routes.rs:85-87, :353-355, :1015-1017, :1163-1165, :1270-1272, :1346-1348, :1424-1426, :1448-1450 and crates/utopia-server/src/api/graph_routes.rs:540-542, :562-564 (five Review reads and five Review writes). web/src/docs/mcp.md:278-292 documents those ten routes, so the two documents contradict each other |

## Doc: docs/design/chat-and-mcp.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "The loop is rig's multi-turn runner (`rig-core` / `rig-agent` ...); every policy is one `AgentHook` in `api/agent.rs`" | structure | verified | `rig-core` in crates/utopia-server/Cargo.toml; crates/utopia-server/src/api/agent.rs:20 imports `AgentHook` |
| "`STALL_NUDGE` and `DONE` are gone" | structure | verified | No `STALL_NUDGE` in crates/ |
| "Proposed and not built: A per-task model (`on_model_select`)" | status | verified | No `on_model_select` in crates/; docs/decisions/0067 open: cuts 1–4 |
| "Proposed and not built: `query_data` over MCP" | status | verified | crates/utopia-server/src/api/mcp.rs:37,198,236,256 comments: `query_data` is deliberately not served |
| "Over MCP an external agent can therefore only propose [0015 d6]" | feature | stale | docs/design/chat-and-mcp.md:91-92. Four MCP tools now write to Review and three of them can finalize: crates/utopia-server/src/api/review_tools.rs:31-46; `review_keep` is applied at once (crates/utopia-server/src/api/review_ops.rs:361-388) and `review_merge` is applied when the gate passes (crates/utopia-server/src/api/review_ops.rs:464-471). The same file's tool list (:46-48, "Tools live once in `tools.rs` and serve chat and MCP alike") names none of the six Review tools, which live in review_tools.rs and are served to MCP only (crates/utopia-server/src/api/mcp.rs:197-203). web/src/docs/mcp.md:3 says the opposite of the sentence |

## Doc: docs/design/extraction.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "One call per chunk, at temperature 0 ... the document's opening (its first 1,500 characters, #680)" | feature | verified | crates/utopia-server/src/extraction_open.rs:133 `Some(0.0)`; crates/utopia-extract/src/lib.rs:31 `OPENING_BUDGET_CHARS: usize = 1500` |
| "the typed path is deleted in #736" | status | verified | migrations/0063_extraction_reads_only_the_open_graph.sql; 34 of 36 backticked code identifiers in the file resolve in the code (count from the prior audit, not recounted; no file was deleted or renamed since) (the two misses are provider JSON fields) |

## Doc: docs/design/governance.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Review holds nods (`pending_facts`), duplicates (`resolution_reviews`), conflicts (`fact_conflicts`) ... low-confidence (`confidence < 0.75`), violations (`axiom_violations`)" | feature | verified | crates/utopia-store/src/review.rs:20 `LOW_CONFIDENCE_BELOW: f32 = 0.75`; migrations 0005, 0012, 0018; crates/utopia-store/src/pending.rs |
| "[0043] (every queue, in PR #699)" | status | stale | docs/decisions/0043 status line: "no code on `dev` (#699 closed) · open: re-land on the open graph" |
| "The errata agent on the typed graph ... is built (migration 0074)" | status | verified | migrations/0074_an_errata_agent_reviews_the_typed_graph.sql; crates/utopia-store/src/errata.rs |

## Doc: docs/design/identity.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Every name ... is a value fact on the system attribute `known_as` ... `entities.aliases` is gone" | feature | verified | migrations/0055_a_name_is_a_fact.sql:119 `ALTER TABLE entities DROP COLUMN aliases`; `known_as` in crates/utopia-core/src/models.rs:860 |
| "Proposed and not built: ... `doc_cache` removed" | status | stale | No `doc_cache` anywhere in crates/ — it is already gone, yet the file lists its removal as not built (twice: cuts 2–4 of 0041 and the #725 retirement list) |

## Doc: docs/design/interface.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "stored as `utopia.theme` in the browser and set on `<html data-theme>`" | feature | verified | web/src/theme.ts:2 |
| "Design rules (`web/DESIGN.md`, enforced by `pnpm guard`)" | command | verified | web/package.json `"guard": "node scripts/style-guard.mjs"`, also first step of `build` (static; not executed) |
| "Kinds: `source.sync_failed`, `llm.unreachable`, `llm.rate_limited`, `llm.out_of_credit`, `data_source.schema_sync_failed`, `document.needs_reader`, `governance.tripped`" | feature | stale | crates/utopia-store/src/alerts.rs:26-68 also defines `mapping.exploration_empty` (:56) and `document.contents_truncated` (:68), which the list omits; the seven listed kinds are present (alerts.rs:26,30,41,47,53,60,65; crates/utopia-server/src/alerting.rs:229) |

## Doc: docs/design/lakehouse-and-actions.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "the scheme picks the engine (`postgres`, the MySQL wire family since #303, `trino`, `databricks`, `snowflake`)" | integration | stale | ClickHouse is also an engine: crates/utopia-server/src/query_engine/clickhouse.rs, migrations/0106_a_clickhouse_warehouse_can_be_mounted.sql; README.md lists ClickHouse |
| "Data sources are registered at the deployment and granted to workspaces (`data_source_grants`, #142)" | feature | verified | migrations/0014_data_source_grants.sql:22 |
| "`concept_mappings` ... is configuration outside the ledger" | structure | verified | migrations/0006_semantic_layer.sql:50 |
| "Actions are recorded and not built [0034]" | status | verified | No action tables or symbols in migrations/ or crates/ |
| "How much schema to fetch from a lakehouse catalog (cap 200 tables today)" | config | unverifiable | crates/utopia-server/src/query_engine/mod.rs:38 has `ROW_CAP: usize = 200`, which is a row cap; a 200-table schema cap was not located by grep — needs a targeted read of the schema-sync code |

## Doc: docs/design/ledger.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "`facts (kb, subject_id, predicate_id NULL, ... end_derived, end_after_period, layer, phrase)`" | structure | verified | migrations/0114 (`end_after_period`), 0061 (open layer and phrase), 0057 (end the engine drew); 24 backticked code identifiers in the file resolve in the code (count from the prior audit, not recounted; no file was deleted or renamed since) |
| "`no_predicate_still_shows.rs`" guards the null-predicate display | structure | verified | crates/utopia-store/tests/store/no_predicate_still_shows.rs |

## Doc: docs/design/ontology.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "A property regression case can also expect no binding (`expected_binding: \"none\"` ...)" — listed under "Proposed and not built" | status | stale | crates/utopia-server/src/api/ontology_regression_routes.rs:26 `expected_binding: ExpectedBinding`; migrations/0118_a_regression_case_can_expect_no_binding.sql — built, but placed in the not-built section |
| Packs and import: five packs embedded, OWL import | feature | verified | crates/utopia-server/src/ontology_packs.rs; crates/utopia-server/src/owl_import.rs; 46 backticked code identifiers in the file resolve in the code (count from the prior audit, not recounted; no file was deleted or renamed since) |

## Doc: docs/design/prior-work.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "the run is at temperature 0 with a judge that counts what the text did not say" | feature | verified | crates/utopia-server/src/extraction_open.rs:133 `Some(0.0)`; scripts/bench/judge_open.mjs |

## Doc: docs/design/rules.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Axioms compiled from the ontology ... run `derive()` over entity-to-entity edges; business rules (`attribute_rules`) ..." | feature | verified | crates/utopia-reason/src/derive.rs:172; migrations/0028:28 `CREATE TABLE attribute_rules` |
| "Proposed and not built: R3 incremental maintenance" | status | verified | No incremental-maintenance code in crates/utopia-reason/src; docs/decisions/0002 open: R3 |

## Doc: docs/design/sources.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "`documents` and `document_versions` keep every version; `ingest_item()` owns identity (`external_key`)" | feature | verified | migrations/0002_ingest.sql; 20 backticked code identifiers in the file resolve in the code (count from the prior audit, not recounted; no file was deleted or renamed since) |
| "Proposed and not built: Feishu and Confluence connectors" | status | verified | `SourceKind` (crates/utopia-core/src/models.rs:306) has neither; grep for `feishu` in crates/ and web/src → 0 files; the only `confluence` hit is a URL string in a test at crates/utopia-server/src/jira_issues.rs:821, not a connector |
| "Proposed and not built: Backup and restore (#712)" | status | stale | crates/utopia-cli/src/main.rs backup and restore, crates/utopia-cli/tests/restore.rs |

## Doc: docs/design/time.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "`record_axis::held_at(T)` and `world_axis::facts_hold_at` / `derived_hold_at` are the only places ..." | structure | verified | crates/utopia-store/src/record_axis.rs:1,45,51 (module doc names `held_at(T)`; functions are `facts_held_at`, `derived_held_at`); crates/utopia-store/src/world_axis.rs:72 `facts_hold_at` |
| "The temporal engine (`temporal.rs`) ... `AUTO_CLOSE_MIN_CONFIDENCE` is gone" | structure | verified | crates/utopia-store/src/temporal.rs exists; no `AUTO_CLOSE_MIN_CONFIDENCE` in crates/ |
| "[0045] (time mentions, proposed)" | status | stale | docs/decisions/0045 status "In progress · cuts 1–3 built"; migrations/0064; crates/utopia-server/src/time_resolution.rs. The file's own not-built list marks only "grades" as built |

## Doc: docs/pipeline.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Chunk 1200 chars · overlap 150" | config | stale | crates/utopia-ingest/src/chunker.rs:45 `BUDGET_TOKENS: usize = 300` (token budget, 0039); no 1200/150 constants in crates/utopia-ingest/src/lib.rs |
| "Eleven reason codes, all recorded in `extraction_drops`" (`not_an_entity_name`, `subject_not_declared`, `attr_domain_mismatch`, `attr_no_value`, `direction_corrected`, `domain_mismatch` among them) | feature | stale | Of those six codes only `attr_domain_mismatch` survives, in two comments (migrations/0003_graph.sql:419, web/src/api.ts:229); none is emitted by crates/ (grep repeated at d5d2356). Only `truncated_reply`, `malformed_item`, `attr_datatype`, `object_missing` (crates/utopia-store/src/extraction_drops.rs) and `low_confidence` remain |
| "The ... signature check ... lives in the store (`ontology::judge_direction`). Extraction calls it when writing new facts" | structure | stale | `judge_direction` appears only in a comment of migrations/0019; no such function in crates/. docs/design/README.md says the write-time direction correction left with the typed path |
| "node scripts/bench/run.mjs --corpus pharma --label seeds-only" / "--corpus holmes" | command | verified | scripts/bench/run.mjs; scripts/bench/corpora/pharma.json, holmes.json, tech.json (static; needs a running server and model, not executed) |

## Doc: scripts/bench/README.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "node scripts/bench/run.mjs --corpus pharma --ontology /tmp/schemaorg.ttl --label schemaorg" | command | verified | scripts/bench/run.mjs handles `--ontology` (3 matches) (static; not executed) |
| "node scripts/bench/recall.mjs --kb <id>" with `--score`, `--reprocess`, `--known`, `--table`; "fetch-sec-filings.mjs"; "judge_open.mjs --kb <id> --sample 200" | command | verified | scripts/bench/recall.mjs (8 flag matches), fetch-sec-filings.mjs, judge_open.mjs exist; `UTOPIA_EXTRACT_KNOWN_IN_PROMPT` in crates/utopia-core/src/config.rs:35 (static) |
| "node scripts/bench/govern.mjs --fresh" / "mappings.mjs --fresh --no-comments" / "ask.mjs --kb <id> --replay ..." | command | verified | govern.mjs, mappings.mjs, ask.mjs exist with the named flags (static) |
| "node --test scripts/bench/lib.test.mjs" | command | verified | Executed in sidecar at d5d2356 → tests 15, pass 15, fail 0 |
| "node scripts/bench/competency.mjs --kb <kb-id> --seed scripts/bench/truth/redocred-typed.questions.json" | command | verified | competency.mjs and truth/redocred-typed.questions.json exist (static) |
| "node scripts/bench/agent.mjs ... --reset --arms ..." / "timewords.mjs --db ... --runs 3" / "temporal.mjs --label rc5" / "typed.mjs --label run1 --judge 200" / "fetch-redocred.mjs --n 100 --seed 1" | command | verified | All five scripts exist; `--arms`/`--reset` in agent.mjs, `--judge` in typed.mjs (static) |
| Directory listing: `corpora/*.json`, `truth/*.json`, `truth/*.wikidata.json`, `p31-to-classes.json`, `make-truth.mjs`, `subset-corpus.mjs`, `subset.mjs` | structure | verified | unchanged: `ls scripts/bench`, `scripts/bench/truth`, `scripts/bench/corpora` show each named file |

## Doc: scripts/bench/corpora/contracts/README.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "`py -3 fetch.py` / `py -3 lease_bench.py setup --label <名字>` / `py -3 lease_bench.py score <kb id>`" | command | verified | scripts/bench/corpora/contracts/fetch.py, lease_bench.py (header documents `setup` and `score`, `QUESTIONS`) (static; needs SEC network access and a running server) |
| "`edgar-manifest.json` ... `edgar/`、`lease-kb-*.json` ... 不进仓库（见 `.gitignore`）" | structure | verified | unchanged: edgar-manifest.json present; .gitignore lists `/scripts/bench/corpora/contracts/edgar/` and `lease-kb-*.json` |

## Doc: web/DESIGN.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "`web/scripts/style-guard.mjs` scans `web/src/**/*.{ts,tsx}` ... the `web` job runs `pnpm build`, and the guard runs first in it" | command | verified | web/package.json `"build": "node scripts/style-guard.mjs && tsc --noEmit && vite build"`; checks.yml web job |
| "`src/components/ui/`, the shadcn components as generated, is skipped entirely (`VENDOR` in the guard)" | structure | verified | web/scripts/style-guard.mjs:150 `const VENDOR = path.join("src", "components", "ui")` |
| "`pages/graphVisuals.ts` reads the tokens ... `palette.ts` ... must stay byte-identical to `crates/utopia-store/src/palette.rs`" | structure | verified | web/src/pages/graphVisuals.ts, web/src/palette.ts, crates/utopia-store/src/palette.rs exist (byte-identity of values not diffed) |
| "`theme.ts` sets the theme on `<html>` both ways at once, `data-theme` and the `.dark` class" | feature | verified | web/src/theme.ts |
| "`web/src/ui/index.tsx` is a shell; `web/src/components/ui/*` is shadcn" | structure | verified | web/src/ui/index.tsx; web/src/components/ui/ (20 component files) |

## Doc: web/src/docs/ingest.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "POST {your-utopia-base}/api/v1/sources/{source_id}/ingest" | feature | verified | crates/utopia-server/src/api/mod.rs:504 |
| "POST {your-utopia-base}/api/v1/sources/{source_id}/statements" | feature | verified | crates/utopia-server/src/api/mod.rs:509 |
| "GET .../api/v1/kbs/{kb_id}/sources/{source_id}/documents/by-external-id?external_id=note-42" | feature | verified | crates/utopia-server/src/api/mod.rs:486 |
| "older versions are at `GET /api/v1/documents/{id}/content?version=N`" | feature | verified | crates/utopia-server/src/api/mod.rs:427 |
| "POST {your-utopia-base}/api/v1/kbs/{kb_id}/search" | feature | verified | crates/utopia-server/src/api/mod.rs:408 |

## Doc: web/src/docs/mcp.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "POST /api/v1/kbs/{kb_id}/mcp ... JSON-RPC 2.0 (MCP protocol version `2025-06-18`, stateless HTTP)" | feature | verified | crates/utopia-server/src/api/mod.rs:240; crates/utopia-server/src/api/mcp.rs:33 `PROTOCOL_VERSION: &str = "2025-06-18"`, :183-205 serve `initialize`, `tools/list`, `tools/call` |
| Tools table: `search_chunks`, `get_document`, `find_entities`, `entity_facts`, `neighbors`, `timeline`, `paths_between`, `changes`, `list_rules`, `rule_matches`, `search_docs`, `remember` | feature | verified | crates/utopia-server/src/api/mcp.rs:43-55 `EXPOSED` holds the eleven read tools, :63 `WRITABLE = ["remember"]`, :67-69 `is_exposed` |
| "`search_chunks` ... each cut at 800 characters" / "`get_document` ... up to 24,000 body characters per call" | config | verified | crates/utopia-server/src/api/tools.rs:19 `TOOL_CHUNK_CHARS = 800`, :21 `DOCUMENT_CHARS = 24_000` |
| "`query_data` ... is a chat tool and is not served over MCP" | feature | verified | crates/utopia-server/src/api/mcp.rs:37, 198, 236 (not in `EXPOSED`; `tools_schema` is called with an empty source list at :199) |
| "Authorization: Bearer utp_pat_…" | config | verified | `utp_pat_` prefix in crates/utopia-core/src/models.rs:1837; migrations/0017_personal_tokens.sql:12; crates/utopia-server/src/api/mcp.rs:89-94 |
| "Expires in \| 90 days by default" | config | verified | crates/utopia-server/src/api/token_routes.rs:27-37 (default 90 days; 0 means no expiry) |
| "Search returns at most six chunks and changes at most 40 events" / "the default 80-fact limit (maximum 300)" | config | verified | crates/utopia-server/src/api/tools.rs:18 `SEARCH_TOP_K = 6`, :26 `CHANGES_LIMIT = 40`; crates/utopia-server/src/api/tools_graph.rs:26 `FACTS_DEFAULT = 80`, :27 `LIST_MAX = 300` |
| "`GET /api/v1/kbs/{kb_id}/export?format=turtle\|jsonld` streams the whole base" / "`?base=https://example.org/`" | feature | verified | crates/utopia-server/src/api/mod.rs:185; crates/utopia-server/src/api/export_routes.rs:30 (`turtle` or `jsonld`), :35 `pub base: Option<String>` |
| "Six tools reach the Review tab. Two read all thirteen of its sections. Four write, for a `write` token held by an editor: `review_keep` ..., `review_merge` ..., `review_revert` ..., and `review_batch` decides up to 50 duplicate pairs in one call ... no token and no tool answers a proposal" | feature | verified | crates/utopia-server/src/api/review_tools.rs:31-36 (six names), :44-46 (four writes), :378-399 (the writes listed only when `can_write`); crates/utopia-server/src/api/mcp.rs:107-110; crates/utopia-server/src/api/review_ops.rs:536 `TOKEN_BATCH_MAX: usize = 50`; crates/utopia-server/src/api/review_routes.rs:921-923, :959-961 (answer handlers take `AuthUser`); tests review_mcp_reads_tests.rs:1040, review_proposal_answer_tests.rs:1075 (not executed) |
| Tools table, Review rows: `review_list` ("any of the thirteen"), `review_item`, `review_keep`, `review_merge` ("a merge it passes is applied at once, a merge it holds changes nothing and waits as a proposal"), `review_revert` ("One merge, or every merge one `review_batch` call made"), `review_batch` ("1 to 50 pending duplicate pairs with one action, `merge` or `keep`"), each write "Listed and refused as `review_keep` is" | feature | verified | crates/utopia-server/src/api/review_tools.rs:70-355 (the six definitions), :378-399, :560-656; crates/utopia-server/src/api/review_ops.rs:489-506, :554-612, :664-682, :720-737; a write tool called without the right is refused by crates/utopia-server/src/api/review_ops.rs:69-84 with a coded tool error (crates/utopia-server/src/api/review_tools.rs:466-480); tests review_token_merge_tests.rs:1712, review_token_revert_tests.rs:1071 (not executed) |
| Review over MCP arguments table: `review_list` (`section` required; `limit`, `offset`; `page`, `per`), `review_item` (`section` optional, default `duplicates`; `id` required), `review_keep` (`id` required, `rationale`), `review_merge` (either `id` or both `source` and `target`; "No argument is marked required in the schema"), `review_revert` ("Exactly one of `merge_id` ... and `batch_id`"; none required), `review_batch` (`ids`, `action` required), with each tool's successful `structuredContent` being the body of its route | feature | verified | crates/utopia-server/src/api/review_tools.rs:91-123 (`"required": ["section"]`), :144-161 (`["id"]`), :176-190, :220-247 and :284-306 (no `required` key), :331-351 (`["ids", "action"]`), :453-455 (`success` puts the operation's JSON in `structuredContent`), :577-597, :614-634 (the either/or forms checked in `call`); crates/utopia-server/src/api/review_ops.rs:387, :464-480, :611, :653 (the bodies) |
| "`review_list` reads all thirteen sections of the Review tab. Eleven are queue sections that share one list route; Overview and Decisions each have their own route" (sections table: keys `overview`, `pending`, `duplicates`, `conflicts`, `unconfirmed`, `lowconf`, `violations`, `defects`, `alignment`, `errata`, `agent`, `decisions`, `merges`) | feature | verified | crates/utopia-server/src/api/review_ops.rs:89-101 `TOKEN_SECTIONS` (the eleven queue keys); crates/utopia-server/src/api/review_tools.rs:504-533 (`overview` → summary, `decisions` → history, others → list); crates/utopia-server/src/api/review_routes.rs:137-205 (one `match` arm per queue key); routes at crates/utopia-server/src/api/mod.rs:534 (list), :535 (history), :536 (summary); test review_mcp_reads_tests.rs:193 (not executed) |
| "A paging argument the section does not take is ignored ... The route's `types` filter for Duplicates is not a tool argument ... Any other section name, `mappings` included, is refused with `unknown_queue` and returns no items" | feature | verified | crates/utopia-server/src/api/review_tools.rs:504-533 (each branch reads only its own paging arguments; `review_ops::list` is called with `types` = `None` at :531); crates/utopia-server/src/api/review_ops.rs:104-106, :121-123; tests review_mcp_tests.rs:421, review_mcp_reads_tests.rs:964 (not executed) |
| "A queue section's result carries `total` ... `counts.alignment` leaves out proposed implication rules, `counts.lowconf` leaves out derived facts that the list includes, and `counts.agent` counts the governor's open proposals only ... An empty section is `"items": []` with `"total": 0`, not an error" | feature | verified | crates/utopia-server/src/api/review_routes.rs:137-221; crates/utopia-store/src/review.rs:80 (`derived_by_rule IS NULL` in the `lowconf` badge), :88-89 (`agent` badge: `status = 'proposed'`), :101-102 (`alignment` badge: phrase and type bindings only); crates/utopia-store/src/alignment_queue.rs:235; crates/utopia-store/src/graph.rs:2105-2107; crates/utopia-store/src/token_agent_actions.rs:1457-1468; tests review_totals_tests.rs:323, :395, :575 (not executed) |
| "`limit` defaults to 10 and is served as 1 to 200 ... `offset` defaults to 0, and a negative value is served as 0 ... The Alignment list is ordered by each item's `decided_at` and then by the item's own identifier" | config | verified | crates/utopia-server/src/api/review_routes.rs:29 `REVIEW_PAGE: i64 = 10`, :134-135 `clamp(1, 200)`, `.max(0)`; crates/utopia-store/src/alignment_queue.rs:162-223 (sort key `decided_at`, then id or kind word); tests review_mcp_tests.rs:330, review_totals_tests.rs:513 (not executed) |
| "`section: "decisions"` pages with the history route's own `page` and `per` ... `page` is zero-based and defaults to 0 (a negative value is served as 0); `per` defaults to 20 and is served as 1 to 100 ... newest first, each with `id`, `action`, `target_kind`, `target_id`, `detail`, `actor_id`, `actor_name` and `created_at`" | feature | verified | crates/utopia-server/src/api/review_ops.rs:130-142 (`per.unwrap_or(20).clamp(1, 100)`, `page.unwrap_or(0).max(0)`); crates/utopia-server/src/api/review_tools.rs:511-521; crates/utopia-store/src/audit.rs:129-141 (selected columns; `ORDER BY e.created_at DESC, e.id DESC`); test review_mcp_reads_tests.rs:262 (not executed) |
| "`section: "agent"` returns rows from two sources in one sequence, newest first ... A row for an action made with a token has `"source": "token"` and names both the person and the token ... `operation` is `review.keep`, `review.merge` (a queue merge), `merge.manual` ... or `merge.revert`; `outcome` is `applied`, `held` ... or `proposed` ... Five more keys are present only when they have a value ...: `reason` ..., `batch_id` ..., `merge_id` ..., and `state` with `state_at` ... `state` is `approved`, `declined` or `superseded` once a person answered the proposal, and `undone` on an applied merge row whose merge has since been reverted" | feature | verified | crates/utopia-store/src/token_agent_actions.rs:1420-1436 `history_row` (the fourteen fixed keys), :1437-1451 (the five optional keys), :1457-1459 (`UNION ALL` of `agent_decisions` and `token_agent_actions`), :1477-1547 (`ORDER BY created_at DESC, id DESC LIMIT/OFFSET` over the union; governor rows from `governance::list`), :234, :604, :783, :948 (the four operation strings), :83-92 (`undone` and its `state_at` read from `entity_merges.reverted_at`), :1088 (the three stored states); crates/utopia-store/src/execution_gate.rs:193-199; tests review_token_reads_tests.rs:883, :927, review_token_batch_tests.rs:926 (not executed) |
| "`review_item` takes the `section` an item is listed in and the item's `id` ... left out, it is `duplicates`" with its bullets (kind word passed as `id` and compared exactly as stored; the Duplicates pair fields and `agent_proposal`: `{"id", "operation", "reason", "owner_name", "token_name", "created_at"}`, absent when no proposal is open; the Merges item with `agent`: `{"action_id", "owner_name", "token_name"}` and `agent_proposal`; `proof` on an Axioms item; one `not_found` answer; `overview` lists no items) | feature | verified | crates/utopia-server/src/api/review_tools.rs:535-545; crates/utopia-server/src/api/review_ops.rs:176-183, :201-286 (item found by paging the section's own list; `overview` → `NotFound` :209-211; `proof` only for `violations` :274-284); crates/utopia-core/src/models.rs:1046-1055 (`ReviewSide`), :1070-1085 (`ReviewItem`, `agent_proposal` skipped when `None`), :1091-1099 (`AgentProposalView`), :1104-1108 (`MergeAgentView`), :1112-1129 (`MergeLogView`); crates/utopia-store/src/token_agent_actions.rs:1310-1333 (the queue-merge proposal of the review id, else a manual-merge proposal on the two entities in either order), :1336-1350, :1354-1385 (only applied token merge rows give `agent`); crates/utopia-store/src/resolution.rs:1829-1844, :2786-2815; tests review_mcp_reads_tests.rs:432, :518, :616, :843 (not executed) |
| "The four write tools ... need the `write` scope and an owner who is at least an editor; the scope is checked before the owner's role. A caller without that right does not see any of the four in `tools/list` ... a call to any other name beginning `review_` is JSON-RPC error -32601 ... A token that is unknown, revoked or expired gets HTTP 401 from the endpoint. A token whose knowledge-base list excludes the base is turned away by the endpoint itself with HTTP 403 before any tool runs ... The token API answers that same token with 404" | feature | verified | crates/utopia-server/src/api/review_ops.rs:52-64 (404 on the token API), :69-84 (scope and base list, then role; `token_read_only`, `token_owner_below_editor`); crates/utopia-server/src/api/review_tools.rs:39-41, :378-399; crates/utopia-server/src/api/mcp.rs:89-98 (`Unauthorized` without a bearer token; `Forbidden` when the token does not cover the base, inside `authorize`, called first in `handle` :177), :210, :234-247 (a name `serves` does not know and `is_exposed` rejects → `rpc_err(id, -32601, ...)`); crates/utopia-store/src/tokens.rs:157-170; crates/utopia-server/src/error.rs:33-35; tests review_mcp_tests.rs:551, review_token_reads_tests.rs:759 (not executed) |
| "A keep made by an agent is recorded under the token's owner and the token's name" / "Each call to any of the six tools, a refused one included, also writes the `mcp.tool_called` row that every MCP tool call writes" | feature | verified | crates/utopia-store/src/token_agent_actions.rs:208-267 (review row with `decided_by` and `decided_token`, ledger row, `review.keep` audit row with the owner as actor and `token_id`, `token_name`); crates/utopia-server/src/api/mcp.rs:219-231 (written after the Review call returns, whatever its result) |
| MCP errors table: `unknown_queue`, `not_found`, `token_read_only`, `token_owner_below_editor`, `already_decided`, `self_merge`, `batch_size`, `agent_rate_limited`; "`structuredContent` is `{"code": "<code>", "message": "<text>"}`, where `code` and `message` are the `code` and `error` of the response the matching token route gives" | feature | verified | crates/utopia-server/src/api/review_tools.rs:466-480 with crates/utopia-server/src/api/review_ops.rs:514-532 (code and message read from the rendered REST error; a 404 becomes `not_found`); crates/utopia-server/src/api/review_ops.rs:104-106, :562-567 (`batch_size`), :293-301 (`agent_rate_limited`); crates/utopia-server/src/auth.rs:252-255; crates/utopia-server/src/api/review_ops.rs:78-81; crates/utopia-store/src/token_agent_actions.rs:273-278, :725-730 (`self_merge`), :920-922 (a reverted merge → `already_decided`), :1146-1148 with crates/utopia-server/src/api/review_ops.rs:675 (unknown batch id → `NotFound`); test review_mcp_tests.rs:774 (not executed) |
| "In the result of `review_batch`, and of `review_revert` with `batch_id`, a refusal of one member is not a tool error: it is that member's `code` and `error` in `outcomes`, and the call itself succeeds. `already_proposed` is not an error either" | feature | verified | crates/utopia-server/src/api/review_ops.rs:581-609 (a member's `Err` becomes an outcome object with `code` and `error`; the function still returns `Ok`), :628-642, :475-477 (`already_proposed` set on an `Ok` answer); test review_token_batch_tests.rs:1208 (not executed) |
| "An error for which the route has no code has `structuredContent` `{"message": "<text>"}` with no `code` key: the governor is deciding the pair right now ..., `review_batch` was called with an `action` other than `merge` or `keep` (the whole batch is refused), or the server failed ... A call with a missing or malformed argument is not run; it returns `isError: true` with a text message and no `structuredContent`" (with the listed cases) | feature | verified | crates/utopia-server/src/api/review_tools.rs:469-472 (`code` added only when there is one); crates/utopia-server/src/api/review_ops.rs:568-571 (`AppError::Validation`, which crates/utopia-server/src/error.rs:50 renders as 422 with no code) and crates/utopia-store/src/token_agent_actions.rs:166-171, :497-502 (the governor `Conflict`, uncoded); crates/utopia-server/src/api/review_tools.rs:403-414 (`bad_argument` builds no structured content), :417-449, :548-553, :577-597, :614-634, :640-650 (one non-uuid entry of `ids` stops the batch); crates/utopia-server/src/api/review_ops.rs:245-247 (a non-uuid `id` outside `alignment` → `NotFound`) |
| Annotations: "`readOnlyHint` on the two reads; `readOnlyHint`, `destructiveHint` and `idempotentHint`, all `false`, on `review_keep`; `readOnlyHint: false`, `destructiveHint: true` and `idempotentHint: false` on `review_merge`, `review_revert` and `review_batch`" | feature | verified | crates/utopia-server/src/api/review_tools.rs:358-373 |
| Token API routes table ("ten of the HTTP routes ...: five reads and five writes"): `GET /kbs/{kb_id}/review?queue=<key>`, `GET /kbs/{kb_id}/review/summary`, `GET /kbs/{kb_id}/review/history?page=&per=`, `GET /kbs/{kb_id}/violations/{violation_id}/proof`, `GET /kbs/{kb_id}/derived/{derived_id}/proof`, `POST /kbs/{kb_id}/review/{review_id}` with `keep` or `merge`, `POST /kbs/{kb_id}/entities/merge`, `POST /kbs/{kb_id}/merges/{merge_id}/revert`, `POST /kbs/{kb_id}/review/batch`, `POST /kbs/{kb_id}/review/agent-batches/{batch_id}/revert` ("The same, and also a signed-in editor of the base") | feature | verified | crates/utopia-server/src/api/review_routes.rs:85-87, :353-355, :1015-1017, :1163-1165, :1270-1272, :1346-1348, :1424-1426, :1448-1450 (`SessionOrToken`); crates/utopia-server/src/api/graph_routes.rs:540-542, :562-564; routes at crates/utopia-server/src/api/mod.rs:459, :464, :534, :535, :536, :537, :553-554, :565, :622-623, :630-631; crates/utopia-server/src/api/review_ops.rs:147-168 (`{steps}`, `{proof}`), :387 (keep response shape); `types` parsed by `TypeFilter::parse` (crates/utopia-store/src/resolution.rs:1870 `unknown_type_filter`); queue left out → `duplicates` (crates/utopia-server/src/api/review_routes.rs:94); tests review_token_reads_tests.rs:198, :994 (not executed) |
| "On every write route a signed-in person's request runs the code it ran before and gets the answer it got before: the gate, the proposals, the limit and the breaker described below apply to token callers only" | feature | verified | In crates/utopia-server/src/api/review_routes.rs each token branch returns before the session code (:360-374, :1022-1034, :1277-1300, :1353-1365); the session code that follows (:375-424, :1035-1100, :1301-1331, :1366-1405) calls `resolution::decide_review`, `decide_reviews`, `revert_merge`, `merge_entities` and neither `guard` nor a gate; tests review_session_unchanged_tests.rs:233, :492 (not executed) |
| "`total` is on every list response, for a signed-in session as well; it is the only key the list response gained. The token API has no route for a single item" | feature | verified | crates/utopia-server/src/api/review_routes.rs:216-221 (one body for both callers through `queue_page`); crates/utopia-server/src/api/mod.rs:534-631 has no single-item GET under `/kbs/{id}/review`; test review_totals_tests.rs:217 (not executed) |
| "recorded three ways in one transaction ... If any of the three cannot be written, the keep fails and the pair stays in the queue" / "A token's keep is **not** treated as a person's decision" | feature | verified | crates/utopia-store/src/token_agent_actions.rs:173-270: one `tx` updates `resolution_reviews` (:208-221), inserts `token_agent_actions` (:229-248), writes the audit row (:258-267), then commits (:269); crates/utopia-store/src/governance.rs:85, :177; crates/utopia-server/src/api/review_ops.rs:361-388 (no `after_human_decision` call); test review_mcp_tests.rs:1040 (not executed) |
| "Both merge routes ask the server's merge gate before anything is written; there is no token merge that skips it ... the entity with more live facts survives, and on a tie the one created earlier. The manual merge names two entities and merges `source` into `target`, as the caller says" | feature | verified | crates/utopia-store/src/token_agent_actions.rs:583-588 (direction from `resolution::merge_direction`, then the gate, before the transaction at :621), :765-766 before :796, :798-805 (`source` into `target` as given); crates/utopia-store/src/resolution.rs:2130-2154 (live-fact counts; on a tie the smaller uuid v7, the earlier one, survives); the only token paths to a merge are these two store functions (crates/utopia-server/src/api/review_ops.rs:498, :696); test review_token_merge_tests.rs:1271 (not executed) |
| "**The gate passes.** The merge is applied at once and the answer is HTTP 200: `{"ok": true, "outcome": "applied", "action_id": "<uuid>", "merge_id": "<uuid>"}` ... written in one transaction ... The audit row is `review.merge` for a queue merge and `merge.manual` for a manual merge ... A manual merge that applies also closes the two entities' own pending duplicate pair ... one more key, `"follow_up_error": "<text>"`" | feature | verified | crates/utopia-server/src/api/review_ops.rs:464-471; crates/utopia-store/src/token_agent_actions.rs:621-683 (merge, review row, ledger row, audit row, commit), :667-682 (`review.merge` with `token_id`, `token_name`, `action_id`, `merge_id` from `audit_detail` :385-404), :810-825 (the pair's own pending review closed as merged), :851-860 (`merge.manual`), :685-690 and :863-868 (`follow_up_error` on an applied outcome); crates/utopia-store/src/resolution.rs:2564 (signature check, temporal reconciliation, disambiguators after the commit); test review_token_merge_tests.rs:525 (not executed) |
| "**The gate holds, or the token may only propose.** Nothing in the graph changes ... the answer is still HTTP 200" with the reason table (`identity`, `contradiction`, `derived`, `answered`, `kept_by_person`, `reverted_merge` as `held`; `merge_by_person`, `merge_by_governor`, `breaker` as `proposed`) and "The first six are the gate's holds, checked in the order of the table ... When the breaker is tripped the reason is `breaker` whatever the gate would have said" | feature | verified | crates/utopia-store/src/execution_gate.rs:245-266 (identity, then contradiction, derived, answered through `hold` :133-144), :288-316 (`kept_by_person`, then `reverted_merge`, by the two entity ids in either order), :73-82; crates/utopia-store/src/token_agent_actions.rs:411-430 (`breaker` returned before the hold is read; a hold → `Held` with `hold.code()`), :434-470 (ledger row and `agent.proposal` audit row only), :934-939; crates/utopia-server/src/api/review_ops.rs:434-457 (`explanation`: "Not applied: ... Nothing was changed; the call waits as a proposal that a signed-in editor answers in the Review tab."), :472-478; tests review_token_merge_tests.rs:639, :1389 (not executed) |
| "A token's merge, like its keep, is **not** treated as a person's decision: the governor does not read a live token merge as precedent, it starts no governor round, and a governor proposal open on the pair is not answered by it" | feature | verified | crates/utopia-store/src/governance.rs:85, :101, :114, :177, :610; crates/utopia-server/src/api/review_ops.rs:489-506, :687-705 (no `after_human_decision`); crates/utopia-store/src/token_agent_actions.rs:633-637 (`decided_token` set on the review row); tests review_token_merge_tests.rs:1855, review_token_guard_tests.rs:1008 (not executed) |
| "There is one open proposal per base, operation and target. The target of a queue merge is its review id; the target of a manual merge is the unordered pair of entity ids ...; the target of a revert is the merge id. A repeated call, or a second call arriving at the same moment, writes nothing and returns the open proposal: the same 200 answer with the existing `action_id` and one more key, `"code": "already_proposed"`" | feature | verified | migrations/0129_a_token_agent_action_keeps_its_reason_its_batch_its_merge_and_its_later_state.sql:56-58; crates/utopia-store/src/token_agent_actions.rs:346-380, :471-488, :598, :777, :946; crates/utopia-server/src/api/review_ops.rs:475-477; test review_token_merge_tests.rs:1580 (not executed) |
| "**Only a signed-in person answers a proposal.** The two answer routes take a browser session and nothing else. Every personal token gets 401 from them" with the routes table (`POST /kbs/{kb_id}/review/agent-proposals/{proposal_id}` → `{"ok": true, "state": "approved"}` plus `"merge_id"`; `POST /kbs/{kb_id}/review/agent-proposals` with 1 to 500 ids → `{"answered": n, "outcomes": [{"id", "state", "merge_id", "code", "error"}]}`) | feature | verified | crates/utopia-server/src/api/review_routes.rs:921-937 (`AuthUser`; `{"ok": true, "state": closed_as}` and `merge_id` when there is one), :959-994 (five keys in both outcome forms, `answered` counted); routes at crates/utopia-server/src/api/mod.rs:544-545, :548-549; `AuthUser` rejects a token credential with `Unauthorized` (crates/utopia-server/src/error.rs:34); test review_proposal_answer_tests.rs:1075 (not executed) |
| Proposal answer bullets: an editor is required (403 below); "**Approve** runs the person's own merge or revert, with that person as actor and no gate ... the audit row of that merge or revert carries `agent_proposal`"; a target that can no longer be decided closes as `superseded`; "**Decline** closes the proposal and changes nothing else"; 409 `already_decided`, 404, bulk `not_found`; 422 `unknown_action`, 422 `batch_size`; an uncoded 409 while a governor round holds the pair; "Approving a manual-merge proposal leaves the two entities' own pending duplicate pair open" | feature | verified | crates/utopia-server/src/api/review_routes.rs:927, :965 (`Role::Editor`), :655-728 (`decide_review` as the user, `review.merge` audit row with `agent_proposal`, `after_human_decision`; `Gone` when the pair is decided, a side is merged away or the row names other entities :685-698), :747-793 (`merge_entities`, no review row touched), :798-838 (`revert_merge`, `after_revert` → the fuse), :850-918 (`NotFound` :859-861, `already_decided` :862-864, decline → `declined` with no other write but its audit row), :599-608, :966-972, :940-954 (bulk codes; an uncoded conflict → `code` null); crates/utopia-store/src/resolution.rs:2056; tests review_proposal_answer_tests.rs:489, :669, :764, :863, :962 (not executed) |
| "Known limit: an approval is not one transaction with the merge or revert it runs" | feature | verified | crates/utopia-server/src/api/review_routes.rs:865-891 (the person's path commits its own transaction, then `token_agent_actions::answer` closes the proposal in a second statement); crates/utopia-store/src/token_agent_actions.rs:1093-1106 (static) |
| Reverting a merge: "A merge a token agent made on this base, through this token or another one, and that has not been reverted, is reverted at once ... A merge a person or the governor made is not reverted by a token ... with `reason` one of `merge_by_person`, `merge_by_governor` and `breaker` ... A merge that was already reverted, by anyone, is 409 `already_decided`. A merge id that is unknown or of another base is 404 ... 422 `invalid_body`, after the scope and role checks" | feature | verified | crates/utopia-store/src/token_agent_actions.rs:917-922, :925-939, :957-959 (a proposal, nothing reverted), :962-1009 (`resolution::revert_merge`, then the `merge.revert` ledger and audit rows), :986-996 (`follow_up_error`); crates/utopia-server/src/api/review_ops.rs:720-737; crates/utopia-server/src/api/review_routes.rs:1277-1300 (`authorize_write` before `invalid_body` at :1286-1292); a token revert never calls `governance::after_revert` (crates/utopia-server/src/api/review_ops.rs:720-737), a person's approval does (crates/utopia-server/src/api/review_routes.rs:836); tests review_token_revert_tests.rs:439, :789, :909, review_proposal_answer_tests.rs:1213 (not executed) |
| "**A revert is not a full undo.** It restores graph state only ... **The untyped target.** A target that had no type before the merge ... keeps the type it took from the source. **Copied statement links.** ... A revert does not remove those statement links ... Review rows are not restored either ... After a revert the merge's own ledger row reads `state: "undone"`, whoever reverted it" | feature | verified | The two differences are the ones the tests pin: crates/utopia-store/tests/a_token_merge_is_gated_recorded_and_undone.rs:421-447, :1652, :1795-1796 and review_token_revert_tests.rs:680, review_proposal_answer_tests.rs:1492, :1586 (present; not executed, so the revert's behaviour is taken from the assertions and from the `review_revert` description at crates/utopia-server/src/api/review_tools.rs:271-276); crates/utopia-store/src/token_agent_actions.rs:83-92 (`undone` read from `entity_merges.reverted_at`, with no condition on who reverted) |
| "Known limits: a token revert is not one transaction with its ledger and audit rows ... A person's revert and a token's revert of one merge sent at the same moment can both run the revert" | feature | verified | crates/utopia-store/src/token_agent_actions.rs:962-1009 (the revert commits first; the two rows are retried three times, then the call fails with "was reverted, but its ledger and audit rows could not be written"), :1015-1054 (the advisory lock orders two token calls only); crates/utopia-server/src/api/review_routes.rs:1301-1331 (the session path takes no lock) (static) |
| Batches: "`POST /kbs/{kb_id}/review/batch` from a token takes 1 to 50 review ids and one action, `merge` or `keep`. (A signed-in person's batch on the same route is unchanged: up to 500 ids, no gate, and its old answer `{"decided", "outcomes": [{"id", "error"}]}`.) ... One outcome per id, in request order. All seven keys are always present ... `decided` counts the `applied` members ... Refused as a whole, with nothing changed and no ledger row: an empty list or more than 50 ids (422 `batch_size`), and an action other than `merge` or `keep` (422 with no code). The scope and role checks come first" | feature | verified | crates/utopia-server/src/api/review_ops.rs:554-612: `authorize_write` :561, `batch_size` :562-567, `AppError::Validation("action must be merge or keep")` :568-571 (422 with no `code` per crates/utopia-server/src/error.rs:50, :66-69), one `batch_id` :572, members in order through `merge_pair` or `keep` :575-580, seven keys in both outcome forms :587-607, `decided` counted on `applied` :583-585; session branch crates/utopia-server/src/api/review_routes.rs:1035-1100 (`BATCH_MAX` 500 :1007, `{"decided", "outcomes"}` :1100, `ReviewBatchOutcome` = `id`, `error` at crates/utopia-core/src/models.rs:1668-1671); `batch_id` on ledger and audit rows crates/utopia-store/src/token_agent_actions.rs:375, :399-401, :255-257; tests review_token_batch_tests.rs:413, :546, :648 (not executed) |
| "**The unit revert.** `POST /kbs/{kb_id}/review/agent-batches/{batch_id}/revert` reverts every merge the batch made, best effort. A `write` token of an editor and a signed-in editor may both call it; there is no button for it in the Review tab" with its answer `{"batch_id", "reverted", "outcomes": [{"merge_id", "outcome", "code", "error"}]}` and bullets (already reverted → that item's `already_decided`; a batch with no merge → 200 with `"reverted": 0`; 404 only for an unknown batch id; `invalid_body` for a token) | feature | verified | crates/utopia-server/src/api/review_routes.rs:1163-1207 (token branch with `invalid_body` :1172-1187; session branch through `revert_as_person` :1108-1151, which folds an already reverted merge into `already_decided` :1128-1134); crates/utopia-server/src/api/review_ops.rs:628-654, :664-682; crates/utopia-store/src/token_agent_actions.rs:1134-1160 (`None` → 404; otherwise the applied merge rows in the order made, possibly none); no `agent-batches` string in web/src outside this document; tests review_token_batch_tests.rs:739, :1208 (not executed) |
| "**The limit.** A token finalizes at most 200 writes per base in a rolling hour ... The 200th write applies; the next call answers **HTTP 429 with code `agent_rate_limited`**, changes nothing and writes no ledger row ... the limit is checked before the gate ... the count is read before the write, not in its transaction" | config | verified | crates/utopia-server/src/api/review_ops.rs:290, :317-332 (`finalized_since` over the last hour; `finalized >= 200` → `rate_limited()` before any store call), :293-301; crates/utopia-server/src/error.rs:45-48 (`TOO_MANY_REQUESTS`), :122-128 (unit test of the mapping, not executed); crates/utopia-store/src/token_agent_actions.rs:1170-1185 (counts `applied` rows of every operation, for that token and base); `guard` runs before the store functions that ask the gate (crates/utopia-server/src/api/review_ops.rs:497-498, :695-696); test review_token_guard_tests.rs:370 (not executed) |
| "**The breaker.** When people have reverted two merges made by one token on one base within seven days, that token's merges and reverts on that base stop being applied: each lands as a proposal with reason `breaker` ... Its keeps still finalize" with its bullets (a revert made by a token and a revert of another token's merge are not counted; a person's revert is recognised from the ledger and from the person's own `merge.revert` audit row; "separate from" the governance fuse) | feature | verified | crates/utopia-store/src/token_agent_actions.rs:1200-1225 (merges with `reverted_at` inside `FUSE_WINDOW_DAYS`, made by this token per the ledger, with no applied token `merge.revert` row or with a `merge.revert` audit row that has an actor and no `token_id`; tripped at `FUSE_REVERTS` or more), :423-425; crates/utopia-server/src/api/review_ops.rs:333-343 (asked for `Merge` and `RevertMerge` only); crates/utopia-store/src/governance.rs:1088-1089 (7 and 2); a token revert calls no `after_revert` (crates/utopia-server/src/api/review_ops.rs:720-737); tests review_token_guard_tests.rs:531, :698 (not executed) |
| Refusal codes table: 401 none; 404 none (five reads; unknown pair, entity, merge or batch id on a write); 422 `unknown_queue`, `unknown_type_filter`, `self_merge`, `batch_size`, none (unknown batch action), `invalid_body`; 403 `token_read_only`, `token_kb_out_of_scope`, `token_owner_below_editor`, `token_operation_not_open`; 409 `already_decided`; 409 none while the governor decides; 429 `agent_rate_limited` | feature | verified | crates/utopia-server/src/error.rs:23-50 (status mapping; `Validation` and `Conflict` carry no code); crates/utopia-server/src/api/review_ops.rs:41-47, :52-64, :69-84, :104-106, :293-301, :562-571; crates/utopia-store/src/resolution.rs:1870 `unknown_type_filter`; crates/utopia-server/src/auth.rs:252-255, :262-265; crates/utopia-store/src/token_agent_actions.rs:190, :273-278, :497-502, :570, :725-730, :740, :918; crates/utopia-server/src/api/review_routes.rs:366-371 (`token_operation_not_open` for an action other than `keep` or `merge`), :1287-1291; tests review_token_reads_tests.rs:405, :694, review_token_merge_tests.rs:1137 (not executed) |
| "The checks run in a fixed order: the token's scope, then its knowledge-base list, then the owner's role, and only then the request itself (batch size, batch action, the revert body, an action that is not open) and the limit" | feature | verified | crates/utopia-server/src/auth.rs:247-268 (scope, then base list); crates/utopia-server/src/api/review_ops.rs:69-84 (then role), :561-571 (batch size and action after `authorize_write`), :368-375, :496-497 (`guard` after `authorize_write`); crates/utopia-server/src/api/review_routes.rs:369-370, :1286-1292, :1179-1185 (`authorize_write` before `not_open` and before `invalid_body`) |
| "**What stays closed to tokens.** ... on duplicate pairs and merges it has the writes described above: keep, merge behind the gate, revert and batch. Nothing else in Review is decided by a token ... Every other route that decides a Review item ... is also session only and answers a token with 401 ... One Review write was open to `write` tokens before the Review tools existed and is unchanged: deciding an Alignment kind word" | feature | verified | crates/utopia-server/src/api/review_routes.rs: `AuthUser` at :252-254, :313-315, :439-441, :921-923, :959-961, :1209-1211, :1233-1235, :1475-1477, :1528-1530, :1679-1681, :1734-1736, :1800-1802, :1863-1865, :1935-1937, :1973-1975, :2155-2157; :2091-2097 (`decide_alignment_kind_word`: `SessionOrToken`, `require_write`); :366-371; crates/utopia-server/src/error.rs:34; test review_token_reads_tests.rs:1049 (not executed). No sentence of this document says merge, manual merge, revert or batch is closed to tokens (the phrases "until a later change opens them" and "401 until" of the prior version are gone; `grep -n -i "closed to\|until a later\|not open"` finds only this paragraph and the `token_operation_not_open` rows) |

## Descriptive docs

- CODE_OF_CONDUCT.md
- CODE_OF_CONDUCT.zh-CN.md
- crates/utopia-ingest/tests/fixtures/calamine/LICENSE-MIT.md

## Alignment (alignment mode only)

Not applicable: alignment mode is off for this audit.

## User rulings

| Queue # | Ruling | User's words | Planned |
|---------|--------|--------------|---------|
| 1 | fix-doc | Delegated (driver D009): remove the references to docs/walkthrough.md — doc-only, changes no behaviour | no |
| 4 | fix-doc | Delegated (driver D010): say which parts of agent memory over MCP remain — doc-only, changes no behaviour | no |
| 6 | fix-doc | Delegated (driver D011): restate the Rust requirement, do not pin rust-version — pinning would change the build | no |
| 11 | fix-doc | Delegated (driver D012): same ruling as queue 1 — doc-only, changes no behaviour | no |
| 13 | accept-drift | Delegated (driver D013): a claim about upstream forge settings, not checkable from the tree and unrelated to this program | n/a (accept-drift) |
| 14 | accept-drift | Delegated (driver D014): same ruling as queue 13 | n/a (accept-drift) |
| 16 | fix-doc | Delegated (driver D015): remove the stdout sentence — implementing stdout output would add behaviour and is left to the owner | no |
| 17 | fix-doc | Delegated (driver D016): drop or explain "no gaps" — doc-only, changes no behaviour | no |
| 20 | fix-doc | Delegated (driver D017): correct ADR 0052's status line — doc-only, changes no behaviour | no |
| 28 | fix-doc | Delegated (driver D018): point to the table-cap constant or correct the number — doc-only, changes no behaviour | no |
| 36 | fix-doc | Delegated (driver D074): state the MCP endpoint's 403 in ADR 0076 decision 6; the endpoint's answer does not change | T004 |
| 37 | fix-doc | Delegated (driver D077): say manual merge, revert and batch are session-only and answer a token with 401 until M003 opens them | T004 |
| 38 | accept-drift | Delegated (driver D078): a forge-side claim like queue 13 and 14 | n/a (accept-drift) |
| 39 | fix-doc | Delegated (driver D123): name migration 0129 beside 0128 in ADR 0076 `## Numbers` — doc-only, changes no behaviour | T010 |
| 40 | fix-doc | Delegated (driver D123): reword the open question to what the store and HTTP tests cover, and say that no full govern round with a model was run after a token write — doc-only, changes no behaviour | T010 |
| 41 | fix-doc | Delegated (driver D123): name the token Review routes of ADR 0076 in docs/design/access-and-audit.md or point to web/src/docs/mcp.md — doc-only, changes no behaviour | T010 |
| 42 | fix-doc | Delegated (driver D123): scope the sentence in docs/design/chat-and-mcp.md to `remember` and name the Review tools — doc-only, changes no behaviour | T010 |

## Remediation queue

| # | Doc | Claim | Verdict | Class | Suggested action |
|---|-----|-------|---------|-------|------------------|
| 1 | README.md | "The [walkthrough](docs/walkthrough.md) runs this end to end with `curl`" | aspirational | NEEDS-USER | Either add docs/walkthrough.md (and un-ignore it in .gitignore) or remove the two references to it |
| 2 | README.md | "Derivation is off by default, since a wrong axiom derives wrong facts" | stale | fix-doc | State that derivation is on by default for new bases since migration 0050 |
| 3 | README.md | "[ ] **Business rules**: rules written by people over an entity's attribute facts ... (#277)" | stale | fix-doc | Move business rules out of the Roadmap into Features (0021 implemented) |
| 4 | README.md | "[ ] **Agent memory over MCP**: episode writes, the retrieve endpoint, and the MCP server" | stale | NEEDS-USER | Say which parts remain; the MCP server and `remember` over MCP are built |
| 5 | README.md | "[ ] **Enterprise**: OIDC SSO, backup and restore commands, benchmarks at 100k documents" | stale | fix-doc | Drop OIDC SSO and backup/restore from the item; keep only the 100k benchmark |
| 6 | README.md | "Requirements: Docker (local development also needs Rust 1.85+, Node 20+, pnpm)" | unverifiable | NEEDS-USER | Decide whether to pin `rust-version = "1.85"` in the workspace so the floor is enforced, or restate it |
| 7 | README.zh-CN.md | "[ ] **问数与映射添加数据湖仓支持**：Iceberg / Delta Lake，以及 Databricks、Snowflake、MaxCompute 的映射探索与 Ontology2SQL 支持" | stale | fix-doc | Align with README.md: only MaxCompute is open |
| 8 | README.zh-CN.md | "[ ] **精确到时刻**：在年 / 月 / 日之外加一档 `instant` 精度" | stale | fix-doc | Remove the item (0024 implemented, migration 0033) |
| 9 | README.zh-CN.md | "[ ] **企业化**：OIDC SSO、备份恢复命令、10 万文档级别的性能基准" | stale | fix-doc | Keep only the 100k benchmark |
| 10 | README.zh-CN.md | "[ ] **业务规则**：由人写下、作用于实体属性事实的规则" | stale | fix-doc | Move to features (0021 implemented) |
| 11 | README.zh-CN.md | Use-case row links the walkthrough (`docs/walkthrough.md`) | aspirational | NEEDS-USER | Same ruling as queue item 1 |
| 12 | CONTRIBUTING.md | "that is how the `migrations` job in CI runs the whole `utopia-store` suite" | stale | fix-doc | Name the `test` job (part of `backend`); the `migrations` job now only replays migrations |
| 15 | .roadmap-proposals/backup-restore.md | "`schema_version`: pulled from the running migrations" | stale | fix-doc | Say it is the `CURRENT_SCHEMA_VERSION` constant in crates/utopia-cli/src/main.rs |
| 16 | .roadmap-proposals/backup-restore.md | "the operator can pipe `utopia backup --output -` to `aws s3 cp - s3://…` today" | aspirational | NEEDS-USER | Either implement stdout output or remove the sentence |
| 17 | docs/decisions/README.md | "File names are `NNNN-short-english-title.md`, four digits, increasing in creation order, no gaps" | stale | NEEDS-USER | Numbers 0055–0059 and 0068 are absent; either explain the reserved numbers or drop "no gaps" |
| 18 | docs/decisions/0005-alert-center.md | "Status: Implemented · open: `document.no_text_layer` is unwired" | stale | fix-doc | Update the status line: the gap is closed by `document.needs_reader` |
| 19 | docs/decisions/0015-recording-a-sentence-is-not-asserting-a-fact.md | "Status: Implemented (#180) · open: `remember` over MCP" | stale | fix-doc | Remove the open item; `remember` is served over MCP (#318) |
| 20 | docs/decisions/0052-document-content-is-a-read-contract.md | "Status: Proposed" | stale | NEEDS-USER | Code and docs/design/access-and-audit.md cite 0052 for shipped routes; decide the correct status word and what stays open |
| 21 | docs/design/README.md | "`proposed`: ... 0044 and 0045 sit in PRs #710 and #724" | stale | fix-doc | Update the status-words paragraph: both records are In progress with cuts built |
| 22 | docs/design/access-and-audit.md | "a base has the roles Viewer, Editor and Admin (`access::kb_role`, `require_kb`)" | stale | fix-doc | Add Owner, matching `enum Role` and README.md |
| 23 | docs/design/access-and-audit.md | "Proposed and not built: OIDC SSO, backup and restore [0016 E, #712]" | stale | fix-doc | Move both to "What it does today" |
| 24 | docs/design/governance.md | "[0043] (every queue, in PR #699)" | stale | fix-doc | PR #699 is closed per 0043's own status line; say the work is to re-land |
| 25 | docs/design/identity.md | "Proposed and not built: ... `doc_cache` removed" | stale | fix-doc | `doc_cache` is already gone from the code; drop it from the not-built lists |
| 26 | docs/design/interface.md | "Kinds: `source.sync_failed`, `llm.unreachable`, `llm.rate_limited`, `llm.out_of_credit`, `data_source.schema_sync_failed`, `document.needs_reader`, `governance.tripped`" | stale | fix-doc | Add `document.contents_truncated` and `mapping.exploration_empty` |
| 27 | docs/design/lakehouse-and-actions.md | "the scheme picks the engine (`postgres`, the MySQL wire family since #303, `trino`, `databricks`, `snowflake`)" | stale | fix-doc | Add ClickHouse (migration 0106) |
| 28 | docs/design/lakehouse-and-actions.md | "How much schema to fetch from a lakehouse catalog (cap 200 tables today)" | unverifiable | NEEDS-USER | Point to the constant that caps tables, or correct the number |
| 29 | docs/design/ontology.md | "A property regression case can also expect no binding (`expected_binding: \"none\"` ...)" — listed under "Proposed and not built" | stale | fix-doc | Move the built regression-case behaviour (migration 0118) to "What it does today" |
| 30 | docs/design/sources.md | "Proposed and not built: Backup and restore (#712)" | stale | fix-doc | Remove; the `utopia-cli` backup/restore commands exist |
| 31 | docs/design/time.md | "[0045] (time mentions, proposed)" | stale | fix-doc | 0045 is In progress with cuts 1–3 built; move the built parts to "What it does today" |
| 32 | docs/pipeline.md | "Chunk 1200 chars · overlap 150" | stale | fix-doc | Rewrite around the token-budget chunker (0039) |
| 33 | docs/pipeline.md | "Eleven reason codes, all recorded in `extraction_drops`" (`not_an_entity_name`, `subject_not_declared`, `attr_domain_mismatch`, `attr_no_value`, `direction_corrected`, `domain_mismatch` among them) | stale | fix-doc | Rewrite the drop table for the open-graph path, as docs/design/README.md already promises |
| 34 | docs/pipeline.md | "The ... signature check ... lives in the store (`ontology::judge_direction`). Extraction calls it when writing new facts" | stale | fix-doc | Remove or rewrite; no such function remains |
| 39 | docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md | "This record is 0076 and the ledger's migration is 0128" | stale | fix-doc | Name migration 0129 in `## Numbers` beside 0128, as the status line and decision 7 already do |
| 40 | docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md | "The precedent exclusion has not been exercised end to end." | stale | NEEDS-USER | Either close the open question citing the precedent tests, or restate what is still unexercised (no test runs a full govern round with a model after a token write) |
| 41 | docs/design/access-and-audit.md | "apart from MCP, every other route is session-only [0014 d6, d7]" | stale | fix-doc | Add the ten token Review routes of ADR 0076 (five reads, five writes) to the `SessionOrToken` list, or point to web/src/docs/mcp.md for them |
| 42 | docs/design/chat-and-mcp.md | "Over MCP an external agent can therefore only propose [0015 d6]" | stale | fix-doc | Scope the sentence to `remember`, and name the six Review tools and where they live (review_tools.rs, MCP only) |
