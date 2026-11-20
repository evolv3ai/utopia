# Docs Audit

Repo root: D:\utopia
Audited: 2026-11-19
Audited HEAD: 6c52ce8f8091b18d8713ce8d509fec756644a4f1
Alignment mode: no

## Summary

| Verdict | Count |
|---------|-------|
| verified | 220 |
| stale | 27 |
| aspirational | 3 |
| unverifiable | 2 |
| descriptive docs (no testable claims) | 3 |

Worst drift: README.md's Roadmap still lists business rules, OIDC SSO and backup/restore commands as unbuilt (and two design files repeat "proposed and not built") while `attribute_rules`, `oidc_routes.rs` and the `utopia-cli` backup/restore crate are in the code. The two documents rewritten in M002 (web/src/docs/mcp.md and ADR 0076) carry no drift: every claim extracted from them checked true against the code at this HEAD.

Method note: delta re-audit against the prior audit (Audited HEAD 904b2a2). All checks ran in the disposable verify sidecar at the audited HEAD. Rows marked `unchanged: ` are carried under the delta rule (verified, not a command claim, and neither the document nor any path the evidence names is in the changed set). Every other row was re-verified at 6c52ce8, in one of three ways. (1) web/src/docs/mcp.md and docs/decisions/0076 were rewritten in M002, so their claims were extracted afresh and each was checked by reading the code (review_ops.rs, review_tools.rs, review_routes.rs, graph_routes.rs, mcp.rs, api/mod.rs and the store list/total functions), with line numbers as they are now. (2) Rows of other documents whose evidence names a changed file (api/mod.rs, the review and store files, web/src/api.ts, ADR 0076) were looked up again and their line numbers corrected where the code moved. (3) For the remaining rows the document and every file the evidence names are byte-identical between the two heads (`git diff --stat 904b2a2 6c52ce8` lists 25 product files, none of them named by those rows), so the recorded file:line evidence was confirmed to hold unchanged; each named path was resolved against `git ls-files` at 6c52ce8, and the absence greps (`STALL_NUDGE`, `on_model_select`, `AUTO_CLOSE_MIN_CONFIDENCE`, `doc_cache`, `judge_direction`, action tables, `feishu`, SPARQL routes, 0049 references, the pipeline drop codes) were run again. Two commands were executed: `node --test scripts/bench/lib.test.mjs` (15 pass) and `cargo fmt --all --check`, which now exits 0 (the prior audit saw exit 1). `cargo test`, `cargo clippy`, `pnpm build`, Docker, the DB-backed review tests and live-model bench runs were not executed; no remote was contacted, so tests cited as evidence are cited by name and line, not by result. For decision records the audited claim is the record's own status line, except 0076, whose decisions were checked individually. Retired rows: queue items 35, 36 and 37 (ADR 0076) were ruled fix-doc and the fixes landed in M002 (status line reworded, decision 6 and decision 12 each carry a dated revision), so the old stale and aspirational claims are replaced by verified ones; the claim "0127 is claimed by upstream #1496" is suppressed by the accept-drift ruling on queue item 38, as the two branch-protection claims in CONTRIBUTING.md and CONTRIBUTING.zh-CN.md are by the rulings on items 13 and 14. No new queue item was opened.

## Doc: .github/PULL_REQUEST_TEMPLATE.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "`CURRENT_SCHEMA_VERSION` in `crates/utopia-cli/src/main.rs` equals the number of files in `migrations/`" | structure | verified | crates/utopia-cli/src/main.rs:85 `CURRENT_SCHEMA_VERSION: u32 = 114`; `ls migrations` → 114 files (0128 added, constant bumped by commit aab411c) |
| "For changes under `web/`: `pnpm build` and `pnpm test` pass" | command | verified | web/package.json scripts `build`, `test` exist; .github/workflows/checks.yml `web` job runs both (static; not executed) |
| "UI strings are in both `web/src/i18n/en.ts` and `zh.ts`" | structure | verified | web/src/i18n/ contains en.ts, zh.ts, index.ts |

## Doc: .roadmap-proposals/backup-restore.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Both halves are implemented" (`utopia backup` and `utopia restore` in a separate `utopia-cli` crate) | feature | verified | crates/utopia-cli/src/main.rs (backup manifest write at :377, restore version gate at :534), crates/utopia-cli/tests/restore.rs; workspace member in Cargo.toml |
| "`restore` will refuse a manifest whose `schema_version` is greater than the running server's" | feature | verified | crates/utopia-cli/src/main.rs:534 `if manifest.schema_version > CURRENT_SCHEMA_VERSION`, :541 older-version warning branch |
| "The sealing key is left out of the archive by default" | feature | verified | crates/utopia-cli/src/main.rs:26-29 `--include-secret-key` doc comment "Off by default"; :432-434 |
| "`schema_version`: pulled from the running migrations" | feature | stale | crates/utopia-cli/src/main.rs:79-85: a hand-maintained constant `CURRENT_SCHEMA_VERSION`, "Kept as a constant — bumping is a deliberate decision" |
| "the operator can pipe `utopia backup --output -` to `aws s3 cp - s3://…` today" | command | aspirational | No `"-"`/stdout output handling in crates/utopia-cli/src/main.rs (grep for `"-"` and stdout finds only `Stdio::null()` at :329); same doc says backup writes a single `*.tar.gz` file and refuses to overwrite |
| "CI provides PostgreSQL 16 clients" | config | verified | unchanged: .github/workflows/checks.yml `test` job installs `postgresql-client-16` |

## Doc: CONTRIBUTING.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "CI runs exactly this" (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `pnpm install --frozen-lockfile && pnpm build`) | command | verified | .github/workflows/checks.yml:41,43,84,182 run these exact commands (lint/test/web jobs). `cargo fmt --all --check` executed in sidecar at 6c52ce8 → exit 0, no diff (the two diffs the prior audit reported in crates/utopia-server/src/api/review_attribution_tests.rs are gone); clippy, test and pnpm build not executed |
| "that is how the `migrations` job in CI runs the whole `utopia-store` suite" | config | stale | .github/workflows/checks.yml: `UTOPIA_TEST_REQUIRE_DB` is set on the `test` job; the `migrations` job only runs `sqlx migrate run` twice and its comment says the DB-backed tests no longer run there |
| "`test_db::url()` reads `UTOPIA_DATABASE_URL`. With `UTOPIA_TEST_REQUIRE_DB=1` also set, a missing database is a failure" | config | verified | crates/utopia-store/src/test_db.rs references both variables |
| "export UTOPIA_DATABASE_URL=postgres://utopia:utopia@localhost:1517/utopia" (1517 is the host-side port) | config | verified | unchanged: docker-compose.yml:27 `${UTOPIA_DB_BIND:-127.0.0.1:1517}:5432`; .env.example same URL |
| "`cargo test --locked -p utopia-store --test human_phrase_materialization_delivery -- --ignored --skip crash_child ...`" and the `busy_defers_without_retaining_connections` lib test | command | verified | crates/utopia-store/tests/human_phrase_materialization_delivery.rs (contains `crash_child`); crates/utopia-store/src/materialize_delivery_tests.rs `busy_defers_without_retaining_connections` (static; not executed) |
| "CI runs `cargo test --workspace`, `pnpm test` and `scripts/bench/lib.test.mjs`, and nothing else" | config | verified | .github/workflows/checks.yml test + web jobs; `node --test scripts/bench/lib.test.mjs` executed in sidecar at 6c52ce8 → tests 15, pass 15, fail 0 |
| "`docker-compose.yml` defaults `UTOPIA_IMAGE` to a fixed tag ... the pin has been on one of them since rc8" | config | verified | unchanged: docker-compose.yml:49 `${UTOPIA_IMAGE:-ghcr.io/deeplethe/utopia:0.1.0-rc9}` |
| "Every workflow declares its own `permissions:`" | config | verified | unchanged: grep `permissions:` → checks.yml, release.yml, star-history.yml each have one block |
| "Source installs need Poppler ... the Docker image includes both" (`poppler-utils poppler-data`) | config | verified | docker/Dockerfile:27 installs `poppler-utils poppler-data`; crates/utopia-ingest/src/parsers.rs uses `pdftotext` |

