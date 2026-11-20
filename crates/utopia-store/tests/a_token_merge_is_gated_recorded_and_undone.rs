//! 令牌 agent 的合并与撤回（0076），打在真库上：路由还没开之前，先在 store 里把这一刀
//! 站得住的五件事钉住。
//!
//! 1. **台账装得下**：迁移 0129 加的七列与「一个目标一条开着的提议」的部分唯一索引，
//!    重放是空操作
//! 2. **合并与它的两行一起落**：闸门放行的令牌合并，合并、审核行、台账行（`applied`，
//!    指着这次合并）、审计行在同一个事务里；读不到令牌的名字就什么都不落；合并提交之后
//!    的后续步骤失败，结果仍是 `applied`，错误挂在 `follow_up_error` 上
//! 3. **闸门的六种留下**：身份来源、矛盾、派生、回答，加上只对令牌问的两种——人分开过的
//!    一对、合过又撤回的一对（按实体 id、不分左右）。留下的调用只留一条提议，图一行不动；
//!    重复的、同时到的调用拿回同一条
//! 4. **合了再撤，图回到原样**（E12，INTENT C1）：`facts`、`entities`（不含 `updated_at`）、
//!    `fact_evidence` 逐行相同；`typed_fact_sources` 原有的行都在，多出来的只有合并抄到
//!    存活事实上的那一条陈述来源（现有的撤回不删它，这一刀不改撤回）。令牌撤不动人或治理 agent 的合并，
//!    只留提议
//! 5. **令牌的合并与撤回不是人的先例**（E25）：治理 agent 读到的每一样都不变；人撤回令牌
//!    的合并照旧算一次撤回
//!
//! 外加竞争：两个调用同时合同一对，正好合一次，输了的令牌听到 `already_decided`。
//!
//! 这个测试自成一个二进制（为什么见 `store/main.rs`）：重放迁移要台账表的排他锁，
//! 「后续步骤失败」那一条要在 `entities` 上临时挂一个只认本库的触发器——都是 kb 之外的
//! 状态。本文件里连库的测试因此排成一队（`SERIAL`）。
//!
//! 没有 `UTOPIA_DATABASE_URL` 时跳过而不是失败。自建自拆，绝不碰已有的库。

use serde_json::{json, Value};
use sqlx::PgPool;
use utopia_core::models::ReviewItem;
use utopia_core::AppError;
use utopia_store::execution_gate::{hold_for_merge, hold_for_token_merge, Hold};
use utopia_store::governance::{self, NewDecision};
use utopia_store::names::NameSource;
use utopia_store::resolution;
use utopia_store::token_agent_actions::{self, TokenAgentAction, TokenWrite, TokenWriteOutcome};
use uuid::Uuid;

const MIGRATION: &str = include_str!(
    "../../../migrations/0129_a_token_agent_action_keeps_its_reason_its_batch_its_merge_and_its_later_state.sql"
);

/// 本文件里连库的测试一个一个跑（每个 `#[tokio::test]` 有自己的运行时，tokio 的 Mutex
/// 不绑运行时）
static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const TOKEN_NAME: &str = "cleanup agent";

struct Base {
    kb: Uuid,
    org: Uuid,
    /// 既亲手裁决、又是令牌的主人：台账的 actor、审核行的 decided_by、合并日志的
    /// merged_by 两头都是同一个人，分得出来的只有令牌的标记
    user: Uuid,
    token: Uuid,
    person_type: Uuid,
    company_type: Uuid,
    /// 只许一个值的关系：两边各指一个不同的人就是矛盾
    ceo_of: Uuid,
    part_of: Uuid,
    based_in: Uuid,
}

async fn base(pool: &PgPool, governance: bool) -> anyhow::Result<Base> {
    let [org, ws, kb, user, person_type, company_type, ceo_of, part_of, based_in] =
        std::array::from_fn(|_| Uuid::now_v7());
    sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, 'token-merge-test')")
        .bind(org)
        .execute(pool)
        .await?;
    sqlx::query(
        "INSERT INTO users (id, org_id, email, display_name, password_hash)
         VALUES ($1, $2, $3, 'Token Owner', 'x')",
    )
    .bind(user)
    .bind(org)
    .bind(format!("token-merge-{}@test.local", user.simple()))
    .execute(pool)
    .await?;
    sqlx::query("INSERT INTO workspaces (id, org_id, name) VALUES ($1, $2, 'token-merge-test')")
        .bind(ws)
        .bind(org)
        .execute(pool)
        .await?;
    sqlx::query(
        "INSERT INTO knowledge_bases (id, workspace_id, name, governance)
         VALUES ($1, $2, 'token-merge-test', $3)",
    )
    .bind(kb)
    .bind(ws)
    .bind(governance)
    .execute(pool)
    .await?;
    for (id, key, label) in [
        (person_type, "person", "Person"),
        (company_type, "org", "Organization"),
    ] {
        sqlx::query("INSERT INTO entity_types (id, kb_id, key, label) VALUES ($1, $2, $3, $4)")
            .bind(id)
            .bind(kb)
            .bind(key)
            .bind(label)
            .execute(pool)
            .await?;
    }
    for (id, key, label, functional) in [
        (ceo_of, "ceo_of", "CEO of", true),
        (part_of, "part_of", "part of", false),
        (based_in, "based_in", "based in", false),
    ] {
        sqlx::query(
            "INSERT INTO relation_types (id, kb_id, key, label, functional)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(id)
        .bind(kb)
        .bind(key)
        .bind(label)
        .bind(functional)
        .execute(pool)
        .await?;
    }
    let (view, _plain) =
        utopia_store::tokens::issue(pool, user, TOKEN_NAME, "write", None, None).await?;
    Ok(Base {
        kb,
        org,
        user,
        token: view.id,
        person_type,
        company_type,
        ceo_of,
        part_of,
        based_in,
    })
}

async fn teardown(pool: &PgPool, b: &Base) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM jobs WHERE kind = 'govern' AND payload->>'kb_id' = $1")
        .bind(b.kb.to_string())
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM knowledge_bases WHERE id = $1")
        .bind(b.kb)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM organizations WHERE id = $1")
        .bind(b.org)
        .execute(pool)
        .await?;
    Ok(())
}

/// 这枚令牌的一次写入：没有理由、不属于哪一批、熔断器没跳
fn write(b: &Base) -> TokenWrite {
    TokenWrite {
        kb_id: b.kb,
        owner_id: b.user,
        token_id: b.token,
        rationale: None,
        batch_id: None,
        breaker: false,
    }
}

fn because(b: &Base, why: &str) -> TokenWrite {
    TokenWrite {
        rationale: Some(why.into()),
        ..write(b)
    }
}

async fn entity(pool: &PgPool, kb: Uuid, type_id: Uuid, name: &str) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO entities (id, kb_id, type_id, canonical_name) VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(kb)
    .bind(type_id)
    .bind(name)
    .execute(pool)
    .await?;
    Ok(id)
}

/// 一对等人的重复项。分数取 0.5：REAL 里是精确的，读回来比较不用容差
async fn pair(pool: &PgPool, kb: Uuid, left: Uuid, right: Uuid) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO resolution_reviews (id, kb_id, left_id, right_id, score, reason, stage)
         VALUES ($1, $2, $3, $4, 0.5, 'namesake', 'human')",
    )
    .bind(id)
    .bind(kb)
    .bind(left)
    .bind(right)
    .execute(pool)
    .await?;
    Ok(id)
}

/// 一条实体指向实体的事实。`attested_from` 留空，与闸门自己的测试同一个写法
async fn fact(pool: &PgPool, kb: Uuid, s: Uuid, p: Option<Uuid>, o: Uuid) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO facts (id, kb_id, subject_id, predicate_id, object_id, attested_from)
         VALUES ($1, $2, $3, $4, $5, NULL)",
    )
    .bind(id)
    .bind(kb)
    .bind(s)
    .bind(p)
    .bind(o)
    .execute(pool)
    .await?;
    Ok(id)
}

/// 一份文档里的一个分块。`config` 给了就是一个 statements 来源下的文档
async fn chunk(pool: &PgPool, kb: Uuid, config: Option<Value>) -> anyhow::Result<Uuid> {
    let (source, document, chunk) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    if let Some(config) = &config {
        sqlx::query(
            "INSERT INTO sources (id, kb_id, kind, name, config)
             VALUES ($1, $2, 'statements', 's', $3)",
        )
        .bind(source)
        .bind(kb)
        .bind(config)
        .execute(pool)
        .await?;
    }
    sqlx::query(
        "INSERT INTO documents (id, kb_id, filename, sha256, source_id)
         VALUES ($1, $2, 'd', $1::text, $3)",
    )
    .bind(document)
    .bind(kb)
    .bind(config.map(|_| source))
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO chunks (id, kb_id, document_id, seq, text) VALUES ($1, $2, $3, 0, 'acme')",
    )
    .bind(chunk)
    .bind(kb)
    .bind(document)
    .execute(pool)
    .await?;
    Ok(chunk)
}

/// 还等着人的那些对，按 id 取其中一条。裁过的对不在队列里，所以要在裁之前取
async fn item(pool: &PgPool, kb: Uuid, id: Uuid) -> anyhow::Result<ReviewItem> {
    governance::queue(pool, kb, 50)
        .await?
        .into_iter()
        .find(|i| i.id == id)
        .ok_or_else(|| anyhow::anyhow!("review {id} is not in the pending queue"))
}

