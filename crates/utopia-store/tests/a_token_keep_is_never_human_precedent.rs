//! 令牌 agent 把一对重复项分开（0076），打在真库上。
//!
//! 要钉住的行为：
//!
//! - **三笔一起落**：审核行（kept、decided_by 是令牌的主人、decided_token 是那枚令牌）、
//!   `token_agent_actions` 的一行、`review.keep` 的审计行（detail 带 token_id 与
//!   token_name 快照）。读不到令牌的名字，三笔都不落，这一对还是 pending
//! - **只定一次**：重复调用、输了竞争、人先裁了，都答 `already_decided`，什么都不写；
//!   别的库的 id 是 `NotFound`；治理 agent 正在裁的一对照旧拒绝
//! - **令牌的决定不是人的先例**（SC9）：先例、台账检索、「这一对人裁过没有」、
//!   「人已经定了」、闸门的线、类型对的习惯，令牌分开一对之前和之后一模一样；
//!   不排 govern 任务，这一对和同名那一对上开着的治理建议原样留着
//! - **人的决定照旧算**（E19）：同一个库里人亲手分开的一对仍是先例，令牌分开的那一对
//!   在哪一族先例里都不出现——哪怕令牌的主人就是这个人
//! - **裁定表**：19 行与所有者的裁定一致
//! - **迁移 0128 可重放**：把它的 SQL 原样再跑一遍，成功，结构一行不变
//!
//! 这个测试自成一个二进制（为什么见 `store/main.rs`）：重放迁移碰的是**全库**的结构，
//! 不属于任何一个 kb——`ALTER TABLE resolution_reviews` 要整张表的排他锁，和合并二进制里
//! 同时在写审核行的测试会互相等。本文件里的测试也因此排成一队（`SERIAL`）：重放拿着
//! 台账表的锁等审核表，分开一对拿着审核行等台账表，两边同时跑就是死锁。
//!
//! 没有 `UTOPIA_DATABASE_URL` 时跳过而不是失败。自建自拆，绝不碰已有的库。
//! 审计台账只增不删，种下的几行留在库里（kb 已删，kb_id 悬空，本就允许）。

use serde_json::{json, Value};
use sqlx::PgPool;
use utopia_core::models::ReviewItem;
use utopia_core::AppError;
use utopia_store::execution_gate::{
    agent_policy, agent_ruling, AgentOperation, AgentOutcome, AgentRuling, Hold,
};
use utopia_store::governance::{self, Gate, NameShape, NewDecision};
use utopia_store::token_agent_actions::{self, TokenAgentAction};
use uuid::Uuid;

const MIGRATION: &str =
    include_str!("../../../migrations/0128_a_token_agent_action_is_kept_in_its_own_ledger.sql");

/// 本文件里连库的测试一个一个跑。每个 `#[tokio::test]` 有自己的运行时，
/// tokio 的 Mutex 不绑运行时，跨得过去
static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct Base {
    kb: Uuid,
    org: Uuid,
    /// 既亲手裁决、又是令牌的主人：最难分的情形——台账的 actor、审核行的 decided_by
    /// 两头都是同一个人，分得出来的只有令牌的标记
    user: Uuid,
    token: Uuid,
    person_type: Uuid,
    company_type: Uuid,
}

const TOKEN_NAME: &str = "cleanup agent";

