//! 令牌 agent 的台账与它的第一个动作：把一对重复项分开（0076）。
//!
//! 令牌以发它的人的身份行事（0014 决定 1），所以审核行的 `decided_by`、台账的 actor
//! 都还是这个人。多出来的三样东西让这一笔认得出不是人亲手做的：
//!
//! - `token_agent_actions` 的一行：谁的令牌、哪一枚、当时叫什么、策略给了什么结果
//! - 审核行的 `decided_token`
//! - 台账行 detail 里的 `token_id` 与 `token_name` 快照
//!
//! **三样与审核行的状态在同一个事务里提交。** 人的裁决路径（`resolution::decide_review`
//! 加路由层的 `audit::record`）是先读后写、没有状态守卫，台账尽力而为；那条路原样不动。
//! 这里不能照抄：归属是这一次写入的一部分，不是事后补的一笔（0076 决定 8）——读不到
//! 令牌的名字、写不进台账，这一对就还是 pending。
//!
//! **令牌的决定不是人的先例**（0076 决定 9）：这里不调 `governance::answer_open`、
//! `governance::supersede_siblings`，也不排 `govern` 任务。这一对上开着的治理建议
//! 原样留着，等人答。

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use utopia_core::{AppError, AppResult};
use uuid::Uuid;

use crate::execution_gate::{agent_policy, AgentOperation, AgentOutcome};