/// 审核行此刻的样子：状态、谁裁的、经哪枚令牌、理由
async fn review_row(
    pool: &PgPool,
    id: Uuid,
) -> anyhow::Result<(String, Option<Uuid>, Option<Uuid>, Option<String>)> {
    Ok(sqlx::query_as(
        "SELECT status, decided_by, decided_token, rationale FROM resolution_reviews WHERE id = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await?)
}

/// 这个库的台账，最早的在前
async fn ledger(pool: &PgPool, kb: Uuid) -> anyhow::Result<Vec<TokenAgentAction>> {
    let mut rows = token_agent_actions::list(pool, kb, 500, 0).await?;
    rows.reverse();
    Ok(rows)
}

/// 台账里存着的那几列——`TokenAgentAction` 不带 `target_key`，`state` 又是读的时候算的，
/// 所以直接读表
async fn stored(
    pool: &PgPool,
    id: Uuid,
) -> anyhow::Result<(Option<String>, Option<String>, Option<Uuid>)> {
    Ok(
        sqlx::query_as("SELECT target_key, state, state_by FROM token_agent_actions WHERE id = $1")
            .bind(id)
            .fetch_one(pool)
            .await?,
    )
}

/// 这个库里某个动作的审计行：actor、target_kind、target_id、detail，最早的在前
async fn audits(
    pool: &PgPool,
    kb: Uuid,
    action: &str,
) -> anyhow::Result<Vec<(Option<Uuid>, String, Option<Uuid>, Value)>> {
    Ok(sqlx::query_as(
        "SELECT actor_id, target_kind, target_id, detail FROM audit_events
          WHERE kb_id = $1 AND action = $2 ORDER BY created_at, id",
    )
    .bind(kb)
    .bind(action)
    .fetch_all(pool)
    .await?)
}

/// 这个库里的合并日志：id、源、目标、谁合的、理由、撤回了没有。最早的在前
#[allow(clippy::type_complexity)]
async fn merges(
    pool: &PgPool,
    kb: Uuid,
) -> anyhow::Result<Vec<(Uuid, Uuid, Uuid, Option<Uuid>, Option<String>, bool)>> {
    Ok(sqlx::query_as(
        "SELECT id, source_id, target_id, merged_by, reason, reverted_at IS NOT NULL
           FROM entity_merges WHERE kb_id = $1 ORDER BY created_at, id",
    )
    .bind(kb)
    .fetch_all(pool)
    .await?)
}

fn is_already_decided<T>(r: &Result<T, AppError>) -> bool {
    matches!(
        r,
        Err(AppError::CodedConflict {
            code: "already_decided",
            ..
        })
    )
}

/// 一张表在这个库里的每一行，整行转成 jsonb 文本，按主键排好。`minus` 是不比的列
async fn rows_of(pool: &PgPool, kb: Uuid, sql: &str) -> anyhow::Result<Vec<String>> {
    Ok(sqlx::query_scalar(sql).bind(kb).fetch_all(pool).await?)
}

/// 「图一行不动」比的东西（E7）：事实、实体、证据、类型化事实的来源、审核行、合并日志，
/// 每一行的每一列
async fn graph(pool: &PgPool, kb: Uuid) -> anyhow::Result<Vec<Vec<String>>> {
    let mut out = state_with(pool, kb, "to_jsonb(e)").await?;
    out.push(
        rows_of(
            pool,
            kb,
            "SELECT to_jsonb(r)::text FROM resolution_reviews r WHERE r.kb_id = $1 ORDER BY r.id",
        )
        .await?,
    );
    out.push(
        rows_of(
            pool,
            kb,
            "SELECT to_jsonb(m)::text FROM entity_merges m WHERE m.kb_id = $1 ORDER BY m.id",
        )
        .await?,
    );
    Ok(out)
}

/// 「合了再撤回到原样」比的东西（E12）：`facts` 的每一行、`entities` 的每一行（不含
/// `updated_at`）、`fact_evidence` 与 `typed_fact_sources` 的每一行
async fn state(pool: &PgPool, kb: Uuid) -> anyhow::Result<Vec<Vec<String>>> {
    state_with(pool, kb, "to_jsonb(e) - 'updated_at'").await
}

const STATE_PARTS: [&str; 4] = ["facts", "entities", "fact_evidence", "typed_fact_sources"];

async fn state_with(pool: &PgPool, kb: Uuid, entity: &str) -> anyhow::Result<Vec<Vec<String>>> {
    Ok(vec![
        rows_of(
            pool,
            kb,
            "SELECT to_jsonb(f)::text FROM facts f WHERE f.kb_id = $1 ORDER BY f.id",
        )
        .await?,
        rows_of(
            pool,
            kb,
            &format!("SELECT ({entity})::text FROM entities e WHERE e.kb_id = $1 ORDER BY e.id"),
        )
        .await?,
        rows_of(
            pool,
            kb,
            "SELECT to_jsonb(fe)::text FROM fact_evidence fe JOIN facts f ON f.id = fe.fact_id
              WHERE f.kb_id = $1 ORDER BY fe.fact_id, fe.chunk_id",
        )
        .await?,
        rows_of(
            pool,
            kb,
            "SELECT to_jsonb(ts)::text FROM typed_fact_sources ts JOIN facts f ON f.id = ts.fact_id
              WHERE f.kb_id = $1 ORDER BY ts.fact_id, ts.statement_id",
        )
        .await?,
    ])
}

/// 两份快照逐部分比；不等时只报差出来的行
fn assert_same(before: &[Vec<String>], after: &[Vec<String>], what: &str) {
    let names = STATE_PARTS
        .iter()
        .copied()
        .chain(["resolution_reviews", "entity_merges"]);
    for ((b, a), part) in before.iter().zip(after).zip(names) {
        let gone: Vec<&String> = b.iter().filter(|l| !a.contains(l)).collect();
        let new: Vec<&String> = a.iter().filter(|l| !b.contains(l)).collect();
        assert!(
            b == a,
            "{what}: {part} differs\nonly before: {gone:#?}\nonly after: {new:#?}"
        );
    }
    assert_eq!(before.len(), after.len());
}

/// E12 的比较（INTENT C1）：`facts`、`entities`（不含 `updated_at`）、`fact_evidence` 逐行相同；
/// `typed_fact_sources` 合并之前的每一行都还在，多出来的**正好**是 `copied` 这一条——合并把
/// 被去重的那条类型化事实的陈述来源抄到了存活事实上，现有的撤回不删它。多出别的行、
/// 少了原有的行、或者这一条没出现（种子没走到去重），都算失败
fn assert_restored(
    before: &[Vec<String>],
    after: &[Vec<String>],
    copied: (Uuid, Uuid),
    what: &str,
) {
    assert_eq!((before.len(), after.len()), (4, 4));
    assert_same(&before[..3], &after[..3], what);
    let (was, is) = (&before[3], &after[3]);
    let missing: Vec<&String> = was.iter().filter(|l| !is.contains(l)).collect();
    assert!(
        missing.is_empty(),
        "{what}: typed_fact_sources lost rows it had before the merge: {missing:#?}"
    );
    let added: Vec<Value> = is
        .iter()
        .filter(|l| !was.contains(l))
        .map(|l| serde_json::from_str(l).expect("a jsonb row"))
        .collect();
    assert_eq!(
        added,
        [json!({ "fact_id": copied.0, "statement_id": copied.1 })],
        "{what}: the only typed_fact_sources row added is the statement link the merge copied onto the surviving fact"
    );
    assert_eq!(is.len(), was.len() + 1, "{what}: no row is duplicated");
}

// ───────────────────────── 1. 迁移 0129 ─────────────────────────

/// 全库结构的一份快照，一行一个对象。与 `a_second_migration_run_changes_no_schema`
/// 同一份查询：直接读 `pg_catalog`，表、列、索引、约束各一类
async fn schema(pool: &PgPool) -> anyhow::Result<Vec<String>> {
    let lines = sqlx::query_scalar::<_, String>(
        r#"
        SELECT format('table|%s.%s|kind=%s|persistence=%s',
                      n.nspname, c.relname, c.relkind, c.relpersistence)
          FROM pg_class c
          JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE c.relkind IN ('r', 'p', 'v', 'm')
           AND n.nspname <> 'information_schema' AND n.nspname !~ '^pg_'
        UNION ALL
        SELECT format('column|%s.%s.%s|pos=%s|type=%s|notnull=%s|default=%s|identity=%s|generated=%s',
                      n.nspname, c.relname, a.attname, a.attnum,
                      format_type(a.atttypid, a.atttypmod), a.attnotnull,
                      coalesce(pg_get_expr(d.adbin, d.adrelid), ''),
                      a.attidentity, a.attgenerated)
          FROM pg_attribute a
          JOIN pg_class c ON c.oid = a.attrelid
          JOIN pg_namespace n ON n.oid = c.relnamespace
          LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum
         WHERE a.attnum > 0 AND NOT a.attisdropped
           AND c.relkind IN ('r', 'p', 'v', 'm')
           AND n.nspname <> 'information_schema' AND n.nspname !~ '^pg_'
        UNION ALL
        SELECT format('index|%s.%s|on=%s|valid=%s|ready=%s|def=%s',
                      n.nspname, c.relname, t.relname, i.indisvalid, i.indisready,
                      pg_get_indexdef(i.indexrelid))
          FROM pg_index i
          JOIN pg_class c ON c.oid = i.indexrelid
          JOIN pg_class t ON t.oid = i.indrelid
          JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE n.nspname <> 'information_schema' AND n.nspname !~ '^pg_'
        UNION ALL
        SELECT format('constraint|%s.%s|on=%s|type=%s|validated=%s|def=%s',
                      n.nspname, k.conname,
                      CASE WHEN k.conrelid = 0 THEN '' ELSE k.conrelid::regclass::text END,
                      k.contype, k.convalidated, pg_get_constraintdef(k.oid))
          FROM pg_constraint k
          JOIN pg_namespace n ON n.oid = k.connamespace
         WHERE n.nspname <> 'information_schema' AND n.nspname !~ '^pg_'
         ORDER BY 1
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(lines)
}

/// 直接往台账里插一行，只为看约束与索引拦不拦
async fn raw_row(
    pool: &PgPool,
    b: &Base,
    outcome: &str,
    target_key: &str,
    state: Option<&str>,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO token_agent_actions
             (id, kb_id, operation, target_kind, target_id, owner_id, token_id, token_name,
              outcome, payload, target_key, state)
         VALUES ($1, $2, 'review.merge', 'review', $3, $4, $5, 'x', $6, '{}', $7, $8)",
    )
    .bind(id)
    .bind(b.kb)
    .bind(Uuid::now_v7())
    .bind(b.user)
    .bind(b.token)
    .bind(outcome)
    .bind(target_key)
    .bind(state)
    .execute(pool)
    .await?;
    Ok(id)
}

async fn the_ledger_shape(pool: &PgPool, b: &Base) -> anyhow::Result<()> {
    // 七列，都可空，类型如约
    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT a.attname || ' ' || format_type(a.atttypid, a.atttypmod)
                || CASE WHEN a.attnotnull THEN ' NOT NULL' ELSE '' END
           FROM pg_attribute a
           JOIN pg_class c ON c.oid = a.attrelid
           JOIN pg_namespace n ON n.oid = c.relnamespace
          WHERE n.nspname = 'public' AND c.relname = 'token_agent_actions'
            AND a.attnum > 0 AND NOT a.attisdropped
          ORDER BY a.attnum",
    )
    .fetch_all(pool)
    .await?;
    for column in [
        "gate_reason text",
        "batch_id uuid",
        "merge_id uuid",
        "target_key text",
        "state text",
        "state_at timestamp with time zone",
        "state_by uuid",
    ] {
        assert!(
            columns.contains(&column.to_string()),
            "token_agent_actions has no nullable column `{column}`: {columns:#?}"
        );
    }

    let before = schema(pool).await?;
    let index = before
        .iter()
        .find(|l| l.starts_with("index|public.token_agent_actions_open_proposal_idx|"))
        .expect("the open-proposal index exists");
    assert!(
        index.contains("CREATE UNIQUE INDEX")
            && index.contains("(kb_id, operation, target_key)")
            && index.contains("WHERE")
            && index.contains("'held'")
            && index.contains("'proposed'")
            && index.contains("state IS NULL"),
        "the open-proposal index is not the partial unique index of the contract: {index}"
    );
    let check = before
        .iter()
        .find(|l| l.starts_with("constraint|public.token_agent_actions_state_check|"))
        .expect("the state check exists");
    for state in ["approved", "declined", "superseded", "undone"] {
        assert!(
            check.contains(state),
            "the state check lacks {state}: {check}"
        );
    }

    // 约束与索引真的拦：不认识的状态写不进；同一个目标第二条开着的提议写不进；
    // 第一条答掉之后可以再开一条；落地的行不受这个索引管
    assert!(raw_row(pool, b, "held", "k1", Some("pending"))
        .await
        .is_err());
    let first = raw_row(pool, b, "held", "k1", None).await?;
    assert!(
        raw_row(pool, b, "proposed", "k1", None).await.is_err(),
        "a second open proposal for the same base, operation and target must be refused"
    );
    raw_row(pool, b, "held", "k2", None).await?;
    raw_row(pool, b, "applied", "k1", None).await?;
    raw_row(pool, b, "applied", "k1", None).await?;
    sqlx::query(
        "UPDATE token_agent_actions SET state = 'declined', state_at = now() WHERE id = $1",
    )
    .bind(first)
    .execute(pool)
    .await?;
    raw_row(pool, b, "proposed", "k1", None).await?;

    // 重放：改了编号的迁移在跑过旧号的库上会这样再执行一遍，那一遍必须是空操作
    sqlx::raw_sql(MIGRATION).execute(pool).await?;
    sqlx::raw_sql(MIGRATION).execute(pool).await?;
    let after = schema(pool).await?;
    let gone: Vec<&String> = before.iter().filter(|l| !after.contains(l)).collect();
    let new: Vec<&String> = after.iter().filter(|l| !before.contains(l)).collect();
    assert!(
        before == after,
        "replaying migration 0129 changed the schema\nonly before: {gone:#?}\nonly after: {new:#?}"
    );
    Ok(())
}

#[tokio::test]
async fn migration_0129_adds_the_ledger_columns_and_the_open_proposal_index_and_replays_as_a_no_op(
) -> anyhow::Result<()> {
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let _serial = SERIAL.lock().await;
    let pool = PgPool::connect(&url).await?;
    let b = base(&pool, false).await?;
    let outcome = the_ledger_shape(&pool, &b).await;
    teardown(&pool, &b).await?;
    outcome
}

// ───────────────────────── 2. 合并与它的两行一起落 ─────────────────────────

fn applied(out: &TokenWriteOutcome, operation: &str) -> Uuid {
    assert_eq!(out.action.outcome, "applied", "{:?}", out.action);
    assert_eq!(out.action.operation, operation);
    assert!(!out.existing);
    assert_eq!(out.action.gate_reason, None);
    assert_eq!(out.action.state, None);
    assert_eq!(out.action.state_at, None);
    assert_eq!(out.action.state_by, None);
    out.action.merge_id.expect("an applied row names its merge")
}

async fn the_rows_of_a_merge(pool: &PgPool, b: &Base, other: &Base) -> anyhow::Result<()> {
    // ── 队列里的一对：方向跟人点合并时一样（事实多的一侧活下来）──
    let small = entity(pool, b.kb, b.company_type, "Acme Corp").await?;
    let big = entity(pool, b.kb, b.company_type, "Acme").await?;
    let city = entity(pool, b.kb, b.company_type, "Port Ellen").await?;
    fact(pool, b.kb, big, Some(b.based_in), city).await?;
    fact(pool, b.kb, big, Some(b.part_of), city).await?;
    let moved = fact(pool, b.kb, small, None, city).await?;
    let review = pair(pool, b.kb, small, big).await?;
    assert_eq!(
        resolution::merge_direction(pool, small, big).await?,
        (big, small),
        "a person's queue merge would keep the side with more facts"
    );

    let out = token_agent_actions::merge_duplicate_pair(
        pool,
        &because(b, "  the same company  "),
        review,
    )
    .await?;
    let merge_id = applied(&out, "review.merge");
    assert_eq!(out.follow_up_error, None);

    // 合并：正好一次，方向如上，merged_by 是令牌的主人，理由是修剪过的那一句
    assert_eq!(
        merges(pool, b.kb).await?,
        [(
            merge_id,
            small,
            big,
            Some(b.user),
            Some("the same company".to_string()),
            false
        )]
    );
    let (into, subject): (Option<Uuid>, Uuid) = sqlx::query_as(
        "SELECT (SELECT merged_into FROM entities WHERE id = $1),
                (SELECT subject_id FROM facts WHERE id = $2)",
    )
    .bind(small)
    .bind(moved)
    .fetch_one(pool)
    .await?;
    assert_eq!((into, subject), (Some(big), big), "the entities merged");

    // 审核行
    assert_eq!(
        review_row(pool, review).await?,
        (
            "merged".to_string(),
            Some(b.user),
            Some(b.token),
            Some("the same company".to_string())
        )
    );

    // 台账行：正好一行，applied，指着这次合并
    let rows = ledger(pool, b.kb).await?;
    assert_eq!(rows.len(), 1);
    let row = &out.action;
    assert_eq!(rows[0].id, row.id);
    assert_eq!(row.kb_id, b.kb);
    assert_eq!(row.target_kind, "review");
    assert_eq!(row.target_id, review);
    assert_eq!(row.owner_id, b.user);
    assert_eq!(row.owner_name.as_deref(), Some("Token Owner"));
    assert_eq!(row.token_id, b.token);
    assert_eq!(row.token_name, TOKEN_NAME);
    assert_eq!(row.batch_id, None);
    assert_eq!(rows[0].merge_id, Some(merge_id));
    assert_eq!(
        stored(pool, row.id).await?,
        (Some(review.to_string()), None, None)
    );
    assert_eq!(
        row.payload,
        json!({
            "left": "Acme Corp", "right": "Acme", "score": 0.5, "why": "the same company",
            "review_id": review, "left_id": small, "right_id": big,
            "source_id": small, "target_id": big,
        }),
        "the payload holds the full request"
    );

    // 审计行：正好一行，actor 是主人，detail 带令牌、台账行、合并
    assert_eq!(
        audits(pool, b.kb, "review.merge").await?,
        [(
            Some(b.user),
            "review".to_string(),
            Some(review),
            json!({
                "left": "Acme Corp", "right": "Acme", "score": 0.5, "why": "the same company",
                "token_id": b.token, "token_name": TOKEN_NAME,
                "action_id": row.id, "merge_id": merge_id,
            })
        )]
    );
    assert!(audits(pool, b.kb, "agent.proposal").await?.is_empty());

    // Agent 一档里的这一行：多了 merge_id，没有值的键不出现
    let history = token_agent_actions::agent_history(pool, b.kb, 10, 0).await?;
    assert_eq!(history.len(), 1);
    let keys: Vec<&String> = history[0].as_object().expect("an object").keys().collect();
    assert_eq!(
        keys,
        [
            "created_at",
            "id",
            "left",
            "merge_id",
            "operation",
            "outcome",
            "owner_id",
            "owner_name",
            "right",
            "source",
            "target_id",
            "target_kind",
            "token_id",
            "token_name",
            "rationale",
        ]
        .iter()
        .map(|k| k.to_string())
        .collect::<std::collections::BTreeSet<_>>()
        .iter()
        .collect::<Vec<_>>()
    );
    assert_eq!(history[0]["merge_id"], json!(merge_id));

    // 已经定了的一对再来：already_decided，没有第二次合并、没有第二行
    let again = token_agent_actions::merge_duplicate_pair(pool, &write(b), review).await;
    assert!(is_already_decided(&again), "{again:?}");
    assert_eq!(merges(pool, b.kb).await?.len(), 1);
    assert_eq!(ledger(pool, b.kb).await?.len(), 1);

    // ── 手动合并：方向是调用方给的，这一对自己等着的审核行一并关上 ──
    let source = entity(pool, b.kb, b.company_type, "Orion").await?;
    let target = entity(pool, b.kb, b.company_type, "Orion Labs").await?;
    // 源的事实更多：人的队列合并会让它活下来，手动合并照调用方说的办
    fact(pool, b.kb, source, Some(b.based_in), city).await?;
    fact(pool, b.kb, source, Some(b.part_of), city).await?;
    let own_review = pair(pool, b.kb, target, source).await?;
    let out = token_agent_actions::merge_entity_pair(pool, &write(b), source, target).await?;
    let manual_id = applied(&out, "merge.manual");
    assert_eq!(out.follow_up_error, None);
    assert_eq!(
        merges(pool, b.kb).await?[1],
        (
            manual_id,
            source,
            target,
            Some(b.user),
            Some("manual merge".to_string()),
            false
        )
    );
    assert_eq!(
        review_row(pool, own_review).await?,
        ("merged".to_string(), Some(b.user), Some(b.token), None),
        "the pair's own pending review is closed as merged, with the token recorded"
    );
    let row = &out.action;
    assert_eq!(row.target_kind, "entity");
    assert_eq!(row.target_id, target);
    let key = format!("{}|{}", source.min(target), source.max(target));
    assert_eq!(stored(pool, row.id).await?, (Some(key), None, None));
    assert_eq!(
        row.payload,
        json!({
            "left": "Orion", "right": "Orion Labs", "source": "Orion", "target": "Orion Labs",
            "why": null, "source_id": source, "target_id": target, "review_id": own_review,
        })
    );
    assert_eq!(
        audits(pool, b.kb, "merge.manual").await?,
        [(
            Some(b.user),
            "entity".to_string(),
            Some(target),
            json!({
                "source": "Orion", "target": "Orion Labs", "why": null, "review_id": own_review,
                "token_id": b.token, "token_name": TOKEN_NAME,
                "action_id": row.id, "merge_id": manual_id,
            })
        )]
    );

    // 成批调用的成员：台账行与审计行都带着批次号
    let batch = Uuid::now_v7();
    let l = entity(pool, b.kb, b.company_type, "Vega").await?;
    let r = entity(pool, b.kb, b.company_type, "Vega Inc.").await?;
    let in_batch = pair(pool, b.kb, l, r).await?;
    let member = TokenWrite {
        batch_id: Some(batch),
        ..write(b)
    };
    let out = token_agent_actions::merge_duplicate_pair(pool, &member, in_batch).await?;
    applied(&out, "review.merge");
    assert_eq!(out.action.batch_id, Some(batch));
    let audit = audits(pool, b.kb, "review.merge").await?;
    assert_eq!(audit[1].3["batch_id"], json!(batch));

    // ── 拒绝：都不留提议、不动图（E3）──
    let rows_before = ledger(pool, b.kb).await?.len();
    let merges_before = merges(pool, b.kb).await?.len();
    let graph_before = graph(pool, b.kb).await?;
    let lone = entity(pool, b.kb, b.company_type, "Lyra").await?;
    let itself = token_agent_actions::merge_entity_pair(pool, &write(b), lone, lone).await;
    assert!(
        matches!(
            itself,
            Err(AppError::Invalid {
                code: "self_merge",
                ..
            })
        ),
        "{itself:?}"
    );
    // 已经并进别人的实体，在哪一侧都是 already_decided
    for (s, t) in [(small, lone), (lone, small)] {
        let gone = token_agent_actions::merge_entity_pair(pool, &write(b), s, t).await;
        assert!(is_already_decided(&gone), "{gone:?}");
    }
    // 不存在的 id、别的库的 id：NotFound
    let elsewhere = entity(pool, other.kb, other.company_type, "Lyra").await?;
    for (s, t) in [
        (lone, Uuid::now_v7()),
        (Uuid::now_v7(), lone),
        (lone, elsewhere),
        (elsewhere, lone),
    ] {
        let missing = token_agent_actions::merge_entity_pair(pool, &write(b), s, t).await;
        assert!(matches!(missing, Err(AppError::NotFound)), "{missing:?}");
    }
    let foreign_l = entity(pool, other.kb, other.company_type, "Zhao Liu").await?;
    let foreign_r = entity(pool, other.kb, other.company_type, "Zhao Liu").await?;
    let foreign = pair(pool, other.kb, foreign_l, foreign_r).await?;
    for id in [foreign, Uuid::now_v7()] {
        let missing = token_agent_actions::merge_duplicate_pair(pool, &write(b), id).await;
        assert!(matches!(missing, Err(AppError::NotFound)), "{missing:?}");
    }
    assert_eq!(review_row(pool, foreign).await?.0, "pending");
    // 队列里的一对，一侧已经被并走（有人手动合了它，审核行还挂着）：already_decided
    let x = entity(pool, b.kb, b.company_type, "Draco").await?;
    let y = entity(pool, b.kb, b.company_type, "Draco Ltd").await?;
    let z = entity(pool, b.kb, b.company_type, "Draco Group").await?;
    let stale = pair(pool, b.kb, x, y).await?;
    resolution::merge_entities(pool, b.kb, x, y, Some(b.user), "test").await?;
    let _ = z;
    let rows_now = ledger(pool, b.kb).await?.len();
    let gone = token_agent_actions::merge_duplicate_pair(pool, &write(b), stale).await;
    assert!(is_already_decided(&gone), "{gone:?}");
    assert_eq!(review_row(pool, stale).await?.0, "pending");
    assert_eq!(ledger(pool, b.kb).await?.len(), rows_now);
    assert_eq!(rows_now, rows_before, "no refusal wrote a ledger row");
    assert_eq!(merges(pool, b.kb).await?.len(), merges_before + 1);
    assert_eq!(token_agent_actions::count(pool, other.kb).await?, 0);
    // 那一次人的手动合并之外，图没有别的变化：把它撤了再比
    let persons = merges(pool, b.kb).await?[merges_before].0;
    resolution::revert_merge(pool, b.kb, persons).await?;
    let graph_after = graph(pool, b.kb).await?;
    assert_eq!(
        graph_before[0], graph_after[0],
        "the refused calls changed no fact"
    );
    assert_eq!(
        graph_before[4].len() + 1,
        graph_after[4].len(),
        "only the one review this section seeded in this base was added"
    );
    Ok(())
}

async fn locked_by_the_governor(pool: &PgPool, b: &Base) -> anyhow::Result<()> {
    let l = entity(pool, b.kb, b.person_type, "Zhang Wei").await?;
    let r = entity(pool, b.kb, b.person_type, "Zhang Wei").await?;
    let review = pair(pool, b.kb, l, r).await?;
    sqlx::query("INSERT INTO jobs (kind, payload, status) VALUES ('govern', $1, 'running')")
        .bind(json!({ "kb_id": b.kb }))
        .execute(pool)
        .await?;
    governance::lock(pool, b.kb, &[review]).await?;
    assert!(governance::locked_by_agent(pool, b.kb, review).await?);

    // 一对被跑着的治理轮次锁着：合并听到的就是分开听到的那一句，队列合并与手动合并都是
    let keep =
        token_agent_actions::keep_duplicate_pair(pool, b.kb, review, b.user, b.token, None).await;
    let Err(AppError::Conflict(keep_says)) = keep else {
        panic!("expected the governor's Conflict for a keep, got {keep:?}");
    };
    for refused in [
        token_agent_actions::merge_duplicate_pair(pool, &write(b), review).await,
        token_agent_actions::merge_entity_pair(pool, &write(b), l, r).await,
    ] {
        match refused {
            Err(AppError::Conflict(says)) => assert_eq!(says, keep_says),
            other => panic!("expected the governor's Conflict, got {other:?}"),
        }
    }
    assert_eq!(review_row(pool, review).await?.0, "pending");
    assert!(ledger(pool, b.kb).await?.is_empty());
    assert!(merges(pool, b.kb).await?.is_empty());

    // 任务一停，标记只是标记：令牌照常能合
    sqlx::query("DELETE FROM jobs WHERE kind = 'govern' AND payload->>'kb_id' = $1")
        .bind(b.kb.to_string())
        .execute(pool)
        .await?;
    let out = token_agent_actions::merge_duplicate_pair(pool, &write(b), review).await?;
    applied(&out, "review.merge");
    Ok(())
}

#[tokio::test]
async fn a_token_merge_the_gate_passes_writes_the_merge_the_review_row_the_ledger_row_and_the_audit_row_together(
) -> anyhow::Result<()> {
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let _serial = SERIAL.lock().await;
    let pool = PgPool::connect(&url).await?;
    let b = base(&pool, false).await?;
    let other = base(&pool, false).await?;
    let governed = base(&pool, true).await?;
    let mut outcome = the_rows_of_a_merge(&pool, &b, &other).await;
    if outcome.is_ok() {
        outcome = locked_by_the_governor(&pool, &governed).await;
    }
    teardown(&pool, &b).await?;
    teardown(&pool, &other).await?;
    teardown(&pool, &governed).await?;
    outcome
}

// ───────────────────────── 读不到名字，什么都不落 ─────────────────────────

async fn nothing_without_a_name(pool: &PgPool, b: &Base) -> anyhow::Result<()> {
    let city = entity(pool, b.kb, b.company_type, "Port Ellen").await?;
    let a = entity(pool, b.kb, b.company_type, "Acme").await?;
    let c = entity(pool, b.kb, b.company_type, "Acme Corp").await?;
    fact(pool, b.kb, a, Some(b.based_in), city).await?;
    fact(pool, b.kb, c, Some(b.based_in), city).await?;
    let review = pair(pool, b.kb, a, c).await?;
    // 一对闸门会留下的：提议同样要记名字
    let held_a = entity(pool, b.kb, b.company_type, "Orion").await?;
    let held_c = entity(pool, b.kb, b.company_type, "Orion Labs").await?;
    let boss = entity(pool, b.kb, b.person_type, "Alice").await?;
    let rival = entity(pool, b.kb, b.person_type, "Bob").await?;
    fact(pool, b.kb, held_a, Some(b.ceo_of), boss).await?;
    fact(pool, b.kb, held_c, Some(b.ceo_of), rival).await?;
    let held_review = pair(pool, b.kb, held_a, held_c).await?;
    // 一次真令牌做的合并：没有名字的令牌撤不动它
    let m_a = entity(pool, b.kb, b.company_type, "Vega").await?;
    let m_c = entity(pool, b.kb, b.company_type, "Vega Inc.").await?;
    let made = token_agent_actions::merge_entity_pair(pool, &write(b), m_a, m_c).await?;
    let made_id = applied(&made, "merge.manual");

    let before = graph(pool, b.kb).await?;
    let rows_before = ledger(pool, b.kb).await?.len();
    let audit_count = || async {
        anyhow::Ok(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM audit_events WHERE kb_id = $1")
                .bind(b.kb)
                .fetch_one(pool)
                .await?,
        )
    };
    let audits_before = audit_count().await?;

    // 一枚 `personal_tokens` 里没有的令牌：名字读不到
    let ghost = TokenWrite {
        token_id: Uuid::now_v7(),
        ..because(b, "why")
    };
    let refusals = [
        token_agent_actions::merge_duplicate_pair(pool, &ghost, review).await,
        token_agent_actions::merge_entity_pair(pool, &ghost, c, a).await,
        token_agent_actions::merge_duplicate_pair(pool, &ghost, held_review).await,
        token_agent_actions::merge_entity_pair(pool, &ghost, held_a, held_c).await,
        token_agent_actions::revert_merge(pool, &ghost, made_id).await,
    ];
    for refused in &refusals {
        assert!(refused.is_err(), "no token row to name: the call must fail");
        assert!(
            !is_already_decided(refused) && !matches!(refused, Err(AppError::NotFound)),
            "a missing token is its own failure, not a decided pair or a missing id: {refused:?}"
        );
    }

    // 什么都没落：没有合并、没有撤回、没有台账行、没有审计行，两对都还等着
    assert_same(
        &before,
        &graph(pool, b.kb).await?,
        "a call with no token to name",
    );
    assert_eq!(ledger(pool, b.kb).await?.len(), rows_before);
    assert_eq!(audit_count().await?, audits_before);
    assert_eq!(
        review_row(pool, review).await?,
        ("pending".to_string(), None, None, None)
    );
    assert_eq!(review_row(pool, held_review).await?.0, "pending");
    assert!(!merges(pool, b.kb).await?[0].5, "the merge is not reverted");

    // 没被弄坏：换一枚真的令牌照样能合、能撤
    let out = token_agent_actions::merge_duplicate_pair(pool, &write(b), review).await?;
    applied(&out, "review.merge");
    let out = token_agent_actions::revert_merge(pool, &write(b), made_id).await?;
    assert_eq!(out.action.outcome, "applied");
    Ok(())
}

#[tokio::test]
async fn a_token_merge_with_no_token_to_name_merges_nothing_and_writes_no_row() -> anyhow::Result<()>
{
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let _serial = SERIAL.lock().await;
    let pool = PgPool::connect(&url).await?;
    let b = base(&pool, false).await?;
    let outcome = nothing_without_a_name(&pool, &b).await;
    teardown(&pool, &b).await?;
    outcome
}

// ───────────────────────── 后续步骤失败，结果仍是 applied ─────────────────────────

/// 合并提交之后的最后一步是重算消歧后缀（`UPDATE entities SET disambiguator`）；事务里的
/// 合并不碰这一列。在这一列上挂一个只认本库的触发器，那一步就必然失败，而合并本身照常提交
const REFUSE: &str = "m003_t001_refuse_disambiguator";

async fn refuse_the_follow_up(pool: &PgPool, kb: Uuid) -> anyhow::Result<()> {
    sqlx::raw_sql(&format!(
        "CREATE OR REPLACE FUNCTION {REFUSE}() RETURNS trigger LANGUAGE plpgsql AS
         $$ BEGIN RAISE EXCEPTION 'the follow-up step is refused for this test'; END $$;
         DROP TRIGGER IF EXISTS {REFUSE} ON entities;
         CREATE TRIGGER {REFUSE} BEFORE UPDATE OF disambiguator ON entities
            FOR EACH ROW WHEN (NEW.kb_id = '{kb}'::uuid) EXECUTE FUNCTION {REFUSE}();"
    ))
    .execute(pool)
    .await?;
    Ok(())
}

async fn allow_the_follow_up(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::raw_sql(&format!(
        "DROP TRIGGER IF EXISTS {REFUSE} ON entities; DROP FUNCTION IF EXISTS {REFUSE}();"
    ))
    .execute(pool)
    .await?;
    Ok(())
}

async fn a_failed_follow_up(pool: &PgPool, b: &Base) -> anyhow::Result<()> {
    let city = entity(pool, b.kb, b.company_type, "Port Ellen").await?;
    for (left, right, manual) in [("Acme", "Acme Corp", false), ("Orion", "Orion Labs", true)] {
        let l = entity(pool, b.kb, b.company_type, left).await?;
        let r = entity(pool, b.kb, b.company_type, right).await?;
        fact(pool, b.kb, r, Some(b.based_in), city).await?;
        let review = pair(pool, b.kb, l, r).await?;
        let rows_before = ledger(pool, b.kb).await?.len();
        let merges_before = merges(pool, b.kb).await?.len();

        let out = if manual {
            token_agent_actions::merge_entity_pair(pool, &write(b), l, r).await
        } else {
            token_agent_actions::merge_duplicate_pair(pool, &write(b), review).await
        };
        // 不是错误：合并已经在了
        let out = out.expect("a follow-up failure is not a failed merge");
        let operation = if manual {
            "merge.manual"
        } else {
            "review.merge"
        };
        let merge_id = applied(&out, operation);
        let error = out
            .follow_up_error
            .as_deref()
            .expect("the follow-up failure is reported on the applied outcome");
        assert!(
            error.contains("the follow-up step is refused for this test"),
            "{error}"
        );

        // 合并、审核行、台账行、审计行都在
        let log = merges(pool, b.kb).await?;
        assert_eq!(log.len(), merges_before + 1);
        assert_eq!(log[merges_before].0, merge_id);
        let merged: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM entities WHERE id = ANY($1) AND merged_into IS NOT NULL",
        )
        .bind(vec![l, r])
        .fetch_one(pool)
        .await?;
        assert_eq!(merged, 1, "the entities merged");
        assert_eq!(
            review_row(pool, review).await?,
            ("merged".to_string(), Some(b.user), Some(b.token), None)
        );
        let rows = ledger(pool, b.kb).await?;
        assert_eq!(rows.len(), rows_before + 1);
        assert_eq!(rows[rows_before].id, out.action.id);
        assert_eq!(rows[rows_before].outcome, "applied");
        assert_eq!(rows[rows_before].merge_id, Some(merge_id));
        let audit = audits(pool, b.kb, operation).await?;
        assert_eq!(audit.len(), 1);
        assert_eq!(audit[0].3["action_id"], json!(out.action.id));
        assert_eq!(audit[0].3["merge_id"], json!(merge_id));
        assert_eq!(audit[0].3["token_name"], json!(TOKEN_NAME));
    }
    Ok(())
}