async fn base(pool: &PgPool, governance: bool) -> anyhow::Result<Base> {
    let (org, ws, kb, user) = (
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
    );
    let (person_type, company_type) = (Uuid::now_v7(), Uuid::now_v7());
    sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, 'token-keep-test')")
        .bind(org)
        .execute(pool)
        .await?;
    sqlx::query(
        "INSERT INTO users (id, org_id, email, display_name, password_hash)
         VALUES ($1, $2, $3, 'Token Owner', 'x')",
    )
    .bind(user)
    .bind(org)
    .bind(format!("token-keep-{}@test.local", user.simple()))
    .execute(pool)
    .await?;
    sqlx::query("INSERT INTO workspaces (id, org_id, name) VALUES ($1, $2, 'token-keep-test')")
        .bind(ws)
        .bind(org)
        .execute(pool)
        .await?;
    sqlx::query(
        "INSERT INTO knowledge_bases (id, workspace_id, name, governance)
         VALUES ($1, $2, 'token-keep-test', $3)",
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
    let (view, _plain) =
        utopia_store::tokens::issue(pool, user, TOKEN_NAME, "write", None, None).await?;
    Ok(Base {
        kb,
        org,
        user,
        token: view.id,
        person_type,
        company_type,
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

async fn ledger_rows(pool: &PgPool, review: Uuid) -> anyhow::Result<Vec<(String, String)>> {
    Ok(sqlx::query_as(
        "SELECT operation, outcome FROM token_agent_actions
          WHERE target_kind = 'review' AND target_id = $1",
    )
    .bind(review)
    .fetch_all(pool)
    .await?)
}

/// 这一对上的 `review.keep` 审计行：actor 与 detail
async fn keep_audits(pool: &PgPool, review: Uuid) -> anyhow::Result<Vec<(Option<Uuid>, Value)>> {
    Ok(sqlx::query_as(
        "SELECT actor_id, detail FROM audit_events
          WHERE action = 'review.keep' AND target_kind = 'review' AND target_id = $1",
    )
    .bind(review)
    .fetch_all(pool)
    .await?)
}

async fn token_keep(
    pool: &PgPool,
    b: &Base,
    review: Uuid,
    why: Option<&str>,
) -> Result<TokenAgentAction, AppError> {
    token_agent_actions::keep_duplicate_pair(pool, b.kb, review, b.user, b.token, why).await
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

/// 人亲手分开一对，照会话路由的做法：`decide_review` 之后由调用方记台账
async fn person_keep(pool: &PgPool, b: &Base, it: &ReviewItem, why: &str) -> anyhow::Result<()> {
    utopia_store::resolution::decide_review(pool, b.kb, it.id, "keep", b.user, Some(why)).await?;
    utopia_store::audit::record(
        pool,
        Some(b.kb),
        b.user,
        "review.keep",
        "review",
        Some(it.id),
        json!({ "left": it.left.name, "right": it.right.name, "score": it.score, "why": why }),
    )
    .await?;
    Ok(())
}

/// 治理 agent 对一对会读到的全部「人的历史」，拼成一个值，前后好比较。
/// 闸门取两个固定的判决与把握：0.8 在两条线（0.75 / 0.85）之间，有没有先例支持
/// 正好决定它是动手还是建议；0.99 的 same 则会被一条「人分开过」的先例拦成建议
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
        "ledger_search": searched.iter().map(|x| x.event_id).collect::<Vec<_>>(),
        "decided_before": prior,
        "decided_before_swapped": prior_swapped,
        "settled_by_people": governance::settled_by_people(&it.left.name, &it.right.name, &p, prior),
        "gate_keep_0.80": gate(false, 0.8),
        "gate_merge_0.99": gate(true, 0.99),
    }))
}

// ───────────────────────── 迁移 0128 可重放 ─────────────────────────

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

/// 一张表的列，按位置：`名字 类型[ NOT NULL]`
async fn columns(pool: &PgPool, table: &str) -> anyhow::Result<Vec<String>> {
    Ok(sqlx::query_scalar(
        "SELECT a.attname || ' ' || format_type(a.atttypid, a.atttypmod)
                || CASE WHEN a.attnotnull THEN ' NOT NULL' ELSE '' END
           FROM pg_attribute a
           JOIN pg_class c ON c.oid = a.attrelid
           JOIN pg_namespace n ON n.oid = c.relnamespace
          WHERE n.nspname = 'public' AND c.relname = $1
            AND a.attnum > 0 AND NOT a.attisdropped
          ORDER BY a.attnum",
    )
    .bind(table)
    .fetch_all(pool)
    .await?)
}

#[tokio::test]
async fn replaying_migration_0128_changes_nothing() -> anyhow::Result<()> {
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let _serial = SERIAL.lock().await;
    let pool = PgPool::connect(&url).await?;

    // 库是迁移过的（`test_db` 的约定）：0128 建的表与列已经在，形状就是约定的那样。
    // 后来的迁移（0129 起）会给这张台账加列，所以这里问的是「0128 建的每一列都在」，
    // 不是「只有这些列」
    let ledger = columns(&pool, "token_agent_actions").await?;
    for column in [
        "id uuid NOT NULL",
        "kb_id uuid NOT NULL",
        "operation text NOT NULL",
        "target_kind text NOT NULL",
        "target_id uuid NOT NULL",
        "owner_id uuid NOT NULL",
        "token_id uuid NOT NULL",
        "token_name text NOT NULL",
        "outcome text NOT NULL",
        "payload jsonb NOT NULL",
        "created_at timestamp with time zone NOT NULL",
    ] {
        assert!(
            ledger.contains(&column.to_string()),
            "token_agent_actions lacks the 0128 column `{column}`: {ledger:#?}"
        );
    }
    assert!(
        columns(&pool, "resolution_reviews")
            .await?
            .contains(&"decided_token uuid".to_string()),
        "resolution_reviews.decided_token is a nullable uuid"
    );

    let before = schema(&pool).await?;
    assert!(
        before
            .iter()
            .any(|l| l.starts_with("table|public.token_agent_actions|")),
        "the snapshot does not see the ledger table: it would prove nothing"
    );
    // 这里跑的就是「第二遍」：改了编号的迁移在跑过旧号的库上会这样再执行一次
    sqlx::raw_sql(MIGRATION).execute(&pool).await?;
    sqlx::raw_sql(MIGRATION).execute(&pool).await?;
    let after = schema(&pool).await?;
    pool.close().await;

    let gone: Vec<&String> = before.iter().filter(|l| !after.contains(l)).collect();
    let new: Vec<&String> = after.iter().filter(|l| !before.contains(l)).collect();
    assert!(
        before == after,
        "replaying migration 0128 changed the schema\nonly before: {gone:#?}\nonly after: {new:#?}"
    );
    Ok(())
}