## Doc: CONTRIBUTING.zh-CN.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "`cargo fmt --all --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace` / `pnpm install --frozen-lockfile && pnpm build`" (CI 跑的就是这些) | command | verified | .github/workflows/checks.yml:41,43,84,182; `cargo fmt --all --check` executed in sidecar at 6c52ce8 → exit 0, no diff; the listed commands are the ones CI runs (others static) |
| "`cargo run -p utopia-store --example migrate` # 空库先迁移；CI 的 backend job 也是这么做的" | command | verified | crates/utopia-store/examples/migrate.rs; checks.yml `test` job step runs the same command (static; not executed) |

## Doc: README.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "The [walkthrough](docs/walkthrough.md) runs this end to end with `curl`" | structure | aspirational | docs/ contains only decisions/, design/, pipeline.md; `git log --all -- docs/walkthrough.md` → no commits; .gitignore ignores `/docs/*` except decisions, design and pipeline.md, so the file cannot be tracked as written |
| "One Rust binary and one Postgres. Full-text search is embedded in the binary, vectors go in pgvector, and the job queue is a table" | structure | verified | Cargo.toml deps `tantivy`, `pgvector`; docker-compose.yml has only `db` (pgvector/pgvector:pg16) and `app`; crates/utopia-store/src/jobs.rs |
| "Upload PDF, DOCX, PPTX, XLSX, XLS, ODS, CSV, TSV, Markdown, HTML or plain text" | feature | verified | crates/utopia-ingest/src/lib.rs:347-350 dispatch on xlsx/xls/ods/pptx/csv/tsv; Cargo.toml `pdf-extract`, `calamine`, `htmd`, `chardetng` |
| "Web pages, RSS, GitHub, Jira, Notion, WebDAV and S3-compatible buckets sync on a schedule" | integration | verified | crates/utopia-core/src/models.rs:306 `SourceKind` has Url, Rss, GithubIssues, GithubRepo, JiraIssues, Notion, Webdav, S3 (and also Gitlab*, AzureBlob, Gcs, GoogleDrive, which the README does not list) |
| "Scans and images are read by a MinerU service or a Volcengine Ark vision model" | integration | verified | crates/utopia-core/src/models.rs:537-542 (MinerU layout service setting); crates/utopia-server/src/readers/ark_ocr.rs |
| "Full-text on Tantivy, vectors on pgvector, fused with RRF" | feature | verified | crates/utopia-search/src/lib.rs and crates/utopia-server/src/retrieval.rs reference RRF; Cargo.toml `tantivy`, `pgvector` |
| "An MCP server for every knowledge base" | feature | verified | crates/utopia-server/src/api/mod.rs:240 `.route("/kbs/{id}/mcp", post(mcp::handle))` |
| "Five ship inside the binary: schema.org, W3C Org, PROV-O, FOAF and IOF Core" | feature | verified | crates/utopia-server/src/ontology_packs.rs:42-78 five `include_bytes!` of crates/utopia-server/packs/*.gz |
| "Derivation is off by default, since a wrong axiom derives wrong facts" | config | stale | migrations/0050_a_new_base_starts_with_its_engines_on.sql `ALTER COLUMN materialize_inferences SET DEFAULT TRUE`; crates/utopia-core/src/models.rs:1208-1211 comment says default on (0050) |
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
| "Optional least-privilege runtime role — set `UTOPIA_APP_DB_PASSWORD` and `UTOPIA_MIGRATION_URL`" | config | verified | unchanged: docker-compose.yml:19,62; docker/init-app-role.sh; migrations/0010_least_privilege_role.sql |
| "Data sources reach only granted workspaces" | feature | verified | unchanged: migrations/0014_data_source_grants.sql:22 `CREATE TABLE data_source_grants` |
| "Accounts are deactivated, not deleted — `users.deactivated_at`" | feature | verified | unchanged: migrations/0001_core.sql contains `deactivated_at` |
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
| "Status: Implemented (#126, #140, #148)" | status | verified | unchanged: commits 97da5ea (#126), bc8620f (#140), 3726191 (#148); migrations/0006_semantic_layer.sql `concept_mappings` |

## Doc: docs/decisions/0012-the-ontology-is-a-contract-not-a-suggestion.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (#190, #196)" | status | verified | unchanged: migrations/0019_a_signature_holds_on_every_path.sql (PR numbers not found as commit subjects; migration is the evidence) |

## Doc: docs/decisions/0013-a-source-should-hand-over-its-history.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented for GitHub, Jira, Notion and GitLab (#134, #135, #213, #1327) ... open: Feishu, Confluence" | status | verified | commits e32c158, 10c9648, 0f3ff5a, 4ed8a59, 63b8fd3; `SourceKind` has Github*, Jira, Notion, Gitlab*; no `feishu` in crates/ or web/src (grep repeated at 6c52ce8) |

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
| "Status: In progress · A, B1–B3, C1 and C2 done · open: B4, C3–C5, D" | status | verified | Sampled: A1 migrations/0018_a_fact_awaiting_a_nod.sql; C2 crates/utopia-core/src/models.rs:1212 "(0016 C2)"; C1 scripts/bench/truth/*.wikidata.json; B4 (R3) has no code |

## Doc: docs/decisions/0017-a-contradiction-points-upstream.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (#238, #243)" | status | verified | unchanged: commits 52809eb, 417b49b; migrations/0020_a_contradiction_points_upstream.sql |

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
| "Status: Implemented (#359)" | status | verified | unchanged: commit 0b18c7c; migrations/0028_a_rule_reads_attributes.sql |

## Doc: docs/decisions/0022-an-unknown-date-is-not-an-open-one.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented in five cuts (#394, #393, #679) · revised 2026-10-03 (#1054)" | status | verified | unchanged: migrations/0030_an_unknown_date_is_not_an_open_one.sql; commits 573fda3 (#679), b4b1123 (#1054) |

## Doc: docs/decisions/0023-rss-observations-are-not-documents.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (#326)" | status | verified | unchanged: commit 28db351; migrations/0032_rss_full_content.sql |

## Doc: docs/decisions/0024-the-world-axis-reaches-the-second.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (migration 0033)" | status | verified | unchanged: migrations/0033_the_world_axis_reaches_the_second.sql |

## Doc: docs/decisions/0025-governance-reads-the-ledger-before-it-decides.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented · cuts 1–4 (#434, #437)" | status | verified | unchanged: commits 1e061e7, d391b99; migrations/0035_governance_reads_the_ledger.sql |

## Doc: docs/decisions/0026-a-decision-records-why.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (migration 0038)" | status | verified | unchanged: migrations/0038_a_decision_records_why.sql |

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
| "Status: Implemented (migration 0039)" | status | verified | unchanged: migrations/0039_a_rule_can_say_or.sql |

## Doc: docs/decisions/0030-a-rule-may-read-what-a-rule-concluded.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: In progress · cuts 1–2 built (migration 0040)" | status | verified | unchanged: migrations/0040_a_premise_may_be_derived.sql |

## Doc: docs/decisions/0031-an-event-holds-at-the-moment-it-names.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (#486, #593) · open: the UI cut" | status | verified | unchanged: commit 2838296 "A relation says whether it happens or holds" (#593) |

## Doc: docs/decisions/0032-a-rule-computes-what-it-concludes.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: In progress · cuts 1–2 built (migration 0041)" | status | verified | unchanged: migrations/0041_a_rule_computes_what_it_concludes.sql |

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
| "Status: In progress · cuts 1 and 1b built (#598, #600)" | status | verified | unchanged: commits 0e3123b, 1333049; migrations/0049_a_relation_declares_its_qualifiers.sql |

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
| "Status: Implemented · cuts 1–3 ... (migrations 0058–0060)" | status | verified | unchanged: migrations/0058, 0059, 0060 present |

## Doc: docs/decisions/0041-a-name-is-a-claim-about-an-entity.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: In progress · cut 1 built (#670), cut 2 channel 2 built 2026-09-23, revised 2026-09-25 (#889)" | status | verified | unchanged: commits f680576, 2f90d8b; migrations/0055_a_name_is_a_fact.sql, 0080_a_name_has_a_vector.sql |

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
| "Status: Implemented 2026-09-25 (#861, migration 0071)" | status | verified | unchanged: commit 535aac0; migrations/0071_a_rule_joins_two_entities.sql |

## Doc: docs/decisions/0048-provenance-references-stay-inside-the-knowledge-base.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented (#832, migration 0070)" | status | verified | unchanged: commit ea0557b; migrations/0070_provenance_never_points_across_a_knowledge_base.sql |

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
| "Status: Implemented · cut 1 (#884) · revised 2026-10-23 (#1375)" | status | verified | commits 22c21ea, ead46cb; crates/utopia-server/src/api/mod.rs:509 `/sources/{source_id}/statements` (line moved from :507); `SourceKind::Statements` |

## Doc: docs/decisions/0060-a-rule-definition-has-a-history.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented 2026-09-25 (#913, migration 0076)" | status | verified | unchanged: commit a656cb0; migrations/0076_a_rule_definition_has_a_history.sql |

## Doc: docs/decisions/0061-the-ontology-is-proposed-from-the-open-graph-and-judged-by-its-questions.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: In progress · accepted 2026-09-26 · cuts 1 and 1.1 built (#947, #950), cut 3 slices 1–2 (#955, #962)" | status | verified | commits b07af8f, 4f9458c, 6f74a08, 6ef840a; migrations 0077, 0078; crates/utopia-server/src/ontology_agent.rs |

## Doc: docs/decisions/0062-the-export-says-what-is-contested.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented 2026-09-26 (#922, migration 0093)" | status | verified | unchanged: commit 06768aa; migrations/0093_an_axiom_that_moved_closes_its_findings.sql |

## Doc: docs/decisions/0063-stopping-a-chat-ends-the-generation.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Status: Implemented 2026-09-27 (#969, migration 0095)" | status | verified | unchanged: commit d9c3aab; migrations/0095_a_stopped_chat_keeps_its_partial_answer.sql |

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
| "Status: Implemented · 2026-10-25 · #1333, #1390, #1425, migration 0125" | status | verified | unchanged: commits 50904e9, 00f1172, 3b09f9f; migrations/0125 present |

## Doc: docs/decisions/0072-a-source-checkpoint-ends-at-a-timestamp-boundary.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Implemented 2026-11-10 (#1359, #1397, migration 0119)" | status | verified | unchanged: commits 6431a5a, 3d307f6; migrations/0119_a_source_checkpoint_ends_at_a_timestamp_boundary.sql |

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
| "Status: In progress · accepted 2026-11-18 · the keep slice built 2026-11-19 (migration 0128): a personal token reads Duplicates and Overview and keeps one duplicate pair through the policy function, over the token API and the MCP tools `review_list`, `review_item` and `review_keep`, with the `token_agent_actions` ledger, the precedent exclusion, and "owner via token name" in Agent History and Activity" | status | verified | migrations/0128_a_token_agent_action_is_kept_in_its_own_ledger.sql; crates/utopia-server/src/api/review_ops.rs:103-116 (list), :192-277 (item), :280-286 (summary), :293-316 (keep through `agent_policy`); review_tools.rs:28-30; crates/utopia-store/src/token_agent_actions.rs:56-180; governance.rs:85; web/src/pages/TokenAgentRow.tsx:11, Review.tsx:2274, KbSettings.tsx:21 (static; DB-backed tests not executed) |
| "the read slice built 2026-11-19 (no migration): a personal token lists all thirteen Review sections with a `total`, reads one listed item of any section and both proof routes" | status | verified | crates/utopia-server/src/api/review_ops.rs:80-92 `TOKEN_SECTIONS` (eleven queue keys), :121-133 `history`, :280-286 `summary`, :192-277 `item`, :138-159 both proofs; review_routes.rs:216-221 (`total` in the list body); graph_routes.rs:540-542, :562-564 (`SessionOrToken`); `ls migrations` → 114 files, highest 0128, the same as the prior baseline (no migration added); tests review_token_reads_tests.rs:198, :994 (not executed) |
| "open: this record has opened only the Keep row of the table to tokens, and the alignment kind-word decision was already open to write tokens under ADR 0014 decision 6; every other row stays closed to tokens (merge, manual merge, revert, batch, mapping decision, the consistency check, every propose-only operation and the proposal store)" | status | verified | crates/utopia-server/src/api/review_routes.rs:1468-1472 `decide_alignment_kind_word` takes `SessionOrToken` and calls `require_write`; `AuthUser` only at :252-254 (close), :313-315 (conflict), :434-436 (governor answer), :567-569 (batch), :641-643, :665-667 (confirm, reject), :689-691 (revert), :737-739 (manual merge), :852-854 (mapping), :905-907, :1111-1113 (violation, defect), :1056-1058, :1240-1242 (consistency check, inference), :1177-1179 (nod), :1312-1314, :1350-1352 (alignment rule, phrase), :1532-1534 (errata); a token's non-keep action on the decision route is refused at :358-365. The prior stale wording ("every row of the table except Keep") is gone from the status line and kept only as a quotation under Status history (0076:357-370) |
| "The handlers behind `GET /api/v1/kbs/{kb_id}/review?queue=duplicates`, `GET /api/v1/kbs/{kb_id}/review/summary` and `POST /api/v1/kbs/{kb_id}/review/{review_id}` (action `keep`) take `SessionOrToken` instead of `AuthUser`" (decision 1) | feature | verified | crates/utopia-server/src/api/review_routes.rs:85-87 (`list`), :351-353 (`decide`), :825-827 (`summary`) take `caller: SessionOrToken`; routes at api/mod.rs:534, 536, 551; the session branch runs the earlier code after the `caller.pat` early return (:92, :358, :831) |
| "The same extractor change now also covers the Decisions history (`GET /api/v1/kbs/{kb_id}/review/history`) and the two proof routes ... and the list handler serves a token all eleven queue sections ... No write handler changed" (decision 1, revision) | feature | verified | crates/utopia-server/src/api/review_routes.rs:801-803 (`history` takes `SessionOrToken`); graph_routes.rs:540-542 `derived_proof`, :562-564 `blocked_proof`; routes at api/mod.rs:535, 459-460, 464; review_ops.rs:80-92, :112-114 (eleven sections for a token); `git diff 904b2a2 6c52ce8 -- crates/utopia-server/src/api/review_routes.rs` shows no write handler gaining a token branch (the write handlers still take `AuthUser`, lines listed in the row above) |
| "`review_list` takes a `section` parameter and applies the existing 1 to 200 page clamp; `review_item` returns one item's evidence; `review_keep` keeps one duplicate pair ... The write tool is listed and callable only with scope `write` and an owner at least Editor" (decision 2) | feature | verified | crates/utopia-server/src/api/review_tools.rs:28-30, :181-202 (`review_keep` filtered out unless `can_write`), :298-360; review_routes.rs:134 `clamp(1, 200)`; mcp.rs:107-110 (`can_write` = write scope and owner at least Editor); a caller without the right that calls it anyway is refused inside review_ops.rs:60-75 |
| "`review_list` now serves all thirteen sections: the eleven queue sections with `limit` and `offset`, `decisions` with the history route's own `page` and `per`, and `overview` with no paging argument. `review_item` takes `section` and `id`; `section` left out means `duplicates` ... A name outside the thirteen still answers `unknown_queue`. Still three tools" (decision 2, revision) | feature | verified | crates/utopia-server/src/api/review_tools.rs:305-334 (`overview` → summary, `decisions` → history with `page`/`per`, anything else → list with `limit`/`offset`), :336-346 (`section` defaults to `duplicates`), :33-35 `serves` matches three names; review_ops.rs:95-97, :112-114, :233-235 (`unknown_queue`); tests review_mcp_reads_tests.rs:192, :963, :1039 (not executed) |
| "The table is compiled in. It is not a per-base setting" / "The policy function lives with the `execution_gate` module ... answers `applied`, `held` or `proposed`. In this cut only the Keep row is reachable by a token" (decisions 3 to 5) | structure | verified | crates/utopia-store/src/execution_gate.rs:124 `AgentOperation`, :161-173 `AgentOutcome` (`applied`, `held`, `proposed`), :181 `agent_ruling`, :200 `agent_policy`; outside tests the only callers pass `AgentOperation::Keep` (crates/utopia-server/src/api/review_ops.rs:301, crates/utopia-store/src/token_agent_actions.rs:67; grep repeated at 6c52ce8) |
| "A read needs an owner at least Viewer and a token that reaches the base; a token whose `kb_ids` exclude the base gets 404" with its revision: "That is the token API's answer only. The MCP endpoint checks the token's base list itself, before any tool runs, and answers such a token with HTTP 403 ... A dead or revoked token gets 401 on both" (decision 6) | feature | verified | Token API: crates/utopia-server/src/api/review_ops.rs:43-55 turns an owner below Viewer and an excluded base into `NotFound`. MCP: mcp.rs:96-98 returns `AppError::Forbidden` inside `authorize`, called first in `handle` (:177); error.rs:33-35 maps NotFound → 404, Unauthorized → 401, Forbidden → 403; crates/utopia-store/src/tokens.rs:157-170 (unknown, revoked or expired → `Unauthorized`); test review_token_reads_tests.rs:759 `a_dead_token_gets_401_and_an_excluded_token_gets_404_on_the_api_and_403_on_the_mcp_endpoint` (not executed). The revision (0076:131-138) is the fix ruled on queue item 36; it agrees with web/src/docs/mcp.md:244 |
| Codes table: `token_read_only`, `token_kb_out_of_scope`, `token_owner_below_editor`, `already_decided`, `token_operation_not_open`; "scope is checked first" (decision 6) | feature | verified | crates/utopia-server/src/auth.rs:247-268 (`require_token_write`: scope, then `kb_ids`); review_ops.rs:32-38, :60-75; crates/utopia-store/src/token_agent_actions.rs:182-187 (`CodedConflict` `already_decided`); error.rs:36-42 |
| "The ledger is `token_agent_actions` ... base, operation, target kind and id, owner, token, token name snapshot, outcome, request payload, time ... told apart by a `source` field" (decision 7) | structure | verified | migrations/0128_a_token_agent_action_is_kept_in_its_own_ledger.sql (columns kb_id, operation, target_kind, target_id, owner_id, token_id, token_name, outcome, payload, created_at); crates/utopia-store/src/token_agent_actions.rs:219-236 `"source": "token"`, :260 `agent_history` (governor rows taken unchanged from `governance::list`); review_routes.rs:199-205 |
| "Every token Review write adds `detail.token_id` and a `detail.token_name` snapshot ... The stored HTTP `User-Agent` is returned to readers marked as declared ... A failed snapshot or audit write fails the action" (decision 8) | feature | verified | crates/utopia-store/src/token_agent_actions.rs:110-119 (name snapshot), :164-178 (audit row written with `record_on` inside the transaction, then commit); audit.rs:189-197 `declared_client`; web/src/pages/TokenAgentRow.tsx:11, :38; test review_mcp_tests.rs:1040 (static; not executed) |
| "The governor's precedent queries and `settled_by_people` exclude audit rows whose detail carries a token id and review rows marked by the new nullable `resolution_reviews.decided_token`. `after_human_decision` is not called for a token decision ... `review/agent/{decision_id}`, stays session-only" (decision 9) | feature | verified | crates/utopia-store/src/governance.rs:85 `NOT_BY_TOKEN`, used at :101, :114, :128, :827; `decided_token IS NULL` at :171, :595; migrations/0128 `ADD COLUMN IF NOT EXISTS decided_token UUID`; crates/utopia-server/src/api/review_ops.rs:293-316 makes no `after_human_decision` call; review_routes.rs:434-436 `agent_answer` takes `AuthUser`; crates/utopia-store/tests/a_token_keep_is_never_human_precedent.rs exists (not executed) |
| "the session cap stays 500" (decision 10) | config | verified | crates/utopia-server/src/api/review_routes.rs:563 `BATCH_MAX: usize = 500`, :574 |
| "To a token: merge, manual merge, revert and batch (how each is answered: first revision below)" with its revision: "Only `merge` sent to the decision route is [answered `token_operation_not_open`]. Manual merge, revert and batch are session-only routes ... they answer a token with 401" (decision 12) | feature | verified | crates/utopia-server/src/api/review_routes.rs:358-365: a token sending any action other than `keep` to the decision route gets `review_ops::not_open()` (403 with the code); :567-569 `batch`, :689-691 `revert_merge`, :737-739 `manual_merge` take `AuthUser` only → 401 (error.rs:34); test review_token_api_tests.rs:352 asserts the code for merge and :366-375 asserts `UNAUTHORIZED` for the three (not executed). The revision (0076:186-192) is the fix ruled on queue item 37; it agrees with web/src/docs/mcp.md:296 |
| "The read slice opened all four ... Still closed to a token: merge, manual merge, revert and batch; every propose-only row and the proposal store; the per-token rate limit and breaker" (decision 12, read-slice revision) | status | verified | Opened: review_ops.rs:103-159, :192-277 (lists, totals, items, proofs). Closed: write handlers take `AuthUser` (review_routes.rs lines in the status row above); no rate limit or breaker exists on the token path (case-insensitive grep for `rate limit` and `breaker` in review_ops.rs and crates/utopia-store/src/token_agent_actions.rs → no hit); no proposal-store table in migrations/ (highest is 0128) |
| "`total` is a new field; `counts` is unchanged ... for Alignment (the badge omits proposed implication rules), Low confidence and Agent History the badge named after the section is not the number of listed rows" / "The Low confidence list is unchanged, and its total counts the listed rows, derived facts included" (decision 13) | feature | verified | crates/utopia-server/src/api/review_routes.rs:136-221 (`total` computed per queue beside `counts`); crates/utopia-store/src/review.rs:80 (`lowconf` badge keeps `derived_by_rule IS NULL`), :88-89 (`agent` badge counts `status = 'proposed'` only), :101-102 (`alignment` badge unions phrase and type bindings, no `implication_rules`); alignment_queue.rs:235 `total`; graph.rs:2105-2107 (`low_confidence_facts_total` without the derived filter); token_agent_actions.rs:246; tests review_totals_tests.rs:217, :395 (not executed) |
| "Decisions keeps `page` and `per` and its `{events, total}`" / "The Alignment list has an identifier tie-break. It is ordered by `decided_at` and then by the item's own identifier" (decision 13) | feature | verified | crates/utopia-server/src/api/review_ops.rs:121-133; review_tools.rs:312-322; crates/utopia-store/src/alignment_queue.rs:162-223 (`OrderKey = (decided_at, id or kind_word, kind index)`, sorted before `skip`/`take`); tests review_totals_tests.rs:513, review_token_reads_tests.rs:640 (not executed) |
| "`review_item` identifies an item by `section` and `id` ... an Alignment kind word has none and is named by its word, compared exactly as stored ... There is one not-found answer ... `overview` lists no items and gives the same answer" / "An Axioms item carries a `proof` key" / "`mappings` is refused for a token with `unknown_queue`. The list route serves it to a session" (decision 13) | feature | verified | crates/utopia-server/src/api/review_ops.rs:167-174 `is_the_item` (exact `kind_word` compare, no trim or case fold), :200-202 (`overview` → `NotFound`), :233-238, :260-262 (one `NotFound`), :265-275 (`proof` only for `violations`, null unless `derived_contradiction`), :80-92 (`mappings` not among token sections); review_routes.rs:171-174 (`mappings` arm for a session); tests review_mcp_reads_tests.rs:517, :615, :842, :963 (not executed) |
| "No Review write is added. No write handler gained a token branch and no MCP write tool was added; reading "Awaiting your nod" gives a token no way to nod" (decision 13) | feature | verified | crates/utopia-server/src/api/review_tools.rs:28-35 (three tools, one write); review_routes.rs:1177-1179 `decide_pending` takes `AuthUser`; tests review_mcp_reads_tests.rs:1039, review_token_reads_tests.rs:1049 (not executed) |
| "`save_agent_proposals`, `open_proposal`, `open_proposals` and `reject_proposal` in the store's ontology module ... `adopt` in `question_routes` (`POST /kbs/{id}/ontology/proposals/adopt`) ... `decide_proposal`, only rejects and answers `adoption_endpoint` ... `unadopt_types` ... behind `type_resolution_undo` ... `adopt_iri_onto_key`" | structure | verified | crates/utopia-store/src/ontology.rs:1979, 2003, 2021, 1963, 1561; crates/utopia-server/src/api/ontology_routes.rs:616-625, 808, 280; ontology_agent.rs:733, 1047, 1216; api/mod.rs:392 (adopt route), :335 (`type_resolution_undo`); crates/utopia-store/src/resolution.rs:2755 `unadopt_types` (moved from :2726) (symbols located; the "no statement moves a row out of adopted" finding not re-derived) |
| "The gaps are not reused (0055 to 0059 and 0068 here; 0081 to 0089, 0107, 0108, 0112 and 0124 among migrations)" | structure | verified | `ls docs/decisions` → 70 records (0054 → 0060, 0067 → 0069); `ls migrations` → 114 files, highest 0128, no 0127 (both listings repeated at 6c52ce8; neither directory changed since the prior baseline) |
| "`scripts/check-upstream-numbers.sh <adr-number> <migration-number>` re-checks both ... Exit 0 is free, 1 names the colliding file, 2 means upstream could not be read" | command | verified | scripts/check-upstream-numbers.sh:3-5 (usage and exit codes), :23-41 `exit 2`, :157 `exit 1`, :160 `exit 0`; reads only `deeplethe/utopia` (:14-17) (static; file byte-identical to the prior baseline; not executed because it calls the upstream forge) |

## Doc: docs/decisions/README.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "File names are `NNNN-short-english-title.md`, four digits, increasing in creation order, no gaps" | structure | stale | docs/decisions/ has no 0055, 0056, 0057, 0058, 0059 or 0068 (0054 → 0060, 0067 → 0069); docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md:285-287 itself states these gaps exist and are not reused, contradicting "no gaps" |
| "The directory is flat" | structure | verified | docs/decisions/ contains only files: 70 records + README.md |
| Index: every record listed with the first word of its own status line | status | verified | 70 index rows (README.md:35-164, 0076 at :91 as In progress) vs 70 record files (`ls docs/decisions` → 71 entries with README.md); 0076's rewritten status line still starts `In progress` (0076:3); the other 69 records are byte-identical to the prior baseline, where each first status word matched its index row |
| "`/docs/*` is git-ignored except `/docs/decisions/`, `/docs/design/` and `/docs/pipeline.md`" | config | verified | .gitignore: `/docs/*`, `!/docs/decisions/`, `!/docs/design/`, `!/docs/pipeline.md` |
| "that page explains how things flow ... with five mermaid diagrams" | structure | verified | `grep -c '```mermaid' docs/pipeline.md` → 5 |
| "A status line is one line. It starts with one of five words" | structure | verified | unchanged: First words across the 70 records: Proposed, Accepted, In progress, Implemented, Superseded (0072 carries the word without the `Status` label); 0076:3 starts `In progress` |

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
| "`personal_tokens` (prefix `utp_pat_` ...) ... writes an audit row `mcp.tool_called`" | feature | verified | migrations/0017_personal_tokens.sql; `utp_pat_` in crates/utopia-core/src/models.rs:1797; crates/utopia-server/src/api/mcp.rs:226,278 |
| "`POST /auth/oidc/exchange` ... `UTOPIA_OIDC_EXCHANGE_AUDIENCES` ... spends it once (`oidc_exchanges`)" | feature | verified | api/mod.rs:127; api/oidc_exchange_tests.rs:88; migrations/0103:7 |
| "`GET /kbs/{id}/sources/{sid}/documents/by-external-id?external_id=…`" | feature | verified | crates/utopia-server/src/api/mod.rs:486 |
| "sealed at rest (`secrets::seal`), listed in `SOURCE_SECRET_KEYS`" | feature | verified | crates/utopia-core/src/secrets.rs; crates/utopia-core/src/models.rs:264 |
| "`GET /kbs/{id}/export?format=turtle\|jsonld` streams the base as RDF" | feature | verified | crates/utopia-server/src/api/export_routes.rs:30 |
| "Proposed and not built: OIDC SSO, backup and restore [0016 E, #712]" | status | stale | crates/utopia-server/src/api/oidc_routes.rs, migrations/0056 (SSO subject link), docs/decisions/0069 Implemented; crates/utopia-cli backup/restore with tests |

## Doc: docs/design/chat-and-mcp.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "The loop is rig's multi-turn runner (`rig-core` / `rig-agent` ...); every policy is one `AgentHook` in `api/agent.rs`" | structure | verified | `rig-core` in crates/utopia-server/Cargo.toml; crates/utopia-server/src/api/agent.rs:20 imports `AgentHook` |
| "`STALL_NUDGE` and `DONE` are gone" | structure | verified | No `STALL_NUDGE` in crates/ |
| "Proposed and not built: A per-task model (`on_model_select`)" | status | verified | No `on_model_select` in crates/; docs/decisions/0067 open: cuts 1–4 |
| "Proposed and not built: `query_data` over MCP" | status | verified | crates/utopia-server/src/api/mcp.rs:37,198,236,256 comments: `query_data` is deliberately not served |

## Doc: docs/design/extraction.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "One call per chunk, at temperature 0 ... the document's opening (its first 1,500 characters, #680)" | feature | verified | crates/utopia-server/src/extraction_open.rs:133 `Some(0.0)`; crates/utopia-extract/src/lib.rs:31 `OPENING_BUDGET_CHARS: usize = 1500` |
| "the typed path is deleted in #736" | status | verified | unchanged: migrations/0063_extraction_reads_only_the_open_graph.sql; 34 of 36 backticked code identifiers in the file resolve in the code (count from the prior audit, not recounted; no file was deleted or renamed since) (the two misses are provider JSON fields) |

## Doc: docs/design/governance.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Review holds nods (`pending_facts`), duplicates (`resolution_reviews`), conflicts (`fact_conflicts`) ... low-confidence (`confidence < 0.75`), violations (`axiom_violations`)" | feature | verified | crates/utopia-store/src/review.rs:20 `LOW_CONFIDENCE_BELOW: f32 = 0.75`; migrations 0005, 0012, 0018; crates/utopia-store/src/pending.rs |
| "[0043] (every queue, in PR #699)" | status | stale | docs/decisions/0043 status line: "no code on `dev` (#699 closed) · open: re-land on the open graph" |
| "The errata agent on the typed graph ... is built (migration 0074)" | status | verified | migrations/0074_an_errata_agent_reviews_the_typed_graph.sql; crates/utopia-store/src/errata.rs |

## Doc: docs/design/identity.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Every name ... is a value fact on the system attribute `known_as` ... `entities.aliases` is gone" | feature | verified | migrations/0055_a_name_is_a_fact.sql:119 `ALTER TABLE entities DROP COLUMN aliases`; `known_as` in crates/utopia-core/src/models.rs:855 |
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
| "Data sources are registered at the deployment and granted to workspaces (`data_source_grants`, #142)" | feature | verified | unchanged: migrations/0014_data_source_grants.sql:22 |
| "`concept_mappings` ... is configuration outside the ledger" | structure | verified | unchanged: migrations/0006_semantic_layer.sql:50 |
| "Actions are recorded and not built [0034]" | status | verified | No action tables or symbols in migrations/ or crates/ |
| "How much schema to fetch from a lakehouse catalog (cap 200 tables today)" | config | unverifiable | crates/utopia-server/src/query_engine/mod.rs:38 has `ROW_CAP: usize = 200`, which is a row cap; a 200-table schema cap was not located by grep — needs a targeted read of the schema-sync code |

## Doc: docs/design/ledger.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "`facts (kb, subject_id, predicate_id NULL, ... end_derived, end_after_period, layer, phrase)`" | structure | verified | unchanged: migrations/0114 (`end_after_period`), 0061 (open layer and phrase), 0057 (end the engine drew); 24 backticked code identifiers in the file resolve in the code (count from the prior audit, not recounted; no file was deleted or renamed since) |
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
| "`documents` and `document_versions` keep every version; `ingest_item()` owns identity (`external_key`)" | feature | verified | unchanged: migrations/0002_ingest.sql; 20 backticked code identifiers in the file resolve in the code (count from the prior audit, not recounted; no file was deleted or renamed since) |
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
| "Eleven reason codes, all recorded in `extraction_drops`" (`not_an_entity_name`, `subject_not_declared`, `attr_domain_mismatch`, `attr_no_value`, `direction_corrected`, `domain_mismatch` among them) | feature | stale | Of those six codes only `attr_domain_mismatch` survives, in two comments (migrations/0003_graph.sql:419, web/src/api.ts:227); none is emitted by crates/ (grep repeated at 6c52ce8). Only `truncated_reply`, `malformed_item`, `attr_datatype`, `object_missing` (crates/utopia-store/src/extraction_drops.rs) and `low_confidence` remain |
| "The ... signature check ... lives in the store (`ontology::judge_direction`). Extraction calls it when writing new facts" | structure | stale | `judge_direction` appears only in a comment of migrations/0019; no such function in crates/. docs/design/README.md says the write-time direction correction left with the typed path |
| "node scripts/bench/run.mjs --corpus pharma --label seeds-only" / "--corpus holmes" | command | verified | scripts/bench/run.mjs; scripts/bench/corpora/pharma.json, holmes.json, tech.json (static; needs a running server and model, not executed) |

## Doc: scripts/bench/README.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "node scripts/bench/run.mjs --corpus pharma --ontology /tmp/schemaorg.ttl --label schemaorg" | command | verified | scripts/bench/run.mjs handles `--ontology` (3 matches) (static; not executed) |
| "node scripts/bench/recall.mjs --kb <id>" with `--score`, `--reprocess`, `--known`, `--table`; "fetch-sec-filings.mjs"; "judge_open.mjs --kb <id> --sample 200" | command | verified | scripts/bench/recall.mjs (8 flag matches), fetch-sec-filings.mjs, judge_open.mjs exist; `UTOPIA_EXTRACT_KNOWN_IN_PROMPT` in crates/utopia-core/src/config.rs:35 (static) |
| "node scripts/bench/govern.mjs --fresh" / "mappings.mjs --fresh --no-comments" / "ask.mjs --kb <id> --replay ..." | command | verified | govern.mjs, mappings.mjs, ask.mjs exist with the named flags (static) |
| "node --test scripts/bench/lib.test.mjs" | command | verified | Executed in sidecar at 6c52ce8 → tests 15, pass 15, fail 0 |
| "node scripts/bench/competency.mjs --kb <kb-id> --seed scripts/bench/truth/redocred-typed.questions.json" | command | verified | competency.mjs and truth/redocred-typed.questions.json exist (static) |
| "node scripts/bench/agent.mjs ... --reset --arms ..." / "timewords.mjs --db ... --runs 3" / "temporal.mjs --label rc5" / "typed.mjs --label run1 --judge 200" / "fetch-redocred.mjs --n 100 --seed 1" | command | verified | All five scripts exist; `--arms`/`--reset` in agent.mjs, `--judge` in typed.mjs (static) |
| Directory listing: `corpora/*.json`, `truth/*.json`, `truth/*.wikidata.json`, `p31-to-classes.json`, `make-truth.mjs`, `subset-corpus.mjs`, `subset.mjs` | structure | verified | `ls scripts/bench`, `scripts/bench/truth`, `scripts/bench/corpora` show each named file |

## Doc: scripts/bench/corpora/contracts/README.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "`py -3 fetch.py` / `py -3 lease_bench.py setup --label <名字>` / `py -3 lease_bench.py score <kb id>`" | command | verified | scripts/bench/corpora/contracts/fetch.py, lease_bench.py (header documents `setup` and `score`, `QUESTIONS`) (static; needs SEC network access and a running server) |
| "`edgar-manifest.json` ... `edgar/`、`lease-kb-*.json` ... 不进仓库（见 `.gitignore`）" | structure | verified | edgar-manifest.json present; .gitignore lists `/scripts/bench/corpora/contracts/edgar/` and `lease-kb-*.json` |

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
| "POST {your-utopia-base}/api/v1/sources/{source_id}/statements" | feature | verified | crates/utopia-server/src/api/mod.rs:509 (line moved from :507) |
| "GET .../api/v1/kbs/{kb_id}/sources/{source_id}/documents/by-external-id?external_id=note-42" | feature | verified | crates/utopia-server/src/api/mod.rs:486 (line moved from :484) |
| "older versions are at `GET /api/v1/documents/{id}/content?version=N`" | feature | verified | crates/utopia-server/src/api/mod.rs:427 (line moved from :425) |
| "POST {your-utopia-base}/api/v1/kbs/{kb_id}/search" | feature | verified | crates/utopia-server/src/api/mod.rs:408 (line moved from :406) |

## Doc: web/src/docs/mcp.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "POST /api/v1/kbs/{kb_id}/mcp ... JSON-RPC 2.0 (MCP protocol version `2025-06-18`, stateless HTTP)" | feature | verified | crates/utopia-server/src/api/mod.rs:240; crates/utopia-server/src/api/mcp.rs:33 `PROTOCOL_VERSION: &str = "2025-06-18"`, :183-205 serve `initialize`, `tools/list`, `tools/call` |
| Tools table: `search_chunks`, `get_document`, `find_entities`, `entity_facts`, `neighbors`, `timeline`, `paths_between`, `changes`, `list_rules`, `rule_matches`, `search_docs`, `remember` | feature | verified | crates/utopia-server/src/api/mcp.rs:43-55 `EXPOSED` holds the eleven read tools, :63 `WRITABLE = ["remember"]`, :67-69 `is_exposed` |
| "`search_chunks` ... each cut at 800 characters" / "`get_document` ... up to 24,000 body characters per call" | config | verified | crates/utopia-server/src/api/tools.rs:19 `TOOL_CHUNK_CHARS = 800`, :21 `DOCUMENT_CHARS = 24_000` |
| "`query_data` ... is a chat tool and is not served over MCP" | feature | verified | crates/utopia-server/src/api/mcp.rs:37, 198, 236 (not in `EXPOSED`; `tools_schema` is called with an empty source list at :199) |
| "Authorization: Bearer utp_pat_…" | config | verified | `utp_pat_` prefix in crates/utopia-core/src/models.rs:1797; migrations/0017_personal_tokens.sql:12; mcp.rs:89-94 |
| "Expires in \| 90 days by default" | config | verified | crates/utopia-server/src/api/token_routes.rs:27-37 (default 90 days; 0 means no expiry) |
| "Search returns at most six chunks and changes at most 40 events" / "the default 80-fact limit (maximum 300)" | config | verified | crates/utopia-server/src/api/tools.rs:18 `SEARCH_TOP_K = 6`, :26 `CHANGES_LIMIT = 40`; tools_graph.rs:26 `FACTS_DEFAULT = 80`, :27 `LIST_MAX = 300` |
| "`GET /api/v1/kbs/{kb_id}/export?format=turtle\|jsonld` streams the whole base" / "`?base=https://example.org/`" | feature | verified | crates/utopia-server/src/api/mod.rs:185; export_routes.rs:30 (`turtle` or `jsonld`), :35 `pub base: Option<String>` |
| Tools table: `review_list` ("any of the thirteen"), `review_item`, `review_keep` ("**Needs a `write` token held by an editor**; a token without it does not see this tool in `tools/list`") | feature | verified | crates/utopia-server/src/api/review_tools.rs:28-35, :181-202 (`review_keep` filtered out unless `can_write`); mcp.rs:107-110 (`can_write` = write scope and owner at least Editor), :197-203; test review_mcp_tests.rs:854 (static; not executed) |
| "`review_list` reads all thirteen sections of the Review tab. Eleven are queue sections that share one list route; Overview and Decisions each have their own route" (sections table: keys `overview`, `pending`, `duplicates`, `conflicts`, `unconfirmed`, `lowconf`, `violations`, `defects`, `alignment`, `errata`, `agent`, `decisions`, `merges`) | feature | verified | crates/utopia-server/src/api/review_ops.rs:80-92 `TOKEN_SECTIONS` (the eleven queue keys); review_tools.rs:305-334 (`overview` → summary, `decisions` → history, others → list); review_routes.rs:139-205 (one `match` arm per queue key); routes at api/mod.rs:534 (list), :535 (history), :536 (summary); test review_mcp_reads_tests.rs:192 (not executed) |
| "A paging argument the section does not take is ignored ... The route's `types` filter for Duplicates is not a tool argument ... Any other section name, `mappings` included, is refused with `unknown_queue` and returns no items" | feature | verified | crates/utopia-server/src/api/review_tools.rs:305-334 (each branch reads only its own paging arguments; `review_ops::list` is called with `types` = `None` at :332); review_ops.rs:95-97, :112-114; test review_mcp_tests.rs:421, review_mcp_reads_tests.rs:963 (not executed) |
| "A queue section's result carries `total` ... `counts.alignment` leaves out proposed implication rules, `counts.lowconf` leaves out derived facts that the list includes, and `counts.agent` counts the governor's open proposals only ... An empty section is `"items": []` with `"total": 0`, not an error" | feature | verified | crates/utopia-server/src/api/review_routes.rs:136-221; crates/utopia-store/src/review.rs:80 (`derived_by_rule IS NULL` in the `lowconf` badge), :88-89 (`agent` badge: `status = 'proposed'`), :101-102 (`alignment` badge: phrase and type bindings only); alignment_queue.rs:235; graph.rs:2105-2107; token_agent_actions.rs:240-251; tests review_totals_tests.rs:323, :395, :575 (not executed) |
| "`limit` defaults to 10 and is served as 1 to 200 ... `offset` defaults to 0, and a negative value is served as 0 ... The Alignment list is ordered by each item's `decided_at` and then by the item's own identifier" | config | verified | crates/utopia-server/src/api/review_routes.rs:29 `REVIEW_PAGE: i64 = 10`, :134-135 `clamp(1, 200)`, `.max(0)`; crates/utopia-store/src/alignment_queue.rs:162-223 (sort key `decided_at`, then id or kind word); tests review_mcp_tests.rs:330, review_totals_tests.rs:513 (not executed) |
| "`section: "decisions"` pages with the history route's own `page` and `per` ... `page` is zero-based and defaults to 0 (a negative value is served as 0); `per` defaults to 20 and is served as 1 to 100 ... newest first, each with `id`, `action`, `target_kind`, `target_id`, `detail`, `actor_id`, `actor_name` and `created_at`" | feature | verified | crates/utopia-server/src/api/review_ops.rs:121-133 (`per.unwrap_or(20).clamp(1, 100)`, `page.unwrap_or(0).max(0)`); review_tools.rs:312-322; crates/utopia-store/src/audit.rs:129-141 (selected columns; `ORDER BY e.created_at DESC, e.id DESC`); test review_mcp_reads_tests.rs:261 (not executed) |
| "`section: "agent"` returns rows from two sources in one sequence, newest first ... A row for an action made with a token has `"source": "token"` and names both the person and the token: `owner_id` and `owner_name` ..., `token_id` and `token_name` ..., beside `id`, `operation`, `target_kind`, `target_id`, `outcome`, `left`, `right`, `rationale` and `created_at`" | feature | verified | crates/utopia-store/src/token_agent_actions.rs:219-236 `history_row` (exactly these keys), :240-242 (`UNION ALL` of `agent_decisions` and `token_agent_actions`), :260-275 (`ORDER BY created_at DESC, id DESC LIMIT/OFFSET` over the union; governor rows taken from `governance::list`), :27-39 (`token_name` is the stored snapshot); tests review_token_reads_tests.rs:883, :927 (not executed) |
| "`review_item` takes the `section` an item is listed in and the item's `id` ... left out, it is `duplicates`" with its bullets (kind word passed as `id` and compared exactly as stored; the Duplicates pair fields; `proof` on an Axioms item; one `not_found` answer; `overview` lists no items) | feature | verified | crates/utopia-server/src/api/review_tools.rs:336-346; review_ops.rs:167-174, :192-277 (item found by paging the section's own list; `overview` → `NotFound` :200-202; `proof` only for `violations` :265-275); crates/utopia-core/src/models.rs:1041-1075 (`ReviewSide`: type_label, color, disambiguator, degree, top_facts; `ReviewItem`: score, reason, stage, left, right, proposal); crates/utopia-store/src/alignment_queue.rs:18-39 (`KindWord` variant has `kind_word` and no `id`); tests review_mcp_reads_tests.rs:431, :517, :615, :842 (not executed) |
| "A token that is unknown, revoked or expired gets HTTP 401 from the endpoint. A token whose knowledge-base list excludes the base is turned away by the endpoint itself with HTTP 403 before any tool runs ... The token API answers that same token with 404" / "the scope is checked before the owner's role" | feature | verified | crates/utopia-server/src/api/mcp.rs:89-98 (`Unauthorized` without a bearer token; `if !auth.covers(kb_id) { return Err(AppError::Forbidden) }` inside `authorize`, called first in `handle` :177); crates/utopia-store/src/tokens.rs:157-170; error.rs:33-35; review_ops.rs:43-55 (404 on the token API), :60-75 (scope, then role); tests review_mcp_tests.rs:551, review_token_reads_tests.rs:759 (not executed) |
| MCP errors table: `unknown_queue`, `not_found`, `token_read_only`, `token_owner_below_editor`, `already_decided`; "`structuredContent` is `{"code": "<code>", "message": "<text>"}`" / "A call with a missing or malformed argument ... returns `isError: true` with a text message and no `structuredContent`" | feature | verified | crates/utopia-server/src/api/review_tools.rs:254-281 (code and message taken from the REST error body; 404 becomes `not_found` at :269; an error without a code has no `code` key), :206-217 (`bad_argument` builds no structured content), :349-354 (non-uuid `id` for `review_keep` is not run); review_ops.rs:60-75, :236-238; auth.rs:253; crates/utopia-store/src/token_agent_actions.rs:182-187 |
| "Each call to any of the three tools also writes the `mcp.tool_called` row" | feature | verified | crates/utopia-server/src/api/mcp.rs:210-231 (written after the call returns, for refused calls too) |
| Annotations: "`readOnlyHint` on the two reads; `readOnlyHint`, `destructiveHint` and `idempotentHint`, all `false`, on `review_keep`" | feature | verified | crates/utopia-server/src/api/review_tools.rs:170-176, :198 |
| Token API routes table ("six of the Review tab's own HTTP routes: five reads and the keep"): `GET /kbs/{kb_id}/review?queue=<key>`, `GET /kbs/{kb_id}/review/summary`, `GET /kbs/{kb_id}/review/history?page=&per=`, `GET /kbs/{kb_id}/violations/{violation_id}/proof`, `GET /kbs/{kb_id}/derived/{derived_id}/proof`, `POST /kbs/{kb_id}/review/{review_id}` with `{"action": "keep"}` answering `{"ok": true, "outcome": "applied", "action_id": "<uuid>"}` | feature | verified | crates/utopia-server/src/api/review_routes.rs:85-87, :351-353, :801-803, :825-827 (`SessionOrToken`); graph_routes.rs:540-542, :562-564; routes at api/mod.rs:459-460, 464, 534, 535, 536, 551; review_ops.rs:138-159 (`{steps}`, `{proof}`), :315 (keep response shape); `types` parsed by `TypeFilter::parse` (crates/utopia-store/src/resolution.rs:1857-1863: `any`, `same`, `conflict`); queue left out → `duplicates` (review_routes.rs:94); test review_token_reads_tests.rs:198, :994 (not executed) |
| "`total` is on every list response, for a signed-in session as well; it is the only key the list response gained. The token API has no route for a single item" | feature | verified | crates/utopia-server/src/api/review_routes.rs:216-221 (one body for both callers through `queue_page`); api/mod.rs:534-617 has no single-item GET under `/kbs/{id}/review`; test review_totals_tests.rs:217 `a_session_list_response_differs_from_before_only_by_the_added_total_key` (not executed) |
| "recorded three ways in one transaction ... If any of the three cannot be written, the keep fails and the pair stays in the queue" / "A token's keep is **not** treated as a person's decision" | feature | verified | crates/utopia-store/src/token_agent_actions.rs:87-178: one `tx` updates `resolution_reviews` (:123-124), inserts `token_agent_actions` (:145-147), writes the audit row (:164-176), then commits (:178); governance.rs:85, :171; crates/utopia-server/src/api/review_ops.rs:293-316 (no `after_human_decision` call); test review_mcp_tests.rs:1040 (not executed) |
| Token API refusals table: 401 none; 404 none on the five reads and for an unknown pair on the keep; 422 `unknown_queue`, `unknown_type_filter`; 403 `token_read_only`, `token_kb_out_of_scope`, `token_owner_below_editor`, `token_operation_not_open`; 409 `already_decided`; 409 none while the governor decides | feature | verified | crates/utopia-server/src/error.rs:24-45 (status mapping); review_ops.rs:32-38, :43-55, :60-75, :95-97; crates/utopia-store/src/resolution.rs:1863 `unknown_type_filter`; auth.rs:253, 263; token_agent_actions.rs:81 (`Conflict`), :104 (`NotFound`), :182-187; test review_token_reads_tests.rs:405, :694 (not executed) |
| "`merge` on the decision route answers `token_operation_not_open`, and the batch ..., revert ... and manual-merge ... routes take a session only and answer a token with 401 until a later change opens them ... the nod route answers a token with 401. One Review write was open to `write` tokens before the Review tools existed and is unchanged: deciding an Alignment kind word" | feature | verified | crates/utopia-server/src/api/review_routes.rs:358-365 (`not_open` for a non-keep action), :567-569, :689-691, :737-739, :1177-1179 (`AuthUser`), :1468-1472 (`decide_alignment_kind_word`: `SessionOrToken`, `require_write`); routes at api/mod.rs:537, 549, 559, 609, 617; error.rs:34; tests review_token_api_tests.rs:352, :366-375 and review_token_reads_tests.rs:1049 (not executed). docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md:186-192 now says the same |

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
