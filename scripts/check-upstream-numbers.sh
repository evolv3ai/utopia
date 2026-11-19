#!/usr/bin/env bash
# 落地前的编号核对（ADR 0076）：一个决策记录编号和一个迁移编号，上游有没有人占了。
# 用法: bash scripts/check-upstream-numbers.sh <adr-number> <migration-number>
#   例: bash scripts/check-upstream-numbers.sh 0076 0128
# 退出码: 0 = 两个编号都没人占；1 = 撞号（打印撞上的文件）；2 = 读不到上游（或参数不对）。
#
# 「占了」有两种：上游 dev 上已经有这个编号的文件；或者某个还开着的上游 PR 带着这个编号的文件。
# 只读上游：一次 PR 列表（GitHub 公开 API，gh 登录了就走 gh），一次 git fetch。
# 不 push、不开 PR、不评论，请求里也不带本机的任何信息。
#
# 读不到就当失败（退出 2），绝不当成「没撞」：没核对过的编号不能落地。
set -u

UPSTREAM_REPO="deeplethe/utopia"
UPSTREAM_BRANCH="dev"
UPSTREAM_URL="https://github.com/${UPSTREAM_REPO}.git"
API_URL="https://api.github.com/repos/${UPSTREAM_REPO}/pulls"
# PR 列表的页数上限：100 页 × 100 条。翻到头还没空，说明列表读得不对，按读不到算
MAX_PAGES=100

unreadable() {
    echo "check-upstream-numbers: cannot read upstream ${UPSTREAM_REPO}: $*" >&2
    exit 2
}

if [ "$#" -ne 2 ]; then
    echo "usage: bash scripts/check-upstream-numbers.sh <adr-number> <migration-number>" >&2
    exit 2
fi
adr="$1"
migration="$2"
case "$adr$migration" in
    [0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9]) ;;
    *)
        echo "check-upstream-numbers: numbers are four digits each (for example 0076 0128)" >&2
        exit 2
        ;;
esac
if [ "${#adr}" -ne 4 ] || [ "${#migration}" -ne 4 ]; then
    echo "check-upstream-numbers: numbers are four digits each (for example 0076 0128)" >&2
    exit 2
fi

command -v git >/dev/null 2>&1 || unreadable "git is not installed"

# 在一个临时的空仓库里读上游，不碰当前仓库的 refs、remote 和配置
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE
export GIT_TERMINAL_PROMPT=0
tmp="$(mktemp -d 2>/dev/null)" || unreadable "no temporary directory"
trap 'rm -rf "$tmp"' EXIT
git init -q --bare "$tmp/upstream.git" >/dev/null 2>&1 || unreadable "git init failed"
upstream_git() {
    git --git-dir="$tmp/upstream.git" -c core.quotepath=off "$@"
}
# 起一个有名字的 remote：按目录树过滤的 fetch 要把过滤条件记在 remote 名下
upstream_git remote add upstream "$UPSTREAM_URL" >/dev/null 2>&1 || unreadable "git remote add failed"

# 一次 fetch：只要提交和目录树，不要文件内容（--filter=blob:none），只要最新一层（--depth 1）
fetch_refs() {
    upstream_git fetch -q --no-tags --depth 1 --filter=blob:none upstream "$@" 2>"$tmp/fetch.err"
}

# 一个 ref 下两个目录里的文件，一行一个完整路径
files_at() {
    upstream_git ls-tree --name-only "$1" docs/decisions/ migrations/
}

# 还开着的 PR 的编号，一页。gh 登录了就用它（额度高）；否则走不带凭证的公开 API。
# 匿名那条路不解析整份 JSON：PR 自己的 html_url 是唯一以 /pull/<n> 结尾的 html_url
# （里程碑的是 /milestone/<n>，用户和仓库的没有 /pull/），所以按它取编号
use_gh=0
if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
    use_gh=1