// ───────────────────────── 三笔一起落 ─────────────────────────

/// 这个库的实体、事实、合并日志，各拼成一段文本：分开一对不该动其中任何一行
async fn graph(pool: &PgPool, kb: Uuid) -> anyhow::Result<(String, String, String)> {
    Ok(sqlx::query_as(
        "SELECT
           (SELECT coalesce(string_agg(e::text, E'\n' ORDER BY e.id), '')
              FROM entities e WHERE e.kb_id = $1),
           (SELECT coalesce(string_agg(f::text, E'\n' ORDER BY f.id), '')
              FROM facts f WHERE f.kb_id = $1),
           (SELECT coalesce(string_agg(m::text, E'\n' ORDER BY m.id), '')
              FROM entity_merges m WHERE m.kb_id = $1)",
    )
    .bind(kb)
    .fetch_one(pool)
    .await?)
}

async fn the_three_rows(pool: &PgPool, b: &Base) -> anyhow::Result<()> {
    let zw1 = entity(pool, b.kb, b.person_type, "Zhang Wei").await?;
    let zw2 = entity(pool, b.kb, b.person_type, "Zhang Wei").await?;
    let li1 = entity(pool, b.kb, b.person_type, "Li Si").await?;
    let li2 = entity(pool, b.kb, b.person_type, "Li Si").await?;
    sqlx::query("INSERT INTO facts (id, kb_id, subject_id, object_id) VALUES ($1, $2, $3, $4)")
        .bind(Uuid::now_v7())
        .bind(b.kb)
        .bind(zw1)
        .bind(li1)
        .execute(pool)
        .await?;
    let first = pair(pool, b.kb, zw1, zw2).await?;
    let second = pair(pool, b.kb, li1, li2).await?;
    let graph_before = graph(pool, b.kb).await?;
    assert!(!graph_before.0.is_empty() && !graph_before.1.is_empty());

    let done = token_keep(pool, b, first, Some("  two different people  ")).await?;

    // 审核行
    assert_eq!(
        review_row(pool, first).await?,
        (
            "kept".to_string(),
            Some(b.user),
            Some(b.token),
            Some("two different people".to_string())
        ),
        "kept, by the owner, through the token, with the trimmed reason"
    );
    let decided_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT decided_at FROM resolution_reviews WHERE id = $1")
            .bind(first)
            .fetch_one(pool)
            .await?;
    assert!(decided_at.is_some());

    // 台账行：正好一行，结果是 applied；返回的就是它
    assert_eq!(
        ledger_rows(pool, first).await?,
        [("review.keep".to_string(), "applied".to_string())]
    );
    assert_eq!(done.kb_id, b.kb);
    assert_eq!(done.operation, "review.keep");
    assert_eq!(done.target_kind, "review");
    assert_eq!(done.target_id, first);
    assert_eq!(done.owner_id, b.user);
    assert_eq!(done.owner_name.as_deref(), Some("Token Owner"));
    assert_eq!(done.token_id, b.token);
    assert_eq!(done.token_name, TOKEN_NAME);
    assert_eq!(done.outcome, "applied");
    assert_eq!(
        done.payload,
        json!({ "left": "Zhang Wei", "right": "Zhang Wei", "score": 0.5,
                "why": "two different people" })
    );

    // 审计行：正好一行，actor 是主人，detail 带着令牌的 id 与当时的名字
    let audits = keep_audits(pool, first).await?;
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].0, Some(b.user));
    assert_eq!(
        audits[0].1,
        json!({ "left": "Zhang Wei", "right": "Zhang Wei", "score": 0.5,
                "why": "two different people",
                "token_id": b.token, "token_name": TOKEN_NAME })
    );

    // 图没动：实体、事实、合并日志一行不差
    assert_eq!(graph(pool, b.kb).await?, graph_before);
    // 另一对没被碰
    assert_eq!(review_row(pool, second).await?.0, "pending");

    // 列表：最新的在前，分页照常
    let later = token_keep(pool, b, second, None).await?;
    assert_eq!(
        later.payload["why"],
        Value::Null,
        "no reason given: null, not an empty string"
    );
    assert_eq!(review_row(pool, second).await?.3, None);
    assert_eq!(token_agent_actions::count(pool, b.kb).await?, 2);
    let listed = token_agent_actions::list(pool, b.kb, 10, 0).await?;
    assert_eq!(
        listed.iter().map(|r| r.id).collect::<Vec<_>>(),
        [later.id, done.id]
    );
    let page = token_agent_actions::list(pool, b.kb, 1, 1).await?;
    assert_eq!(page.iter().map(|r| r.id).collect::<Vec<_>>(), [done.id]);
    assert_eq!(token_agent_actions::count(pool, Uuid::now_v7()).await?, 0);

    // 令牌改名、再删掉：台账行还在，读到的仍是当时的名字；审核行仍记着是令牌裁的
    sqlx::query("UPDATE personal_tokens SET name = 'renamed' WHERE id = $1")
        .bind(b.token)
        .execute(pool)
        .await?;
    let listed = token_agent_actions::list(pool, b.kb, 10, 0).await?;
    assert!(listed.iter().all(|r| r.token_name == TOKEN_NAME));
    sqlx::query("DELETE FROM personal_tokens WHERE id = $1")
        .bind(b.token)
        .execute(pool)
        .await?;
    let listed = token_agent_actions::list(pool, b.kb, 10, 0).await?;
    assert_eq!(listed.len(), 2, "the ledger outlives the token");
    assert!(listed
        .iter()
        .all(|r| r.token_id == b.token && r.token_name == TOKEN_NAME));
    assert_eq!(review_row(pool, first).await?.2, Some(b.token));
    Ok(())
}

