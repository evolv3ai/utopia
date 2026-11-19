# Gap Review — 1: project Verify

Reviewed HEAD: c7db84941a3f8bea4f41365c78fd9b9e83434608
Gap verdict: blocked
Risk: project Verify
Waves checked: 1, 2

## Checked evidence

- **Check**: `docker exec utopia-db-1 psql -U utopia -d utopia -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS m001_project WITH (FORCE)" -c "CREATE DATABASE m001_project" && export UTOPIA_DATABASE_URL=postgres://utopia:utopia@127.0.0.1:1517/m001_project && cargo run -p utopia-store --example migrate && UTOPIA_TEST_REQUIRE_DB=1 cargo test --workspace && (cd web && pnpm install --frozen-lockfile && pnpm test && pnpm build)`
- **Observed**: Exit 101; exact stdout and stderr are in the command/commit ledger entry.
- **Reference**: .project/build/verify-ledger.jsonl — c7db84941a3f8bea4f41365c78fd9b9e83434608, command above

## Finding

- **Found**: Project Verify failed at the reviewed commit.
- **Fix direction**: Resolve the recorded command failure before shipping.
