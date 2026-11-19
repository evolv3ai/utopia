# Gap Review — 1: project Verify

Reviewed HEAD: f2a4edd2faa4035aadc9f18851883500b0e7a00c
Gap verdict: pass
Risk: project Verify
Waves checked: 1, 2

## Checked evidence

- **Check**: `docker exec utopia-db-1 psql -U utopia -d utopia -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS m002_project WITH (FORCE)" -c "CREATE DATABASE m002_project" -c "DROP DATABASE IF EXISTS m002_project_store WITH (FORCE)" -c "CREATE DATABASE m002_project_store" && export UTOPIA_DATABASE_URL=postgres://utopia:utopia@127.0.0.1:1517/m002_project && cargo run -p utopia-store --example migrate && UTOPIA_TEST_REQUIRE_DB=1 cargo test --workspace --exclude utopia-store && export UTOPIA_DATABASE_URL=postgres://utopia:utopia@127.0.0.1:1517/m002_project_store && cargo run -p utopia-store --example migrate && UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-store && (cd web && pnpm install --frozen-lockfile && pnpm test && pnpm build)`
- **Observed**: Exit 0; exact stdout and stderr are in the command/commit ledger entry.
- **Reference**: .project/build/verify-ledger.jsonl — f2a4edd2faa4035aadc9f18851883500b0e7a00c, command above

## Finding

- **Found**: Project Verify passed at the reviewed commit.
- **Fix direction**: none