#[tokio::test]
async fn a_token_keep_writes_the_review_row_the_ledger_row_and_the_audit_row_together(
) -> anyhow::Result<()> {
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let _serial = SERIAL.lock().await;
    let pool = PgPool::connect(&url).await?;
    let b = base(&pool, false).await?;
    let outcome = the_three_rows(&pool, &b).await;
    teardown(&pool, &b).await?;
    outcome
}

// ───────────────────────── 读不到名字，什么都不落 ─────────────────────────

async fn nothing_without_a_name(pool: &PgPool, b: &Base) -> anyhow::Result<()> {
    let a = entity(pool, b.kb, b.person_type, "Zhang Wei").await?;
    let c = entity(pool, b.kb, b.person_type, "Zhang Wei").await?;
    let review = pair(pool, b.kb, a, c).await?;

    // 一枚 `personal_tokens` 里没有的令牌：名字读不到
    let ghost = Uuid::now_v7();
    let refused =
        token_agent_actions::keep_duplicate_pair(pool, b.kb, review, b.user, ghost, Some("why"))
            .await;
    assert!(refused.is_err(), "no token row to name: the keep must fail");
    assert!(
        !is_already_decided(&refused) && !matches!(refused, Err(AppError::NotFound)),
        "a missing token is its own failure, not a decided pair or a missing review"
    );

    assert_eq!(
        review_row(pool, review).await?,
        ("pending".to_string(), None, None, None),
        "the pair is still pending and untouched"
    );
    assert!(ledger_rows(pool, review).await?.is_empty());
    assert!(keep_audits(pool, review).await?.is_empty());
    assert_eq!(token_agent_actions::count(pool, b.kb).await?, 0);

    // 这一对没被弄坏：换一枚真的令牌照样能裁
    token_keep(pool, b, review, None).await?;
    assert_eq!(review_row(pool, review).await?.0, "kept");
    Ok(())
}

