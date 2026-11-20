//! 令牌 agent 的台账与它在 Review 里的写入：分开一对、合并、撤回合并（0076）。
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
//!
//! **合并与撤回过闸门**（裁定表里 Merge、RevertMerge 是 `FinalizeBehindGate`）：
//!
//! - 合并的两个入口——[`merge_duplicate_pair`]（队列里的一对）与 [`merge_entity_pair`]
//!   （手动指定源与目标）——**自己**先问 `execution_gate::hold_for_token_merge`，再问
//!   `agent_policy`，然后才开事务。闸门写在这两个函数里面，调用方绕不过去：没有哪条
//!   令牌的路能不问闸门就走到合并
//! - 闸门放行：一个事务里做完合并（与人的合并同一段代码，`resolution::merge_entities_in`）、
//!   审核行、台账行（`applied`，`merge_id` 指着这次合并）、审计行。合并提交之后的那几步
//!   （签名检查、时态对账、消歧后缀）在事务之外；它们失败时合并已经在了，所以错误记在
//!   [`TokenWriteOutcome::follow_up_error`] 上，结果仍是 `applied`
//! - 闸门留下、或者这枚令牌的熔断器跳了（`TokenWrite::breaker`）：**什么都不动**，只在
//!   台账里留一条开着的提议（`held` / `proposed`，`gate_reason` 说为什么）加一条
//!   `agent.proposal` 审计行，等人在 Review 里答。同一个库、同一种操作、同一个目标只有
//!   一条开着的提议（0129 的部分唯一索引）：重复调用拿回已有的那一条，`existing` 为 true
//! - 撤回（[`revert_merge`]）：台账上是令牌 agent 做的合并，直接撤；人或治理 agent 做的
//!   合并，只留提议——令牌因此碰不到治理的保险丝

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use utopia_core::{AppError, AppResult};
use uuid::Uuid;

use crate::execution_gate::{
    agent_policy, hold_for_token_merge, AgentOperation, AgentOutcome, Hold,
};

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
    /// 没有直接落地的理由：闸门的（`held`）或只能提议的（`proposed`）。落地的行为空
    pub gate_reason: Option<String>,
    /// 成批调用的号；单笔为空
    pub batch_id: Option<Uuid>,
    /// 落地的合并行：它做出的合并；`merge.revert` 行：它要撤的合并；被批准的合并提议：
    /// 批准时做出的合并
    pub merge_id: Option<Uuid>,
    /// 后来怎样了。`approved` / `declined` / `superseded` 是存下的（人答提议时写）；
    /// `undone` **不存**：落地的合并行，它的合并在 `entity_merges` 里带着 `reverted_at`
    /// 时读作 `undone`——人照旧从合并历史撤回，这里一行不用多写就看得出来
    pub state: Option<String>,
    /// 答提议的时刻；`undone` 时是撤回的时刻
    pub state_at: Option<DateTime<Utc>>,
    /// 答提议的人
    pub state_by: Option<Uuid>,
}

/// 读台账的列。`t` 是台账行，`u`、`m` 是 [`JOINS`] 连上的主人与合并日志。
/// 两个 CASE 就是「`undone` 不存、读的时候算」
const COLS: &str = "t.id, t.kb_id, t.operation, t.target_kind, t.target_id, t.owner_id,
        u.display_name AS owner_name, t.token_id, t.token_name, t.outcome, t.payload,
        t.created_at, t.gate_reason, t.batch_id, t.merge_id,
        CASE WHEN t.outcome = 'applied' AND t.operation IN ('review.merge', 'merge.manual')
                  AND m.reverted_at IS NOT NULL
             THEN 'undone' ELSE t.state END AS state,
        CASE WHEN t.outcome = 'applied' AND t.operation IN ('review.merge', 'merge.manual')
                  AND m.reverted_at IS NOT NULL
             THEN m.reverted_at ELSE t.state_at END AS state_at,
        t.state_by";
const JOINS: &str = "LEFT JOIN users u ON u.id = t.owner_id
        LEFT JOIN entity_merges m ON m.id = t.merge_id";

/// 一次令牌写入的调用方：哪个库、谁的令牌、哪一枚，带来的理由，属于哪一批，
/// 以及这枚令牌的熔断器此刻是不是跳着（跳着的话合并与撤回只留提议）
#[derive(Debug, Clone)]
pub struct TokenWrite {
    pub kb_id: Uuid,
    pub owner_id: Uuid,
    pub token_id: Uuid,
    pub rationale: Option<String>,
    pub batch_id: Option<Uuid>,
    pub breaker: bool,
}

