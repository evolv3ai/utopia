#!/usr/bin/env bash
# 证明连库验证在这台机器上站得住：compose 的 Postgres 16 + pgvector 上，每次验证用自己的库。
# 用法: bash scripts/prove-db-isolation.sh <prefix>   （在仓库根目录跑，例如 m001_t001）
#
# 它建两个库 <prefix>_a、<prefix>_b（先删后建，重跑从干净处开始），然后：
#   构建 A = 检出的这棵树：迁移 <prefix>_a，UTOPIA_TEST_REQUIRE_DB=1 下跑通
#            token_read_tests（8 个）和 token_setup_tests（9 个），再迁移一遍，
#            并用 a_second_migration_run_changes_no_schema 确认第二遍什么结构都没改。
#   构建 B = 同一棵树多一条探针迁移：迁移 <prefix>_b，跑通 token_read_tests，
#            并确认探针版本只在 <prefix>_b 的 _sqlx_migrations 里，<prefix>_a 里没有。
# 全部成立才打印 `db-isolation: ok` 并以 0 退出；任何一步不成立都以非 0 退出。
#
# 可用环境变量改默认值：
#   UTOPIA_PROVE_DB_CONTAINER（utopia-db-1） UTOPIA_PROVE_DB_HOST（127.0.0.1）
#   UTOPIA_PROVE_DB_PORT（1517） UTOPIA_PROVE_DB_USER（utopia）
#   UTOPIA_PROVE_DB_PASSWORD（utopia） UTOPIA_PROVE_DB_ADMIN_DB（utopia）
set -euo pipefail

PREFIX="${1:-}"
# 前缀会原样拼进 SQL 标识符，只收小写字母、数字、下划线
if ! [[ "$PREFIX" =~ ^[a-z][a-z0-9_]{0,40}$ ]]; then
  echo "usage: bash scripts/prove-db-isolation.sh <prefix>   (lowercase letters, digits, underscores)" >&2
  exit 2
fi

CONTAINER="${UTOPIA_PROVE_DB_CONTAINER:-utopia-db-1}"
HOST="${UTOPIA_PROVE_DB_HOST:-127.0.0.1}"
PORT="${UTOPIA_PROVE_DB_PORT:-1517}"
DB_USER="${UTOPIA_PROVE_DB_USER:-utopia}"
DB_PASSWORD="${UTOPIA_PROVE_DB_PASSWORD:-utopia}"
ADMIN_DB="${UTOPIA_PROVE_DB_ADMIN_DB:-utopia}"

DB_A="${PREFIX}_a"
DB_B="${PREFIX}_b"
# 探针迁移：版本号高过任何真实迁移，内容是一条无害、可重放的语句
PROBE_VERSION=9999
PROBE_FILE="migrations/${PROBE_VERSION}_a_probe_migration_exists_only_in_build_b.sql"
# 迁移是 `sqlx::migrate!` 在编译期读进去的，而这个 crate 没有 build.rs：目录里多一个或
# 少一个文件 cargo 并不知道。动一下调用宏的那个源文件的修改时间，才会真的重编
STORE_SRC="crates/utopia-store/src/db.rs"
READ_TESTS_MIN=8
SETUP_TESTS_MIN=9

step() { echo "--- $1"; }
fail() { echo "FAIL: $1" >&2; exit 1; }

[ -f Cargo.toml ] && [ -d migrations ] && [ -f "$STORE_SRC" ] \
  || fail "run this from the repository root"

url_of() { echo "postgres://${DB_USER}:${DB_PASSWORD}@${HOST}:${PORT}/$1"; }

# 不用 `docker compose exec`：验证跑在旁路工作树里，目录名不是 compose 的项目名。
# MSYS_NO_PATHCONV：Git Bash 会把像路径的参数改写成 Windows 路径
psql_in() {
  local db="$1"
  shift
  MSYS_NO_PATHCONV=1 docker exec "$CONTAINER" psql -U "$DB_USER" -d "$db" -v ON_ERROR_STOP=1 "$@"
}

# 一条查询的单个值；Windows 上 docker 的输出可能带 \r
scalar() {
  psql_in "$1" -At -c "$2" | tr -d '\r'
}

probe_rows() {
  scalar "$1" "SELECT count(*) FROM _sqlx_migrations WHERE version = ${PROBE_VERSION}"
}

migrate_db() {
  UTOPIA_DATABASE_URL="$(url_of "$1")" cargo run -p utopia-store --example migrate
}

# 跑一条 cargo test，输出既给人看也留一份给后面数数（去掉 Windows 上可能带的 \r）
LOG_DIR="$(mktemp -d)"
run_tests() {
  local db="$1" log="$2"
  shift 2
  UTOPIA_DATABASE_URL="$(url_of "$db")" UTOPIA_TEST_REQUIRE_DB=1 cargo test "$@" 2>&1 | tr -d '\r' | tee "$log"
}