/// 台账的一行。`owner_name` 是读的时候连 `users` 取的（人改了名字就读到新的，人没了
/// 就是 None）；`token_name` 是动作那一刻存下的快照，令牌改名、撤销、删除都不动它
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct TokenAgentAction {
    pub id: Uuid,
    pub kb_id: Uuid,
    pub operation: String,
    pub target_kind: String,
    pub target_id: Uuid,
    pub owner_id: Uuid,
    pub owner_name: Option<String>,
    pub token_id: Uuid,
    pub token_name: String,
    pub outcome: String,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

const COLS: &str = "t.id, t.kb_id, t.operation, t.target_kind, t.target_id, t.owner_id,
        u.display_name AS owner_name, t.token_id, t.token_name, t.outcome, t.payload,
        t.created_at";

/// 经令牌把一对重复项分开。一个事务：审核行、台账行、审计行，要么都在要么都不在。
///
/// - 这个库里没有这一行：`NotFound`
/// - 这一行已经不是 pending（重复调用、输了竞争、人先裁了）：`already_decided`
/// - 治理 agent 正在裁这一对：与 `resolution::decide_review` 同一句 `Conflict`。
///   那不是 `already_decided`——还什么都没定
/// - 读不到令牌的名字、写不进台账或审计：报错，什么都不提交
pub async fn keep_duplicate_pair(
    pool: &PgPool,
    kb_id: Uuid,
    review_id: Uuid,
    owner_id: Uuid,
    token_id: Uuid,
    rationale: Option<&str>,
) -> AppResult<TokenAgentAction> {
    let rationale = rationale.map(str::trim).filter(|s| !s.is_empty());
    // 裁定表在闸门那边（0076 决定 5）；分开没有闸门，所以不带 hold。表要是哪天把这一行
    // 改成了别的，这里拒绝落地，而不是照旧写一行 applied
    let outcome = agent_policy(AgentOperation::Keep, None);
    if outcome != AgentOutcome::Applied {
        return Err(AppError::Other(anyhow::anyhow!(
            "the agent policy does not finalize a keep (outcome {})",
            outcome.as_str()
        )));
    }

    // agent 正在裁的一对（0025）：与人的路径同一个检查、同一句话。它只对 pending 的行
    // 回 true，所以不会抢在 NotFound / already_decided 前面。**放在事务之前**：这个函数
    // 自己从池里取连接，在事务里调就是「拿着一条连接等另一条」，并发一高整个池互相等死。
    // 检查之后治理任务才标上 adjudicating 的那一点空隙，与人的路径一样由状态守卫兜住——
    // 它落地时这一对已经不是 pending
    if crate::governance::locked_by_agent(pool, kb_id, review_id).await? {
        return Err(AppError::Conflict(
            "The agent is deciding this pair right now; wait for its proposal or turn governance off."
                .into(),
        ));
    }

    let mut tx = pool.begin().await?;

    // 锁住这一行再看状态：两个调用同时到，后到的等前一个提交，然后读到 kept。
    // 名字与分数在这里一并取——台账与审计记的是裁决前的样子，和会话路由一样
    let row: Option<(String, String, String, f32)> = sqlx::query_as(
        "SELECT rr.status, a.canonical_name, b.canonical_name, rr.score
           FROM resolution_reviews rr
           JOIN entities a ON a.id = rr.left_id
           JOIN entities b ON b.id = rr.right_id
          WHERE rr.id = $1 AND rr.kb_id = $2
            FOR UPDATE OF rr",
    )
    .bind(review_id)
    .bind(kb_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((status, left, right, score)) = row else {
        return Err(AppError::NotFound);
    };
    if status != "pending" {
        return Err(already_decided());
    }
    // 令牌的名字：在事务里按 id 读。读不到是错误，不是空名字——没有名字的归属等于没有归属
    let token_name: Option<String> =
        sqlx::query_scalar("SELECT name FROM personal_tokens WHERE id = $1")
            .bind(token_id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some(token_name) = token_name else {
        return Err(AppError::Other(anyhow::anyhow!(
            "token {token_id} has no personal_tokens row: its name cannot be recorded"
        )));
    };

    // 状态守卫留在 UPDATE 上：上面的锁已经保证了，这一条是第二道
    let updated = sqlx::query(
        "UPDATE resolution_reviews
            SET status = 'kept', decided_at = now(), decided_by = $3, decided_token = $4,
                rationale = $5
          WHERE id = $1 AND kb_id = $2 AND status = 'pending'",
    )
    .bind(review_id)
    .bind(kb_id)
    .bind(owner_id)
    .bind(token_id)
    .bind(rationale)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if updated != 1 {
        return Err(already_decided());
    }

    let payload = serde_json::json!({
        "left": left, "right": right, "score": score, "why": rationale,
    });
    let action: TokenAgentAction = sqlx::query_as(&format!(
        "WITH t AS (
             INSERT INTO token_agent_actions
                 (id, kb_id, operation, target_kind, target_id, owner_id, token_id,
                  token_name, outcome, payload)
             VALUES ($1, $2, 'review.keep', 'review', $3, $4, $5, $6, $7, $8)
             RETURNING *)
         SELECT {COLS} FROM t LEFT JOIN users u ON u.id = t.owner_id"
    ))
    .bind(Uuid::now_v7())
    .bind(kb_id)
    .bind(review_id)
    .bind(owner_id)
    .bind(token_id)
    .bind(&token_name)
    .bind(outcome.as_str())
    .bind(&payload)
    .fetch_one(&mut *tx)
    .await?;

    // 审计行与图上的变更同一条连接、同一个事务：写不进去，上面两笔一起回滚
    crate::audit::record_on(
        &mut tx,
        Some(kb_id),
        Some(owner_id),
        "review.keep",
        "review",
        Some(review_id),
        serde_json::json!({
            "left": left, "right": right, "score": score, "why": rationale,
            "token_id": token_id, "token_name": token_name,
        }),
    )
    .await?;

    tx.commit().await?;
    Ok(action)
}

fn already_decided() -> AppError {
    AppError::CodedConflict {
        code: "already_decided",
        message: "This pair was already decided.".into(),
    }
}

/// 一个库的令牌 agent 台账，最新的在前
pub async fn list(
    pool: &PgPool,
    kb_id: Uuid,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<TokenAgentAction>> {
    let rows = sqlx::query_as(&format!(
        "SELECT {COLS} FROM token_agent_actions t LEFT JOIN users u ON u.id = t.owner_id
          WHERE t.kb_id = $1 ORDER BY t.created_at DESC, t.id DESC LIMIT $2 OFFSET $3"
    ))
    .bind(kb_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn count(pool: &PgPool, kb_id: Uuid) -> AppResult<i64> {
    let n = sqlx::query_scalar("SELECT count(*) FROM token_agent_actions WHERE kb_id = $1")
        .bind(kb_id)
        .fetch_one(pool)
        .await?;
    Ok(n)
}

/// 台账的一行在 Agent 一档里的样子（0076 决定 7）。`source: "token"` 把它和治理 agent 的
/// 行分开——那些行没有这个键。`left` / `right` / `rationale` 取自动作那一刻存下的 payload，
/// `token_name` 是存下的快照：这里不读 `personal_tokens`，令牌改名、撤销之后照旧
fn history_row(a: &TokenAgentAction) -> serde_json::Value {
    serde_json::json!({
        "source": "token",
        "id": a.id,
        "operation": a.operation,
        "target_kind": a.target_kind,
        "target_id": a.target_id,
        "owner_id": a.owner_id,
        "owner_name": a.owner_name,
        "token_id": a.token_id,
        "token_name": a.token_name,
        "outcome": a.outcome,
        "left": a.payload.get("left"),
        "right": a.payload.get("right"),
        "rationale": a.payload.get("why"),
        "created_at": a.created_at,
    })
}

/// 两张表并在一起的那个顺序：按动作时间，最新的在前。与 `governance::list`、[`list`]
/// 各自的排序是同一把钥匙，所以并起来之后每一边内部的先后都不变
const BOTH: &str = "(SELECT 'governor' AS src, id, created_at FROM agent_decisions WHERE kb_id = $1
          UNION ALL
          SELECT 'token' AS src, id, created_at FROM token_agent_actions WHERE kb_id = $1) u";

/// [`agent_history`] 翻到底一共多少行（0076：列表响应里的 `total`）：两种来源的行都数。
/// 数的是列表分页用的那同一个并集；`counts.agent`（徽标）只数开着的建议，不是这个数
pub async fn agent_history_total(pool: &PgPool, kb_id: Uuid) -> AppResult<i64> {
    Ok(sqlx::query_scalar(&format!("SELECT count(*) FROM {BOTH}"))
        .bind(kb_id)
        .fetch_one(pool)
        .await?)
}

/// Agent 一档的一页：治理 agent 的行加令牌 agent 台账的行，`limit` / `offset` 作用在
/// 并起来的序列上。
///
/// 治理 agent 的行就是 `governance::list` 序列化出来的样子，一个键不多一个键不少——
/// 所以这里不自己写它的查询，而是算出这一页里它的行在 `governance::list` 里是哪一段
/// （前面有几条、这一页有几条），原样取那一段。两边排序相同，这一段必然是连续的。
/// 台账的行按 id 取
pub async fn agent_history(
    pool: &PgPool,
    kb_id: Uuid,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<serde_json::Value>> {
    // 窗口函数先于 LIMIT / OFFSET 算：每一行都知道排在它前面的治理行有几条
    let page: Vec<(String, Uuid, i64)> = sqlx::query_as(&format!(
        "SELECT src, id,
                count(*) FILTER (WHERE src = 'governor') OVER (
                    ORDER BY created_at DESC, id DESC
                    ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING) AS governors_before
           FROM {BOTH}
          ORDER BY created_at DESC, id DESC LIMIT $2 OFFSET $3"
    ))
    .bind(kb_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    let Some(governors_before) = page.first().map(|row| row.2) else {
        return Ok(Vec::new());
    };

    let governors = page.iter().filter(|row| row.0 == "governor").count() as i64;
    let mut governor_rows = std::collections::HashMap::new();
    if governors > 0 {
        for d in crate::governance::list(pool, kb_id, governors, governors_before).await? {
            governor_rows.insert(d.id, serde_json::json!(d));
        }
    }
    let token_ids: Vec<Uuid> = page
        .iter()
        .filter(|row| row.0 == "token")
        .map(|row| row.1)
        .collect();
    let mut token_rows = std::collections::HashMap::new();
    if !token_ids.is_empty() {
        let actions: Vec<TokenAgentAction> = sqlx::query_as(&format!(
            "SELECT {COLS} FROM token_agent_actions t LEFT JOIN users u ON u.id = t.owner_id
              WHERE t.kb_id = $1 AND t.id = ANY($2)"
        ))
        .bind(kb_id)
        .bind(&token_ids)
        .fetch_all(pool)
        .await?;
        for a in &actions {
            token_rows.insert(a.id, history_row(a));
        }
    }

    let mut out = Vec::with_capacity(page.len());
    for (src, id, _) in &page {
        let row = if src == "governor" {
            match governor_rows.remove(id) {
                Some(row) => Some(row),
                // 两次读之间治理 agent 又写了一行，那一段就错开一格：这一行按 id 补取，
                // 仍是 `governance` 自己序列化的样子
                None => match crate::governance::get(pool, kb_id, *id).await {
                    Ok(d) => Some(serde_json::json!(d)),
                    Err(AppError::NotFound) => None,
                    Err(e) => return Err(e),
                },
            }
        } else {
            token_rows.remove(id)
        };
        out.extend(row);
    }
    Ok(out)
}