#[tokio::test]
async fn a_follow_up_failure_after_the_merge_is_reported_on_an_applied_outcome(
) -> anyhow::Result<()> {
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let _serial = SERIAL.lock().await;
    let pool = PgPool::connect(&url).await?;
    let b = base(&pool, false).await?;
    let mut outcome = refuse_the_follow_up(&pool, b.kb).await;
    if outcome.is_ok() {
        outcome = a_failed_follow_up(&pool, &b).await;
    }
    allow_the_follow_up(&pool).await?;
    teardown(&pool, &b).await?;
    outcome
}

// ───────────────────────── 3. 闸门的六种留下 ─────────────────────────

/// 一条留下的调用该有的样子：台账行（`held`、理由、目标）、一条 `agent.proposal` 审计行
async fn assert_held(
    pool: &PgPool,
    b: &Base,
    out: &TokenWriteOutcome,
    (operation, target_kind, target_id, target_key): (&str, &str, Uuid, &str),
    (outcome, reason): (&str, &str),
) -> anyhow::Result<()> {
    let row = &out.action;
    assert!(!out.existing, "the first held call writes the proposal");
    assert_eq!(out.follow_up_error, None);
    assert_eq!(row.outcome, outcome, "{row:?}");
    assert_eq!(row.gate_reason.as_deref(), Some(reason), "{row:?}");
    assert_eq!(row.operation, operation);
    assert_eq!(row.target_kind, target_kind);
    assert_eq!(row.target_id, target_id);
    assert_eq!(row.owner_id, b.user);
    assert_eq!(row.token_id, b.token);
    assert_eq!(row.token_name, TOKEN_NAME);
    assert_eq!(
        (row.state.as_deref(), row.state_at, row.state_by),
        (None, None, None)
    );
    assert_eq!(
        stored(pool, row.id).await?,
        (Some(target_key.to_string()), None, None),
        "an open proposal: its target key is stored and its state is null"
    );
    let audit: Vec<_> = audits(pool, b.kb, "agent.proposal")
        .await?
        .into_iter()
        .filter(|a| a.2 == Some(row.id))
        .collect();
    assert_eq!(
        audit.len(),
        1,
        "one agent.proposal audit row for the proposal"
    );
    assert_eq!(audit[0].0, Some(b.user));
    assert_eq!(audit[0].1, "agent_action");
    let mut detail = json!({
        "operation": operation, "outcome": outcome, "reason": reason,
        "token_id": b.token, "token_name": TOKEN_NAME, "action_id": row.id,
    });
    if let Some(merge_id) = row.merge_id {
        detail["merge_id"] = json!(merge_id);
    }
    assert_eq!(audit[0].3, detail);
    Ok(())
}

