//! 审核队列 API：消解疑似重复对 + 低置信事实 + 合并日志/回滚。

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use utopia_core::models::Role;
use uuid::Uuid;

use super::graph_routes::require_kb;
use super::review_ops::{self, ReviewCaller};
use crate::auth::{AuthUser, SessionOrToken};
use crate::error::ApiResult;
use crate::state::AppState;

#[cfg(test)]
#[path = "review_routes_phrase_tests.rs"]
mod phrase_tests;

#[cfg(test)]
#[path = "review_conflict_tests.rs"]
mod conflict_tests;

#[cfg(test)]
#[path = "review_attribution_tests.rs"]
mod attribution_tests;

/// 一页多少条。**服务端的默认，不是上限**——前端可以要更少，多则被 clamp 挡住
const REVIEW_PAGE: i64 = 10;

/// 事实快照（决策台账用）：reject 后事实从图里消失，台账必须自包含展示文本。
/// 一律在动作执行前取。
pub(crate) async fn fact_snapshot(
    state: &AppState,
    kb_id: Uuid,
    fact_id: Uuid,
) -> Option<serde_json::Value> {
    // 宾语可能是实体或字面值；结构化字面值优先取 summary（与队列卡片同一显示口径）
    let row: Option<(String, Option<String>, Option<String>, f32)> = sqlx::query_as(
        "SELECT s.canonical_name, COALESCE(r.label, fact_surface_predicate(f.id)),
                COALESCE(o.canonical_name, f.object_value ->> 'summary',
                         f.object_value #>> '{}'), f.confidence
         FROM facts f
         JOIN entities s ON s.id = f.subject_id
         LEFT JOIN relation_types r ON r.id = f.predicate_id
         LEFT JOIN entities o ON o.id = f.object_id
         WHERE f.id = $1 AND f.kb_id = $2",
    )
    .bind(fact_id)
    .bind(kb_id)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten();
    row.map(|(s, p, o, c)| json!({ "subject": s, "predicate": p, "object": o, "confidence": c }))
}

#[derive(Deserialize)]
pub struct ReviewQuery {
    /// 要看哪一档。缺省 duplicates
    #[serde(default)]
    pub queue: Option<String>,
    /// 只对 duplicates 有意义：any（缺省）/ same（两边同类）/ conflict（两边类型冲突）
    #[serde(default)]
    pub types: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

/// 审核队列：**计数与内容分开取**。
///
/// 从前一次把八个队列全端回来，每个固定 100 条，前端再客户端分页——于是左栏
/// 的徽标是截断后的数字（库里 164 条低置信事实，界面写 100），而第十一页之后
/// 的东西界面上不存在。
///
/// 现在：计数每次都回（八个 COUNT 一条查询，走的是与列表同一套 WHERE），
/// 内容只回当前那一档的一页。切换分档或翻页各发一次请求，代价是多几次往返，
/// 换来的是**数字不再骗人**，而且一个有十万条待办的库也翻得到底。
///
/// 个人令牌也能读（0076）：令牌走 `review_ops::list`，Review 的十一个分档都开，`mappings`
/// 不是分档、仍只给会话；会话照旧过下面的 `require_kb`。两边过了检查拿的都是
/// [`queue_page`] 的那一页
pub async fn list(
    State(state): State<AppState>,
    caller: SessionOrToken,
    Path(kb_id): Path<Uuid>,
    Query(q): Query<ReviewQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let user = caller.user;
    if let Some(token) = caller.pat {
        let caller = ReviewCaller { user, token, kb_id };
        let section = q.queue.as_deref().unwrap_or("duplicates");
        return Ok(Json(
            review_ops::list(
                &state,
                &caller,
                section,
                q.types.as_deref(),
                q.limit,
                q.offset,
            )
            .await?,
        ));
    }
    require_kb(&state, &user, kb_id, Role::Viewer).await?;
    let queue = q.queue.as_deref().unwrap_or("duplicates");
    Ok(Json(
        queue_page(&state, kb_id, queue, q.types.as_deref(), q.limit, q.offset).await?,
    ))
}

/// 一档的一页：计数、档名、这一页的条目，以及这一档翻到底一共多少行（`total`，0076）。
///
/// **这里不做授权。** 会话那一段先过 `require_kb`，令牌那一段先过 `review_ops` 的读检查，
/// 两边过了才到这里——档的 `match` 只写这一处，两个入口拿到的是同一页。
///
/// `counts` 是左栏的徽标，键和值都不动。它里面与档同名的键在三档上不是列表的行数：
/// `alignment` 不数蕴含规则，`agent` 是开着的建议数，`lowconf` 不数推出来的事实。按它
/// 翻页，最后几行翻不到。`total` 另算：每一档用列表自己的 FROM / JOIN / WHERE 数一遍，
/// 连接丢掉的行不数，所以翻到 `offset = total` 正好是空页
pub(super) async fn queue_page(
    state: &AppState,
    kb_id: Uuid,
    queue: &str,
    types: Option<&str>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<serde_json::Value, utopia_core::AppError> {
    let pool = &state.pool;
    let counts = utopia_store::review::counts(pool, kb_id).await?;
    // 上限挡住「limit=1000000 把库拖垮」，同时留出足够一页的余量
    let limit = limit.unwrap_or(REVIEW_PAGE).clamp(1, 200);
    let offset = offset.unwrap_or(0).max(0);

    let (mut items, total) = match queue {
        // 记忆抽出、等人点头的事实（0015）。排第一：它是人自己说的话
        "pending" => (
            json!(utopia_store::pending::list(pool, kb_id, limit, offset).await?),
            utopia_store::pending::total(pool, kb_id).await?,
        ),
        "duplicates" => {
            let types = utopia_store::resolution::TypeFilter::parse(types)?;
            (
                json!(
                    utopia_store::resolution::list_reviews(pool, kb_id, types, limit, offset)
                        .await?
                ),
                utopia_store::resolution::list_reviews_total(pool, kb_id, types).await?,
            )
        }
        "conflicts" => (
            json!(utopia_store::temporal::list_conflicts(pool, kb_id, limit, offset).await?),
            utopia_store::temporal::list_conflicts_total(pool, kb_id).await?,
        ),
        "unconfirmed" => (
            json!(utopia_store::graph::stale_facts(pool, kb_id, limit, offset).await?),
            utopia_store::graph::stale_facts_total(pool, kb_id).await?,
        ),
        "lowconf" => {
            let below = utopia_store::review::LOW_CONFIDENCE_BELOW;
            (
                json!(
                    utopia_store::graph::low_confidence_facts(pool, kb_id, below, limit, offset)
                        .await?
                ),
                utopia_store::graph::low_confidence_facts_total(pool, kb_id, below).await?,
            )
        }
        "mappings" => (
            json!(utopia_store::mappings::proposed(pool, kb_id, limit, offset).await?),
            utopia_store::mappings::proposed_total(pool, kb_id).await?,
        ),
        // 对齐器两票不一致的签名与类别词（#725 对齐队列，0044 决定 3）
        "alignment" => (
            json!(utopia_store::alignment_queue::list(pool, kb_id, limit, offset).await?),
            utopia_store::alignment_queue::total(pool, kb_id).await?,
        ),
        // 勘误 agent 留给人的动作（0044 决定 7）：闸门拦下的撤、改、加
        "errata" => (
            json!(utopia_store::errata::held(pool, kb_id, limit, offset).await?),
            utopia_store::errata::held_total(pool, kb_id).await?,
        ),
        "violations" => (
            json!(utopia_store::reasoning::open_violations(pool, kb_id, limit, offset).await?),
            utopia_store::reasoning::open_violations_total(pool, kb_id).await?,
        ),
        "defects" => (
            json!(utopia_store::reasoning::open_defects(pool, kb_id, limit, offset).await?),
            utopia_store::reasoning::open_defects_total(pool, kb_id).await?,
        ),
        "merges" => (
            json!(utopia_store::resolution::list_merges(pool, kb_id, limit, offset).await?),
            utopia_store::resolution::list_merges_total(pool, kb_id).await?,
        ),
        // agent 的每一笔（0025）：建议、自动裁决与人的回答，最新的在前。令牌 agent 台账的行
        // 按动作时间并进同一页（0076 决定 7），带 `source: "token"`；治理 agent 的行原样
        "agent" => (
            json!(
                utopia_store::token_agent_actions::agent_history(pool, kb_id, limit, offset)
                    .await?
            ),
            utopia_store::token_agent_actions::agent_history_total(pool, kb_id).await?,
        ),
        // 认不出的档名当成契约错误报出来，而不是悄悄回空——悄悄回空会让前端
        // 拼错一个字母之后看到「这一档清空了」
        other => {
            return Err(utopia_core::AppError::invalid(
                "unknown_queue",
                format!("no review queue named {other}"),
            ))
        }
    };
    // 令牌 agent 开着的提议挂在它指着的那一条上（0076）：键只在有提议时出现，没有提议的
    // 条目与从前逐字相等。会话、令牌、MCP 拿的都是这一页，所以只挂这一处
    review_ops::attach_section_proposals(state, kb_id, queue, &mut items).await?;

    Ok(json!({
        "counts": counts,
        "queue": queue,
        "items": items,
        "total": total,
    }))
}

#[derive(Deserialize)]
pub struct CloseFactBody {
    pub valid_to: chrono::DateTime<chrono::Utc>,
    /// year | month | day。**写多少位就是多少精度**（与图谱页的区间编辑同一约定）；
    /// 不给按日——只发日期的老客户端就是这个意思
    #[serde(default)]
    pub valid_to_precision: Option<String>,
}

/// 人给的世界轴精度。从前这里一律写 "day"（「人在界面上选的是一个日期」），于是
/// 「2023 年 6 月结束」只能编成 6 月 1 日——在无知的地方填一个确定的值，正是
/// `facts.valid_to_precision` 那条注释说的病
pub(super) fn world_precision(p: Option<&str>) -> Result<&'static str, utopia_core::AppError> {
    let Some(p) = p else { return Ok("day") };
    utopia_store::graph::WORLD_PRECISIONS
        .iter()
        .copied()
        .find(|w| *w == p)
        .ok_or_else(|| {
            utopia_core::AppError::invalid(
                "bad_precision",
                "A closing date's precision is year, month, day, hour, minute or second.",
            )
        })
}

/// 人工闭合一条事实的有效区间（"这事在某时结束了"）——走作废+改写，
/// 与自动闭合同一机制，账本可回放。
///
/// 写令牌也能调（0076）：它**只留一条提议**（`review_ops::propose_fact`），事实不动，等
/// 登录的人在 Review 里答。令牌在进下面这一段之前就离开了处理函数；会话照旧跑
/// [`fact_closing`]，回答与审计行同从前一个字不差
pub async fn close_fact(
    State(state): State<AppState>,
    caller: SessionOrToken,
    Path((kb_id, fact_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<CloseFactBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let user = caller.user;
    if let Some(token) = caller.pat {
        let caller = ReviewCaller { user, token, kb_id };
        let answer =
            review_ops::propose_fact(&state, &caller, fact_id, "close", Some(&body)).await?;
        return Ok(Json(answer));
    }
    require_kb(&state, &user, kb_id, Role::Editor).await?;
    fact_closing(&state, kb_id, &user, fact_id, &body, None).await?;
    Ok(Json(json!({ "ok": true })))
}

/// 人闭合一条事实的那几句，从会话处理函数里原样挪出来：校验、快照、store 的改写、审计行、
/// 刷新。会话调它时 `proposal` 是 None，一切同从前；批准令牌的提议时带上提议的 id，审计
/// detail 里多一个 `agent_proposal`。actor 两种情形都是这个人，detail 里不放 `token_id`。
///
/// 回 store 改没改：`close_superseded` 说「没动」（检查之后、改写之前这条事实被别人作废了）
/// 时是 false。会话不看它——从前就不看；批准提议时没动就什么都不记，那条提议算过时
async fn fact_closing(
    state: &AppState,
    kb_id: Uuid,
    user: &utopia_core::models::User,
    fact_id: Uuid,
    body: &CloseFactBody,
    proposal: Option<Uuid>,
) -> Result<bool, utopia_core::AppError> {
    // 归属与状态校验：本 KB 的、未作废、开放区间的事实才可闭合
    let ok: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM facts
         WHERE id = $1 AND kb_id = $2 AND invalidated_at IS NULL AND valid_to IS NULL",
    )
    .bind(fact_id)
    .bind(kb_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(utopia_core::AppError::Db)?;
    if ok.is_none() {
        return Err(utopia_core::AppError::NotFound);
    }
    let precision = world_precision(body.valid_to_precision.as_deref())?;
    let snap = fact_snapshot(state, kb_id, fact_id).await;
    let changed =
        utopia_store::temporal::close_superseded(&state.pool, fact_id, body.valid_to, precision)
            .await?
            .is_some();
    if proposal.is_some() && !changed {
        return Ok(false);
    }
    if let Some(mut d) = snap {
        d["valid_to"] = json!(body.valid_to.to_rfc3339());
        d["valid_to_precision"] = json!(precision);
        if let Some(proposal) = proposal {
            d["agent_proposal"] = json!(proposal);
        }
        let _ = utopia_store::audit::record(
            &state.pool,
            Some(kb_id),
            user.id,
            "fact.close",
            "fact",
            Some(fact_id),
            d,
        )
        .await;
    }
    state.emit_review(kb_id);
    Ok(changed)
}

#[derive(Deserialize)]
pub struct ConflictBody {
    /// close | retime_new | keep | reject_new
    pub action: String,
    /// close 且新事实无起点时必填
    #[serde(default)]
    pub close_at: Option<chrono::DateTime<chrono::Utc>>,
    /// `close_at` 的精度（year 到 second），不给按日
    #[serde(default)]
    pub close_at_precision: Option<String>,
    /// retime_new 时必填：只改新事实起点，终点仍由原文或时间线定。
    #[serde(default)]
    pub new_from: Option<chrono::DateTime<chrono::Utc>>,
    /// `new_from` 的精度（year 到 second），不给按日。
    #[serde(default)]
    pub new_from_precision: Option<String>,
}

/// 时态冲突裁决（S3：自动闭合拿不准的那些）。
///
/// 写令牌也能调（0076）：它**只留一条提议**（`review_ops::propose_conflict`），矛盾行与
/// 两条事实都不动，等登录的人在 Review 里答。令牌在进下面这一段之前就离开了处理函数；
/// 会话照旧跑 [`conflict_resolution`]，回答与审计行同从前一个字不差
pub async fn resolve_conflict(
    State(state): State<AppState>,
    caller: SessionOrToken,
    Path((kb_id, conflict_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<ConflictBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let user = caller.user;
    if let Some(token) = caller.pat {
        let caller = ReviewCaller { user, token, kb_id };
        let answer = review_ops::propose_conflict(&state, &caller, conflict_id, &body).await?;
        return Ok(Json(answer));
    }
    require_kb(&state, &user, kb_id, Role::Editor).await?;
    conflict_resolution(&state, kb_id, &user, conflict_id, &body, None).await?;
    Ok(Json(json!({ "ok": true })))
}

/// 人裁一处时态矛盾的那几句，从会话处理函数里原样挪出来：两个精度、store 的那一个事务
/// （改写、结案、审计行一起提交）、刷新。会话调它时 `proposal` 是 None，store 写下的审计
/// detail 与从前一个键不差；批准令牌的提议时带上提议的 id，detail 里多一个
/// `agent_proposal`（审计行在 store 的事务里写，所以 id 递给 store）。actor 都是这个人
async fn conflict_resolution(
    state: &AppState,
    kb_id: Uuid,
    user: &utopia_core::models::User,
    conflict_id: Uuid,
    body: &ConflictBody,
    proposal: Option<Uuid>,
) -> Result<(), utopia_core::AppError> {
    let precision = world_precision(body.close_at_precision.as_deref())?;
    let new_precision = world_precision(body.new_from_precision.as_deref())?;
    utopia_store::temporal::ConflictDecision {
        action: &body.action,
        close_at: body.close_at,
        close_precision: precision,
        new_from: body.new_from,
        new_precision,
    }
    .resolve_as_reviewer(&state.pool, kb_id, conflict_id, user.id, proposal)
    .await?;
    state.emit_review(kb_id);
    Ok(())
}

#[derive(Deserialize)]
pub struct DecideBody {
    /// merge | keep
    pub action: String,
    /// 人拍板时写的那一句（0026）：什么让你这么定。可空
    #[serde(default)]
    pub rationale: Option<String>,
}

/// 人裁一对重复项。写令牌也能裁（0076）：分开走 `review_ops::keep`，合并走
/// `review_ops::merge_pair`——合并先过服务端的闸门，被留下的不落地、只留一条提议。
/// 两样都经裁定表、记进令牌 agent 的台账，不算人的先例。令牌在进下面这一段之前就
/// 离开了处理函数：会话照旧跑下面这一段，不问闸门
pub async fn decide(
    State(state): State<AppState>,
    caller: SessionOrToken,
    Path((kb_id, review_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<DecideBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let user = caller.user;
    if let Some(token) = caller.pat {
        let caller = ReviewCaller { user, token, kb_id };
        let rationale = body.rationale.as_deref();
        let answer = match body.action.as_str() {
            "keep" => review_ops::keep(&state, &caller, review_id, rationale, None).await?,
            "merge" => review_ops::merge_pair(&state, &caller, review_id, rationale, None).await?,
            _ => {
                // 先过 scope 与角色，再说「这个动作没开」：只读令牌问一个没开的动作，
                // 答的仍是 `token_read_only`。什么都不写
                review_ops::authorize_write(&state, &caller).await?;
                return Err(review_ops::not_open().into());
            }
        };
        return Ok(Json(answer));
    }
    require_kb(&state, &user, kb_id, Role::Editor).await?;
    let snap: Option<(String, String, f32)> = sqlx::query_as(
        "SELECT a.canonical_name, b.canonical_name, rr.score
         FROM resolution_reviews rr
         JOIN entities a ON a.id = rr.left_id
         JOIN entities b ON b.id = rr.right_id
         WHERE rr.id = $1 AND rr.kb_id = $2",
    )
    .bind(review_id)
    .bind(kb_id)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten();
    utopia_store::resolution::decide_review(
        &state.pool,
        kb_id,
        review_id,
        &body.action,
        user.id,
        body.rationale.as_deref(),
    )
    .await?;
    if let Some((l, r, score)) = snap {
        // 理由也进台账：先例是从这里读的，不是从 resolution_reviews
        let _ = utopia_store::audit::record(
            &state.pool,
            Some(kb_id),
            user.id,
            if body.action == "merge" {
                "review.merge"
            } else {
                "review.keep"
            },
            "review",
            Some(review_id),
            json!({ "left": l, "right": r, "score": score, "why": body.rationale }),
        )
        .await;
    }
    crate::governance::after_human_decision(
        &state,
        kb_id,
        &[review_id],
        Some(&body.action),
        user.id,
    )
    .await;
    state.emit_review(kb_id);
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct AgentAnswerBody {
    /// merge | keep 答一条建议（与建议相同是接受，不同是改判）；
    /// revert 撤回一条自动合并；merge 也能推翻一条自动分开
    pub action: String,
    /// 什么让你这么答（0026）。改判 agent 的时候尤其值得写：那一句就是下次的先例
    #[serde(default)]
    pub rationale: Option<String>,
}

/// 人回答 agent 的一笔（0025）。回答走的是人的裁决路径：decided_by 是这个人，
/// 台账记的是这个人——它就此成为下一轮的先例。
///
/// **只收会话**（0076）：这条路由是「以人的身份合并、分开、撤回一对」的第二条路，个人令牌
/// 在提取器那里就是 401。令牌想说它会怎么答，走 [`propose_governor`] 的那条路由，只留一条
/// 提议。下面的语句原样挪进了 [`governor_answer`]，会话的回答与审计行同从前一个字不差
pub async fn agent_answer(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((kb_id, decision_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<AgentAnswerBody>,
) -> ApiResult<Json<serde_json::Value>> {
    require_kb(&state, &user, kb_id, Role::Editor).await?;
    governor_answer(&state, kb_id, &user, decision_id, &body, None).await?;
    Ok(Json(json!({ "ok": true })))
}

/// 人回答治理 agent 的一笔的那几句，从会话处理函数里原样挪出来：读这一笔、按它的状态与
/// 动作走人的裁决路径（`decide_review` / `revert_merge` / `merge_entities`）、把这一笔记成
/// 答过、审计行、保险丝、治理的后续、刷新。会话调它时 `proposal` 是 None，审计 detail 与
/// 从前一个键不差；批准令牌的提议时带上提议的 id，detail 里多一个 `agent_proposal`——这次
/// 回答是这个人的，也看得出是从哪条提议来的。actor 两种情形都是这个人，detail 里不放
/// `token_id`。它不是一个事务：中途报的错原样报出去
async fn governor_answer(
    state: &AppState,
    kb_id: Uuid,
    user: &utopia_core::models::User,
    decision_id: Uuid,
    body: &AgentAnswerBody,
    proposal: Option<Uuid>,
) -> Result<(), utopia_core::AppError> {
    // 审计 detail：不带提议时原样，带提议时只多 `agent_proposal` 一个键
    let named = |mut detail: serde_json::Value| {
        if let Some(proposal) = proposal {
            detail["agent_proposal"] = json!(proposal);
        }
        detail
    };
    let d = utopia_store::governance::get(&state.pool, kb_id, decision_id).await?;
    let (l, r) = (
        d.left.clone().unwrap_or_default(),
        d.right.clone().unwrap_or_default(),
    );
    match (d.status.as_str(), body.action.as_str()) {
        ("proposed", act @ ("merge" | "keep")) => {
            utopia_store::resolution::decide_review(
                &state.pool,
                kb_id,
                d.target_id,
                act,
                user.id,
                body.rationale.as_deref(),
            )
            .await?;
            let status = if act == d.action {
                "accepted"
            } else {
                "overridden"
            };
            utopia_store::governance::settle(&state.pool, kb_id, decision_id, status, user.id)
                .await?;
            let _ = utopia_store::audit::record(
                &state.pool,
                Some(kb_id),
                user.id,
                if act == "merge" { "review.merge" } else { "review.keep" },
                "review",
                Some(d.target_id),
                named(
                    json!({ "left": l, "right": r, "agent_decision": decision_id, "agent_action": d.action,
                        "why": body.rationale }),
                ),
            )
            .await;
        }
        ("applied", "revert") => {
            let merge_id = d.merge_id.ok_or_else(|| {
                utopia_core::AppError::invalid(
                    "not_a_merge",
                    "Only an applied merge can be reverted; a pair kept apart can be merged instead.",
                )
            })?;
            utopia_store::resolution::revert_merge(&state.pool, kb_id, merge_id).await?;
            utopia_store::governance::settle(&state.pool, kb_id, decision_id, "reverted", user.id)
                .await?;
            let _ = utopia_store::audit::record(
                &state.pool,
                Some(kb_id),
                user.id,
                "merge.revert",
                "merge",
                Some(merge_id),
                named(json!({ "source": l, "target": r, "agent_decision": decision_id })),
            )
            .await;
            // 撤回 agent 的合并到了两次：保险丝
            crate::governance::fuse(state, kb_id).await;
        }
        ("applied", "merge") if d.action == "keep" => {
            let (left_id, right_id): (Uuid, Uuid) = sqlx::query_as(
                "SELECT left_id, right_id FROM resolution_reviews WHERE id = $1 AND kb_id = $2",
            )
            .bind(d.target_id)
            .bind(kb_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(utopia_core::AppError::from)?
            .ok_or(utopia_core::AppError::NotFound)?;
            let (target, source) =
                utopia_store::resolution::merge_direction(&state.pool, left_id, right_id).await?;
            let merge_id = utopia_store::resolution::merge_entities(
                &state.pool,
                kb_id,
                source,
                target,
                Some(user.id),
                "overrode agent keep",
            )
            .await?;
            utopia_store::governance::settle(
                &state.pool,
                kb_id,
                decision_id,
                "overridden",
                user.id,
            )
            .await?;
            let _ = utopia_store::audit::record(
                &state.pool,
                Some(kb_id),
                user.id,
                "merge.manual",
                "merge",
                Some(merge_id),
                named(json!({ "source": l, "target": r, "agent_decision": decision_id })),
            )
            .await;
        }
        (status, action) => {
            return Err(utopia_core::AppError::invalid(
                "agent_answer",
                format!("cannot {action} an agent decision that is {status}"),
            ))
        }
    }
    crate::governance::after_human_decision(state, kb_id, &[d.target_id], None, user.id).await;
    state.emit_review(kb_id);
    Ok(())
}

// ───────────────────────── 人答令牌 agent 的提议（0076） ─────────────────────────
//
// 令牌的合并被闸门留下、或者令牌想撤一次不是令牌 agent 做的合并时，图上什么都没动，
// 台账里留着一条开着的提议。答它的只能是登录的人：下面两个处理函数只收 `AuthUser`，
// 个人令牌——提出这条提议的那一枚也一样——在提取器那里就是 401，MCP 也没有答提议的工具。
//
// **批准跑的是人自己的路径，不是令牌的路径**：`resolution::decide_review` /
// `merge_entities` / `revert_merge`，actor 是这个人，不问闸门（人的合并从来不问），
// 后面跟着会话处理函数今天做的那几步——审计行、治理的后续。审计行的 detail 多一个
// `agent_proposal`，指着这条提议：这次合并是这个人的决定，也看得出是从哪条提议来的。
// detail 里**不带** `token_id`：带了它，这一行就会被当成令牌做的、挡在先例之外。
//
// 会话的 `decide` / `manual_merge` / `revert_merge` 一个字没动。下面三个 `approve_*`
// 是照着它们的语句另写的，只多了 `agent_proposal` 和「目标还裁不裁得了」的检查

#[derive(Deserialize)]
pub struct ProposalAnswerBody {
    /// approve | decline
    pub action: String,
}

#[derive(Deserialize)]
pub struct ProposalBulkBody {
    pub ids: Vec<Uuid>,
    /// approve | decline，整批一个动作
    pub action: String,
}

/// 同一个进程里提议一条一条地答。批准是「看目标还在不在、跑人的路径、把提议关上」三步，
/// 人的路径自己开事务、自己提交，三步之间别人插进来就会出现「合并是甲做的，提议却被乙
/// 关成了 superseded」。拿一把进程内的锁而不是数据库的咨询锁：后者要占着一条连接等
/// 另一条（见 `token_agent_actions::keep_duplicate_pair`）。多个进程之间仍由
/// `token_agent_actions::answer` 的守卫兜底：`state` 还空着才关得上，输的答 `already_decided`
static ANSWERING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn already_answered() -> utopia_core::AppError {
    utopia_core::AppError::CodedConflict {
        code: "already_decided",
        message: "This proposal was already answered.".into(),
    }
}

fn proposal_action(action: &str) -> Result<bool, utopia_core::AppError> {
    match action {
        "approve" => Ok(true),
        "decline" => Ok(false),
        other => Err(utopia_core::AppError::invalid(
            "unknown_action",
            format!("action must be approve or decline, got {other}"),
        )),
    }
}

/// 批准一条提议时人的路径走到了哪里
enum Approved {
    /// 做完了；带着人的路径留下的东西
    Done(Applied),
    /// 目标已经没法裁了：这一对已经定了、一侧并走了、合并已经撤回了。什么都没做
    Gone,
}

/// 批准时人的路径留下的东西，回答里**有值的才出现**：合并的三种带 `merge_id`（做出或
/// 撤回的那次合并）；排下投递任务的（短语、规则）带 `job_id`；确认一条等点头的事实带
/// `fact_id`。别的操作三样都没有
#[derive(Default)]
struct Applied {
    merge_id: Option<Uuid>,
    job_id: Option<i64>,
    fact_id: Option<Uuid>,
}

impl Applied {
    fn merge(merge_id: Option<Uuid>) -> Self {
        Self {
            merge_id,
            ..Self::default()
        }
    }

    /// 把有值的那几样放进一个回答（单条的响应体、批量里的一条结果）。`merge_id` 不在
    /// 这里放：单条有值才带、批量每一条都带，两边照 M003 各放各的
    fn put_into(&self, answer: &mut serde_json::Value) {
        if let Some(job_id) = self.job_id {
            answer["job_id"] = json!(job_id);
            answer["queued"] = json!(true);
        }
        if let Some(fact_id) = self.fact_id {
            answer["fact_id"] = json!(fact_id);
        }
    }
}

/// 人的路径报的错：是「目标没了」就折成 [`Approved::Gone`]，别的原样报出去
/// （比如治理 agent 正在裁这一对——那不是没了，提议留着，等一会儿再答）
fn gone_or(e: utopia_core::AppError) -> Result<Approved, utopia_core::AppError> {
    if utopia_store::token_agent_actions::target_gone(&e) {
        Ok(Approved::Gone)
    } else {
        Err(e)
    }
}

/// 批准提议时写的那条审计行，与会话处理函数一样尽力而为；写不进去留一条日志
async fn record_approval(
    state: &AppState,
    kb_id: Uuid,
    user_id: Uuid,
    action: &str,
    target_kind: &str,
    target_id: Uuid,
    detail: serde_json::Value,
) {
    if let Err(e) = utopia_store::audit::record(
        &state.pool,
        Some(kb_id),
        user_id,
        action,
        target_kind,
        Some(target_id),
        detail,
    )
    .await
    {
        tracing::warn!(%kb_id, action, error = %e, "批准提议的审计行没写进去");
    }
}

/// 批准一条队列合并的提议：照会话的 `decide` 走——`decide_review`（merge）、`review.merge`
/// 审计行、`after_human_decision`
async fn approve_queue_merge(
    state: &AppState,
    kb_id: Uuid,
    user_id: Uuid,
    p: &utopia_store::token_agent_actions::TokenAgentAction,
) -> Result<Approved, utopia_core::AppError> {
    let review_id = p.target_id;
    #[allow(clippy::type_complexity)]
    let snap: Option<(
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
    .bind(kb_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(utopia_core::AppError::from)?;
    let Some((status, left_id, right_id, l, r, score, left_gone, right_gone)) = snap else {
        return Ok(Approved::Gone);
    };
    // 这一对已经裁过，或者一侧已经并进了别人：提议说的那件事已经不成立
    if status != "pending" || left_gone.is_some() || right_gone.is_some() {
        return Ok(Approved::Gone);
    }
    // 审核行还等着，但已经不是提议时的那两个实体了：别的合并把它的一侧改指到了存活的
    // 实体上。令牌提的不是这一对，不能拿它的提议去合
    let (was_left, was_right) = (payload_id(p, "left_id")?, payload_id(p, "right_id")?);
    if (left_id, right_id) != (was_left, was_right) && (left_id, right_id) != (was_right, was_left)
    {
        return Ok(Approved::Gone);
    }
    // 人没有另写理由：令牌当时的那一句留在提议上，不冒充成这个人的话
    if let Err(e) = utopia_store::resolution::decide_review(
        &state.pool,
        kb_id,
        review_id,
        "merge",
        user_id,
        None,
    )
    .await
    {
        return gone_or(e);
    }
    let merge_id =
        utopia_store::token_agent_actions::merge_between(&state.pool, kb_id, left_id, right_id)
            .await?;
    record_approval(
        state,
        kb_id,
        user_id,
        "review.merge",
        "review",
        review_id,
        json!({ "left": l, "right": r, "score": score, "why": null, "agent_proposal": p.id }),
    )
    .await;
    crate::governance::after_human_decision(state, kb_id, &[review_id], Some("merge"), user_id)
        .await;
    Ok(Approved::Done(Applied::merge(merge_id)))
}

/// 提议的 payload 里存下的一个实体 id
fn payload_id(
    p: &utopia_store::token_agent_actions::TokenAgentAction,
    key: &str,
) -> Result<Uuid, utopia_core::AppError> {
    p.payload
        .get(key)
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| {
            utopia_core::AppError::Other(anyhow::anyhow!("proposal {} has no {key}", p.id))
        })
}

/// 批准一条手动合并的提议：照会话的 `manual_merge` 走——`merge_entities`、`merge.manual`
/// 审计行。方向是提议存下的那一个（第一个提出的调用方给的源与目标）。与人的手动合并一样，
/// 不关这两个实体自己那条等着的审核行，也没有治理的后续
async fn approve_manual_merge(
    state: &AppState,
    kb_id: Uuid,
    user_id: Uuid,
    p: &utopia_store::token_agent_actions::TokenAgentAction,
) -> Result<Approved, utopia_core::AppError> {
    let (source, target) = (payload_id(p, "source_id")?, payload_id(p, "target_id")?);
    let sides: Vec<(Uuid, String, Option<Uuid>)> = sqlx::query_as(
        "SELECT id, canonical_name, merged_into FROM entities WHERE kb_id = $1 AND id = ANY($2)",
    )
    .bind(kb_id)
    .bind(vec![source, target])
    .fetch_all(&state.pool)
    .await
    .map_err(utopia_core::AppError::from)?;
    let side = |id: Uuid| sides.iter().find(|s| s.0 == id);
    let (Some(s), Some(t)) = (side(source), side(target)) else {
        return Ok(Approved::Gone);
    };
    if s.2.is_some() || t.2.is_some() {
        return Ok(Approved::Gone);
    }
    let merge_id = match utopia_store::resolution::merge_entities(
        &state.pool,
        kb_id,
        source,
        target,
        Some(user_id),
        "manual merge",
    )
    .await
    {
        Ok(merge_id) => merge_id,
        Err(e) => return gone_or(e),
    };
    record_approval(
        state,
        kb_id,
        user_id,
        "merge.manual",
        "merge",
        merge_id,
        json!({ "source": s.1, "target": t.1, "why": null, "agent_proposal": p.id }),
    )
    .await;
    Ok(Approved::Done(Applied::merge(Some(merge_id))))
}

/// 批准一条撤回的提议：照会话的 `revert_merge` 走——`resolution::revert_merge`、`merge.revert`
/// 审计行、`governance::after_revert`。撤的要是治理 agent 自己裁的合并，保险丝照今天那样数
/// 这一次：这是人的撤回
async fn approve_revert(
    state: &AppState,
    kb_id: Uuid,
    user_id: Uuid,
    p: &utopia_store::token_agent_actions::TokenAgentAction,
) -> Result<Approved, utopia_core::AppError> {
    let merge_id = p.merge_id.unwrap_or(p.target_id);
    let snap: Option<(String, String, bool)> = sqlx::query_as(
        "SELECT s.canonical_name, t.canonical_name, m.reverted_at IS NOT NULL
         FROM entity_merges m
         JOIN entities s ON s.id = m.source_id
         JOIN entities t ON t.id = m.target_id
         WHERE m.id = $1 AND m.kb_id = $2",
    )
    .bind(merge_id)
    .bind(kb_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(utopia_core::AppError::from)?;
    let Some((s, t, reverted)) = snap else {
        return Ok(Approved::Gone);
    };
    if reverted {
        return Ok(Approved::Gone);
    }
    if let Err(e) = utopia_store::resolution::revert_merge(&state.pool, kb_id, merge_id).await {
        return gone_or(e);
    }
    record_approval(
        state,
        kb_id,
        user_id,
        "merge.revert",
        "merge",
        merge_id,
        json!({ "source": s, "target": t, "agent_proposal": p.id }),
    )
    .await;
    crate::governance::after_revert(state, kb_id, merge_id, user_id).await;
    Ok(Approved::Done(Applied::merge(Some(merge_id))))
}

// ── 合并之外的提议（0076）：Review 其余各档里只能提议的操作 ──
//
// 这些提议的 payload 里存着那次调用的请求体原样（`request`）。批准就是拿这份请求体去跑
// **人自己的那段代码**——每个开给令牌的处理函数都把会话的语句原样挪进了一个函数，会话
// 调它时不带提议的 id，这里带上，它只在审计 detail 里多写一个 `agent_proposal`。actor 是
// 批准的人；detail 里**不带** `token_id`，令牌当时写的理由也不当成这个人的话传进去。
//
// 跑之前先问目标还裁不裁得了：读的是带类型的行（`review_targets::target_state`），
// 不认错误的原话。那是合并的三种的做法（`gone_or`），留给它们

/// 提议存下的请求体，解回它那条路由的请求结构。解不回来是台账行坏了，不是请求错了
fn stored_request<T: serde::de::DeserializeOwned>(
    p: &utopia_store::token_agent_actions::TokenAgentAction,
) -> Result<T, utopia_core::AppError> {
    let request = p
        .payload
        .get("request")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    serde_json::from_value(request).map_err(|e| {
        utopia_core::AppError::Other(anyhow::anyhow!(
            "proposal {} holds a request that no longer reads as its route's body: {e}",
            p.id
        ))
    })
}

/// 一条合并之外的提议要做的那个决定，连同解回来的请求体。每一档的任务在这里加一个变体，
/// [`decision_of`] 里加一个操作名，[`approve_decision`] 里加一支
enum Decision {
    Defect(DecideDefectReq),
    ConfirmFact,
    RejectFact,
    CloseFact(CloseFactBody),
    Conflict(ConflictBody),
    Violation(DecideViolationReq),
    Errata(DecideErrataReq),
    AlignmentPhrase(DecideAlignmentPhraseReq),
    AlignmentRule(DecideAlignmentRuleReq),
    OntologyReject(super::ontology_routes::DecideProposalReq),
    OntologyAdopt(super::question_routes::AdoptReq),
    PendingNod(DecidePendingBody),
    Governor(AgentAnswerBody),
}

/// 这条提议的操作认不认得。`None` = 认不得：答的人拿到的还是从前那个 500
fn decision_of(
    p: &utopia_store::token_agent_actions::TokenAgentAction,
) -> Result<Option<Decision>, utopia_core::AppError> {
    Ok(match p.operation.as_str() {
        "defect.decide" => Some(Decision::Defect(stored_request(p)?)),
        // 确认与拒绝没有请求体
        "fact.confirm" => Some(Decision::ConfirmFact),
        "fact.reject" => Some(Decision::RejectFact),
        "fact.close" => Some(Decision::CloseFact(stored_request(p)?)),
        "conflict.resolve" => Some(Decision::Conflict(stored_request(p)?)),
        "violation.decide" => Some(Decision::Violation(stored_request(p)?)),
        "errata.decide" => Some(Decision::Errata(stored_request(p)?)),
        "alignment.phrase" => Some(Decision::AlignmentPhrase(stored_request(p)?)),
        "alignment.rule" => Some(Decision::AlignmentRule(stored_request(p)?)),
        // 本体提案的两种动作各是各的路由、各是各的请求体：提议存下的 `action` 说是哪一种
        "ontology.proposal" => Some(
            if p.payload.get("action").and_then(|a| a.as_str()) == Some("adopt") {
                Decision::OntologyAdopt(stored_request(p)?)
            } else {
                Decision::OntologyReject(stored_request(p)?)
            },
        ),
        // 等点头的事实与治理的一笔：存下的请求体只有 `action`。治理那一个解回来的
        // `rationale` 因此恒为空——令牌的理由留在提议上，不当成这个人的话
        "pending.nod" => Some(Decision::PendingNod(stored_request(p)?)),
        "governor.answer" => Some(Decision::Governor(stored_request(p)?)),
        _ => None,
    })
}

/// 批准一条合并之外的提议。
///
/// - 先读目标此刻的样子（带上提议存下的请求体与指纹）：不在了或已经定了就是
///   [`Approved::Gone`]，人的代码一行不跑
/// - 还开着：跑人的代码，actor 是这个人。它报的错原样报出去（带的是人的 code），提议留着。
///   只有「一条带守卫的语句或一个事务」的函数报的 `NotFound`——检查之后、落笔之前目标被
///   别人定了——才折成 `Gone`：那种函数什么都没写
async fn approve_decision(
    state: &AppState,
    kb_id: Uuid,
    user: &utopia_core::models::User,
    p: &utopia_store::token_agent_actions::TokenAgentAction,
    decision: Decision,
) -> Result<Approved, utopia_core::AppError> {
    let request = p
        .payload
        .get("request")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let fingerprint = p.payload.get("fingerprint").filter(|f| !f.is_null());
    let target = utopia_store::review_targets::target_state(
        &state.pool,
        kb_id,
        &p.operation,
        p.target_id,
        &request,
        fingerprint,
    )
    .await?;
    if target != utopia_store::review_targets::TargetState::Open {
        return Ok(Approved::Gone);
    }
    match decision {
        // 缺陷的裁决是一条带守卫的 UPDATE：`NotFound` 就是输了竞争，什么都没写
        Decision::Defect(req) => {
            match defect_decision(state, kb_id, user, p.target_id, &req, Some(p.id)).await {
                Ok(()) => Ok(Approved::Done(Applied::default())),
                Err(utopia_core::AppError::NotFound) => Ok(Approved::Gone),
                Err(e) => Err(e),
            }
        }
        // 确认是一条带守卫的 UPDATE，拒绝是一个事务：`NotFound` 就是输了竞争，什么都没写
        Decision::ConfirmFact => {
            match fact_confirmation(state, kb_id, user, p.target_id, Some(p.id)).await {
                Ok(()) => Ok(Approved::Done(Applied::default())),
                Err(utopia_core::AppError::NotFound) => Ok(Approved::Gone),
                Err(e) => Err(e),
            }
        }
        Decision::RejectFact => {
            match fact_rejection(state, kb_id, user, p.target_id, Some(p.id)).await {
                Ok(()) => Ok(Approved::Done(Applied::default())),
                Err(utopia_core::AppError::NotFound) => Ok(Approved::Gone),
                Err(e) => Err(e),
            }
        }
        // 闭合：store 说「没动」就是过时——这条事实在检查之后被别人作废了，没有哪一行
        // 是这次批准改的，不能关成 approved。它报的错（`NotFound` 也算）原样报出去
        Decision::CloseFact(body) => {
            if fact_closing(state, kb_id, user, p.target_id, &body, Some(p.id)).await? {
                Ok(Approved::Done(Applied::default()))
            } else {
                Ok(Approved::Gone)
            }
        }
        // 矛盾的裁决是一个事务：`NotFound` 就是它在检查之后被别人裁了，什么都没写
        Decision::Conflict(body) => {
            match conflict_resolution(state, kb_id, user, p.target_id, &body, Some(p.id)).await {
                Ok(()) => Ok(Approved::Done(Applied::default())),
                Err(utopia_core::AppError::NotFound) => Ok(Approved::Gone),
                Err(e) => Err(e),
            }
        }
        // 违规的裁决与勘误的回答都不是一个事务：撤了事实、执行了动作之后，后面的一步还
        // 可能出错。它们报的错（`NotFound` 也算）原样报出去，提议留着——已经落下的那几步
        // 是这个人的，不能说成「过时、什么都没做」
        Decision::Violation(req) => {
            violation_decision(state, kb_id, user, p.target_id, &req, Some(p.id)).await?;
            Ok(Approved::Done(Applied::default()))
        }
        Decision::Errata(req) => {
            errata_decision(state, kb_id, user, p.target_id, &req, Some(p.id)).await?;
            Ok(Approved::Done(Applied::default()))
        }
        // 短语与规则：判定连同它的投递任务一次提交。批准之后改动只是**排着**——回答带上
        // 任务的 id（`job_id` 与 `queued`），不说成已经落到图上（N6）。它们报的错原样报出去
        Decision::AlignmentPhrase(req) => {
            let job_id =
                alignment_phrase_decision(state, kb_id, user, p.target_id, &req, Some(p.id))
                    .await?;
            Ok(Approved::Done(Applied {
                job_id: Some(job_id),
                ..Applied::default()
            }))
        }
        Decision::AlignmentRule(req) => {
            let job_id =
                alignment_rule_decision(state, kb_id, user, p.target_id, &req, Some(p.id)).await?;
            Ok(Approved::Done(Applied {
                job_id: Some(job_id),
                ..Applied::default()
            }))
        }
        // 拒绝一条本体提案：上面已经读过它的状态（还开着、是代理提的、payload 没变）。人的
        // 那一条 UPDATE 对不在的提案也答 ok，所以由它自己重读一遍：确实是这个人这一次拒掉
        // 的才写审计行、算做完；被别人抢先定了就是过时，什么都没写
        Decision::OntologyReject(req) => {
            if super::ontology_routes::proposal_rejection(state, kb_id, user, &req, Some(p.id))
                .await?
            {
                Ok(Approved::Done(Applied::default()))
            } else {
                Ok(Approved::Gone)
            }
        }
        // 采纳是一个事务：`NotFound` 就是提案在检查之后被别人定了，什么都没写。别的错
        // （`unknown_class`、`bad_property_shape`、`alignment_busy`……）整个事务回滚了，
        // 原样报出去，提议留着
        Decision::OntologyAdopt(req) => {
            match crate::ontology_agent::adopt_for_proposal(
                state,
                kb_id,
                &req.section,
                &req.key,
                req.edits,
                user.id,
                p.id,
            )
            .await
            {
                Ok(_) => Ok(Approved::Done(Applied::default())),
                Err(utopia_core::AppError::NotFound) => Ok(Approved::Gone),
                Err(e) => Err(e),
            }
        }
        // 等点头的事实：批准**就是这个人的点头**——落事实（或记拒绝）、他的审计行、
        // 时间解析的任务，都与他自己点头时一样。点头不是一个事务，它报的错（`NotFound`
        // 也算）原样报出去，提议留着；上面已经读过那一行还在。确认带回落下的事实
        Decision::PendingNod(body) => {
            let fact_id = pending_nod(state, kb_id, user, p.target_id, &body.action, p.id).await?;
            Ok(Approved::Done(Applied {
                fact_id,
                ..Applied::default()
            }))
        }
        // 治理的一笔：跑的就是人回答治理的那段代码，以这个人的身份、没有理由。它不是一个
        // 事务，报的错原样报出去，提议留着
        Decision::Governor(body) => {
            // 理由在这里再清一次：就算哪一行台账的请求体里带着它，也传不进人的回答
            let body = AgentAnswerBody {
                action: body.action,
                rationale: None,
            };
            governor_answer(state, kb_id, user, p.target_id, &body, Some(p.id)).await?;
            Ok(Approved::Done(Applied::default()))
        }
    }
}

/// 批准一条「对等点头的事实点头或摇头」的提议时，这个人的那一次点头。
///
/// 语句照 [`decide_pending`] 抄的（那个处理函数一个字没动，令牌也进不去）：确认走
/// `pending::confirm`、审计行 `fact.nod_confirmed`、排一次时间解析、推三个事件；否决走
/// `pending::reject`（记的是这个人）、审计行 `fact.nod_rejected`、推两个事件。actor 是批准
/// 的这个人；与他自己点头只差一处——审计 detail 里多一个 `agent_proposal`，指着那条提议。
/// detail 里不放 `token_id`。回确认时落下的那条事实，否决是 None。
///
/// **只有 [`approve_decision`] 调它**，而那里只有登录的人到得了：令牌没有一条路走到这里
async fn pending_nod(
    state: &AppState,
    kb_id: Uuid,
    user: &utopia_core::models::User,
    pending_id: Uuid,
    action: &str,
    proposal: Uuid,
) -> Result<Option<Uuid>, utopia_core::AppError> {
    // 快照是一个对象：原样，只多 `agent_proposal` 一个键
    let named = |mut detail: serde_json::Value| {
        detail["agent_proposal"] = json!(proposal);
        detail
    };
    match action {
        "confirm" => {
            let done = utopia_store::pending::confirm(&state.pool, kb_id, pending_id).await?;
            let _ = utopia_store::audit::record(
                &state.pool,
                Some(kb_id),
                user.id,
                "fact.nod_confirmed",
                "fact",
                Some(done.fact_id),
                named(done.snapshot),
            )
            .await;
            // 点头落下的开放陈述带着时间词：按文档重排一次时间解析（0045），
            // 幂等，同一篇排着就不重复
            let _ = utopia_store::jobs::enqueue_unless_queued(
                &state.pool,
                "resolve_time",
                json!({ "document_id": done.document_id }),
            )
            .await;
            state.emit_pending(kb_id);
            state.emit_review(kb_id);
            state.emit_graph(kb_id);
            Ok(Some(done.fact_id))
        }
        "reject" => {
            let snap = utopia_store::pending::reject(&state.pool, kb_id, pending_id, Some(user.id))
                .await?;
            let _ = utopia_store::audit::record(
                &state.pool,
                Some(kb_id),
                user.id,
                "fact.nod_rejected",
                "pending_fact",
                Some(pending_id),
                named(snap),
            )
            .await;
            state.emit_pending(kb_id);
            state.emit_review(kb_id);
            Ok(None)
        }
        other => Err(utopia_core::AppError::invalid(
            "unknown_action",
            format!("action must be confirm or reject, got {other}"),
        )),
    }
}

/// 答一条提议。回关成了什么（`approved` / `declined` / `superseded`），以及批准时人的
/// 路径留下的东西（做出或撤回的那次合并、排下的任务、写下的事实）。
///
/// - 这个库里没有这条提议（认不出的 id、别的库的、不是提议的台账行）：`NotFound`
/// - 已经答过：`already_decided`，什么都不动
/// - 拒绝：只关提议，图不动
/// - 批准：目标还裁得了就跑人的路径、关成 `approved`；裁不了就关成 `superseded`，
///   什么都不做
///
/// 单条与批量都走这里：批量的每一条与单条是同一个回答
async fn answer_proposal(
    state: &AppState,
    kb_id: Uuid,
    user: &utopia_core::models::User,
    proposal_id: Uuid,
    approve: bool,
) -> Result<(&'static str, Applied), utopia_core::AppError> {
    let _one_at_a_time = ANSWERING.lock().await;
    let pool = &state.pool;
    let user_id = user.id;
    let p = utopia_store::token_agent_actions::proposal(pool, kb_id, proposal_id)
        .await?
        .ok_or(utopia_core::AppError::NotFound)?;
    if p.state.is_some() {
        return Err(already_answered());
    }
    let (closed_as, applied) = if approve {
        let approved = match p.operation.as_str() {
            "review.merge" => approve_queue_merge(state, kb_id, user_id, &p).await?,
            "merge.manual" => approve_manual_merge(state, kb_id, user_id, &p).await?,
            "merge.revert" => approve_revert(state, kb_id, user_id, &p).await?,
            other => match decision_of(&p)? {
                Some(decision) => approve_decision(state, kb_id, user, &p, decision).await?,
                None => {
                    return Err(utopia_core::AppError::Other(anyhow::anyhow!(
                        "proposal {proposal_id} has an operation nobody can approve: {other}"
                    )))
                }
            },
        };
        match approved {
            Approved::Done(applied) => ("approved", applied),
            Approved::Gone => ("superseded", Applied::default()),
        }
    } else {
        ("declined", Applied::default())
    };
    let won = utopia_store::token_agent_actions::answer(
        pool,
        kb_id,
        proposal_id,
        closed_as,
        user_id,
        applied.merge_id,
    )
    .await?;
    if !won {
        // 只有另一个进程同时答了同一条才会到这里（进程内有 ANSWERING）
        return Err(already_answered());
    }
    // 批准的审计行是人的路径那一条（带 `agent_proposal`）；拒绝与过时各记自己的一条。
    // 记的是人：detail 里不放 `token_id`
    if closed_as != "approved" {
        let action = if closed_as == "declined" {
            "agent.proposal.declined"
        } else {
            "agent.proposal.superseded"
        };
        let mut detail = json!({ "agent_proposal": proposal_id, "operation": p.operation,
                                 "reason": p.gate_reason, "proposed_by_token": p.token_name });
        // 一对的两侧只有合并与撤回的提议才有：payload 里有才带，别的提议不写两个空键
        for side in ["left", "right"] {
            if let Some(name) = p.payload.get(side) {
                detail[side] = name.clone();
            }
        }
        record_approval(
            state,
            kb_id,
            user_id,
            action,
            "agent_action",
            proposal_id,
            detail,
        )
        .await;
    }
    Ok((closed_as, applied))
}

/// 人答令牌 agent 的一条提议（0076）。只收会话；要这个库的 Editor
pub async fn agent_proposal_answer(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((kb_id, proposal_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<ProposalAnswerBody>,
) -> ApiResult<Json<serde_json::Value>> {
    require_kb(&state, &user, kb_id, Role::Editor).await?;
    let approve = proposal_action(&body.action)?;
    let (closed_as, applied) = answer_proposal(&state, kb_id, &user, proposal_id, approve).await?;
    state.emit_review(kb_id);
    let mut answer = json!({ "ok": true, "state": closed_as });
    if let Some(merge_id) = applied.merge_id {
        answer["merge_id"] = json!(merge_id);
    }
    applied.put_into(&mut answer);
    Ok(Json(answer))
}

/// 批量里一条没答成时的 code 与一句话。数据库与内部错误不把细节带出去
fn outcome_error(e: &utopia_core::AppError) -> (Option<&'static str>, String) {
    use utopia_core::AppError as E;
    match e {
        E::NotFound => (Some("not_found"), e.to_string()),
        E::CodedConflict { code, message } | E::CodedForbidden { code, message } => {
            (Some(code), message.clone())
        }
        E::Invalid { code, message, .. } => (Some(code), message.clone()),
        E::Db(_) | E::Other(_) => {
            tracing::error!(error = %e, "answering a proposal in a batch failed");
            (None, "Internal server error".into())
        }
        other => (None, other.to_string()),
    }
}

/// 一次答多条提议（0076）：1 到 500 条，一个动作。每一条走 [`answer_proposal`]，与单条
/// 同一个回答；回来按请求的次序一条一个结果，一条没成不拖累别的。0 条或超过 500 条
/// 整个拒绝，一条都不答
pub async fn agent_proposal_answers(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(kb_id): Path<Uuid>,
    Json(body): Json<ProposalBulkBody>,
) -> ApiResult<Json<serde_json::Value>> {
    require_kb(&state, &user, kb_id, Role::Editor).await?;
    if body.ids.is_empty() || body.ids.len() > BATCH_MAX {
        return Err(utopia_core::AppError::invalid(
            "batch_size",
            format!("between 1 and {BATCH_MAX} ids per batch"),
        )
        .into());
    }
    let approve = proposal_action(&body.action)?;
    let mut outcomes = Vec::with_capacity(body.ids.len());
    let mut answered = 0usize;
    for &id in &body.ids {
        outcomes.push(
            match answer_proposal(&state, kb_id, &user, id, approve).await {
                Ok((closed_as, applied)) => {
                    answered += 1;
                    // 五个键每一条都在；`job_id` / `queued` / `fact_id` 有值才多出来
                    let mut outcome = json!({ "id": id, "state": closed_as,
                                              "merge_id": applied.merge_id,
                                              "code": null, "error": null });
                    applied.put_into(&mut outcome);
                    outcome
                }
                Err(e) => {
                    let (code, error) = outcome_error(&e);
                    json!({ "id": id, "state": null, "merge_id": null,
                            "code": code, "error": error })
                }
            },
        );
    }
    state.emit_review(kb_id);
    Ok(Json(json!({ "answered": answered, "outcomes": outcomes })))
}

#[derive(Deserialize)]
pub struct BatchBody {
    pub ids: Vec<Uuid>,
    /// merge | keep，整批一个动作
    pub action: String,
    /// 整批一句理由（0026）：一批同一个动作，多半也是同一个依据
    #[serde(default)]
    pub rationale: Option<String>,
}

/// 一批 id 最多多少条：够一屏「全选」，又挡住一次请求把库锁上几分钟
const BATCH_MAX: usize = 500;

/// 批量裁决（#428）：与单条 `decide` 同一条路——每条各自合并/分开、各自记
/// 台账，回来的是逐条结果。一次只推一遍前端刷新。
///
/// 写令牌也能成批裁（0076）：令牌走 `review_ops::batch`——最多 50 对，每一对走单笔的
/// 那条令牌路径（合并先过闸门），服务端发一个 `batch_id`；它在进下面这一段之前就离开了
/// 处理函数。会话照旧跑下面这一段：上限 500，不问闸门，回答的形状不变
pub async fn batch(
    State(state): State<AppState>,
    caller: SessionOrToken,
    Path(kb_id): Path<Uuid>,
    Json(body): Json<BatchBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let user = caller.user;
    if let Some(token) = caller.pat {
        let caller = ReviewCaller { user, token, kb_id };
        return Ok(Json(
            review_ops::batch(
                &state,
                &caller,
                &body.ids,
                &body.action,
                body.rationale.as_deref(),
            )
            .await?,
        ));
    }
    require_kb(&state, &user, kb_id, Role::Editor).await?;
    if body.ids.is_empty() || body.ids.len() > BATCH_MAX {
        return Err(utopia_core::AppError::invalid(
            "batch_size",
            format!("between 1 and {BATCH_MAX} ids per batch"),
        )
        .into());
    }
    // 台账要的名字与分数在裁决前取——合并之后 source 一侧的名字已经不在原处了
    let snaps: Vec<(Uuid, String, String, f32)> = sqlx::query_as(
        "SELECT rr.id, a.canonical_name, b.canonical_name, rr.score
         FROM resolution_reviews rr
         JOIN entities a ON a.id = rr.left_id
         JOIN entities b ON b.id = rr.right_id
         WHERE rr.kb_id = $1 AND rr.id = ANY($2)",
    )
    .bind(kb_id)
    .bind(&body.ids)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();
    let outcomes = utopia_store::resolution::decide_reviews(
        &state.pool,
        kb_id,
        &body.ids,
        &body.action,
        user.id,
        body.rationale.as_deref(),
    )
    .await?;
    let action = if body.action == "merge" {
        "review.merge"
    } else {
        "review.keep"
    };
    for o in outcomes.iter().filter(|o| o.error.is_none()) {
        if let Some((_, l, r, score)) = snaps.iter().find(|s| s.0 == o.id) {
            let _ = utopia_store::audit::record(
                &state.pool,
                Some(kb_id),
                user.id,
                action,
                "review",
                Some(o.id),
                json!({ "left": l, "right": r, "score": score, "batch": body.ids.len(),
                        "why": body.rationale }),
            )
            .await;
        }
    }
    let decided_ids: Vec<Uuid> = outcomes
        .iter()
        .filter(|o| o.error.is_none())
        .map(|o| o.id)
        .collect();
    crate::governance::after_human_decision(
        &state,
        kb_id,
        &decided_ids,
        Some(&body.action),
        user.id,
    )
    .await;
    state.emit_review(kb_id);
    let decided = decided_ids.len();
    Ok(Json(json!({ "decided": decided, "outcomes": outcomes })))
}

/// 整批撤回里人的那一支撤一次合并：照会话的 `revert_merge` 走——合并前后的名字、
/// `resolution::revert_merge`、`merge.revert` 审计行（尽力而为）、`governance::after_revert`。
/// 这是人的撤回：记在这个人名下，detail 里不带 `token_id`，只多一个 `agent_batch` 说它是
/// 跟着哪一批撤的；撤的是令牌 agent 的合并，所以熔断器照人的撤回那样数它。
/// 已经撤回的合并折成 `already_decided`——那条路自己说的是一句没有 code 的 409
async fn revert_as_person(
    state: &AppState,
    kb_id: Uuid,
    user_id: Uuid,
    batch_id: Uuid,
    merge_id: Uuid,
) -> Result<serde_json::Value, utopia_core::AppError> {
    let snap: Option<(String, String)> = sqlx::query_as(
        "SELECT s.canonical_name, t.canonical_name
         FROM entity_merges m
         JOIN entities s ON s.id = m.source_id
         JOIN entities t ON t.id = m.target_id
         WHERE m.id = $1 AND m.kb_id = $2",
    )
    .bind(merge_id)
    .bind(kb_id)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten();
    if let Err(e) = utopia_store::resolution::revert_merge(&state.pool, kb_id, merge_id).await {
        if utopia_store::token_agent_actions::merge_already_reverted(&e) {
            return Err(utopia_core::AppError::CodedConflict {
                code: "already_decided",
                message: "This merge was already reverted.".into(),
            });
        }
        return Err(e);
    }
    if let Some((s, t)) = snap {
        record_approval(
            state,
            kb_id,
            user_id,
            "merge.revert",
            "merge",
            merge_id,
            json!({ "source": s, "target": t, "agent_batch": batch_id }),
        )
        .await;
    }
    crate::governance::after_revert(state, kb_id, merge_id, user_id).await;
    Ok(json!({ "outcome": "applied" }))
}

/// 把令牌的一批做出的合并整批撤回（0076）：`POST /review/agent-batches/{batch_id}/revert`。
///
/// 编辑者的写令牌与登录的编辑者都能调。令牌走 `review_ops::revert_batch`——每一次合并
/// 经 `review_ops::revert_merge`；人走人的撤回路径（[`revert_as_person`]），一次合并一次。
/// 两边回的是同一个形状：`{batch_id, reverted, outcomes}`，一次合并一条结果。尽力而为：
/// 已经撤回的是那一条的 `already_decided`，这一批里分开的、留成提议的不碰。这个库的台账里
/// 没有哪一行带着这个号是 404。Review 页上没有它的按钮。
///
/// 请求体按原始字节收下，只有令牌那一支去解析（空的，或 `{"rationale": "…"}`）：
/// 与单笔撤回的路由同一个规矩
pub async fn agent_batch_revert(
    State(state): State<AppState>,
    caller: SessionOrToken,
    Path((kb_id, batch_id)): Path<(Uuid, Uuid)>,
    body: axum::body::Bytes,
) -> ApiResult<Json<serde_json::Value>> {
    let user = caller.user;
    if let Some(token) = caller.pat {
        let caller = ReviewCaller { user, token, kb_id };
        let body = if body.iter().all(u8::is_ascii_whitespace) {
            TokenRevertBody::default()
        } else {
            match serde_json::from_slice::<TokenRevertBody>(&body) {
                Ok(body) => body,
                Err(_) => {
                    // 先说「你写不了」，再说「你写错了」
                    review_ops::authorize_write(&state, &caller).await?;
                    return Err(utopia_core::AppError::invalid(
                        "invalid_body",
                        "The request body must be empty or a JSON object such as \
                          {\"rationale\": \"why\"}",
                    )
                    .into());
                }
            }
        };
        return Ok(Json(
            review_ops::revert_batch(&state, &caller, batch_id, body.rationale.as_deref()).await?,
        ));
    }
    require_kb(&state, &user, kb_id, Role::Editor).await?;
    // 与令牌那一支同一把进程内的锁：两个同时到的整批撤回排队，后到的每一条都是
    // `already_decided`，而不是两边各跑一遍同一次合并的撤回
    let _turn = review_ops::unit_revert_turn().await;
    let merges = utopia_store::token_agent_actions::batch_merges(&state.pool, kb_id, batch_id)
        .await?
        .ok_or(utopia_core::AppError::NotFound)?;
    let mut outcomes = Vec::with_capacity(merges.len());
    for merge_id in merges {
        let answered = revert_as_person(&state, kb_id, user.id, batch_id, merge_id).await;
        outcomes.push(review_ops::unit_revert_outcome(merge_id, answered).await);
    }
    state.emit_review(kb_id);
    Ok(Json(review_ops::unit_revert_answer(batch_id, outcomes)))
}

/// 人确认一条事实。写令牌也能调（0076）：它**只留一条提议**（`review_ops::propose_fact`），
/// 事实不动，等登录的人在 Review 里答。令牌在进下面这一段之前就离开了处理函数；会话照旧
/// 跑 [`fact_confirmation`]，回答与审计行同从前一个字不差
pub async fn confirm_fact(
    State(state): State<AppState>,
    caller: SessionOrToken,
    Path((kb_id, fact_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    let user = caller.user;
    if let Some(token) = caller.pat {
        let caller = ReviewCaller { user, token, kb_id };
        let answer = review_ops::propose_fact(&state, &caller, fact_id, "confirm", None).await?;
        return Ok(Json(answer));
    }
    require_kb(&state, &user, kb_id, Role::Editor).await?;
    fact_confirmation(&state, kb_id, &user, fact_id, None).await?;
    Ok(Json(json!({ "ok": true })))
}

/// 人确认一条事实的那几句，从会话处理函数里原样挪出来：快照、带守卫的 UPDATE、审计行、
/// 刷新。会话调它时 `proposal` 是 None，审计 detail 与从前一个键不差；批准令牌的提议时
/// 带上提议的 id，detail 里多一个 `agent_proposal`。actor 两种情形都是这个人，detail 里
/// 不放 `token_id`
async fn fact_confirmation(
    state: &AppState,
    kb_id: Uuid,
    user: &utopia_core::models::User,
    fact_id: Uuid,
    proposal: Option<Uuid>,
) -> Result<(), utopia_core::AppError> {
    let snap = fact_snapshot(state, kb_id, fact_id).await;
    utopia_store::graph::confirm_fact(&state.pool, kb_id, fact_id).await?;
    if let Some(mut d) = snap {
        if let Some(proposal) = proposal {
            d["agent_proposal"] = json!(proposal);
        }
        let _ = utopia_store::audit::record(
            &state.pool,
            Some(kb_id),
            user.id,
            "fact.confirm",
            "fact",
            Some(fact_id),
            d,
        )
        .await;
    }
    state.emit_review(kb_id);
    Ok(())
}

/// 人拒绝一条事实。写令牌也能调（0076）：与确认一样只留一条提议；会话照旧跑
/// [`fact_rejection`]
pub async fn reject_fact(
    State(state): State<AppState>,
    caller: SessionOrToken,
    Path((kb_id, fact_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    let user = caller.user;
    if let Some(token) = caller.pat {
        let caller = ReviewCaller { user, token, kb_id };
        let answer = review_ops::propose_fact(&state, &caller, fact_id, "reject", None).await?;
        return Ok(Json(answer));
    }
    require_kb(&state, &user, kb_id, Role::Editor).await?;
    fact_rejection(&state, kb_id, &user, fact_id, None).await?;
    Ok(Json(json!({ "ok": true })))
}

/// 人拒绝一条事实的那几句，从会话处理函数里原样挪出来；与 [`fact_confirmation`] 同一个
/// 约定：`proposal` 只往审计 detail 里多写一个 `agent_proposal`
async fn fact_rejection(
    state: &AppState,
    kb_id: Uuid,
    user: &utopia_core::models::User,
    fact_id: Uuid,
    proposal: Option<Uuid>,
) -> Result<(), utopia_core::AppError> {
    let snap = fact_snapshot(state, kb_id, fact_id).await;
    utopia_store::graph::reject_fact(&state.pool, kb_id, fact_id).await?;
    if let Some(mut d) = snap {
        if let Some(proposal) = proposal {
            d["agent_proposal"] = json!(proposal);
        }
        let _ = utopia_store::audit::record(
            &state.pool,
            Some(kb_id),
            user.id,
            "fact.reject",
            "fact",
            Some(fact_id),
            d,
        )
        .await;
    }
    state.emit_review(kb_id);
    Ok(())
}

/// 令牌撤回时可以带的那一句理由。会话不读请求体
#[derive(Deserialize, Default)]
struct TokenRevertBody {
    #[serde(default)]
    rationale: Option<String>,
}

/// 撤回一次合并。写令牌也能撤（0076）：令牌走 `review_ops::revert_merge`——令牌 agent
/// 做的合并直接撤，人或治理 agent 做的只留一条提议，不碰保险丝；它在进下面这一段之前
/// 就离开了处理函数。会话照旧跑下面这一段。
///
/// 请求体按原始字节收下：会话从来不带请求体、也不读它，这里不能因为换了提取器就开始
/// 挑它的毛病。只有令牌那一支去解析——空的就是没有理由，`{"rationale": "…"}` 是理由
pub async fn revert_merge(
    State(state): State<AppState>,
    caller: SessionOrToken,
    Path((kb_id, merge_id)): Path<(Uuid, Uuid)>,
    body: axum::body::Bytes,
) -> ApiResult<Json<serde_json::Value>> {
    let user = caller.user;
    if let Some(token) = caller.pat {
        let caller = ReviewCaller { user, token, kb_id };
        let body = if body.iter().all(u8::is_ascii_whitespace) {
            TokenRevertBody::default()
        } else {
            match serde_json::from_slice::<TokenRevertBody>(&body) {
                Ok(body) => body,
                Err(_) => {
                    // 先说「你写不了」，再说「你写错了」：与别的令牌写入同一个次序
                    review_ops::authorize_write(&state, &caller).await?;
                    return Err(utopia_core::AppError::invalid(
                        "invalid_body",
                        "The request body must be empty or a JSON object such as \
                          {\"rationale\": \"why\"}",
                    )
                    .into());
                }
            }
        };
        return Ok(Json(
            review_ops::revert_merge(&state, &caller, merge_id, body.rationale.as_deref(), None)
                .await?,
        ));
    }
    require_kb(&state, &user, kb_id, Role::Editor).await?;
    let snap: Option<(String, String)> = sqlx::query_as(
        "SELECT s.canonical_name, t.canonical_name
         FROM entity_merges m
         JOIN entities s ON s.id = m.source_id
         JOIN entities t ON t.id = m.target_id
         WHERE m.id = $1 AND m.kb_id = $2",
    )
    .bind(merge_id)
    .bind(kb_id)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten();
    utopia_store::resolution::revert_merge(&state.pool, kb_id, merge_id).await?;
    if let Some((s, t)) = snap {
        let _ = utopia_store::audit::record(
            &state.pool,
            Some(kb_id),
            user.id,
            "merge.revert",
            "merge",
            Some(merge_id),
            json!({ "source": s, "target": t }),
        )
        .await;
    }
    // 撤的是 agent 自己裁的合并：那一行记成 reverted，并看保险丝（0025）
    crate::governance::after_revert(&state, kb_id, merge_id, user.id).await;
    state.emit_review(kb_id);
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct ManualMergeBody {
    pub source: Uuid,
    pub target: Uuid,
    /// 什么让你把它们并起来（0026）。可空
    #[serde(default)]
    pub rationale: Option<String>,
}

/// 手动合并（实体面板"Merge into…"入口）。写令牌也能合（0076）：令牌走
/// `review_ops::merge_entities`，先过服务端的闸门，被留下的只留一条提议；它在进下面
/// 这一段之前就离开了处理函数。会话照旧跑下面这一段，不问闸门
pub async fn manual_merge(
    State(state): State<AppState>,
    caller: SessionOrToken,
    Path(kb_id): Path<Uuid>,
    Json(body): Json<ManualMergeBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let user = caller.user;
    if let Some(token) = caller.pat {
        let caller = ReviewCaller { user, token, kb_id };
        return Ok(Json(
            review_ops::merge_entities(
                &state,
                &caller,
                body.source,
                body.target,
                body.rationale.as_deref(),
            )
            .await?,
        ));
    }
    require_kb(&state, &user, kb_id, Role::Editor).await?;
    let snap: Option<(String,)> = sqlx::query_as(
        "SELECT s.canonical_name || ' → ' || t.canonical_name
         FROM entities s, entities t WHERE s.id = $1 AND t.id = $2",
    )
    .bind(body.source)
    .bind(body.target)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten();
    let why = body
        .rationale
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let merge_id = utopia_store::resolution::merge_entities(
        &state.pool,
        kb_id,
        body.source,
        body.target,
        Some(user.id),
        why.unwrap_or("manual merge"),
    )
    .await?;
    if let Some((pair,)) = snap {
        let (s, t) = pair.split_once(" → ").unwrap_or((pair.as_str(), ""));
        let _ = utopia_store::audit::record(
            &state.pool,
            Some(kb_id),
            user.id,
            "merge.manual",
            "merge",
            Some(merge_id),
            json!({ "source": s, "target": t, "why": why }),
        )
        .await;
    }
    state.emit_review(kb_id);
    Ok(Json(json!({ "merge_id": merge_id })))
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    #[serde(default)]
    pub page: i64,
    #[serde(default = "default_history_per")]
    pub per: i64,
}

fn default_history_per() -> i64 {
    20
}

/// 决策台账：review 域的审计事件，服务端分页。
///
/// 个人令牌也能读（0076），走 `review_ops::history`，拿到的是同一个 `{events, total}`；
/// 会话照旧跑下面这一段
pub async fn history(
    State(state): State<AppState>,
    caller: SessionOrToken,
    Path(kb_id): Path<Uuid>,
    Query(q): Query<HistoryQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let user = caller.user;
    if let Some(token) = caller.pat {
        let caller = ReviewCaller { user, token, kb_id };
        return Ok(Json(
            review_ops::history(&state, &caller, Some(q.page), Some(q.per)).await?,
        ));
    }
    require_kb(&state, &user, kb_id, Role::Viewer).await?;
    let per = q.per.clamp(1, 100);
    let page = q.page.max(0);
    let (events, total) =
        utopia_store::audit::review_history(&state.pool, kb_id, per, page * per).await?;
    Ok(Json(json!({ "events": events, "total": total })))
}

/// 审核台的总览（#377）：等着办的、办过的、库的成色。三段聚合，一次返回。
///
/// 个人令牌也能读（0076），走 `review_ops::summary`；会话照旧
pub async fn summary(
    State(state): State<AppState>,
    caller: SessionOrToken,
    Path(kb_id): Path<Uuid>,
) -> ApiResult<Json<utopia_core::models::ReviewSummary>> {
    let user = caller.user;
    if let Some(token) = caller.pat {
        let caller = ReviewCaller { user, token, kb_id };
        return Ok(Json(review_ops::summary(&state, &caller).await?));
    }
    require_kb(&state, &user, kb_id, Role::Viewer).await?;
    Ok(Json(
        utopia_store::review_summary::summary(&state.pool, kb_id).await?,
    ))
}

#[derive(Deserialize)]
pub struct DecideMappingReq {
    /// confirmed | rejected
    pub status: String,
}

/// 对一条语义层映射表态（0011）。
///
/// **改状态不删行**：确认发生过、拒绝也发生过。而拒绝留痕当下就有用——
/// 下一轮探索会再次算出被拒绝过的那条，`propose` 的 `WHERE status = 'proposed'`
/// 据此不把它刷回待看。
///
/// 写令牌也能定（0076）：裁定表里映射是「直接落地」，令牌走 `review_ops::decide_mapping`——
/// 过限速、状态与台账行、审计行一个事务。令牌在进下面这一段之前就离开了处理函数；会话
/// 照旧跑下面的代码，回答与审计行同从前一个字不差
pub async fn decide_mapping(
    State(state): State<AppState>,
    caller: SessionOrToken,
    Path((kb_id, mapping_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<DecideMappingReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let user = caller.user;
    if let Some(token) = caller.pat {
        let caller = ReviewCaller { user, token, kb_id };
        let answer = review_ops::decide_mapping(&state, &caller, mapping_id, &req.status).await?;
        return Ok(Json(answer));
    }
    require_kb(&state, &user, kb_id, Role::Editor).await?;
    if !matches!(req.status.as_str(), "confirmed" | "rejected") {
        return Err(utopia_core::AppError::invalid(
            "bad_status",
            "status must be confirmed or rejected",
        )
        .into());
    }
    utopia_store::mappings::decide(&state.pool, kb_id, mapping_id, &req.status, user.id).await?;
    let _ = utopia_store::audit::record(
        &state.pool,
        Some(kb_id),
        user.id,
        "mapping.decided",
        "concept_mapping",
        Some(mapping_id),
        json!({ "status": req.status }),
    )
    .await;
    state.emit_review(kb_id);
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct DecideViolationReq {
    /// fact_retracted | fact_closed | axiom_relaxed | accepted
    pub resolution: String,
    /// `fact_closed` 必填：旧断言在哪一天结束
    #[serde(default)]
    pub close_at: Option<chrono::DateTime<chrono::Utc>>,
    /// `close_at` 的精度（year | month | day），不给按日
    #[serde(default)]
    pub close_at_precision: Option<String>,
    /// `fact_retracted` 时撤哪条：双事实与环上的违规必填，单事实的可省（#202）
    #[serde(default)]
    pub fact_id: Option<Uuid>,
}

/// 人裁决一处公理违规。
///
/// **三个出路而不是两个。** `axiom_relaxed` 是这一档独有的：矛盾可能出在定义
/// 而不是数据——用户导的本体把某个属性声明成反对称，而他自己的语料里那关系
/// 其实双向。这时该改的是本体，不是二十条事实。
///
/// **「数据错了」真的撤事实**（#202）：此前只记决定，事实照样活在图里，而队列又
/// 不再提它。撤哪条由请求指名（`fact_id`），单事实的种类可省；撤完重算一遍检查，
/// 那条事实牵连的其它违规一并清掉。改公理仍然走本体页——那是另一个页面的事。
///
/// 写令牌也能调（0076）：它**只留一条提议**（`review_ops::propose_violation`），违规行与
/// 事实都不动，等登录的人在 Review 里答。令牌在进下面这一段之前就离开了处理函数；会话
/// 照旧跑 [`violation_decision`]，回答与审计行同从前一个字不差
pub async fn decide_violation(
    State(state): State<AppState>,
    caller: SessionOrToken,
    Path((kb_id, violation_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<DecideViolationReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let user = caller.user;
    if let Some(token) = caller.pat {
        let caller = ReviewCaller { user, token, kb_id };
        let answer = review_ops::propose_violation(&state, &caller, violation_id, &req).await?;
        return Ok(Json(answer));
    }
    require_kb(&state, &user, kb_id, Role::Editor).await?;
    violation_decision(&state, kb_id, &user, violation_id, &req, None).await?;
    Ok(Json(json!({ "ok": true })))
}

/// 人裁一处公理违规的那几句，从会话处理函数里原样挪出来：校验、撤或闭合事实、记决定、
/// 让派生落地、重算检查、审计行、刷新。会话调它时 `proposal` 是 None，一切同从前；批准
/// 令牌的提议时带上提议的 id，这一次裁决写下的每一行审计（`violation.decided`，以及撤、
/// 闭合事实时的 `fact.reject` / `fact.close`）的 detail 里多一个 `agent_proposal`。actor
/// 两种情形都是这个人，detail 里不放 `token_id`。
///
/// 这几步不是一个事务：事实撤了、决定记了之后，后面的重算还可能出错。批准提议时它报的错
/// 原样报出去，提议留着——不折成「过时」（已经落下的那几步是这个人的，不是没发生）
async fn violation_decision(
    state: &AppState,
    kb_id: Uuid,
    user: &utopia_core::models::User,
    violation_id: Uuid,
    req: &DecideViolationReq,
    proposal: Option<Uuid>,
) -> Result<(), utopia_core::AppError> {
    if !matches!(
        req.resolution.as_str(),
        "fact_retracted" | "fact_closed" | "axiom_relaxed" | "accepted"
    ) {
        return Err(utopia_core::AppError::invalid(
            "bad_resolution",
            "resolution 只能是 fact_retracted、fact_closed、axiom_relaxed 或 accepted",
        ));
    }
    let row: Option<(String, Uuid, Uuid, Vec<Uuid>)> = sqlx::query_as(
        "SELECT kind, left_fact, right_fact, path FROM axiom_violations
          WHERE id = $1 AND kb_id = $2 AND status = 'open'",
    )
    .bind(violation_id)
    .bind(kb_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(utopia_core::AppError::Db)?;
    let Some((kind, left, right, path)) = row else {
        return Err(utopia_core::AppError::NotFound);
    };
    let repaired = kind == "derived_contradiction";
    // 撤事实的那条路自己会把违规标成 resolved；其余出路在下面统一记
    let mut decided = false;
    match (repaired, req.resolution.as_str()) {
        (_, "fact_retracted") => {
            let Some(target) =
                utopia_store::reasoning::pick_retraction(left, right, &path, req.fact_id)
            else {
                return Err(utopia_core::AppError::invalid(
                    "fact_required",
                    "这处违规涉及多条事实，要说撤哪一条，且只能是它列出的那几条",
                ));
            };
            let snap = fact_snapshot(state, kb_id, target).await;
            utopia_store::reasoning::retract_from_violation(
                &state.pool,
                kb_id,
                violation_id,
                Some(target),
                user.id,
            )
            .await?;
            decided = true;
            if let Some(mut d) = snap {
                if let Some(proposal) = proposal {
                    d["agent_proposal"] = json!(proposal);
                }
                let _ = utopia_store::audit::record(
                    &state.pool,
                    Some(kb_id),
                    user.id,
                    "fact.reject",
                    "fact",
                    Some(target),
                    d,
                )
                .await;
            }
        }
        (true, "fact_closed") => {
            let Some(at) = req.close_at else {
                return Err(utopia_core::AppError::invalid(
                    "close_at_required",
                    "fact_closed 要给出结束日期",
                ));
            };
            let open: Option<(Uuid,)> = sqlx::query_as(
                "SELECT id FROM facts
                  WHERE id = $1 AND invalidated_at IS NULL AND valid_to IS NULL",
            )
            .bind(left)
            .fetch_optional(&state.pool)
            .await
            .map_err(utopia_core::AppError::Db)?;
            if open.is_none() {
                return Err(utopia_core::AppError::invalid(
                    "not_open",
                    "这条断言已有结束日期，或已被撤",
                ));
            }
            let precision = world_precision(req.close_at_precision.as_deref())?;
            let snap = fact_snapshot(state, kb_id, left).await;
            utopia_store::temporal::close_superseded(&state.pool, left, at, precision).await?;
            if let Some(mut d) = snap {
                d["valid_to"] = json!(at.to_rfc3339());
                d["valid_to_precision"] = json!(precision);
                if let Some(proposal) = proposal {
                    d["agent_proposal"] = json!(proposal);
                }
                let _ = utopia_store::audit::record(
                    &state.pool,
                    Some(kb_id),
                    user.id,
                    "fact.close",
                    "fact",
                    Some(left),
                    d,
                )
                .await;
            }
        }
        (false, "fact_closed") => {
            return Err(utopia_core::AppError::invalid(
                "bad_resolution",
                "fact_closed 只用于 derived_contradiction",
            ));
        }
        _ => {}
    }
    if !decided {
        utopia_store::reasoning::decide(&state.pool, kb_id, violation_id, &req.resolution, user.id)
            .await?;
    }
    // 路清了就让派生落地，人不必再去点一次「推一遍」。撤与闭合把断言挪开了，
    // 认可则在 materialize 里放行
    if repaired {
        utopia_store::reasoning::materialize(&state.pool, kb_id).await?;
    }
    // 撤掉的事实可能还挂在别的违规里：重算一遍，那些行随之清掉，队列不留死账
    if req.resolution == "fact_retracted" {
        utopia_store::reasoning::run(&state.pool, kb_id).await?;
    }
    let mut detail = json!({ "resolution": req.resolution });
    if let Some(proposal) = proposal {
        detail["agent_proposal"] = json!(proposal);
    }
    let _ = utopia_store::audit::record(
        &state.pool,
        Some(kb_id),
        user.id,
        "violation.decided",
        "axiom_violation",
        Some(violation_id),
        detail,
    )
    .await;
    state.emit_review(kb_id);
    Ok(())
}

/// 手动跑一遍一致性检查。
///
/// **同步跑而不是排任务**：这是纯计算，没有模型调用也没有网络——一个几万条
/// 事实的库跑下来是毫秒级。排进任务队列只会让人点完按钮盯着一个"排队中"，
/// 而队列真正要解决的是"这活儿要跑几分钟"。
///
/// `predicates_with_axioms` 一并回给前端：**零和零的含义不同**。没有公理时
/// 结论是"没有判据"，界面该说"先导一份带公理的本体"，而不是"未发现矛盾"。
///
/// 写令牌也能跑（0076）：裁定表里这一行是「直接落地」，令牌走 `review_ops::run_check`——
/// 过限速，跑的就是下面的 [`consistency_run`]，跑完写一行台账与带令牌名字的审计行。会话
/// 照旧：回答与审计行同从前一个字不差
pub async fn run_consistency_check(
    State(state): State<AppState>,
    caller: SessionOrToken,
    Path(kb_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let user = caller.user;
    if let Some(token) = caller.pat {
        let caller = ReviewCaller { user, token, kb_id };
        return Ok(Json(
            review_ops::run_check(&state, &caller, "consistency").await?,
        ));
    }
    require_kb(&state, &user, kb_id, Role::Editor).await?;
    let (detail, body) = consistency_run(&state, kb_id).await?;
    let _ = utopia_store::audit::record(
        &state.pool,
        Some(kb_id),
        user.id,
        "consistency.checked",
        "knowledge_base",
        Some(kb_id),
        detail,
    )
    .await;
    state.emit_review(kb_id);
    Ok(Json(body))
}

/// 跑一遍一致性检查，回（审计行的 detail，响应体）。从会话处理函数里原样挪出来，没有新
/// 加的锁：会话与令牌（`review_ops::run_check`）跑的是同一段，所以令牌拿到的报告就是人
/// 拿到的那一份。**这里不做授权、不写审计**——谁跑的由调用方记
pub(super) async fn consistency_run(
    state: &AppState,
    kb_id: Uuid,
) -> Result<(serde_json::Value, serde_json::Value), utopia_core::AppError> {
    let report = utopia_store::reasoning::run(&state.pool, kb_id).await?;
    // 本体自洽性一并算。两者判据同源（都是本体声明的公理），分两个按钮
    // 只会让人点两次
    let onto = utopia_store::reasoning::check_ontology(&state.pool, kb_id).await?;
    let detail = json!({
        "edges": report.edges,
        "predicates_with_axioms": report.predicates_with_axioms,
        "found": report.found,
        "inserted": report.inserted,
        "cleared": report.cleared,
        "cycles_capped": report.cycles_capped,
        "defects_found": onto.found,
    });
    let body = json!({
        "edges": report.edges,
        "predicates_with_axioms": report.predicates_with_axioms,
        "found": report.found,
        "inserted": report.inserted,
        "cleared": report.cleared,
        // 环没搜完的谓词数（#642）：不为零时环只报了一部分，界面该说「这个谓词的环
        // 太多，先看公理」，而不是让人以为那就是全部
        "cycles_capped": report.cycles_capped,
        // 本体自己那一档单独回。**不加进 found**：两个数不是一类东西，
        // 加起来之后「3 处矛盾」既可能是三条事实抵触，也可能是本体自己写反了三处
        "classes": onto.classes,
        "defects_found": onto.found,
        "defects_new": onto.inserted,
    });
    Ok((detail, body))
}

#[derive(Deserialize)]
pub struct DecideDefectReq {
    /// fixed | accepted
    pub resolution: String,
}

/// 人对一处本体缺陷表态。
///
/// **两个出路而不是三个**：本体缺陷压根没看数据，所以没有「数据错了」这一条。
///
/// 写令牌也能调（0076）：它**只留一条提议**（`review_ops::propose_defect`），缺陷行不动，
/// 等登录的人在 Review 里答。令牌在进下面这一段之前就离开了处理函数；会话照旧跑
/// [`defect_decision`]，回答与审计行同从前一个字不差
pub async fn decide_defect(
    State(state): State<AppState>,
    caller: SessionOrToken,
    Path((kb_id, defect_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<DecideDefectReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let user = caller.user;
    if let Some(token) = caller.pat {
        let caller = ReviewCaller { user, token, kb_id };
        let answer =
            review_ops::propose_defect(&state, &caller, defect_id, &req.resolution).await?;
        return Ok(Json(answer));
    }
    require_kb(&state, &user, kb_id, Role::Editor).await?;
    defect_decision(&state, kb_id, &user, defect_id, &req, None).await?;
    Ok(Json(json!({ "ok": true })))
}

/// 人对一处本体缺陷表态的那几句，从会话处理函数里原样挪出来：校验、带守卫的 UPDATE、
/// 审计行、刷新。会话调它时 `proposal` 是 None，审计 detail 与从前一个键不差；批准令牌的
/// 提议时带上提议的 id，detail 里多一个 `agent_proposal`——这次裁决是这个人的，也看得出
/// 是从哪条提议来的。actor 两种情形都是这个人，detail 里不放 `token_id`
async fn defect_decision(
    state: &AppState,
    kb_id: Uuid,
    user: &utopia_core::models::User,
    defect_id: Uuid,
    req: &DecideDefectReq,
    proposal: Option<Uuid>,
) -> Result<(), utopia_core::AppError> {
    if !matches!(req.resolution.as_str(), "fixed" | "accepted") {
        return Err(utopia_core::AppError::invalid(
            "bad_resolution",
            "resolution 只能是 fixed 或 accepted",
        ));
    }
    utopia_store::reasoning::decide_defect(&state.pool, kb_id, defect_id, &req.resolution, user.id)
        .await?;
    let mut detail = json!({ "resolution": req.resolution });
    if let Some(proposal) = proposal {
        detail["agent_proposal"] = json!(proposal);
    }
    let _ = utopia_store::audit::record(
        &state.pool,
        Some(kb_id),
        user.id,
        "defect.decided",
        "ontology_defect",
        Some(defect_id),
        detail,
    )
    .await;
    state.emit_review(kb_id);
    Ok(())
}

/// 跑一遍推理（R1）。
///
/// **受 `materialize_inferences` 开关约束。** 默认关，因为这一步往图里加东西，
/// 而 0001 判据 2 说「本体是引导不是执法」——声明可能是错的，不该在用户没表态时
/// 就按它改图。开关关着时不静默跳过：回一个明确的错，界面才说得出为什么没动。
///
/// 同步跑，与一致性检查同一个理由：纯计算，没有模型调用也没有网络。
#[derive(Deserialize)]
pub struct PendingQuery {
    pub chunk_id: Uuid,
}

/// 一句记忆抽出的全部待确认项——对话里那张确认卡按这个取（0015）。
/// Viewer 也能看：看得见提议、看不见按钮，与 Review 页同一口径
pub async fn pending_for_chunk(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(kb_id): Path<Uuid>,
    Query(q): Query<PendingQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    require_kb(&state, &user, kb_id, Role::Viewer).await?;
    let items = utopia_store::pending::for_chunk(&state.pool, kb_id, q.chunk_id).await?;
    Ok(Json(json!({ "items": items })))
}

#[derive(Deserialize)]
pub struct DecidePendingBody {
    /// `confirm` 进账本；`reject` 记进 `rejected_facts`，下一轮重抽不再提
    pub action: String,
}

/// 人对一条待确认事实点头或摇头。
///
/// 两个动作都进决策台账，快照自包含——行删了之后台账上还读得出当时确认的是什么。
/// 确认走与抽取相同的那条路（事实 + 证据 + 时态对账），所以一句「Mira 交给 Devin 了」
/// 点头之后，Mira 那条会像从文档里抽出来时一样被闭合
pub async fn decide_pending(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((kb_id, pending_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<DecidePendingBody>,
) -> ApiResult<Json<serde_json::Value>> {
    require_kb(&state, &user, kb_id, Role::Editor).await?;
    match body.action.as_str() {
        "confirm" => {
            let done = utopia_store::pending::confirm(&state.pool, kb_id, pending_id).await?;
            let _ = utopia_store::audit::record(
                &state.pool,
                Some(kb_id),
                user.id,
                "fact.nod_confirmed",
                "fact",
                Some(done.fact_id),
                done.snapshot,
            )
            .await;
            // 点头落下的开放陈述带着时间词：按文档重排一次时间解析（0045），
            // 幂等，同一篇排着就不重复
            let _ = utopia_store::jobs::enqueue_unless_queued(
                &state.pool,
                "resolve_time",
                json!({ "document_id": done.document_id }),
            )
            .await;
            state.emit_pending(kb_id);
            state.emit_review(kb_id);
            state.emit_graph(kb_id);
            Ok(Json(json!({
                "ok": true,
                "fact_id": done.fact_id,
                "created": done.created,
                "conflicts": done.conflicts,
            })))
        }
        "reject" => {
            let snap = utopia_store::pending::reject(&state.pool, kb_id, pending_id, Some(user.id))
                .await?;
            let _ = utopia_store::audit::record(
                &state.pool,
                Some(kb_id),
                user.id,
                "fact.nod_rejected",
                "pending_fact",
                Some(pending_id),
                snap,
            )
            .await;
            state.emit_pending(kb_id);
            state.emit_review(kb_id);
            Ok(Json(json!({ "ok": true })))
        }
        other => Err(utopia_core::AppError::invalid(
            "unknown_action",
            format!("action must be confirm or reject, got {other}"),
        )
        .into()),
    }
}

// ───────────────────── 令牌自己的两个入口（0076） ─────────────────────
//
// 等点头的事实与治理的一笔，令牌**不从人的路由进**：上面的 `decide_pending`、
// `pending_for_chunk` 与 `agent_answer` 照旧只收 `AuthUser`，个人令牌在提取器那里就是 401。
// 令牌走下面两条 `…/propose` 路由，它们只留一条提议（`review_ops::propose_pending` /
// `propose_governor`），什么都不改；答提议的仍然只能是登录的人。
//
// 两条路由只收令牌：会话来的答 403 `token_required`，什么都不做。请求体按原始字节收下，
// 过了写的检查才解析——被拒绝的调用方先听到「你写不了」，再听到「你写错了」

/// 令牌那一支的请求体：解不出来时先过写的检查（scope、库、角色），再答 `invalid_body`
async fn token_body<T: serde::de::DeserializeOwned>(
    state: &AppState,
    caller: &ReviewCaller,
    body: &[u8],
    shape: &str,
) -> Result<T, utopia_core::AppError> {
    match serde_json::from_slice::<T>(body) {
        Ok(body) => Ok(body),
        Err(_) => {
            review_ops::authorize_write(state, caller).await?;
            Err(utopia_core::AppError::invalid(
                "invalid_body",
                format!("The request body must be a JSON object such as {shape}"),
            ))
        }
    }
}

/// 令牌提议对一条等点头的事实点头或摇头（0076）。**只留提议**：`pending_facts` 那一行不动，
/// 不落事实、不记拒绝；登录的人在 Review 里批准时，那一次点头是这个人的。请求体是
/// `{"action": "confirm" | "reject"}`。会话调这里是 403 `token_required`——人点头走
/// [`decide_pending`]
pub async fn propose_pending(
    State(state): State<AppState>,
    caller: SessionOrToken,
    Path((kb_id, pending_id)): Path<(Uuid, Uuid)>,
    body: axum::body::Bytes,
) -> ApiResult<Json<serde_json::Value>> {
    let user = caller.user;
    let Some(token) = caller.pat else {
        return Err(review_ops::token_required().into());
    };
    let caller = ReviewCaller { user, token, kb_id };
    let body: DecidePendingBody =
        token_body(&state, &caller, &body, r#"{"action": "confirm"}"#).await?;
    Ok(Json(
        review_ops::propose_pending(&state, &caller, pending_id, &body.action).await?,
    ))
}

/// 令牌提议怎么回答治理 agent 的一笔（0076）。**只留提议**：治理的那一行、那一对、保险丝都
/// 不动，不排治理任务；登录的人在 Review 里批准时，跑的是他自己回答治理的那段代码
/// （[`governor_answer`]），不带令牌的理由。请求体是
/// `{"action": "merge" | "keep" | "revert", "rationale": …}`。会话调这里是 403
/// `token_required`——人回答治理走 [`agent_answer`]
pub async fn propose_governor(
    State(state): State<AppState>,
    caller: SessionOrToken,
    Path((kb_id, decision_id)): Path<(Uuid, Uuid)>,
    body: axum::body::Bytes,
) -> ApiResult<Json<serde_json::Value>> {
    let user = caller.user;
    let Some(token) = caller.pat else {
        return Err(review_ops::token_required().into());
    };
    let caller = ReviewCaller { user, token, kb_id };
    let body: AgentAnswerBody = token_body(
        &state,
        &caller,
        &body,
        r#"{"action": "keep", "rationale": "why"}"#,
    )
    .await?;
    Ok(Json(
        review_ops::propose_governor(
            &state,
            &caller,
            decision_id,
            &body.action,
            body.rationale.as_deref(),
        )
        .await?,
    ))
}

/// 跑一遍物化推导。
///
/// 写令牌也能跑（0076）：与一致性检查同一行裁定，令牌走 `review_ops::run_check`——开关
/// 关着答的是同一个 `inference_off`（一行不写），开着就过限速、跑下面的
/// [`inference_run`]、写一行台账与带令牌名字的审计行。会话照旧
pub async fn run_inference(
    State(state): State<AppState>,
    caller: SessionOrToken,
    Path(kb_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let user = caller.user;
    if let Some(token) = caller.pat {
        let caller = ReviewCaller { user, token, kb_id };
        return Ok(Json(
            review_ops::run_check(&state, &caller, "inference").await?,
        ));
    }
    require_kb(&state, &user, kb_id, Role::Editor).await?;
    inference_on(&state, kb_id).await?;
    let (detail, body) = inference_run(&state, kb_id).await?;
    let _ = utopia_store::audit::record(
        &state.pool,
        Some(kb_id),
        user.id,
        "inference.materialized",
        "knowledge_base",
        Some(kb_id),
        detail,
    )
    .await;
    state.emit_graph(kb_id);
    Ok(Json(body))
}

/// 这个库开没开物化推导：没开是 `inference_off`。会话与令牌问的是同一句
pub(super) async fn inference_on(
    state: &AppState,
    kb_id: Uuid,
) -> Result<(), utopia_core::AppError> {
    let on: bool =
        sqlx::query_scalar("SELECT materialize_inferences FROM knowledge_bases WHERE id = $1")
            .bind(kb_id)
            .fetch_one(&state.pool)
            .await
            .map_err(utopia_core::AppError::Db)?;
    if !on {
        return Err(utopia_core::AppError::invalid(
            "inference_off",
            "materialized inference is off for this knowledge base",
        ));
    }
    Ok(())
}

/// 跑一遍物化推导，回（审计行的 detail，响应体）——两样是同一组数。从会话处理函数里原样
/// 挪出来，没有新加的锁；**这里不做授权、不看开关、不写审计**，那三样归调用方
pub(super) async fn inference_run(
    state: &AppState,
    kb_id: Uuid,
) -> Result<(serde_json::Value, serde_json::Value), utopia_core::AppError> {
    let report = utopia_store::reasoning::materialize(&state.pool, kb_id).await?;
    let numbers = json!({
        "rules": report.rules,
        "edges": report.edges,
        "derived": report.derived,
        "inserted": report.inserted,
        "invalidated": report.invalidated,
        "capped": report.capped,
    });
    Ok((numbers.clone(), numbers))
}

#[derive(Deserialize)]
pub struct DecideAlignmentPhraseReq {
    /// 属性的键；空 = 没有属性对得上，陈述留在开放图谱
    #[serde(default)]
    pub property: Option<String>,
    /// forward | reverse；给了属性就得给
    #[serde(default)]
    pub direction: Option<String>,
    /// start | end | none：状态属性下，带着某一刻的陈述那一刻标的是开始、结束，还是都不是
    /// （#966）。不给 = 未知：这样的陈述不算类型化行，签名留在队列里等人补
    #[serde(default)]
    pub marks: Option<String>,
}

/// 人定一条短语签名绑到哪个属性（#725 对齐队列）。写成人的判定，代理此后不再改它；
/// 类型化图谱立刻按新绑定重算。
#[derive(Deserialize)]
pub struct DecideAlignmentRuleReq {
    pub approve: bool,
}

/// 人批或驳一条蕴含规则（0044 决定 3 第五片）。与短语判定同一套：决定和它的后续工作
/// 一次提交，答 202 和 job id。批准且要读数的先排 `read_phrases`（填缓存后自己排物化），
/// 否则直接排物化——驳回也要重算，隐含行得退掉
///
/// 写令牌也能调（0076）：它**只留一条提议**（`review_ops::propose_alignment_rule`），规则行
/// 不动、不排任务，答的是 200 与提议——不是 202，什么都还没收下。令牌在进下面这一段之前
/// 就离开了处理函数；会话照旧跑 [`alignment_rule_decision`]，仍答 202 和 job id
pub async fn decide_alignment_rule(
    State(state): State<AppState>,
    caller: SessionOrToken,
    Path((kb_id, rule_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<DecideAlignmentRuleReq>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    let user = caller.user;
    if let Some(token) = caller.pat {
        let caller = ReviewCaller { user, token, kb_id };
        let answer =
            review_ops::propose_alignment_rule(&state, &caller, rule_id, req.approve).await?;
        return Ok((axum::http::StatusCode::OK, Json(answer)));
    }
    require_kb(&state, &user, kb_id, Role::Editor).await?;
    let job_id = alignment_rule_decision(&state, kb_id, &user, rule_id, &req, None).await?;
    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(json!({ "ok": true, "job_id": job_id, "status": "accepted" })),
    ))
}

/// 人批或驳一条蕴含规则的那几句，从会话处理函数里原样挪出来：读规则、判定连同它的任务
/// 一次提交、审计行、刷新。回排下的任务的 id：**这时候改动只是排着，还没落到图上**。
/// 会话调它时 `proposal` 是 None，审计 detail 与从前一个键不差；批准令牌的提议时带上提议
/// 的 id，detail 里多一个 `agent_proposal`。actor 两种情形都是这个人，detail 里不放
/// `token_id`
async fn alignment_rule_decision(
    state: &AppState,
    kb_id: Uuid,
    user: &utopia_core::models::User,
    rule_id: Uuid,
    req: &DecideAlignmentRuleReq,
    proposal: Option<Uuid>,
) -> Result<i64, utopia_core::AppError> {
    let rule = utopia_store::implication_rules::get(&state.pool, kb_id, rule_id)
        .await?
        .ok_or(utopia_core::AppError::NotFound)?;
    let votes = json!({ "person": if req.approve { "approve" } else { "reject" } });
    let job_id = utopia_store::implication_rules::decide_with_delivery(
        &state.pool,
        kb_id,
        rule_id,
        req.approve,
        &votes,
    )
    .await?
    .ok_or(utopia_core::AppError::NotFound)?;
    let mut detail = json!({ "trigger": rule.trigger, "phrase": rule.phrase, "reading": rule.reading,
                             "approve": req.approve, "job_id": job_id });
    if let Some(proposal) = proposal {
        detail["agent_proposal"] = json!(proposal);
    }
    let _ = utopia_store::audit::record(
        &state.pool,
        Some(kb_id),
        user.id,
        "alignment.rule_decided",
        "implication_rule",
        Some(rule_id),
        detail,
    )
    .await;
    state.emit_review(kb_id);
    Ok(job_id)
}

/// 人定一条短语签名绑到哪个属性。写令牌也能调（0076）：它**只留一条提议**
/// （`review_ops::propose_alignment_phrase`），绑定行不动、不排任务，答的是 200 与提议。
/// 令牌在进下面这一段之前就离开了处理函数；会话照旧跑 [`alignment_phrase_decision`]，
/// 仍答 202 和 job id
pub async fn decide_alignment_phrase(
    State(state): State<AppState>,
    caller: SessionOrToken,
    Path((kb_id, binding_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<DecideAlignmentPhraseReq>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    let user = caller.user;
    if let Some(token) = caller.pat {
        let caller = ReviewCaller { user, token, kb_id };
        let answer =
            review_ops::propose_alignment_phrase(&state, &caller, binding_id, &req).await?;
        return Ok((axum::http::StatusCode::OK, Json(answer)));
    }
    require_kb(&state, &user, kb_id, Role::Editor).await?;
    let job_id = alignment_phrase_decision(&state, kb_id, &user, binding_id, &req, None).await?;
    // 202：收下了，投影在路上。不编一个 typed: {added: 0} 出来——那不是这次请求知道的事
    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(json!({ "ok": true, "job_id": job_id, "status": "accepted" })),
    ))
}

/// 短语判定的请求体说的是什么：绑到哪个属性、方向、那一刻标的是哪一端。会话处理函数里
/// 校验请求体的那几句原样挪到这里——人判定时跑它，令牌留提议之前跑的也是它，所以两边
/// 拿到的是同一个 code、同一句话（`unknown_property`、`unknown_marks`、
/// `marks_needs_state`、`direction_required`）。只读本体，什么都不写
pub(super) async fn phrase_choice<'a>(
    state: &AppState,
    kb_id: Uuid,
    req: &'a DecideAlignmentPhraseReq,
) -> Result<(Option<Uuid>, Option<&'a str>, Option<&'a str>), utopia_core::AppError> {
    let property = match req
        .property
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        None => None,
        Some(key) => {
            let found = utopia_store::ontology::relation_type_views(&state.pool, kb_id)
                .await?
                .into_iter()
                .find(|p| p.key == key)
                .map(|p| (p.id, p.temporal == "state"))
                .ok_or_else(|| {
                    utopia_core::AppError::invalid("unknown_property", "no property with that key")
                })?;
            Some(found)
        }
    };
    let marks = match (
        property,
        req.marks
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    ) {
        (_, None) => None,
        (Some((_, true)), Some(m @ ("start" | "end" | "none"))) => Some(m),
        (Some((_, true)), Some(_)) => {
            return Err(utopia_core::AppError::invalid(
                "unknown_marks",
                "marks is one of start, end or none",
            ))
        }
        (_, Some(_)) => {
            return Err(utopia_core::AppError::invalid(
                "marks_needs_state",
                "only a state property says which end a moment marks",
            ))
        }
    };
    let property = property.map(|(id, _)| id);
    let direction = match (property, req.direction.as_deref()) {
        (None, _) => None,
        (Some(_), Some(d @ ("forward" | "reverse"))) => Some(d),
        (Some(_), _) => {
            return Err(utopia_core::AppError::invalid(
                "direction_required",
                "a bound signature needs a direction: forward or reverse",
            ))
        }
    };
    Ok((property, direction, marks))
}

/// 人定一条短语签名的那几句，从会话处理函数里原样挪出来：读签名、校验请求体
/// （[`phrase_choice`]）、判定连同它的重算任务一次提交、审计行、刷新。回排下的任务的 id：
/// **这时候改动只是排着，还没落到图上**。会话调它时 `proposal` 是 None，审计 detail 与
/// 从前一个键不差；批准令牌的提议时带上提议的 id，detail 里多一个 `agent_proposal`。
/// actor 两种情形都是这个人，detail 里不放 `token_id`
async fn alignment_phrase_decision(
    state: &AppState,
    kb_id: Uuid,
    user: &utopia_core::models::User,
    binding_id: Uuid,
    req: &DecideAlignmentPhraseReq,
    proposal: Option<Uuid>,
) -> Result<i64, utopia_core::AppError> {
    let sig = utopia_store::phrase_bindings::signature_of(&state.pool, kb_id, binding_id)
        .await?
        .ok_or(utopia_core::AppError::NotFound)?;
    let (property, direction, marks) = phrase_choice(state, kb_id, req).await?;
    let votes = json!({ "person": { "property": req.property, "direction": direction,
                                    "marks": marks } });
    // 判定和它的重算任务一次提交（0051）。这里**不再**同步重算：等物化锁占的是池里的
    // 连接，而正在跑的那次对齐可能已经读完最后一遍，谁也不替这条判定投影。一个 job
    // 只在判定提交后可见，worker 读的是当前绑定；屏幕上等的是 `review` / `graph` 事件
    let job_id = utopia_store::phrase_bindings::decide_with_delivery(
        &state.pool,
        kb_id,
        &sig,
        utopia_store::phrase_bindings::Decision {
            relation_type_id: property,
            direction,
            status: if property.is_some() { "bound" } else { "none" },
            votes: &votes,
            decided_by: "person",
            basis: None,
            marks,
            marks_asked: false,
        },
    )
    .await?
    .ok_or_else(|| utopia_core::AppError::Conflict("the decision was not written".into()))?;
    let mut detail = json!({ "phrase": sig.phrase, "property": req.property, "direction": direction,
                             "marks": marks, "job_id": job_id });
    if let Some(proposal) = proposal {
        detail["agent_proposal"] = json!(proposal);
    }
    let _ = utopia_store::audit::record(
        &state.pool,
        Some(kb_id),
        user.id,
        "alignment.phrase_decided",
        "phrase_binding",
        Some(binding_id),
        detail,
    )
    .await;
    state.emit_review(kb_id);
    Ok(job_id)
}

#[derive(Deserialize)]
pub struct DecideAlignmentKindWordReq {
    /// 类的键；空 = 没有类对得上
    #[serde(default)]
    pub class: Option<String>,
}

/// 人定一个类别词绑到哪个类（#725 对齐队列）。绑上的类写到它名下的实体上；两端的类变了，
/// 短语签名跟着变，所以再排一次短语对齐——与判定同一事务排下（`decide_and_apply_human`），
/// 提交之后再排的话，进程在两步之间退出就只剩判定。
///
/// 写令牌也能定（0014 决定 6）：判定记在发令牌的人名下，与网页上点的一样
pub async fn decide_alignment_kind_word(
    State(state): State<AppState>,
    caller: SessionOrToken,
    Path((kb_id, kind_word)): Path<(Uuid, String)>,
    Json(req): Json<DecideAlignmentKindWordReq>,
) -> ApiResult<Json<serde_json::Value>> {
    caller.require_write(Some(kb_id))?;
    let token_id = caller.token_id();
    let SessionOrToken { user, pat } = caller;
    require_kb(&state, &user, kb_id, Role::Editor).await?;
    if let Some(token) = pat {
        // 令牌过了上面两道从前就有的检查（所以主人不够格时仍是那句不带 code 的 403），
        // 之后只有 store 的那一次调用换了：判定连同一行台账一个事务，审计行多带令牌的
        // 名字与台账行的 id。回答同从前；**不过限速**（0076：这条路由不为令牌收窄）
        let caller = ReviewCaller { user, token, kb_id };
        let answer =
            review_ops::kind_word_decision(&state, &caller, &kind_word, req.class.as_deref())
                .await?;
        return Ok(Json(answer));
    }
    let class = kind_word_class(&state, kb_id, req.class.as_deref()).await?;
    let votes = json!({ "person": req.class });
    let written = utopia_store::type_bindings::decide_and_apply_human(
        &state.pool,
        kb_id,
        &kind_word,
        class,
        &votes,
    )
    .await?;
    if !written {
        return Err(utopia_core::AppError::NotFound.into());
    }
    let _ = utopia_store::audit::record(
        &state.pool,
        Some(kb_id),
        user.id,
        "alignment.kind_word_decided",
        "type_binding",
        None,
        utopia_store::audit::via_token(
            json!({ "kind_word": kind_word, "class": req.class }),
            token_id,
        ),
    )
    .await;
    state.emit_review(kb_id);
    state.emit_graph(kb_id);
    Ok(Json(json!({ "ok": true })))
}

/// 请求里的类键对上这个库的哪个类：空 = 没有类对得上（None），认不出的键是
/// `unknown_class`。从类别词的处理函数里原样挪出来，会话与令牌问的是同一句
pub(super) async fn kind_word_class(
    state: &AppState,
    kb_id: Uuid,
    class: Option<&str>,
) -> Result<Option<Uuid>, utopia_core::AppError> {
    match class.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(key) => Ok(Some(
            utopia_store::graph::entity_types(&state.pool, kb_id)
                .await?
                .into_iter()
                .find(|c| c.key == key)
                .map(|c| c.id)
                .ok_or_else(|| {
                    utopia_core::AppError::invalid("unknown_class", "no class with that key")
                })?,
        )),
    }
}

#[derive(Deserialize)]
pub struct DecideErrataReq {
    pub approve: bool,
}

/// 人答勘误 agent 留下的一笔（0044 决定 7）：批了就执行那个动作，否了只记一笔。
///
/// 写令牌也能调（0076）：它**只留一条提议**（`review_ops::propose_errata`），那一笔仍然
/// 留着、事实不动，等登录的人在 Review 里答。令牌在进下面这一段之前就离开了处理函数；
/// 会话照旧跑 [`errata_decision`]，回答与审计行同从前一个字不差
pub async fn decide_errata(
    State(state): State<AppState>,
    caller: SessionOrToken,
    Path((kb_id, action_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<DecideErrataReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let user = caller.user;
    if let Some(token) = caller.pat {
        let caller = ReviewCaller { user, token, kb_id };
        let answer = review_ops::propose_errata(&state, &caller, action_id, req.approve).await?;
        return Ok(Json(answer));
    }
    require_kb(&state, &user, kb_id, Role::Editor).await?;
    errata_decision(&state, kb_id, &user, action_id, &req, None).await?;
    Ok(Json(json!({ "ok": true })))
}

/// 人答勘误 agent 留下的一笔的那几句，从会话处理函数里原样挪出来：store 的执行与记账、
/// 审计行、刷新。会话调它时 `proposal` 是 None，审计 detail 与从前一个键不差；批准令牌的
/// 提议时带上提议的 id，detail 里多一个 `agent_proposal`。actor 两种情形都是这个人，
/// detail 里不放 `token_id`。
///
/// store 的那一步不是一个事务（先执行动作，再把那一笔标成答过）：批准提议时它报的错原样
/// 报出去，提议留着——不折成「过时」
async fn errata_decision(
    state: &AppState,
    kb_id: Uuid,
    user: &utopia_core::models::User,
    action_id: Uuid,
    req: &DecideErrataReq,
    proposal: Option<Uuid>,
) -> Result<(), utopia_core::AppError> {
    let found =
        utopia_store::errata::decide_held(&state.pool, kb_id, action_id, req.approve, user.id)
            .await?;
    if !found {
        return Err(utopia_core::AppError::NotFound);
    }
    let mut detail = json!({ "approve": req.approve });
    if let Some(proposal) = proposal {
        detail["agent_proposal"] = json!(proposal);
    }
    let _ = utopia_store::audit::record(
        &state.pool,
        Some(kb_id),
        user.id,
        "errata.decided",
        "errata_action",
        Some(action_id),
        detail,
    )
    .await;
    state.emit_review(kb_id);
    state.emit_graph(kb_id);
    Ok(())
}