/// 一次令牌写入的结果。`action` 是台账行——它的 `outcome` 说这一笔落地了（`applied`）
/// 还是成了提议（`held` / `proposed`）
#[derive(Debug, Clone)]
pub struct TokenWriteOutcome {
    pub action: TokenAgentAction,
    /// 这次调用撞上了一条已经开着的提议：什么都没写，`action` 是那一条
    pub existing: bool,
    /// 合并（或撤回）已经提交，之后的某一步失败了。**结果仍是 `applied`**
    pub follow_up_error: Option<String>,
}

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
         SELECT {COLS} FROM t {JOINS}"
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

/// `resolution::revert_merge` 对一条已经撤回的合并答的那一句。那个函数不归这里改，
/// 所以按原话认；令牌听到的是 `already_decided`
const ALREADY_REVERTED: &str = "Merge already reverted";

/// 输了竞争的合并：[`crate::resolution::merge_entities_in`] 说一侧已经并走了，
/// 对令牌就是「这一对已经定了」。别的错误原样
fn lost_merge_race(e: AppError) -> AppError {
    match e {
        AppError::Conflict(said) if said == crate::resolution::ALREADY_MERGED => already_decided(),
        other => other,
    }
}

/// 合并的事务被 Postgres 选作死锁的牺牲品（两个方向相反的合并各锁住一个实体等另一个：
/// A 并进 B 对上 B 并进 A）或序列化失败时，整个事务已经回滚、什么都没提交，重来一遍是
/// 安全的。重来时另一方多半已经提交，这一次读到的是「已经并走了」，令牌听到的是
/// `already_decided` 而不是一句数据库错误
const RETRIES_AFTER_DEADLOCK: u8 = 2;

fn deadlocked(e: &AppError) -> bool {
    match e {
        AppError::Db(sqlx::Error::Database(db)) => {
            matches!(db.code().as_deref(), Some("40P01") | Some("40001"))
        }
        _ => false,
    }
}

fn trimmed(rationale: &Option<String>) -> Option<&str> {
    rationale
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// 令牌的名字：按 id 读。读不到是错误，不是空名字——没有名字的归属等于没有归属，
/// 所以每一条写入都先读它，读不到就什么都不写
async fn token_name_on(conn: &mut sqlx::PgConnection, token_id: Uuid) -> AppResult<String> {
    let name: Option<String> = sqlx::query_scalar("SELECT name FROM personal_tokens WHERE id = $1")
        .bind(token_id)
        .fetch_optional(conn)
        .await?;
    name.ok_or_else(|| {
        AppError::Other(anyhow::anyhow!(
            "token {token_id} has no personal_tokens row: its name cannot be recorded"
        ))
    })
}

/// 要写进台账的一行里，随操作不同的那几样
struct NewRow<'a> {
    operation: &'a str,
    target_kind: &'a str,
    target_id: Uuid,
    target_key: &'a str,
    outcome: AgentOutcome,
    gate_reason: Option<&'a str>,
    merge_id: Option<Uuid>,
    payload: &'a serde_json::Value,
}

/// 写一行台账。`None` = 没写进去：同一个库、同一种操作、同一个目标已经有一条开着的
/// 提议（0129 的部分唯一索引）。落地的行不在那个索引里，永远写得进去。
///
/// `ON CONFLICT DO NOTHING` 遇到另一个还没提交的同键插入会等它：它提交了，这里零行；
/// 它回滚了，这里照常插入。所以同时到的两个调用正好留下一行
async fn insert_row(
    conn: &mut sqlx::PgConnection,
    w: &TokenWrite,
    token_name: &str,
    row: &NewRow<'_>,
) -> AppResult<Option<TokenAgentAction>> {
    Ok(sqlx::query_as(&format!(
        "WITH t AS (
             INSERT INTO token_agent_actions
                 (id, kb_id, operation, target_kind, target_id, owner_id, token_id,
                  token_name, outcome, payload, gate_reason, batch_id, merge_id, target_key)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
             ON CONFLICT (kb_id, operation, target_key)
                   WHERE outcome IN ('held', 'proposed') AND state IS NULL
                DO NOTHING
             RETURNING *)
         SELECT {COLS} FROM t {JOINS}"
    ))
    .bind(Uuid::now_v7())
    .bind(w.kb_id)
    .bind(row.operation)
    .bind(row.target_kind)
    .bind(row.target_id)
    .bind(w.owner_id)
    .bind(w.token_id)
    .bind(token_name)
    .bind(row.outcome.as_str())
    .bind(row.payload)
    .bind(row.gate_reason)
    .bind(w.batch_id)
    .bind(row.merge_id)
    .bind(row.target_key)
    .fetch_optional(conn)
    .await?)
}