async fn the_six_holds(pool: &PgPool, b: &Base) -> anyhow::Result<()> {
    let alice = entity(pool, b.kb, b.person_type, "Alice").await?;
    let bob = entity(pool, b.kb, b.person_type, "Bob").await?;
    let company = |name: &'static str| entity(pool, b.kb, b.company_type, name);
    let mut held: Vec<(&str, Uuid, Uuid)> = Vec::new();

    // 身份来源：a 的名字是一个把名字当身份标识的来源说的
    let (a, c) = (company("acme/a.ts").await?, company("acme/a").await?);
    let identity = chunk(pool, b.kb, Some(json!({ "names": "identity" }))).await?;
    utopia_store::names::record(
        pool,
        b.kb,
        a,
        "acme/a.ts",
        Some(NameSource {
            chunk_id: identity,
            quote: "acme/a.ts",
        }),
        None,
    )
    .await?;
    held.push(("identity", a, c));

    // 矛盾：两边各有一个不同的 CEO
    let (a, c) = (company("Borealis").await?, company("Borealis Corp").await?);
    fact(pool, b.kb, a, Some(b.ceo_of), alice).await?;
    fact(pool, b.kb, c, Some(b.ceo_of), bob).await?;
    held.push(("contradiction", a, c));

    // 派生：一条靠着 a 推出来的、还成立的派生事实
    let (a, c) = (company("Cygnus").await?, company("Cygnus Ltd").await?);
    let rule = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO rules (id, kb_id, predicate_id, kind) VALUES ($1, $2, $3, 'transitive')",
    )
    .bind(rule)
    .bind(b.kb)
    .bind(b.part_of)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO derived_facts (id, kb_id, subject_id, predicate_id, object_id, rule_id)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(Uuid::now_v7())
    .bind(b.kb)
    .bind(a)
    .bind(b.part_of)
    .bind(alice)
    .bind(rule)
    .execute(pool)
    .await?;
    held.push(("derived", a, c));

    // 回答：c 在一轮回答里被认过
    let (a, c) = (company("Draco").await?, company("Draco Group").await?);
    let conversation = Uuid::now_v7();
    sqlx::query("INSERT INTO conversations (id, kb_id, user_id) VALUES ($1, $2, $3)")
        .bind(conversation)
        .bind(b.kb)
        .bind(b.user)
        .execute(pool)
        .await?;
    sqlx::query(
        "INSERT INTO conversation_messages (id, conversation_id, role, content, resolved)
         VALUES ($1, $2, 'assistant', 'hi', $3)",
    )
    .bind(Uuid::now_v7())
    .bind(conversation)
    .bind(json!([{ "id": c.to_string(), "name": "x", "type": null }]))
    .execute(pool)
    .await?;
    held.push(("answered", a, c));

    // 人分开过的一对：从前的审核行是 (c, a)，现在问的是 (a, c)——按实体 id、不分左右
    let (a, c) = (company("Eridanus").await?, company("Eridanus Inc.").await?);
    let earlier = pair(pool, b.kb, c, a).await?;
    resolution::decide_review(pool, b.kb, earlier, "keep", b.user, Some("two companies")).await?;
    held.push(("kept_by_person", a, c));

    // 合过又撤回的一对：从前是 c 并进 a，现在问的是 (a, c)，手动合并要的是 a 并进 c
    let (a, c) = (company("Fornax").await?, company("Fornax Labs").await?);
    let undone = resolution::merge_entities(pool, b.kb, c, a, Some(b.user), "test").await?;
    resolution::revert_merge(pool, b.kb, undone).await?;
    held.push(("reverted_merge", a, c));

    for (code, a, c) in held {
        // 闸门自己怎么说：新的两种只有令牌的闸门给，`hold_for_merge` 永远只给原来四种
        let hold = hold_for_token_merge(pool, b.kb, a, c).await?;
        let swapped = hold_for_token_merge(pool, b.kb, c, a).await?;
        assert_eq!(hold.as_ref().map(Hold::code), Some(code));
        assert_eq!(swapped.as_ref().map(Hold::code), Some(code), "either order");
        let plain = hold_for_merge(pool, b.kb, a, c).await?;
        match code {
            "kept_by_person" | "reverted_merge" => assert_eq!(plain, None, "{code}"),
            _ => assert_eq!(plain, hold, "{code}"),
        }
        assert!(
            !matches!(plain, Some(Hold::KeptByPerson | Hold::RevertedMerge)),
            "hold_for_merge returned a hold only the token gate may give"
        );

        // 队列合并：一条提议，理由点名，图一行不动
        let review = pair(pool, b.kb, a, c).await?;
        let before = graph(pool, b.kb).await?;
        let rows_before = ledger(pool, b.kb).await?.len();
        let out =
            token_agent_actions::merge_duplicate_pair(pool, &because(b, "same"), review).await?;
        assert_held(
            pool,
            b,
            &out,
            ("review.merge", "review", review, &review.to_string()),
            ("held", code),
        )
        .await?;
        assert_eq!(out.action.merge_id, None);
        assert_eq!(out.action.payload["review_id"], json!(review));
        assert_eq!(out.action.payload["why"], json!("same"));
        assert_same(&before, &graph(pool, b.kb).await?, code);
        assert_eq!(review_row(pool, review).await?.0, "pending");

        // 手动合并（方向反过来给）：同一种理由的另一条提议，图仍一行不动
        let out = token_agent_actions::merge_entity_pair(pool, &write(b), a, c).await?;
        let key = format!("{}|{}", a.min(c), a.max(c));
        assert_held(
            pool,
            b,
            &out,
            ("merge.manual", "entity", c, &key),
            ("held", code),
        )
        .await?;
        assert_eq!(out.action.payload["source_id"], json!(a));
        assert_eq!(out.action.payload["target_id"], json!(c));
        assert_same(&before, &graph(pool, b.kb).await?, code);
        assert_eq!(ledger(pool, b.kb).await?.len(), rows_before + 2);
    }
    assert!(
        merges(pool, b.kb)
            .await?
            .iter()
            .all(|m| m.5 || m.3.is_none()),
        "no held call merged anything: the only merge in the base is the reverted one"
    );
    assert_eq!(merges(pool, b.kb).await?.len(), 1);
    assert!(audits(pool, b.kb, "review.merge").await?.is_empty());
    assert!(audits(pool, b.kb, "merge.manual").await?.is_empty());

    // 令牌分开过的一对不留：那不是人的决定
    let (a, c) = (company("Gemini").await?, company("Gemini Co").await?);
    let by_token = pair(pool, b.kb, c, a).await?;
    token_agent_actions::keep_duplicate_pair(pool, b.kb, by_token, b.user, b.token, None).await?;
    assert_eq!(hold_for_token_merge(pool, b.kb, a, c).await?, None);
    let review = pair(pool, b.kb, a, c).await?;
    let out = token_agent_actions::merge_duplicate_pair(pool, &write(b), review).await?;
    applied(&out, "review.merge");

    // 人分开之后令牌又分开了一次：最近一次裁决是令牌的，不算「人分开的一对」
    let (a, c) = (company("Hydra").await?, company("Hydra Co").await?);
    let first = pair(pool, b.kb, a, c).await?;
    resolution::decide_review(pool, b.kb, first, "keep", b.user, None).await?;
    assert_eq!(
        hold_for_token_merge(pool, b.kb, c, a).await?,
        Some(Hold::KeptByPerson)
    );
    let second = pair(pool, b.kb, a, c).await?;
    token_agent_actions::keep_duplicate_pair(pool, b.kb, second, b.user, b.token, None).await?;
    assert_eq!(hold_for_token_merge(pool, b.kb, a, c).await?, None);

    // 没撤回的合并不算「合过又撤回」：另一对实体之间活着的合并不拦别人
    assert_eq!(Hold::KeptByPerson.code(), "kept_by_person");
    assert_eq!(Hold::RevertedMerge.code(), "reverted_merge");
    assert_eq!(Hold::Identity.code(), "identity");
    assert_eq!(Hold::Contradiction("x".into()).code(), "contradiction");
    assert_eq!(Hold::Derived(1).code(), "derived");
    assert_eq!(Hold::Answered(1).code(), "answered");
    Ok(())
}