# 过滤条件一个都没匹配上时 cargo 照样以 0 退出，所以要数：模块里报 ok 的测试够不够数，
# 有没有哪个没报 ok
passed_in() {
  grep -cE "^test (.*::)?$2::[A-Za-z0-9_:]+ \.\.\. ok\$" "$1" || true
}
not_ok_in() {
  grep -E "^test (.*::)?$2::[A-Za-z0-9_:]+ \.\.\. " "$1" | grep -cvE ' \.\.\. ok$' || true
}
require_module() {
  local log="$1" module="$2" min="$3" passed bad
  passed="$(passed_in "$log" "$module")"
  bad="$(not_ok_in "$log" "$module")"
  [ "$bad" -eq 0 ] || fail "$module: $bad test(s) did not report ok"
  [ "$passed" -ge "$min" ] || fail "$module: $passed test(s) reported ok, expected at least $min"
  echo "$module: $passed ok"
}

# 上一次被强杀留下的探针先清掉，否则「构建 A」其实带着探针
if [ -e "$PROBE_FILE" ]; then
  rm -f "$PROBE_FILE"
  touch "$STORE_SRC"
fi

GIT_BEFORE="$(git status --porcelain)"
PROBE_USED=0

cleanup() {
  local status=$?
  trap - EXIT
  if [ "$PROBE_USED" -eq 1 ] || [ -e "$PROBE_FILE" ]; then
    rm -f "$PROBE_FILE"
    # target/ 里的 utopia-store 还带着探针：让下一次构建重编，别把它带进别的库
    touch "$STORE_SRC" || true
  fi
  rm -rf "$LOG_DIR"
  # 成功也好失败也好，工作树要和开始时一样
  if [ "$(git status --porcelain)" != "$GIT_BEFORE" ]; then
    echo "FAIL: the script left the worktree changed:" >&2
    git status --porcelain >&2
    [ "$status" -ne 0 ] || status=1
  fi
  exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM HUP

step "the instance is Postgres 16 with pgvector available"
server_version_num="$(scalar "$ADMIN_DB" "SHOW server_version_num")"
[ "${server_version_num:0:2}" = "16" ] || fail "expected Postgres 16, got server_version_num=$server_version_num"
[ "$(scalar "$ADMIN_DB" "SELECT count(*) FROM pg_available_extensions WHERE name = 'vector'")" = "1" ] \
  || fail "the pgvector extension is not available on $CONTAINER"

step "drop and create $DB_A and $DB_B"
for db in "$DB_A" "$DB_B"; do
  psql_in "$ADMIN_DB" -c "DROP DATABASE IF EXISTS ${db} WITH (FORCE)" -c "CREATE DATABASE ${db}"
done

step "build A: migrate $DB_A"
migrate_db "$DB_A"
[ "$(scalar "$DB_A" "SELECT count(*) FROM _sqlx_migrations WHERE success")" -gt 0 ] \
  || fail "$DB_A holds no applied migration"
[ "$(probe_rows "$DB_A")" = "0" ] || fail "build A carries the probe migration"

step "build A: token read and token setup tests against $DB_A"
run_tests "$DB_A" "$LOG_DIR/a-token.log" -p utopia-server -- token_read_tests token_setup_tests
require_module "$LOG_DIR/a-token.log" token_read_tests "$READ_TESTS_MIN"
require_module "$LOG_DIR/a-token.log" token_setup_tests "$SETUP_TESTS_MIN"

step "build A: a second migration run against $DB_A"
migrate_db "$DB_A"
run_tests "$DB_A" "$LOG_DIR/a-second.log" -p utopia-store \
  --test a_second_migration_run_changes_no_schema
grep -qE '^test a_second_migration_run_changes_no_schema \.\.\. ok$' "$LOG_DIR/a-second.log" \
  || fail "a_second_migration_run_changes_no_schema did not report ok"

step "build B: the same tree plus one probe migration"
PROBE_USED=1
printf -- '-- 探针：只存在于构建 B，证明两套迁移各用各的库。无害，可重放。\nSELECT 1;\n' > "$PROBE_FILE"
touch "$STORE_SRC"

step "build B: migrate $DB_B"
migrate_db "$DB_B"

step "build B: token read tests against $DB_B"
run_tests "$DB_B" "$LOG_DIR/b-token.log" -p utopia-server -- token_read_tests
require_module "$LOG_DIR/b-token.log" token_read_tests "$READ_TESTS_MIN"

step "the probe migration is in $DB_B and not in $DB_A"
[ "$(scalar "$DB_B" "SELECT count(*) FROM _sqlx_migrations WHERE version = ${PROBE_VERSION} AND success")" = "1" ] \
  || fail "$DB_B does not hold migration $PROBE_VERSION: build B was not rebuilt with it"
[ "$(probe_rows "$DB_A")" = "0" ] || fail "$DB_A holds migration $PROBE_VERSION: the databases are not isolated"
a_count="$(scalar "$DB_A" "SELECT count(*) FROM _sqlx_migrations")"
b_count="$(scalar "$DB_B" "SELECT count(*) FROM _sqlx_migrations")"
[ "$b_count" -eq $((a_count + 1)) ] \
  || fail "$DB_B holds $b_count migrations and $DB_A $a_count: expected exactly one more"
echo "$DB_A: $a_count migrations, $DB_B: $b_count migrations"

echo "db-isolation: ok"