/// 令牌写入的审计 detail：操作自己的那几项，加上认得出「不是人亲手做的」的那几项——
/// `token_id`、`token_name` 快照、台账行的 id，指着合并时带 `merge_id`，成批时带 `batch_id`。
/// `token_id` 这个键同时把这一行挡在治理 agent 的先例之外（`governance::NOT_BY_TOKEN`）
fn audit_detail(
    mut detail: serde_json::Value,
    w: &TokenWrite,
    token_name: &str,
    action_id: Uuid,
    merge_id: Option<Uuid>,
) -> serde_json::Value {
    if let Some(fields) = detail.as_object_mut() {
        fields.insert("token_id".into(), serde_json::json!(w.token_id));
        fields.insert("token_name".into(), serde_json::json!(token_name));
        fields.insert("action_id".into(), serde_json::json!(action_id));
        if let Some(merge_id) = merge_id {
            fields.insert("merge_id".into(), serde_json::json!(merge_id));
        }
        if let Some(batch_id) = w.batch_id {
            fields.insert("batch_id".into(), serde_json::json!(batch_id));
        }
    }
    detail
}

/// 策略对一次合并或撤回的回答，折成「要不要留成提议、理由是什么」。`None` = 落地。
///
/// 熔断器跳着的令牌一律只留提议，理由是 `breaker`——哪怕闸门也有话说（闸门的话照样
/// 记在 payload 的 `hold` 里）。裁定表要是哪天把这一行改成了只能提议，这里拒绝，
/// 而不是替它编一个理由
fn held_back(
    op: AgentOperation,
    hold: Option<&Hold>,
    breaker: bool,
) -> AppResult<Option<(AgentOutcome, &'static str)>> {
    let outcome = agent_policy(op, hold);
    if outcome == AgentOutcome::Proposed {
        return Err(AppError::Other(anyhow::anyhow!(
            "the agent policy does not finalize {op:?} (outcome {})",
            outcome.as_str()
        )));
    }
    if breaker {
        return Ok(Some((AgentOutcome::Proposed, "breaker")));
    }
    Ok(match hold {
        Some(hold) if outcome == AgentOutcome::Held => Some((AgentOutcome::Held, hold.code())),
        _ => None,
    })
}

/// 留一条提议：一个事务里台账行加 `agent.proposal` 审计行，图上什么都不碰。
/// 已经有一条开着的，就什么都不写，把那一条拿回来（`existing`）
async fn land_proposal(
    pool: &PgPool,
    w: &TokenWrite,
    row: &NewRow<'_>,
) -> AppResult<TokenWriteOutcome> {
    // 最多三轮：「撞上了开着的提议，回头去读它时它刚好被人答掉」才会进下一轮
    for _ in 0..3 {
        let mut tx = pool.begin().await?;
        let token_name = token_name_on(&mut tx, w.token_id).await?;
        if let Some(action) = insert_row(&mut tx, w, &token_name, row).await? {
            crate::audit::record_on(
                &mut tx,
                Some(w.kb_id),
                Some(w.owner_id),
                "agent.proposal",
                "agent_action",
                Some(action.id),
                audit_detail(
                    serde_json::json!({
                        "operation": row.operation,
                        "outcome": row.outcome.as_str(),
                        "reason": row.gate_reason,
                    }),
                    w,
                    &token_name,
                    action.id,
                    row.merge_id,
                ),
            )
            .await?;
            tx.commit().await?;
            return Ok(TokenWriteOutcome {
                action,
                existing: false,
                follow_up_error: None,
            });
        }
        drop(tx);
        let open: Option<TokenAgentAction> = sqlx::query_as(&format!(
            "SELECT {COLS} FROM token_agent_actions t {JOINS}
              WHERE t.kb_id = $1 AND t.operation = $2 AND t.target_key = $3
                AND t.outcome IN ('held', 'proposed') AND t.state IS NULL"
        ))
        .bind(w.kb_id)
        .bind(row.operation)
        .bind(row.target_key)
        .fetch_optional(pool)
        .await?;
        if let Some(action) = open {
            return Ok(TokenWriteOutcome {
                action,
                existing: true,
                follow_up_error: None,
            });
        }
    }
    Err(AppError::Other(anyhow::anyhow!(
        "the open proposal for {} {} kept changing; nothing was written",
        row.operation,
        row.target_key
    )))
}