#[tokio::test]
async fn each_of_the_six_holds_lands_one_proposal_naming_it_and_changes_nothing(
) -> anyhow::Result<()> {
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let _serial = SERIAL.lock().await;
    let pool = PgPool::connect(&url).await?;
    let b = base(&pool, false).await?;
    let outcome = the_six_holds(&pool, &b).await;
    teardown(&pool, &b).await?;
    outcome
}

// ───────────────────────── 一个目标一条开着的提议 ─────────────────────────

/// 闸门会留下的一对（派生）：两个公司，其中一个身上有一条还成立的派生事实
async fn held_pair(pool: &PgPool, b: &Base, name: &str) -> anyhow::Result<(Uuid, Uuid)> {
    let a = entity(pool, b.kb, b.company_type, name).await?;
    let c = entity(pool, b.kb, b.company_type, &format!("{name} Ltd")).await?;
    let rule: Option<Uuid> = sqlx::query_scalar("SELECT id FROM rules WHERE kb_id = $1 LIMIT 1")
        .bind(b.kb)
        .fetch_optional(pool)
        .await?;
    let rule = match rule {
        Some(rule) => rule,
        None => {
            let rule = Uuid::now_v7();
            sqlx::query(
                "INSERT INTO rules (id, kb_id, predicate_id, kind)
                 VALUES ($1, $2, $3, 'transitive')",
            )
            .bind(rule)
            .bind(b.kb)
            .bind(b.part_of)
            .execute(pool)
            .await?;
            rule
        }
    };
    sqlx::query(
        "INSERT INTO derived_facts (id, kb_id, subject_id, predicate_id, object_id, rule_id)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(Uuid::now_v7())
    .bind(b.kb)
    .bind(a)
    .bind(b.part_of)
    .bind(c)
    .bind(rule)
    .execute(pool)
    .await?;
    Ok((a, c))
}

async fn one_open_proposal(pool: &PgPool, b: &Base) -> anyhow::Result<()> {
    let open = |operation: &'static str| async move {
        anyhow::Ok(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM token_agent_actions
                  WHERE kb_id = $1 AND operation = $2
                    AND outcome IN ('held', 'proposed') AND state IS NULL",
            )
            .bind(b.kb)
            .bind(operation)
            .fetch_one(pool)
            .await?,
        )
    };

    // ── 重复：第二次什么都不写，拿回第一条 ──
    let (a, c) = held_pair(pool, b, "Acme").await?;
    let review = pair(pool, b.kb, a, c).await?;
    let before = graph(pool, b.kb).await?;
    let first = token_agent_actions::merge_duplicate_pair(pool, &because(b, "one"), review).await?;
    assert!(!first.existing);
    assert_eq!(first.action.outcome, "held");
    let again = token_agent_actions::merge_duplicate_pair(pool, &because(b, "two"), review).await?;
    assert!(again.existing, "a repeat meets the open proposal");
    assert_eq!(again.action.id, first.action.id);
    assert_eq!(
        again.action.payload["why"],
        json!("one"),
        "nothing was rewritten"
    );
    assert_eq!(again.follow_up_error, None);
    assert_eq!(open("review.merge").await?, 1);

    // 手动合并的目标是不分方向的一对：A 并进 B 与 B 并进 A 是同一条提议
    let first = token_agent_actions::merge_entity_pair(pool, &write(b), a, c).await?;
    assert!(!first.existing);
    let swapped = token_agent_actions::merge_entity_pair(pool, &write(b), c, a).await?;
    assert!(swapped.existing);
    assert_eq!(swapped.action.id, first.action.id);
    assert_eq!(open("merge.manual").await?, 1);
    assert_eq!(audits(pool, b.kb, "agent.proposal").await?.len(), 2);

    // ── 同时到的两个：正好一条 ──
    let (a, c) = held_pair(pool, b, "Orion").await?;
    let review = pair(pool, b.kb, a, c).await?;
    let (w1, w2) = (because(b, "one"), because(b, "two"));
    let (one, two) = tokio::join!(
        token_agent_actions::merge_duplicate_pair(pool, &w1, review),
        token_agent_actions::merge_duplicate_pair(pool, &w2, review)
    );
    let (one, two) = (one?, two?);
    assert_eq!(
        one.action.id, two.action.id,
        "both got the one open proposal"
    );
    assert_eq!(
        [one.existing, two.existing].iter().filter(|e| **e).count(),
        1,
        "exactly one of two simultaneous held calls wrote the proposal"
    );
    assert_eq!(open("review.merge").await?, 2);
    let w = write(b);
    let (one, two) = tokio::join!(
        token_agent_actions::merge_entity_pair(pool, &w, a, c),
        token_agent_actions::merge_entity_pair(pool, &w, c, a)
    );
    let (one, two) = (one?, two?);
    assert_eq!(one.action.id, two.action.id);
    assert_eq!(
        [one.existing, two.existing].iter().filter(|e| **e).count(),
        1
    );
    assert_eq!(open("merge.manual").await?, 2);
    assert_eq!(
        audits(pool, b.kb, "agent.proposal").await?.len(),
        4,
        "one audit row per proposal, none for the calls that met an existing one"
    );
    assert_eq!(ledger(pool, b.kb).await?.len(), 4);

    // ── 熔断器跳着的令牌：闸门放行的一对也只留提议，理由是 breaker ──
    let l = entity(pool, b.kb, b.company_type, "Vega").await?;
    let r = entity(pool, b.kb, b.company_type, "Vega Inc.").await?;
    let clean = pair(pool, b.kb, l, r).await?;
    assert_eq!(hold_for_token_merge(pool, b.kb, l, r).await?, None);
    let tripped = TokenWrite {
        breaker: true,
        ..write(b)
    };
    let out = token_agent_actions::merge_duplicate_pair(pool, &tripped, clean).await?;
    assert_held(
        pool,
        b,
        &out,
        ("review.merge", "review", clean, &clean.to_string()),
        ("proposed", "breaker"),
    )
    .await?;
    let repeat = token_agent_actions::merge_duplicate_pair(pool, &tripped, clean).await?;
    assert!(repeat.existing);
    assert_eq!(repeat.action.id, out.action.id);
    let key = format!("{}|{}", l.min(r), l.max(r));
    let out = token_agent_actions::merge_entity_pair(pool, &tripped, l, r).await?;
    assert_held(
        pool,
        b,
        &out,
        ("merge.manual", "entity", r, &key),
        ("proposed", "breaker"),
    )
    .await?;

    // 这一整段里没有一次合并，图一行没动（新种下的实体与审核行之外）
    assert!(merges(pool, b.kb).await?.is_empty());
    let after = graph(pool, b.kb).await?;
    assert_eq!(before[0], after[0], "no fact changed");
    assert_eq!(before[2], after[2]);
    assert_eq!(before[3], after[3]);
    assert!(
        before[1].iter().all(|e| after[1].contains(e)),
        "no entity changed"
    );
    assert!(
        before[4].iter().all(|r| after[4].contains(r)),
        "no review changed"
    );
    assert_eq!(review_row(pool, clean).await?.0, "pending");

    // 一条提议答掉之后（这里直接写状态，答的路由在后面的任务里），同一个目标可以再开一条
    sqlx::query(
        "UPDATE token_agent_actions SET state = 'declined', state_at = now(), state_by = $2
          WHERE id = $1",
    )
    .bind(repeat.action.id)
    .bind(b.user)
    .execute(pool)
    .await?;
    let reopened = token_agent_actions::merge_duplicate_pair(pool, &tripped, clean).await?;
    assert!(!reopened.existing);
    assert_ne!(reopened.action.id, repeat.action.id);
    // 答掉的那一条读回来带着它的状态
    let declined = ledger(pool, b.kb)
        .await?
        .into_iter()
        .find(|r| r.id == repeat.action.id)
        .expect("the declined proposal is still in the ledger");
    assert_eq!(declined.state.as_deref(), Some("declined"));
    assert!(declined.state_at.is_some());
    assert_eq!(declined.state_by, Some(b.user));
    Ok(())
}