#[tokio::test]
async fn a_token_keep_with_no_token_to_name_changes_nothing() -> anyhow::Result<()> {
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

// ───────────────────────── 只定一次 ─────────────────────────

async fn decided_once(pool: &PgPool, b: &Base, other: &Base) -> anyhow::Result<()> {
    let mut reviews = Vec::new();
    for name in ["Zhang Wei", "Li Si", "Wang Wu"] {
        let l = entity(pool, b.kb, b.person_type, name).await?;
        let r = entity(pool, b.kb, b.person_type, name).await?;
        reviews.push(pair(pool, b.kb, l, r).await?);
    }
    let (repeated, by_person, raced) = (reviews[0], reviews[1], reviews[2]);

    // 重复调用
    token_keep(pool, b, repeated, Some("first")).await?;
    let again = token_keep(pool, b, repeated, Some("second")).await;
    assert!(
        is_already_decided(&again),
        "a repeat answers already_decided"
    );
    assert_eq!(ledger_rows(pool, repeated).await?.len(), 1);
    assert_eq!(keep_audits(pool, repeated).await?.len(), 1);
    assert_eq!(
        review_row(pool, repeated).await?.3.as_deref(),
        Some("first"),
        "the repeat did not rewrite the reason"
    );

    // 人先裁了：令牌再来是 already_decided，那一行仍是人的决定
    utopia_store::resolution::decide_review(pool, b.kb, by_person, "keep", b.user, None).await?;
    let late = token_keep(pool, b, by_person, None).await;
    assert!(
        is_already_decided(&late),
        "a pair a person decided answers already_decided"
    );
    assert!(ledger_rows(pool, by_person).await?.is_empty());
    assert!(keep_audits(pool, by_person).await?.is_empty());
    assert_eq!(
        review_row(pool, by_person).await?,
        ("kept".to_string(), Some(b.user), None, None),
        "still the person's own decision: no token on the row"
    );

    // 两个调用同时到（E10）：正好一个落地，另一个 already_decided，台账与审计各一行
    let (one, two) = tokio::join!(
        token_keep(pool, b, raced, Some("one")),
        token_keep(pool, b, raced, Some("two"))
    );
    assert_eq!(
        [one.is_ok(), two.is_ok()].iter().filter(|ok| **ok).count(),
        1,
        "exactly one of two simultaneous keeps is applied"
    );
    assert!(is_already_decided(&one) || is_already_decided(&two));
    assert_eq!(
        ledger_rows(pool, raced).await?,
        [("review.keep".to_string(), "applied".to_string())]
    );
    assert_eq!(keep_audits(pool, raced).await?.len(), 1);

    // 别的库的 id：NotFound，那一对原样等着；根本不存在的 id 也一样
    let l = entity(pool, other.kb, other.person_type, "Zhao Liu").await?;
    let r = entity(pool, other.kb, other.person_type, "Zhao Liu").await?;
    let elsewhere = pair(pool, other.kb, l, r).await?;
    assert!(matches!(
        token_keep(pool, b, elsewhere, None).await,
        Err(AppError::NotFound)
    ));
    assert_eq!(review_row(pool, elsewhere).await?.0, "pending");
    assert!(ledger_rows(pool, elsewhere).await?.is_empty());
    assert!(keep_audits(pool, elsewhere).await?.is_empty());
    assert!(matches!(
        token_keep(pool, b, Uuid::now_v7(), None).await,
        Err(AppError::NotFound)
    ));

    assert_eq!(
        token_agent_actions::count(pool, b.kb).await?,
        2,
        "only the two applied keeps are in this base's ledger"
    );
    assert_eq!(token_agent_actions::count(pool, other.kb).await?, 0);
    Ok(())
}

#[tokio::test]
async fn a_second_keep_of_a_decided_pair_answers_already_decided() -> anyhow::Result<()> {
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let _serial = SERIAL.lock().await;
    let pool = PgPool::connect(&url).await?;
    let b = base(&pool, false).await?;
    let other = base(&pool, false).await?;
    let outcome = decided_once(&pool, &b, &other).await;
    teardown(&pool, &b).await?;
    teardown(&pool, &other).await?;
    outcome
}

async fn held_by_the_governor(pool: &PgPool, b: &Base) -> anyhow::Result<()> {
    let l = entity(pool, b.kb, b.person_type, "Zhang Wei").await?;
    let r = entity(pool, b.kb, b.person_type, "Zhang Wei").await?;
    let review = pair(pool, b.kb, l, r).await?;
    // 三样齐了才算锁：开关开着（建库时开的）、govern 任务在跑、这一对标着 adjudicating
    sqlx::query("INSERT INTO jobs (kind, payload, status) VALUES ('govern', $1, 'running')")
        .bind(json!({ "kb_id": b.kb }))
        .execute(pool)
        .await?;
    governance::lock(pool, b.kb, &[review]).await?;
    assert!(governance::locked_by_agent(pool, b.kb, review).await?);

    let refused = token_keep(pool, b, review, None).await;
    let person = utopia_store::resolution::decide_review(pool, b.kb, review, "keep", b.user, None)
        .await
        .expect_err("a person is refused too");
    match (&refused, &person) {
        (Err(AppError::Conflict(token_says)), AppError::Conflict(person_hears)) => {
            assert_eq!(token_says, person_hears, "the same refusal a person gets");
        }
        _ => panic!("expected the governor's Conflict for both, got {refused:?} and {person:?}"),
    }
    assert_eq!(review_row(pool, review).await?.0, "pending");
    assert!(ledger_rows(pool, review).await?.is_empty());
    assert!(keep_audits(pool, review).await?.is_empty());

    // 任务一停，标记只是标记：令牌照常能裁
    sqlx::query("DELETE FROM jobs WHERE kind = 'govern' AND payload->>'kb_id' = $1")
        .bind(b.kb.to_string())
        .execute(pool)
        .await?;
    token_keep(pool, b, review, None).await?;
    assert_eq!(review_row(pool, review).await?.0, "kept");
    Ok(())
}

#[tokio::test]
async fn a_pair_the_governor_is_deciding_is_refused_to_a_token_as_to_a_person() -> anyhow::Result<()>
{
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let _serial = SERIAL.lock().await;
    let pool = PgPool::connect(&url).await?;
    let b = base(&pool, true).await?;
    let outcome = held_by_the_governor(&pool, &b).await;
    teardown(&pool, &b).await?;
    outcome
}

// ───────────────────────── 令牌的决定不是人的先例（SC9） ─────────────────────────

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

async fn no_precedent(pool: &PgPool, b: &Base) -> anyhow::Result<()> {
    // 三个张伟，两对：令牌分开第一对；第二对与它同名、共用一个实体，是它的簇
    let zw1 = entity(pool, b.kb, b.person_type, "Zhang Wei").await?;
    let zw2 = entity(pool, b.kb, b.person_type, "Zhang Wei").await?;
    let zw3 = entity(pool, b.kb, b.person_type, "Zhang Wei").await?;
    let kept = pair(pool, b.kb, zw1, zw2).await?;
    let sibling = pair(pool, b.kb, zw2, zw3).await?;
    let kept_item = item(pool, b.kb, kept).await?;
    let sibling_item = item(pool, b.kb, sibling).await?;
    // 两对上各有一条治理 agent 开着的建议
    let run_id = Uuid::now_v7();
    let on_kept = governance::record(pool, b.kb, proposal(run_id, kept, "merge")).await?;
    let on_sibling = governance::record(pool, b.kb, proposal(run_id, sibling, "keep")).await?;

    let kept_before = history(pool, b.kb, &kept_item).await?;
    let sibling_before = history(pool, b.kb, &sibling_item).await?;
    // 这个库里人什么都没裁过。闸门在「没有先例」的线上：0.8 的 keep 只够建议，
    // 0.99 的 merge 够自己动手——令牌的决定要是漏进先例，这两个值都会翻
    for before in [&kept_before, &sibling_before] {
        assert_eq!(before["gate_keep_0.80"], format!("{:?}", Gate::Propose));
        assert_eq!(before["gate_merge_0.99"], format!("{:?}", Gate::Apply));
        assert_eq!(before["decided_before"], Value::Null);
        assert_eq!(before["settled_by_people"], Value::Null);
        assert_eq!(before["ledger_search"], json!([]));
    }
    let stats = |p: &governance::Precedents| {
        let t = p.type_pair.clone().expect("both sides are typed");
        (t.merged, t.kept, t.reverted)
    };
    assert_eq!(
        stats(&governance::precedents_for(pool, b.kb, &kept_item).await?),
        (0, 0, 0)
    );

    token_keep(pool, b, kept, Some("two different people")).await?;
    assert_eq!(review_row(pool, kept).await?.0, "kept");

    // 先例、台账检索、裁过没有、人定了没有、闸门：一个字都没变
    assert_eq!(
        history(pool, b.kb, &kept_item).await?,
        kept_before,
        "the kept pair reads the same human history as before the token keep"
    );
    assert_eq!(
        history(pool, b.kb, &sibling_item).await?,
        sibling_before,
        "the same-name sibling reads the same human history as before the token keep"
    );
    // 类型对的习惯没动：Person×Person 仍是零笔人的决定
    let p = governance::precedents_for(pool, b.kb, &kept_item).await?;
    assert_eq!(
        stats(&p),
        (0, 0, 0),
        "a token keep is not a habit of people"
    );
    assert!(p.same_pair.is_empty() && p.same_name.is_empty() && p.reverts.is_empty());
    // 线没有降：没有先例支持，0.8 的 keep 仍只是建议
    assert!(!p.supports(false));
    assert_eq!(
        governance::gate(Some(false), 0.8, false, NameShape::Identical, &p),
        Gate::Propose
    );
    // 没有被当成「人已经定了」
    assert_eq!(
        governance::decided_before(pool, b.kb, zw1, zw2).await?,
        None
    );
    assert_eq!(
        governance::settled_by_people("Zhang Wei", "Zhang Wei", &p, None),
        None
    );

    // 没有排 govern 任务
    let queued: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM jobs WHERE kind = 'govern' AND payload->>'kb_id' = $1",
    )
    .bind(b.kb.to_string())
    .fetch_one(pool)
    .await?;
    assert_eq!(queued, 0, "a token keep starts no govern round");

    // 两条建议都还开着：没有被回答，也没有被作废
    assert_eq!(
        governance::get(pool, b.kb, on_kept).await?.status,
        "proposed"
    );
    assert_eq!(
        governance::get(pool, b.kb, on_sibling).await?.status,
        "proposed"
    );
    assert_eq!(governance::count_proposed(pool, b.kb).await?, 2);
    Ok(())
}