fn governor_is_deciding() -> AppError {
    AppError::Conflict(
        "The agent is deciding this pair right now; wait for its proposal or turn governance off."
            .into(),
    )
}

/// 合并提交之后的那几步。失败不是合并失败：记下来，交给调用方挂在 `applied` 的结果上
async fn follow_up(
    pool: &PgPool,
    kb_id: Uuid,
    done: &crate::resolution::MergeDone,
) -> Option<String> {
    match crate::resolution::merge_follow_up(pool, kb_id, done).await {
        Ok(()) => None,
        Err(e) => {
            tracing::warn!(%kb_id, merge_id = %done.merge_id, error = %e, "令牌合并提交之后的后续步骤失败");
            Some(e.to_string())
        }
    }
}

/// 经令牌合并队列里的一对重复项。方向与人在队列里点合并时相同（`merge_direction`）。
///
/// - 这个库里没有这一行：`NotFound`
/// - 这一行已经不是 pending，或者一侧已经并进了别人，或者输了竞争：`already_decided`
/// - 治理 agent 正在裁这一对：与 [`keep_duplicate_pair`] 同一句 `Conflict`
/// - 闸门留下、或熔断器跳着：一条提议，图不动
/// - 读不到令牌的名字、写不进台账或审计：报错，什么都不提交——合并也不提交
pub async fn merge_duplicate_pair(
    pool: &PgPool,
    w: &TokenWrite,
    review_id: Uuid,
) -> AppResult<TokenWriteOutcome> {
    let mut again = RETRIES_AFTER_DEADLOCK;
    loop {
        match merge_duplicate_pair_once(pool, w, review_id).await {
            Err(e) if again > 0 && deadlocked(&e) => again -= 1,
            answered => return answered,
        }
    }
}

async fn merge_duplicate_pair_once(
    pool: &PgPool,
    w: &TokenWrite,
    review_id: Uuid,
) -> AppResult<TokenWriteOutcome> {
    let why = trimmed(&w.rationale);
    // 名字与分数在裁决之前取：台账与审计记的是裁决前的样子
    #[allow(clippy::type_complexity)]
    let row: Option<(
        String,
        Uuid,
        Uuid,
        String,
        String,
        f32,
        Option<Uuid>,
        Option<Uuid>,
    )> = sqlx::query_as(
        "SELECT rr.status, rr.left_id, rr.right_id, a.canonical_name, b.canonical_name,
                rr.score, a.merged_into, b.merged_into
           FROM resolution_reviews rr
           JOIN entities a ON a.id = rr.left_id
           JOIN entities b ON b.id = rr.right_id
          WHERE rr.id = $1 AND rr.kb_id = $2",
    )
    .bind(review_id)
    .bind(w.kb_id)
    .fetch_optional(pool)
    .await?;
    let Some((status, left_id, right_id, left, right, score, left_gone, right_gone)) = row else {
        return Err(AppError::NotFound);
    };
    if status != "pending" {
        return Err(already_decided());
    }
    // 与分开同一个检查、同一句话，同样放在事务之前（为什么见 `keep_duplicate_pair`）
    if crate::governance::locked_by_agent(pool, w.kb_id, review_id).await? {
        return Err(governor_is_deciding());
    }
    // 一侧已经并进了别人（比如有人手动合了这两个，审核行还挂着）：这一对没有可合的了
    if left_gone.is_some() || right_gone.is_some() {
        return Err(already_decided());
    }
    let (target_id, source_id) =
        crate::resolution::merge_direction(pool, left_id, right_id).await?;

    // **先问闸门**（0076：没有不过闸门的令牌合并），再问裁定表
    let hold = hold_for_token_merge(pool, w.kb_id, left_id, right_id).await?;
    let held = held_back(AgentOperation::Merge, hold.as_ref(), w.breaker)?;

    let mut payload = serde_json::json!({
        "left": left, "right": right, "score": score, "why": why,
        "review_id": review_id, "left_id": left_id, "right_id": right_id,
        "source_id": source_id, "target_id": target_id,
    });
    if let Some(hold) = &hold {
        payload["hold"] = serde_json::json!(hold.to_string());
    }
    let target_key = review_id.to_string();
    if let Some((outcome, reason)) = held {
        return land_proposal(
            pool,
            w,
            &NewRow {
                operation: "review.merge",
                target_kind: "review",
                target_id: review_id,
                target_key: &target_key,
                outcome,
                gate_reason: Some(reason),
                merge_id: None,
                payload: &payload,
            },
        )
        .await;
    }

    // 一个事务：合并、审核行、台账行、审计行。次序与人的路径相同——先合并（时间线锁、
    // 事实、实体），再关审核行——所以不引入新的加锁次序。审核行的状态守卫在 UPDATE 上：
    // 它等行锁、按提交后的新行重算，这一对要是已经被别人裁了就是零行，整个事务回滚，
    // 合并也一起回滚
    let mut tx = pool.begin().await?;
    let token_name = token_name_on(&mut tx, w.token_id).await?;
    let done = crate::resolution::merge_entities_in(
        &mut tx,
        w.kb_id,
        source_id,
        target_id,
        Some(w.owner_id),
        why.unwrap_or("review decision"),
    )
    .await
    .map_err(lost_merge_race)?;
    let updated = sqlx::query(
        "UPDATE resolution_reviews
            SET status = 'merged', decided_at = now(), decided_by = $3, decided_token = $4,
                rationale = $5
          WHERE id = $1 AND kb_id = $2 AND status = 'pending'",
    )
    .bind(review_id)
    .bind(w.kb_id)
    .bind(w.owner_id)
    .bind(w.token_id)
    .bind(why)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if updated != 1 {
        return Err(already_decided());
    }
    let action = insert_row(
        &mut tx,
        w,
        &token_name,
        &NewRow {
            operation: "review.merge",
            target_kind: "review",
            target_id: review_id,
            target_key: &target_key,
            outcome: AgentOutcome::Applied,
            gate_reason: None,
            merge_id: Some(done.merge_id),
            payload: &payload,
        },
    )
    .await?
    .ok_or_else(|| AppError::Other(anyhow::anyhow!("an applied ledger row was not written")))?;
    crate::audit::record_on(
        &mut tx,
        Some(w.kb_id),
        Some(w.owner_id),
        "review.merge",
        "review",
        Some(review_id),
        audit_detail(
            serde_json::json!({ "left": left, "right": right, "score": score, "why": why }),
            w,
            &token_name,
            action.id,
            Some(done.merge_id),
        ),
    )
    .await?;
    tx.commit().await?;

    let follow_up_error = follow_up(pool, w.kb_id, &done).await;
    Ok(TokenWriteOutcome {
        action,
        existing: false,
        follow_up_error,
    })
}