#[tokio::test]
async fn a_repeated_or_simultaneous_held_call_returns_the_one_open_proposal() -> anyhow::Result<()>
{
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let _serial = SERIAL.lock().await;
    let pool = PgPool::connect(&url).await?;
    let b = base(&pool, false).await?;
    let outcome = one_open_proposal(&pool, &b).await;
    teardown(&pool, &b).await?;
    outcome
}

// ───────────────────────── 4. 合了再撤，回到原样（E12） ─────────────────────────

/// 一对有血有肉的重复项，两边都有类型（E12 比的是有类型的实体：没类型的存活方会留下
/// 它从被并方拿来的类型，那是现有撤回的行为，这一刀不改）：
///
/// - 两边各有名字事实，各带一条证据
/// - 两边各有一条「based in Port Ellen」的类型化事实，各有自己的证据与自己的陈述来源：
///   合并时后记的那条作废，它的证据与来源抄到存活的那条上
/// - 被并方是一条事实的主语、另一条事实的宾语：两条都搬到存活方
/// - 两边之间有一条互指的事实：合并后是自环，作废
/// - 两边的画像不同：合并时加权合起来
struct Rich {
    survivor: Uuid,
    absorbed: Uuid,
    review: Uuid,
    /// 合并会抄到存活事实上的那一条陈述来源：（先记的那条类型化事实，后记那条的陈述）。
    /// 现有的撤回不删它（INTENT C1），所以撤回之后它是 `typed_fact_sources` 里唯一多出来的行
    copied_link: (Uuid, Uuid),
}

async fn rich_pair(pool: &PgPool, b: &Base, name: &str) -> anyhow::Result<Rich> {
    let [survivor, absorbed, city, alice] = std::array::from_fn(|_| Uuid::now_v7());
    let [st_kept, st_dropped, kept, dropped, employs, works_at, cross] =
        std::array::from_fn(|_| Uuid::now_v7());
    let (kb, company, person, based_in, part_of) =
        (b.kb, b.company_type, b.person_type, b.based_in, b.part_of);
    let (c_kept, c_dropped) = (chunk(pool, kb, None).await?, chunk(pool, kb, None).await?);
    sqlx::raw_sql(&format!(
        "INSERT INTO entities (id, kb_id, type_id, canonical_name, profile_embedding, profile_n) VALUES
              ('{survivor}', '{kb}', '{company}', '{name}', '[1,0,0]'::vector, 3),
              ('{absorbed}', '{kb}', '{company}', '{name} Corp', '[0,1,0]'::vector, 1),
              ('{city}', '{kb}', '{company}', '{name} City', NULL, 0),
              ('{alice}', '{kb}', '{person}', '{name} Alice', NULL, 0);
         INSERT INTO facts (id, kb_id, subject_id, object_id, layer, phrase, confidence) VALUES
              ('{st_kept}', '{kb}', '{survivor}', '{city}', 'open', 'is based in', 0.9),
              ('{st_dropped}', '{kb}', '{absorbed}', '{city}', 'open', 'is based in', 0.9);
         INSERT INTO facts (id, kb_id, subject_id, predicate_id, object_id, confidence,
                            from_statement_id, recorded_at) VALUES
              ('{kept}', '{kb}', '{survivor}', '{based_in}', '{city}', 0.9, '{st_kept}', '2024-01-01'),
              ('{dropped}', '{kb}', '{absorbed}', '{based_in}', '{city}', 0.9, '{st_dropped}', '2024-02-01'),
              ('{employs}', '{kb}', '{absorbed}', '{part_of}', '{alice}', 0.8, NULL, '2024-03-01'),
              ('{works_at}', '{kb}', '{alice}', '{part_of}', '{absorbed}', 0.8, NULL, '2024-03-02'),
              ('{cross}', '{kb}', '{survivor}', '{part_of}', '{absorbed}', 0.7, NULL, '2024-03-03');
         INSERT INTO typed_fact_sources (fact_id, statement_id) VALUES
              ('{kept}', '{st_kept}'), ('{dropped}', '{st_dropped}');"
    ))
    .execute(pool)
    .await?;
    for (fact_id, chunk_id, quote) in [
        (kept, c_kept, "based in Port Ellen"),
        (dropped, c_dropped, "headquartered in Port Ellen"),
        (employs, c_dropped, "employs Alice"),
    ] {
        sqlx::query(
            "INSERT INTO fact_evidence (fact_id, chunk_id, quote, document_id)
             SELECT $1, $2, $3, document_id FROM chunks WHERE id = $2",
        )
        .bind(fact_id)
        .bind(chunk_id)
        .bind(quote)
        .execute(pool)
        .await?;
    }
    for (id, called, chunk_id) in [
        (survivor, name.to_string(), c_kept),
        (absorbed, format!("{name} Corp"), c_dropped),
    ] {
        utopia_store::names::record(
            pool,
            kb,
            id,
            &called,
            Some(NameSource {
                chunk_id,
                quote: &called,
            }),
            None,
        )
        .await?;
        // 消歧后缀在实体建好时就算过一遍，这里照做：之后的重算就不是第一次
        resolution::refresh_disambiguators(pool, kb, &called).await?;
    }
    let review = pair(pool, kb, absorbed, survivor).await?;
    Ok(Rich {
        survivor,
        absorbed,
        review,
        copied_link: (kept, st_dropped),
    })
}

async fn merged_then_reverted(pool: &PgPool, b: &Base) -> anyhow::Result<()> {
    for manual in [false, true] {
        let name = if manual { "Orion" } else { "Acme" };
        let rich = rich_pair(pool, b, name).await?;
        let before = state(pool, b.kb).await?;
        assert!(before.iter().all(|part| !part.is_empty()));
        let rows_before = ledger(pool, b.kb).await?.len();
        // 队列合并的方向跟人点合并时一样（事实多的一侧活下来）；手动合并照调用方说的办
        let expected = if manual {
            (rich.absorbed, rich.survivor)
        } else {
            let (target, source) =
                resolution::merge_direction(pool, rich.absorbed, rich.survivor).await?;
            (source, target)
        };

        let merged = if manual {
            token_agent_actions::merge_entity_pair(pool, &write(b), rich.absorbed, rich.survivor)
                .await?
        } else {
            token_agent_actions::merge_duplicate_pair(pool, &write(b), rich.review).await?
        };
        let operation = if manual {
            "merge.manual"
        } else {
            "review.merge"
        };
        let merge_id = applied(&merged, operation);
        assert_eq!(merged.follow_up_error, None);
        let (source, target): (Uuid, Uuid) =
            sqlx::query_as("SELECT source_id, target_id FROM entity_merges WHERE id = $1")
                .bind(merge_id)
                .fetch_one(pool)
                .await?;
        assert_eq!((source, target), expected);
        // 合并真的动了每一部分：否则下面的「回到原样」什么都证明不了
        let during = state(pool, b.kb).await?;
        for (part, (was, is)) in STATE_PARTS.iter().zip(before.iter().zip(&during)) {
            assert_ne!(
                was, is,
                "the merge did not change {part}: the seed is too thin"
            );
        }

        // 令牌撤回令牌 agent 的合并：直接撤
        let reverted =
            token_agent_actions::revert_merge(pool, &because(b, "not the same"), merge_id).await?;
        assert!(!reverted.existing);
        assert_eq!(reverted.follow_up_error, None);
        let row = &reverted.action;
        assert_eq!(row.outcome, "applied");
        assert_eq!(row.operation, "merge.revert");
        assert_eq!(row.target_kind, "merge");
        assert_eq!(row.target_id, merge_id);
        assert_eq!(
            row.merge_id,
            Some(merge_id),
            "a revert row names the merge it targets"
        );
        assert_eq!(row.gate_reason, None);
        assert_eq!(row.state, None, "the revert row itself has no later state");
        assert_eq!(
            stored(pool, row.id).await?,
            (Some(merge_id.to_string()), None, None)
        );
        assert_eq!(row.payload["merge_id"], json!(merge_id));
        assert_eq!(row.payload["why"], json!("not the same"));

        // E12（INTENT C1）：facts、entities（不含 updated_at）、fact_evidence 逐行相同；
        // typed_fact_sources 原有的行一行不少，多出来的只有合并抄过去的那一条
        let after = state(pool, b.kb).await?;
        assert_restored(
            &before,
            &after,
            rich.copied_link,
            "after a token merge and its token revert",
        );

        // 合并的那一行读作 undone，时刻是撤回的时刻——库里没有为此写任何东西
        let rows = ledger(pool, b.kb).await?;
        assert_eq!(rows.len(), rows_before + 2);
        let merge_row = &rows[rows_before];
        assert_eq!(merge_row.id, merged.action.id);
        assert_eq!(merge_row.state.as_deref(), Some("undone"));
        let reverted_at: Option<chrono::DateTime<chrono::Utc>> =
            sqlx::query_scalar("SELECT reverted_at FROM entity_merges WHERE id = $1")
                .bind(merge_id)
                .fetch_one(pool)
                .await?;
        assert!(reverted_at.is_some());
        assert_eq!(merge_row.state_at, reverted_at);
        assert_eq!(
            stored(pool, merge_row.id).await?.1,
            None,
            "`undone` is read, never stored"
        );
        let history = token_agent_actions::agent_history(pool, b.kb, 50, 0).await?;
        let shown = history
            .iter()
            .find(|h| h["id"] == json!(merge_row.id))
            .expect("the merge is in Agent History");
        assert_eq!(shown["state"], json!("undone"));
        assert!(shown.get("state_at").is_some());

        // 审计行：merge.revert，对着合并，带令牌——所以不是人的撤回先例
        let audit = audits(pool, b.kb, "merge.revert").await?;
        let last = audit.last().expect("a merge.revert audit row");
        let called = |id: Uuid| {
            if id == rich.survivor {
                name.to_string()
            } else {
                format!("{name} Corp")
            }
        };
        let (source_name, target_name) = (called(expected.0), called(expected.1));
        assert_eq!(
            (last.0, last.1.as_str(), last.2),
            (Some(b.user), "merge", Some(merge_id))
        );
        assert_eq!(
            last.3,
            json!({
                "source": source_name, "target": target_name, "why": "not the same",
                "token_id": b.token, "token_name": TOKEN_NAME,
                "action_id": row.id, "merge_id": merge_id,
            })
        );

        // 第二次撤回：already_decided，什么都不写
        let second = token_agent_actions::revert_merge(pool, &write(b), merge_id).await;
        assert!(is_already_decided(&second), "{second:?}");
        assert_eq!(ledger(pool, b.kb).await?.len(), rows_before + 2);
        assert_eq!(audits(pool, b.kb, "merge.revert").await?.len(), audit.len());
        assert_restored(
            &before,
            &state(pool, b.kb).await?,
            rich.copied_link,
            "after a second revert",
        );
    }
    Ok(())
}