#[tokio::test]
async fn a_token_keep_changes_no_precedent_no_settlement_and_queues_no_govern_round(
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

// ───────────────────────── 人的决定照旧算（E19） ─────────────────────────

async fn beside_a_person(pool: &PgPool, b: &Base) -> anyhow::Result<()> {
    // 人分开 Apple / Apple Inc.；令牌分开 Orion / Orion Labs。两对各带一个同名的旁对，
    // 用来看「这个名字对别的名字」那一族
    let apple = entity(pool, b.kb, b.company_type, "Apple").await?;
    let apple_inc = entity(pool, b.kb, b.company_type, "Apple Inc.").await?;
    let apple_records = entity(pool, b.kb, b.company_type, "Apple Records").await?;
    let orion = entity(pool, b.kb, b.company_type, "Orion").await?;
    let orion_labs = entity(pool, b.kb, b.company_type, "Orion Labs").await?;
    let orion_systems = entity(pool, b.kb, b.company_type, "Orion Systems").await?;
    let by_person = pair(pool, b.kb, apple, apple_inc).await?;
    let beside_person = pair(pool, b.kb, apple, apple_records).await?;
    let by_token = pair(pool, b.kb, orion, orion_labs).await?;
    let beside_token = pair(pool, b.kb, orion, orion_systems).await?;
    let person_item = item(pool, b.kb, by_person).await?;
    let beside_person_item = item(pool, b.kb, beside_person).await?;
    let token_item = item(pool, b.kb, by_token).await?;
    let beside_token_item = item(pool, b.kb, beside_token).await?;

    person_keep(pool, b, &person_item, "different legal entities").await?;
    // 「今天」的样子：库里还没有令牌的决定
    let today = history(pool, b.kb, &person_item).await?;
    let today_beside = history(pool, b.kb, &beside_person_item).await?;

    // 同一个人的令牌，在同一个库里分开另一对
    token_keep(pool, b, by_token, Some("different companies")).await?;

    // 人的那一笔一个字都没变
    assert_eq!(history(pool, b.kb, &person_item).await?, today);
    assert_eq!(
        history(pool, b.kb, &beside_person_item).await?,
        today_beside
    );
    // 而且确实在：同对先例一条（分开，带着理由）、按实体认裁过、名字不同所以算人已经定了
    let p = governance::precedents_for(pool, b.kb, &person_item).await?;
    assert_eq!(p.same_pair.len(), 1);
    assert!(!p.same_pair[0].merged());
    assert_eq!(
        p.same_pair[0].why.as_deref(),
        Some("different legal entities")
    );
    let prior = governance::decided_before(pool, b.kb, apple, apple_inc).await?;
    assert_eq!(prior, Some(false));
    assert_eq!(
        governance::settled_by_people("Apple", "Apple Inc.", &p, prior),
        Some(false)
    );
    assert_eq!(
        governance::settled_by_people("Apple", "Apple Inc.", &p, None),
        Some(false),
        "the name precedent alone settles it too"
    );
    // 旁对读到它：这个名字对别的名字被人分开过
    let p = governance::precedents_for(pool, b.kb, &beside_person_item).await?;
    assert_eq!(p.same_name.len(), 1);
    assert_eq!(
        governance::ledger_search(pool, b.kb, "apple", 10)
            .await?
            .len(),
        1
    );

    // 令牌的那一笔哪里都不在
    let p = governance::precedents_for(pool, b.kb, &token_item).await?;
    assert!(
        p.same_pair.is_empty(),
        "a token keep is no same-pair precedent"
    );
    assert!(p.same_name.is_empty() && p.reverts.is_empty());
    let prior = governance::decided_before(pool, b.kb, orion, orion_labs).await?;
    assert_eq!(
        prior, None,
        "a token keep is not `decided before` by people"
    );
    assert_eq!(
        governance::decided_before(pool, b.kb, orion_labs, orion).await?,
        None
    );
    assert_eq!(
        governance::settled_by_people("Orion", "Orion Labs", &p, prior),
        None
    );
    let p = governance::precedents_for(pool, b.kb, &beside_token_item).await?;
    assert!(
        p.same_name.is_empty(),
        "nor a same-name precedent for its sibling"
    );
    assert!(governance::ledger_search(pool, b.kb, "orion", 10)
        .await?
        .is_empty());
    // 类型对的习惯：Organization×Organization 只数到人的那一笔，不是两笔
    let t = p.type_pair.expect("both sides are typed");
    assert_eq!((t.merged, t.kept, t.reverted), (0, 1, 0));

    // 两行在库里确实都是 kept、decided_by 都是同一个人：分开它们的只有令牌的标记
    assert_eq!(
        review_row(pool, by_person).await?,
        (
            "kept".to_string(),
            Some(b.user),
            None,
            Some("different legal entities".to_string())
        )
    );
    assert_eq!(
        review_row(pool, by_token).await?,
        (
            "kept".to_string(),
            Some(b.user),
            Some(b.token),
            Some("different companies".to_string())
        )
    );
    Ok(())
}

#[tokio::test]
async fn a_persons_keep_still_counts_beside_a_token_keep() -> anyhow::Result<()> {
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let _serial = SERIAL.lock().await;
    let pool = PgPool::connect(&url).await?;
    let b = base(&pool, true).await?;
    let outcome = beside_a_person(&pool, &b).await;
    teardown(&pool, &b).await?;
    outcome
}

// ───────────────────────── 裁定表 ─────────────────────────

#[test]
fn the_compiled_table_holds_the_owners_ruling() {
    use AgentOperation::*;
    let finalize = [Keep, AlignmentKindWord, Mapping, ConsistencyRun];
    let behind_gate = [Merge, RevertMerge, Batch];
    let propose_only = [
        ConfirmFact,
        RejectFact,
        CloseFact,
        ResolveConflict,
        PendingNod,
        AnswerGovernor,
        AlignmentPhrase,
        AlignmentRule,
        Errata,
        OntologyProposal,
        Violation,
        Defect,
    ];
    // 19 行，一行不多一行不少，没有哪个操作出现两次
    let all: std::collections::HashSet<AgentOperation> = finalize
        .iter()
        .chain(&behind_gate)
        .chain(&propose_only)
        .copied()
        .collect();
    assert_eq!(all.len(), 19);
    assert_eq!(finalize.len() + behind_gate.len() + propose_only.len(), 19);

    let hold = Hold::Derived(3);
    for op in finalize {
        assert_eq!(agent_ruling(op), AgentRuling::Finalize, "{op:?}");
        assert_eq!(agent_policy(op, None), AgentOutcome::Applied, "{op:?}");
        // 没有闸门的行不看 hold
        assert_eq!(
            agent_policy(op, Some(&hold)),
            AgentOutcome::Applied,
            "{op:?}"
        );
    }
    for op in behind_gate {
        assert_eq!(agent_ruling(op), AgentRuling::FinalizeBehindGate, "{op:?}");
        assert_eq!(agent_policy(op, None), AgentOutcome::Applied, "{op:?}");
        assert_eq!(agent_policy(op, Some(&hold)), AgentOutcome::Held, "{op:?}");
        assert_eq!(
            agent_policy(op, Some(&Hold::Identity)),
            AgentOutcome::Held,
            "{op:?}"
        );
    }
    for op in propose_only {
        assert_eq!(agent_ruling(op), AgentRuling::ProposeOnly, "{op:?}");
        assert_eq!(agent_policy(op, None), AgentOutcome::Proposed, "{op:?}");
        assert_eq!(
            agent_policy(op, Some(&hold)),
            AgentOutcome::Proposed,
            "{op:?}"
        );
    }

    assert_eq!(AgentOutcome::Applied.as_str(), "applied");
    assert_eq!(AgentOutcome::Held.as_str(), "held");
    assert_eq!(AgentOutcome::Proposed.as_str(), "proposed");
}