/// 经令牌手动合并两个实体：源并进目标，方向是调用方给的。这两个实体自己要是有一条
/// 等着的审核行，一并关成 merged、记上令牌——人的手动合并不关它，这里关，免得一对
/// 已经合了的实体还挂在队列里等裁。
///
/// - 源与目标是同一个：`self_merge`（与 `merge_entities` 同一句）
/// - 哪个 id 这个库里没有：`NotFound`
/// - 一侧已经并进了别人，或者输了竞争：`already_decided`
/// - 这一对自己的审核行正被治理 agent 裁着：与 [`keep_duplicate_pair`] 同一句 `Conflict`
/// - 闸门留下、或熔断器跳着：一条提议，图不动。目标的钥匙是两个 id 按升序连起来，
///   所以 A 并进 B 与 B 并进 A 是同一条提议
pub async fn merge_entity_pair(
    pool: &PgPool,
    w: &TokenWrite,
    source: Uuid,
    target: Uuid,
) -> AppResult<TokenWriteOutcome> {
    let mut again = RETRIES_AFTER_DEADLOCK;
    loop {
        match merge_entity_pair_once(pool, w, source, target).await {
            Err(e) if again > 0 && deadlocked(&e) => again -= 1,
            answered => return answered,
        }
    }
}

async fn merge_entity_pair_once(
    pool: &PgPool,
    w: &TokenWrite,
    source: Uuid,
    target: Uuid,
) -> AppResult<TokenWriteOutcome> {
    let why = trimmed(&w.rationale);
    if source == target {
        return Err(AppError::invalid(
            "self_merge",
            "Cannot merge an entity into itself",
        ));
    }
    let sides: Vec<(Uuid, String, Option<Uuid>)> = sqlx::query_as(
        "SELECT id, canonical_name, merged_into FROM entities WHERE kb_id = $1 AND id = ANY($2)",
    )
    .bind(w.kb_id)
    .bind(vec![source, target])
    .fetch_all(pool)
    .await?;
    let side = |id: Uuid| sides.iter().find(|s| s.0 == id);
    let (Some(source_row), Some(target_row)) = (side(source), side(target)) else {
        return Err(AppError::NotFound);
    };
    if source_row.2.is_some() || target_row.2.is_some() {
        return Err(already_decided());
    }
    let (source_name, target_name) = (source_row.1.clone(), target_row.1.clone());

    // 这两个实体自己的那条等着的审核行（一对只会有一条 pending）
    let own_review: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM resolution_reviews
          WHERE kb_id = $1 AND status = 'pending'
            AND ((left_id = $2 AND right_id = $3) OR (left_id = $3 AND right_id = $2))",
    )
    .bind(w.kb_id)
    .bind(source)
    .bind(target)
    .fetch_optional(pool)
    .await?;
    if let Some(review_id) = own_review {
        if crate::governance::locked_by_agent(pool, w.kb_id, review_id).await? {
            return Err(governor_is_deciding());
        }
    }

    // **先问闸门**，再问裁定表
    let hold = hold_for_token_merge(pool, w.kb_id, source, target).await?;
    let held = held_back(AgentOperation::Merge, hold.as_ref(), w.breaker)?;

    // `left` / `right` 与 `source` / `target` 是同一对名字：Agent 一档读前两个
    let mut payload = serde_json::json!({
        "left": source_name, "right": target_name,
        "source": source_name, "target": target_name, "why": why,
        "source_id": source, "target_id": target,
    });
    if let Some(hold) = &hold {
        payload["hold"] = serde_json::json!(hold.to_string());
    }
    let target_key = format!("{}|{}", source.min(target), source.max(target));
    if let Some((outcome, reason)) = held {
        return land_proposal(
            pool,
            w,
            &NewRow {
                operation: "merge.manual",
                target_kind: "entity",
                target_id: target,
                target_key: &target_key,
                outcome,
                gate_reason: Some(reason),
                merge_id: None,
                payload: &payload,
            },
        )
        .await;
    }

    let mut tx = pool.begin().await?;
    let token_name = token_name_on(&mut tx, w.token_id).await?;
    let done = crate::resolution::merge_entities_in(
        &mut tx,
        w.kb_id,
        source,
        target,
        Some(w.owner_id),
        why.unwrap_or("manual merge"),
    )
    .await
    .map_err(lost_merge_race)?;
    // 合并在事务里把涉及源实体的别的 pending 行关掉或改指了，唯独放过这一对自己的那条
    // （「由调用方标记 merged」）：这里就是那个调用方
    let closed: Option<Uuid> = sqlx::query_scalar(
        "UPDATE resolution_reviews
            SET status = 'merged', decided_at = now(), decided_by = $4, decided_token = $5,
                rationale = $6
          WHERE kb_id = $1 AND status = 'pending'
            AND ((left_id = $2 AND right_id = $3) OR (left_id = $3 AND right_id = $2))
          RETURNING id",
    )
    .bind(w.kb_id)
    .bind(source)
    .bind(target)
    .bind(w.owner_id)
    .bind(w.token_id)
    .bind(why)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(review_id) = closed {
        payload["review_id"] = serde_json::json!(review_id);
    }
    let action = insert_row(
        &mut tx,
        w,
        &token_name,
        &NewRow {
            operation: "merge.manual",
            target_kind: "entity",
            target_id: target,
            target_key: &target_key,
            outcome: AgentOutcome::Applied,
            gate_reason: None,
            merge_id: Some(done.merge_id),
            payload: &payload,
        },
    )
    .await?
    .ok_or_else(|| AppError::Other(anyhow::anyhow!("an applied ledger row was not written")))?;
    let mut detail =
        serde_json::json!({ "source": source_name, "target": target_name, "why": why });
    if let Some(review_id) = closed {
        detail["review_id"] = serde_json::json!(review_id);
    }
    crate::audit::record_on(
        &mut tx,
        Some(w.kb_id),
        Some(w.owner_id),
        "merge.manual",
        "entity",
        Some(target),
        audit_detail(detail, w, &token_name, action.id, Some(done.merge_id)),
    )
    .await?;
    tx.commit().await?;

    let follow_up_error = follow_up(pool, w.kb_id, &done).await;
    Ok(TokenWriteOutcome {
        action,
        existing: false,
        follow_up_error,
    })
}