#[tokio::test]
async fn a_token_merge_then_its_revert_returns_facts_entities_and_evidence_to_their_state(
) -> anyhow::Result<()> {
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let _serial = SERIAL.lock().await;
    let pool = PgPool::connect(&url).await?;
    let b = base(&pool, false).await?;
    let outcome = merged_then_reverted(&pool, &b).await;
    teardown(&pool, &b).await?;
    outcome
}

// ───────────────────────── 人或治理 agent 的合并，令牌只能提议撤回 ─────────────────────────

async fn not_the_tokens_to_revert(pool: &PgPool, b: &Base, other: &Base) -> anyhow::Result<()> {
    let company = |name: &'static str| entity(pool, b.kb, b.company_type, name);
    let city = company("Port Ellen").await?;
    let mut made = Vec::new();
    for (name, by) in [("Acme", Some(b.user)), ("Orion", None)] {
        let s = entity(pool, b.kb, b.company_type, &format!("{name} Corp")).await?;
        let t = entity(pool, b.kb, b.company_type, name).await?;
        fact(pool, b.kb, s, Some(b.based_in), city).await?;
        made.push(resolution::merge_entities(pool, b.kb, s, t, by, "test").await?);
    }
    let (by_person, by_governor) = (made[0], made[1]);

    for (merge_id, reason) in [
        (by_person, "merge_by_person"),
        (by_governor, "merge_by_governor"),
    ] {
        let before = graph(pool, b.kb).await?;
        let out = token_agent_actions::revert_merge(pool, &because(b, "undo it"), merge_id).await?;
        assert_held(
            pool,
            b,
            &out,
            ("merge.revert", "merge", merge_id, &merge_id.to_string()),
            ("proposed", reason),
        )
        .await?;
        assert_eq!(
            out.action.merge_id,
            Some(merge_id),
            "the proposal names the merge"
        );
        assert_eq!(out.action.payload["merge_id"], json!(merge_id));
        assert_eq!(out.action.payload["why"], json!("undo it"));
        // 什么都没撤：图一行不动，合并还活着
        assert_same(&before, &graph(pool, b.kb).await?, reason);
        // 重复：拿回同一条
        let again = token_agent_actions::revert_merge(pool, &write(b), merge_id).await?;
        assert!(again.existing);
        assert_eq!(again.action.id, out.action.id);
    }
    assert!(merges(pool, b.kb).await?.iter().all(|m| !m.5));
    assert!(audits(pool, b.kb, "merge.revert").await?.is_empty());
    assert_eq!(ledger(pool, b.kb).await?.len(), 2);

    // 熔断器跳着的令牌：令牌 agent 自己的合并也只能提议撤回
    let s = company("Vega Inc.").await?;
    let t = company("Vega").await?;
    let own = token_agent_actions::merge_entity_pair(pool, &write(b), s, t).await?;
    let own_id = applied(&own, "merge.manual");
    let tripped = TokenWrite {
        breaker: true,
        ..write(b)
    };
    let before = graph(pool, b.kb).await?;
    let out = token_agent_actions::revert_merge(pool, &tripped, own_id).await?;
    assert_held(
        pool,
        b,
        &out,
        ("merge.revert", "merge", own_id, &own_id.to_string()),
        ("proposed", "breaker"),
    )
    .await?;
    assert_same(&before, &graph(pool, b.kb).await?, "breaker");
    // 熔断器跳着时，人的合并的理由也是 breaker——已有那条开着的提议照旧拿回来
    let again = token_agent_actions::revert_merge(pool, &tripped, by_person).await?;
    assert!(again.existing);
    assert_eq!(again.action.gate_reason.as_deref(), Some("merge_by_person"));

    // 不存在的合并、别的库的合并：NotFound
    let s = entity(pool, other.kb, other.company_type, "Lyra Corp").await?;
    let t = entity(pool, other.kb, other.company_type, "Lyra").await?;
    let foreign = token_agent_actions::merge_entity_pair(pool, &write(other), s, t).await?;
    let foreign_id = applied(&foreign, "merge.manual");
    for id in [Uuid::now_v7(), foreign_id] {
        let missing = token_agent_actions::revert_merge(pool, &write(b), id).await;
        assert!(matches!(missing, Err(AppError::NotFound)), "{missing:?}");
    }
    assert!(!merges(pool, other.kb).await?[0].5);

    // 另一枚令牌做的合并：哪一枚令牌都能撤（E14）——这里是另一个库里主人自己的令牌，
    // 换成本库的另一枚也是同一条路：认的是台账，不是哪一枚
    let (second, _plain) =
        utopia_store::tokens::issue(pool, b.user, "second agent", "write", None, None).await?;
    let by_second = TokenWrite {
        token_id: second.id,
        ..write(b)
    };
    let s = company("Hydra Co").await?;
    let t = company("Hydra").await?;
    let theirs = token_agent_actions::merge_entity_pair(pool, &by_second, s, t).await?;
    let theirs_id = applied(&theirs, "merge.manual");
    let out = token_agent_actions::revert_merge(pool, &write(b), theirs_id).await?;
    assert_eq!(out.action.outcome, "applied");
    assert_eq!(out.action.token_id, b.token);

    // 人照旧从合并历史撤回令牌 agent 的合并（今天那条不变的路）：台账行读作 undone，
    // 令牌再来撤是 already_decided（E16）
    resolution::revert_merge(pool, b.kb, own_id).await?;
    let row = ledger(pool, b.kb)
        .await?
        .into_iter()
        .find(|r| r.id == own.action.id)
        .expect("the token merge is in the ledger");
    assert_eq!(row.state.as_deref(), Some("undone"));
    assert!(row.state_at.is_some());
    let late = token_agent_actions::revert_merge(pool, &write(b), own_id).await;
    assert!(is_already_decided(&late), "{late:?}");
    Ok(())
}

#[tokio::test]
async fn a_token_revert_of_a_persons_or_the_governors_merge_lands_a_proposal_and_reverts_nothing(
) -> anyhow::Result<()> {
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let _serial = SERIAL.lock().await;
    let pool = PgPool::connect(&url).await?;
    let b = base(&pool, false).await?;
    let other = base(&pool, false).await?;
    let outcome = not_the_tokens_to_revert(&pool, &b, &other).await;
    teardown(&pool, &b).await?;
    teardown(&pool, &other).await?;
    outcome
}

// ───────────────────────── 竞争：正好合一次 ─────────────────────────

/// 这两个实体之间有几次合并（哪个方向都算）
async fn merges_between(pool: &PgPool, a: Uuid, c: Uuid) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "SELECT count(*) FROM entity_merges
          WHERE (source_id = $1 AND target_id = $2) OR (source_id = $2 AND target_id = $1)",
    )
    .bind(a)
    .bind(c)
    .fetch_one(pool)
    .await?)
}

async fn applied_rows(pool: &PgPool, kb: Uuid) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "SELECT count(*) FROM token_agent_actions WHERE kb_id = $1 AND outcome = 'applied'",
    )
    .bind(kb)
    .fetch_one(pool)
    .await?)
}

async fn two_callers(pool: &PgPool, b: &Base) -> anyhow::Result<()> {
    let city = entity(pool, b.kb, b.company_type, "Port Ellen").await?;
    let w = write(b);
    let mut expected_rows = 0;
    for round in 0..6 {
        let fresh = |kind: &str| {
            let name = format!("{kind} {round}");
            async move {
                let a = entity(pool, b.kb, b.company_type, &name).await?;
                let c = entity(pool, b.kb, b.company_type, &format!("{name} Corp")).await?;
                fact(pool, b.kb, a, Some(b.based_in), city).await?;
                fact(pool, b.kb, c, Some(b.part_of), city).await?;
                anyhow::Ok((a, c, pair(pool, b.kb, a, c).await?))
            }
        };

        // 两个令牌调用合队列里的同一对
        let (a, c, review) = fresh("Acme").await?;
        let (one, two) = tokio::join!(
            token_agent_actions::merge_duplicate_pair(pool, &w, review),
            token_agent_actions::merge_duplicate_pair(pool, &w, review)
        );
        assert_eq!(
            [one.is_ok(), two.is_ok()].iter().filter(|ok| **ok).count(),
            1,
            "exactly one of two simultaneous token merges is applied: {one:?} / {two:?}"
        );
        assert!(is_already_decided(&one) || is_already_decided(&two));
        assert_eq!(merges_between(pool, a, c).await?, 1);
        expected_rows += 1;
        assert_eq!(applied_rows(pool, b.kb).await?, expected_rows);

        // 人（照会话路由调 `decide_review`）与令牌合队列里的同一对
        let (a, c, review) = fresh("Orion").await?;
        let (person, token) = tokio::join!(
            resolution::decide_review(pool, b.kb, review, "merge", b.user, None),
            token_agent_actions::merge_duplicate_pair(pool, &w, review)
        );
        assert_eq!(
            merges_between(pool, a, c).await?,
            1,
            "exactly one merge exists: person {person:?}, token {token:?}"
        );
        match &token {
            Ok(out) => {
                applied(out, "review.merge");
                expected_rows += 1;
            }
            Err(_) => assert!(
                is_already_decided(&token),
                "a losing token call gets already_decided, got {token:?}"
            ),
        }
        assert_eq!(applied_rows(pool, b.kb).await?, expected_rows);
        assert_eq!(review_row(pool, review).await?.0, "merged");

        // 人（照会话路由调 `merge_entities`）与令牌手动合同一对，方向相反
        let (a, c, _review) = fresh("Vega").await?;
        let (person, token) = tokio::join!(
            resolution::merge_entities(pool, b.kb, a, c, Some(b.user), "manual merge"),
            token_agent_actions::merge_entity_pair(pool, &w, c, a)
        );
        assert_eq!(
            merges_between(pool, a, c).await?,
            1,
            "exactly one merge exists: person {person:?}, token {token:?}"
        );
        match &token {
            Ok(out) => {
                applied(out, "merge.manual");
                expected_rows += 1;
                assert!(person.is_err(), "the person lost and was told so");
            }
            Err(_) => {
                assert!(
                    is_already_decided(&token),
                    "a losing token call gets already_decided, got {token:?}"
                );
                assert!(person.is_ok());
            }
        }
        assert_eq!(applied_rows(pool, b.kb).await?, expected_rows);

        // 两个令牌调用手动合同一对，方向相反
        let (a, c, _review) = fresh("Hydra").await?;
        let (one, two) = tokio::join!(
            token_agent_actions::merge_entity_pair(pool, &w, a, c),
            token_agent_actions::merge_entity_pair(pool, &w, c, a)
        );
        assert_eq!(
            [one.is_ok(), two.is_ok()].iter().filter(|ok| **ok).count(),
            1,
            "{one:?} / {two:?}"
        );
        assert!(is_already_decided(&one) || is_already_decided(&two));
        assert_eq!(merges_between(pool, a, c).await?, 1);
        expected_rows += 1;
        assert_eq!(applied_rows(pool, b.kb).await?, expected_rows);
    }
    // 每一行落地的台账行都有它的审计行，一行不多
    let audited: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events
          WHERE kb_id = $1 AND action IN ('review.merge', 'merge.manual')
            AND (detail -> 'token_id') IS NOT NULL",
    )
    .bind(b.kb)
    .fetch_one(pool)
    .await?;
    assert_eq!(audited, expected_rows);

    // 人先到（不是同时）：人照旧得到从前的回答，令牌得到 already_decided
    let a = entity(pool, b.kb, b.company_type, "Lyra").await?;
    let c = entity(pool, b.kb, b.company_type, "Lyra Corp").await?;
    let review = pair(pool, b.kb, a, c).await?;
    resolution::decide_review(pool, b.kb, review, "merge", b.user, Some("same")).await?;
    assert_eq!(
        review_row(pool, review).await?,
        (
            "merged".to_string(),
            Some(b.user),
            None,
            Some("same".to_string())
        )
    );
    let late = token_agent_actions::merge_duplicate_pair(pool, &w, review).await;
    assert!(is_already_decided(&late), "{late:?}");
    let late = token_agent_actions::merge_entity_pair(pool, &w, a, c).await;
    assert!(is_already_decided(&late), "{late:?}");
    // 人再合一次已经并走的实体：从前的那一句 Conflict，一个字不差
    let again = resolution::merge_entities(pool, b.kb, a, c, Some(b.user), "x").await;
    match again {
        Err(AppError::Conflict(says)) => assert_eq!(says, "Entity already merged"),
        other => panic!("expected the person's Conflict, got {other:?}"),
    }
    assert_eq!(merges_between(pool, a, c).await?, 1);
    Ok(())
}