fi
open_pr_page() {
    local page="$1"
    if [ "$use_gh" -eq 1 ]; then
        if gh api -X GET "repos/${UPSTREAM_REPO}/pulls?state=open&per_page=100&page=${page}" \
            --jq '.[].number' 2>/dev/null; then
            return 0
        fi
        # gh 这一次没答上来（凭证过期之类），退回匿名那条路
        use_gh=0
    fi
    command -v curl >/dev/null 2>&1 || return 1
    local body
    body="$(curl -fsSL --max-time 60 \
        -H 'Accept: application/vnd.github+json' \
        -H 'User-Agent: check-upstream-numbers' \
        "${API_URL}?state=open&per_page=100&page=${page}")" || return 1
    # 答回来的必须是一个 JSON 数组；限流和报错答的是对象
    case "$(printf '%s' "$body" | tr -d ' \t\r\n' | cut -c1)" in
        '[') ;;
        *) return 1 ;;
    esac
    printf '%s' "$body" \
        | grep -oE "\"html_url\": *\"https://github.com/${UPSTREAM_REPO}/pull/[0-9]+\"" \
        | grep -oE '[0-9]+"$' | tr -d '"' | sort -un
}

# 1. 上游 dev
fetch_refs "+refs/heads/${UPSTREAM_BRANCH}:refs/upstream/dev" \
    || unreadable "git fetch of ${UPSTREAM_BRANCH} failed: $(tr '\n' ' ' <"$tmp/fetch.err")"
files_at refs/upstream/dev >"$tmp/dev.files" || unreadable "listing ${UPSTREAM_BRANCH} failed"
# 两个目录都得列得出东西：空的列表不是「没撞」，是没读到
grep -q '^docs/decisions/[0-9][0-9][0-9][0-9]-.*\.md$' "$tmp/dev.files" \
    || unreadable "no decision record listed on ${UPSTREAM_BRANCH}"
grep -q '^migrations/[0-9][0-9][0-9][0-9]_.*\.sql$' "$tmp/dev.files" \
    || unreadable "no migration listed on ${UPSTREAM_BRANCH}"

# 撞号的行：「<文件>\t<在哪>」
: >"$tmp/collisions"
collect() {
    # $1 = 文件列表，$2 = 在哪
    local file
    while IFS= read -r file; do
        case "$file" in
            "docs/decisions/${adr}-"*.md | "migrations/${migration}_"*.sql)
                printf '%s\t%s\n' "$file" "$2" >>"$tmp/collisions"
                ;;
        esac
    done <"$1"
}
collect "$tmp/dev.files" "upstream ${UPSTREAM_BRANCH}"

# 2. 还开着的 PR：它的头上有、dev 上没有的文件，就是它要加的
page=1
pr_count=0
while :; do
    [ "$page" -le "$MAX_PAGES" ] || unreadable "the open pull request list did not end"
    numbers="$(open_pr_page "$page")" || unreadable "listing open pull requests failed (page ${page})"
    [ -n "$numbers" ] || break
    refspecs=()
    for n in $numbers; do
        case "$n" in
            '' | *[!0-9]*) unreadable "unexpected pull request number '${n}'" ;;
        esac
        refspecs+=("+refs/pull/${n}/head:refs/upstream/pr/${n}")
    done
    fetch_refs "${refspecs[@]}" \
        || unreadable "git fetch of open pull requests failed: $(tr '\n' ' ' <"$tmp/fetch.err")"
    for n in $numbers; do
        files_at "refs/upstream/pr/${n}" >"$tmp/pr.files" \
            || unreadable "listing pull request #${n} failed"
        # dev 上已经有的不算这个 PR 加的
        grep -Fxv -f "$tmp/dev.files" "$tmp/pr.files" >"$tmp/pr.added" || true
        collect "$tmp/pr.added" "open upstream pull request #${n}"
        pr_count=$((pr_count + 1))
    done
    page=$((page + 1))
done

if [ -s "$tmp/collisions" ]; then
    while IFS="$(printf '\t')" read -r file where; do
        echo "collision: ${file} (${where})"
    done <"$tmp/collisions"
    exit 1
fi
echo "free: decision record ${adr} and migration ${migration} (upstream ${UPSTREAM_BRANCH} and ${pr_count} open pull requests checked)"
exit 0