/// 经令牌撤回一次合并。
///
/// - 这个库里没有这次合并：`NotFound`
/// - 已经撤回了（谁撤的都算）：`already_decided`
/// - 台账上是**令牌 agent 做的**合并（哪一枚令牌都行）：直接撤，写 `merge.revert` 的
///   台账行与审计行
/// - 别的合并——人做的（包括人批准令牌的提议做出的）、治理 agent 或裁决器做的——只留
///   提议（`merge_by_person` / `merge_by_governor`）；熔断器跳着的令牌也只留提议
///   （`breaker`）。所以令牌撤不动人的合并，也碰不到治理的保险丝
///
/// **撤回与它的两行不在同一个事务里。** `resolution::revert_merge` 自己开事务、自己提交，
/// 那个函数是人的撤回路径，这一刀不改它；而拿着一个开着的事务去调它，就是「拿着一条
/// 连接等另一条」（见 [`keep_duplicate_pair`]）。所以次序是：先读令牌的名字（读不到就
/// 什么都不做），撤回，再用一个事务写台账行与审计行——写不进去就重试，三次都不行
/// 才把错误报出去，报的话里说明合并已经撤回了。
///
/// 进程要是正好死在撤回提交之后、那两行提交之前：合并是撤回了的（`reverted_at` 在），
/// 但没有 `merge.revert` 的台账行与审计行。读的人看到那次合并的台账行是 `undone`，
/// 找不到是谁撤的；照台账的认法它不算「令牌撤的」，于是类型对的习惯把它数成一次撤回
/// （治理 agent 的线只会更高，不会更低）；再来一次令牌撤回答 `already_decided`
pub async fn revert_merge(
    pool: &PgPool,
    w: &TokenWrite,
    merge_id: Uuid,
) -> AppResult<TokenWriteOutcome> {
    let why = trimmed(&w.rationale);
    #[allow(clippy::type_complexity)]
    let row: Option<(
        Uuid,
        Uuid,
        String,
        String,
        Option<Uuid>,
        Option<DateTime<Utc>>,
    )> = sqlx::query_as(
        "SELECT m.source_id, m.target_id, s.canonical_name, t.canonical_name,
                m.merged_by, m.reverted_at
           FROM entity_merges m
           JOIN entities s ON s.id = m.source_id
           JOIN entities t ON t.id = m.target_id
          WHERE m.id = $1 AND m.kb_id = $2",
    )
    .bind(merge_id)
    .bind(w.kb_id)
    .fetch_optional(pool)
    .await?;
    let Some((source_id, target_id, source_name, target_name, merged_by, reverted_at)) = row else {
        return Err(AppError::NotFound);
    };
    if reverted_at.is_some() {
        return Err(already_decided());
    }

    // 只能提议的规则：裁定表（撤回没有闸门）、熔断器、这次合并是谁做的
    let by_token_agent: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM token_agent_actions
                         WHERE kb_id = $1 AND merge_id = $2 AND outcome = 'applied'
                           AND operation IN ('review.merge', 'merge.manual'))",
    )
    .bind(w.kb_id)
    .bind(merge_id)
    .fetch_one(pool)
    .await?;
    let held = match held_back(AgentOperation::RevertMerge, None, w.breaker)? {
        Some(held) => Some(held),
        None if by_token_agent => None,
        None if merged_by.is_some() => Some((AgentOutcome::Proposed, "merge_by_person")),
        None => Some((AgentOutcome::Proposed, "merge_by_governor")),
    };

    let payload = serde_json::json!({
        "left": source_name, "right": target_name,
        "source": source_name, "target": target_name, "why": why,
        "merge_id": merge_id, "source_id": source_id, "target_id": target_id,
    });
    let target_key = merge_id.to_string();
    let new_row = |outcome: AgentOutcome, gate_reason: Option<&'static str>| NewRow {
        operation: "merge.revert",
        target_kind: "merge",
        target_id: merge_id,
        target_key: &target_key,
        outcome,
        gate_reason,
        merge_id: Some(merge_id),
        payload: &payload,
    };
    if let Some((outcome, reason)) = held {
        return land_proposal(pool, w, &new_row(outcome, Some(reason))).await;
    }

    // 先读名字：读不到就不撤
    let token_name = token_name_on(&mut *pool.acquire().await?, w.token_id).await?;
    let follow_up_error = match crate::resolution::revert_merge(pool, w.kb_id, merge_id).await {
        Ok(()) => None,
        Err(AppError::NotFound) => return Err(AppError::NotFound),
        Err(AppError::Conflict(said)) if said == ALREADY_REVERTED => return Err(already_decided()),
        Err(e) => {
            // 撤回的事务提交之后还有一步（消歧后缀）。错误出在那里的话撤回已经在了：
            // 照样写那两行，错误挂在 applied 的结果上。没撤成就把错误原样报出去
            let reverted: bool = sqlx::query_scalar(
                "SELECT reverted_at IS NOT NULL FROM entity_merges WHERE id = $1 AND kb_id = $2",
            )
            .bind(merge_id)
            .bind(w.kb_id)
            .fetch_optional(pool)
            .await?
            .unwrap_or(false);
            if !reverted {
                return Err(e);
            }
            tracing::warn!(kb_id = %w.kb_id, %merge_id, error = %e, "令牌撤回提交之后的后续步骤失败");
            Some(e.to_string())
        }
    };

    let detail = serde_json::json!({ "source": source_name, "target": target_name, "why": why });
    let row = new_row(AgentOutcome::Applied, None);
    let mut last = None;
    for attempt in 1..=3 {
        match record_revert(pool, w, &token_name, &row, &detail).await {
            Ok(Some(action)) => {
                return Ok(TokenWriteOutcome {
                    action,
                    existing: false,
                    follow_up_error,
                })
            }
            // 另一个令牌调用同时撤了同一次合并、并且先写下了它的行：这一次算输了
            Ok(None) => return Err(already_decided()),
            Err(e) => {
                tracing::warn!(kb_id = %w.kb_id, %merge_id, attempt, error = %e, "令牌撤回的台账行与审计行没写进去");
                last = Some(e);
            }
        }
    }
    Err(AppError::Other(anyhow::anyhow!(
        "merge {merge_id} was reverted, but its ledger and audit rows could not be written: {}",
        last.map(|e| e.to_string()).unwrap_or_default()
    )))
}