#[tokio::test]
async fn of_two_callers_merging_one_pair_one_is_applied_and_the_token_that_loses_gets_already_decided(
) -> anyhow::Result<()> {
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let _serial = SERIAL.lock().await;
    let pool = PgPool::connect(&url).await?;
    let b = base(&pool, false).await?;
    let outcome = two_callers(&pool, &b).await;
    teardown(&pool, &b).await?;
    outcome
}

// ───────────────────────── 5. 令牌的合并与撤回不是人的先例（E25） ─────────────────────────

fn proposal(run_id: Uuid, target_id: Uuid, action: &'static str) -> NewDecision<'static> {
    NewDecision {
        run_id,
        target_id,
        action,
        confidence: 0.7,
        reason: None,
        precedents: json!([]),
        status: "proposed",
        merge_id: None,
        question: None,
        trace: json!([]),
        calls: 0,
    }
}

/// 治理 agent 对一对会读到的全部「人的历史」，拼成一个值，前后好比较：先例三族与类型对的
/// 习惯（`precedents_for`，里面就是 `type_pair_stats`）、台账检索、「人裁过没有」
/// （`decided_before`，两个方向）、「人已经定了」（`settled_by_people`）、闸门（`gate`）
/// 在两条线之间与之上的两个判决
async fn history(pool: &PgPool, kb: Uuid, it: &ReviewItem) -> anyhow::Result<Value> {
    let p = governance::precedents_for(pool, kb, it).await?;
    let searched = governance::ledger_search(pool, kb, &it.left.name, 50).await?;
    let prior = governance::decided_before(pool, kb, it.left.id, it.right.id).await?;
    let prior_swapped = governance::decided_before(pool, kb, it.right.id, it.left.id).await?;
    let shape = governance::name_shape(&it.left.name, &it.right.name);
    let gate = |same: bool, conf: f32| {
        format!("{:?}", governance::gate(Some(same), conf, false, shape, &p))
    };
    Ok(json!({
        "precedents": governance::precedents_json(&p),
        "lines": governance::render_lines(&p),
        "type_pair": p.type_pair.as_ref().map(|t| (t.merged, t.kept, t.reverted)),
        "supports_merge": p.supports(true),
        "supports_keep": p.supports(false),
        "ledger_search": searched.iter().map(|x| x.event_id).collect::<Vec<_>>(),
        "decided_before": prior,
        "decided_before_swapped": prior_swapped,
        "settled_by_people": governance::settled_by_people(&it.left.name, &it.right.name, &p, prior),
        "gate_keep_0.80": gate(false, 0.8),
        "gate_merge_0.80": gate(true, 0.8),
        "gate_merge_0.99": gate(true, 0.99),
    }))
}

/// 人照会话路由撤回一次合并：`resolution::revert_merge` 之后由调用方记台账
async fn person_revert(pool: &PgPool, b: &Base, merge_id: Uuid) -> anyhow::Result<()> {
    let (source, target): (String, String) = sqlx::query_as(
        "SELECT s.canonical_name, t.canonical_name FROM entity_merges m
           JOIN entities s ON s.id = m.source_id JOIN entities t ON t.id = m.target_id
          WHERE m.id = $1",
    )
    .bind(merge_id)
    .fetch_one(pool)
    .await?;
    resolution::revert_merge(pool, b.kb, merge_id).await?;
    utopia_store::audit::record(
        pool,
        Some(b.kb),
        b.user,
        "merge.revert",
        "merge",
        Some(merge_id),
        json!({ "source": source, "target": target }),
    )
    .await?;
    Ok(())
}

async fn no_precedent(pool: &PgPool, b: &Base) -> anyhow::Result<()> {
    let company = |name: &'static str| entity(pool, b.kb, b.company_type, name);
    // 五笔人的合并垫底：类型对的习惯够数了（TYPE_PAIR_MIN），0.8 的 merge 因此够自己动手。
    // 令牌的动作只要漏进任何一处，这条线就会动
    for i in 0..governance::TYPE_PAIR_MIN {
        let l = entity(pool, b.kb, b.company_type, &format!("Habit {i}")).await?;
        let r = entity(pool, b.kb, b.company_type, &format!("Habit {i} Inc.")).await?;
        let review = pair(pool, b.kb, l, r).await?;
        resolution::decide_review(pool, b.kb, review, "merge", b.user, None).await?;
    }

    // 令牌要动的三对，加一对同名的旁观者（看「这个名字对别的名字」那一族）
    let (a1, a2) = (company("Acme").await?, company("Acme Corp").await?);
    let (o1, o2) = (company("Orion").await?, company("Orion Labs").await?);
    let (v1, v2) = (company("Vega").await?, company("Vega Inc.").await?);
    let acme_records = company("Acme Records").await?;
    let queue_pair = pair(pool, b.kb, a1, a2).await?;
    let manual_pair = pair(pool, b.kb, o1, o2).await?;
    let person_reverts = pair(pool, b.kb, v1, v2).await?;
    let bystander = pair(pool, b.kb, acme_records, a2).await?;
    let mut items = Vec::new();
    for id in [queue_pair, manual_pair, person_reverts, bystander] {
        items.push(item(pool, b.kb, id).await?);
    }
    // 两条治理 agent 开着的建议：令牌的动作不回答它们、不作废它们
    let run_id = Uuid::now_v7();
    let on_pair = governance::record(pool, b.kb, proposal(run_id, queue_pair, "keep")).await?;
    let on_bystander = governance::record(pool, b.kb, proposal(run_id, bystander, "keep")).await?;

    let mut before = Vec::new();
    for it in &items {
        before.push(history(pool, b.kb, it).await?);
    }
    for h in &before {
        assert_eq!(
            h["type_pair"],
            json!([5, 0, 0]),
            "five merges by people, none reverted"
        );
        assert_eq!(h["supports_merge"], json!(true));
        assert_eq!(
            h["gate_merge_0.80"],
            json!("Apply"),
            "the habit lowers the bar"
        );
        assert_eq!(h["decided_before"], Value::Null);
        assert_eq!(h["settled_by_people"], Value::Null);
        assert_eq!(h["ledger_search"], json!([]));
    }
    let unchanged = |what: &'static str| {
        let (items, before) = (&items, &before);
        async move {
            for (it, was) in items.iter().zip(before) {
                assert_eq!(
                    &history(pool, b.kb, it).await?,
                    was,
                    "{what}: {} / {} reads a different human history",
                    it.left.name,
                    it.right.name
                );
            }
            let queued: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM jobs WHERE kind = 'govern' AND payload->>'kb_id' = $1",
            )
            .bind(b.kb.to_string())
            .fetch_one(pool)
            .await?;
            assert_eq!(queued, 0, "{what}: no govern round is queued");
            for id in [on_pair, on_bystander] {
                assert_eq!(
                    governance::get(pool, b.kb, id).await?.status,
                    "proposed",
                    "{what}: the governor's open proposals stay open"
                );
            }
            anyhow::Ok(())
        }
    };

    // ── 一次活着的令牌合并（队列）：哪一处都不算 ──
    let merged = token_agent_actions::merge_duplicate_pair(
        pool,
        &because(b, "the same company"),
        queue_pair,
    )
    .await?;
    let queue_merge = applied(&merged, "review.merge");
    unchanged("a live token queue merge").await?;
    // 审核行是 merged、decided_by 是这个人——分得出来的只有 decided_token
    assert_eq!(
        review_row(pool, queue_pair).await?,
        (
            "merged".to_string(),
            Some(b.user),
            Some(b.token),
            Some("the same company".to_string())
        )
    );
    assert_eq!(governance::decided_before(pool, b.kb, a1, a2).await?, None);

    // ── 一次活着的令牌手动合并 ──
    let merged = token_agent_actions::merge_entity_pair(pool, &write(b), o1, o2).await?;
    let manual_merge = applied(&merged, "merge.manual");
    unchanged("a live token manual merge").await?;

    // ── 令牌合的、令牌撤的：仍然哪一处都不算，类型对的撤回数还是零 ──
    for merge_id in [queue_merge, manual_merge] {
        let out = token_agent_actions::revert_merge(pool, &write(b), merge_id).await?;
        assert_eq!(out.action.outcome, "applied");
        unchanged("a merge made and reverted by tokens").await?;
    }
    let p = governance::precedents_for(pool, b.kb, &items[0]).await?;
    assert!(p.reverts.is_empty() && p.same_pair.is_empty() && p.same_name.is_empty());
    assert_eq!(
        p.type_pair.as_ref().map(|t| t.reverted),
        Some(0),
        "two merges of this type pair are reverted in the base, both made and reverted by tokens"
    );
    assert_eq!(merges(pool, b.kb).await?.iter().filter(|m| m.5).count(), 2);

    // ── 人撤回令牌的合并：照任何一次撤回那样算 ──
    let merged = token_agent_actions::merge_duplicate_pair(pool, &write(b), person_reverts).await?;
    let third = applied(&merged, "review.merge");
    unchanged("a third live token merge").await?;
    person_revert(pool, b, third).await?;
    let p = governance::precedents_for(pool, b.kb, &items[2]).await?;
    assert_eq!(
        p.type_pair.as_ref().map(|t| (t.merged, t.kept, t.reverted)),
        Some((5, 0, 1)),
        "a person's revert of a token merge counts in the type-pair habit"
    );
    assert_eq!(
        p.reverts.len(),
        1,
        "and is a revert precedent for these names"
    );
    assert!(
        !p.supports(true),
        "a reverted type pair is no longer a merge habit"
    );
    assert_eq!(
        format!(
            "{:?}",
            governance::gate(
                Some(true),
                0.99,
                false,
                governance::name_shape("Vega", "Vega Inc."),
                &p
            )
        ),
        "Propose",
        "a revert precedent on either name makes the governor propose"
    );
    // 别的对上，类型对的习惯同样看得见这一次人的撤回——别的都没变
    let p = governance::precedents_for(pool, b.kb, &items[0]).await?;
    assert_eq!(p.type_pair.as_ref().map(|t| t.reverted), Some(1));
    assert!(p.reverts.is_empty());
    Ok(())
}

#[tokio::test]
async fn a_token_merge_and_a_merge_made_and_reverted_by_tokens_count_in_no_governor_precedent_read(
) -> anyhow::Result<()> {
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let _serial = SERIAL.lock().await;
    let pool = PgPool::connect(&url).await?;
    let b = base(&pool, true).await?;
    let outcome = no_precedent(&pool, &b).await;
    teardown(&pool, &b).await?;
    outcome
}