/// 令牌撤回之后的那两行，一个事务。`None` = 这次合并已经有一行落地的 `merge.revert`
/// （同时到的另一个令牌调用写的）：一次合并只记一次令牌撤回。事务级咨询锁让两个同时到的
/// 调用排队，后到的看得见先到的那一行
async fn record_revert(
    pool: &PgPool,
    w: &TokenWrite,
    token_name: &str,
    row: &NewRow<'_>,
    detail: &serde_json::Value,
) -> AppResult<Option<TokenAgentAction>> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('token-revert:' || $1, 0))")
        .bind(row.target_key)
        .execute(&mut *tx)
        .await?;
    let recorded: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM token_agent_actions
                         WHERE kb_id = $1 AND merge_id = $2 AND outcome = 'applied'
                           AND operation = 'merge.revert')",
    )
    .bind(w.kb_id)
    .bind(row.target_id)
    .fetch_one(&mut *tx)
    .await?;
    if recorded {
        return Ok(None);
    }
    let action = insert_row(&mut tx, w, token_name, row)
        .await?
        .ok_or_else(|| AppError::Other(anyhow::anyhow!("an applied ledger row was not written")))?;
    crate::audit::record_on(
        &mut tx,
        Some(w.kb_id),
        Some(w.owner_id),
        "merge.revert",
        "merge",
        Some(row.target_id),
        audit_detail(detail.clone(), w, token_name, action.id, row.merge_id),
    )
    .await?;
    tx.commit().await?;
    Ok(Some(action))
}

/// 一个库的令牌 agent 台账，最新的在前
pub async fn list(
    pool: &PgPool,
    kb_id: Uuid,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<TokenAgentAction>> {
    let rows = sqlx::query_as(&format!(
        "SELECT {COLS} FROM token_agent_actions t {JOINS}
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
///
/// 后来的几个键（`reason`、`batch_id`、`merge_id`、`state`、`state_at`）**有值才出现**：
/// 分开一对的行因此与从前一个键不差
fn history_row(a: &TokenAgentAction) -> serde_json::Value {
    let mut row = serde_json::json!({
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
    });
    if let Some(fields) = row.as_object_mut() {
        let mut put = |key: &str, value: Option<serde_json::Value>| {
            if let Some(value) = value {
                fields.insert(key.into(), value);
            }
        };
        put(
            "reason",
            a.gate_reason.as_ref().map(|v| serde_json::json!(v)),
        );
        put("batch_id", a.batch_id.map(|v| serde_json::json!(v)));
        put("merge_id", a.merge_id.map(|v| serde_json::json!(v)));
        put("state", a.state.as_ref().map(|v| serde_json::json!(v)));
        put("state_at", a.state_at.map(|v| serde_json::json!(v)));
    }
    row
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
            "SELECT {COLS} FROM token_agent_actions t {JOINS}
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
