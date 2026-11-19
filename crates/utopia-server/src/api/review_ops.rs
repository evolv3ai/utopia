//! 令牌走 Review 的共享操作层（0076）：一个操作一个函数，REST 的令牌分支与 MCP 工具
//! 调的是同一个。
//!
//! 每个函数拿到的是已经解出来的调用方（[`ReviewCaller`]：令牌的主人、令牌、哪个库），
//! **授权在函数里面做**，不靠调用它的那一层先问过——两个入口各问一遍，迟早有一边漏。
//! 授权、裁定表、台账、审计都只在这里出现一次。
//!
//! 会话不走这里：`review_routes` 里会话分支原样跑从前的代码（0076 决定 2）。
//!
//! 读的一侧整个开给令牌：十一个列表档、总览、决策台账，以及违规与派生事实的两条证明。
//! 写的一侧只有「把一对重复项分开」这一样；别的裁决、合并、撤销、批量都还关着——
//! 读得到「等你点头」不等于能替人点头。

use serde_json::json;
use utopia_core::models::{ReviewSummary, Role, User};
use utopia_core::AppError;
use utopia_store::execution_gate::{agent_policy, AgentOperation, AgentOutcome};
use uuid::Uuid;

use crate::state::AppState;

/// 经令牌进来的调用方：令牌以发它的人的身份行事（0014 决定 1），所以角色问的是这个人，
/// 令牌只往窄里收（scope 与 `kb_ids`）
pub(crate) struct ReviewCaller {
    pub user: User,
    pub token: utopia_store::tokens::Authenticated,
    pub kb_id: Uuid,
}

/// 这个操作这一刀没开给令牌，或者裁定表没让它直接落地
pub(crate) fn not_open() -> AppError {
    AppError::CodedForbidden {
        code: "token_operation_not_open",
        message: "This Review operation is not open to tokens; a signed-in person decides it"
            .into(),
    }
}

/// 读的检查，与 `documents_routes::require_reader_kb` 同一套：主人至少是 Viewer，
/// 令牌的 `kb_ids` 没把这个库排除掉。够不着的库答 404，和不存在的库同一个回答——
/// 不告诉令牌「那里有个库」
async fn authorize_read(state: &AppState, caller: &ReviewCaller) -> Result<(), AppError> {
    match utopia_store::access::require_kb(&state.pool, &caller.user, caller.kb_id, Role::Viewer)
        .await
    {
        Ok(_) => {}
        Err(AppError::Forbidden) => return Err(AppError::NotFound),
        Err(e) => return Err(e),
    }
    if !caller.token.covers(caller.kb_id) {
        return Err(AppError::NotFound);
    }
    Ok(())
}

/// 写的检查。顺序是契约：先令牌的 scope（`token_read_only`），再它的 `kb_ids`
/// （`token_kb_out_of_scope`），最后才是主人的角色（`token_owner_below_editor`）。
/// 前两种是「换一枚令牌就行」，第三种换令牌没用——所以各带各的 code
pub(crate) async fn authorize_write(
    state: &AppState,
    caller: &ReviewCaller,
) -> Result<(), AppError> {
    crate::auth::require_token_write(&caller.token, Some(caller.kb_id))?;
    match utopia_store::access::require_kb(&state.pool, &caller.user, caller.kb_id, Role::Editor)
        .await
    {
        Ok(_) => Ok(()),
        Err(AppError::Forbidden) => Err(AppError::CodedForbidden {
            code: "token_owner_below_editor",
            message: "This token's owner is not an editor of this knowledge base".into(),
        }),
        Err(e) => Err(e),
    }
}

/// 开给令牌的十一个列表档，就是 Review 页左栏的那十一档。`mappings` 是列表路由认的
/// 第十二个档名，但它不是 Review 的分档，不在这里；`overview` 与 `decisions` 不是列表档，
/// 各走 [`summary`] 与 [`history`]
const TOKEN_SECTIONS: [&str; 11] = [
    "pending",
    "duplicates",
    "conflicts",
    "unconfirmed",
    "lowconf",
    "violations",
    "defects",
    "alignment",
    "errata",
    "agent",
    "merges",
];

/// 一档的一页。回的东西就是会话拿到的那一页——两边调的是同一个
/// `review_routes::queue_page`（同一组 store 函数、同一个 clamp、同一个 `total`），这里
/// 只管授权和「哪些档开给令牌」，档的 `match` 不在这里再写一遍。不在那十一档里的名字
/// （`mappings` 也算）答 `unknown_queue`，不回空列表
pub(crate) async fn list(
    state: &AppState,
    caller: &ReviewCaller,
    section: &str,
    types: Option<&str>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<serde_json::Value, AppError> {
    authorize_read(state, caller).await?;
    if !TOKEN_SECTIONS.contains(&section) {
        return Err(AppError::invalid(
            "unknown_queue",
            format!("no review queue named {section}"),
        ));
    }
    super::review_routes::queue_page(state, caller.kb_id, section, types, limit, offset).await
}

/// 决策台账的一页，与会话从 `GET /review/history` 读到的同一个 `{events, total}`：
/// 同一个 `audit::review_history`、同一个翻页口径（`page` 缺省 0、不小于 0；`per` 缺省 20、
/// 夹到 1 到 100）。台账有自己的 `page`/`per`，不借列表档的 `limit`/`offset`
pub(crate) async fn history(
    state: &AppState,
    caller: &ReviewCaller,
    page: Option<i64>,
    per: Option<i64>,
) -> Result<serde_json::Value, AppError> {
    authorize_read(state, caller).await?;
    let per = per.unwrap_or(20).clamp(1, 100);
    let page = page.unwrap_or(0).max(0);
    let (events, total) =
        utopia_store::audit::review_history(&state.pool, caller.kb_id, per, page * per).await?;
    Ok(json!({ "events": events, "total": total }))
}

/// 一处违规的证明链，与会话从 `GET /violations/{id}/proof` 读到的同一个 `{steps}`：
/// 只有 `derived_contradiction` 展得开，别的种类和认不出的 id 是 `steps: null`，不是错误。
/// 读的检查在取任何一行之前（N3）：够不着的库答 404，不带证据
pub(crate) async fn violation_proof(
    state: &AppState,
    caller: &ReviewCaller,
    violation_id: Uuid,
) -> Result<serde_json::Value, AppError> {
    authorize_read(state, caller).await?;
    let steps =
        utopia_store::reasoning::blocked_proof(&state.pool, caller.kb_id, violation_id).await?;
    Ok(json!({ "steps": steps }))
}

/// 一条派生事实的证明，与会话从 `GET /derived/{id}/proof` 读到的同一个 `{proof}`；
/// 派生已失效或不存在时是 `proof: null`。读的检查与上面同一个，同样在取数之前
pub(crate) async fn derived_proof(
    state: &AppState,
    caller: &ReviewCaller,
    derived_id: Uuid,
) -> Result<serde_json::Value, AppError> {
    authorize_read(state, caller).await?;
    let proof = utopia_store::reasoning::proof(&state.pool, caller.kb_id, derived_id).await?;
    Ok(json!({ "proof": proof }))
}

/// 一对重复项的卡片：与列表里那一条同一个对象（两边的名字、类型、区分语、度数、
/// 事实摘要，以及开着的建议）。读的检查与 `list` 同一个。
///
/// 不是这个库里一对还在等的重复项——认不出的 id、裁过的、别的库的——都是同一个
/// `NotFound`，不带任何证据
pub(crate) async fn item(
    state: &AppState,
    caller: &ReviewCaller,
    review_id: Uuid,
) -> Result<utopia_core::models::ReviewItem, AppError> {
    authorize_read(state, caller).await?;
    utopia_store::resolution::pending_review(&state.pool, caller.kb_id, review_id)
        .await?
        .ok_or(AppError::NotFound)
}

/// 审核台的总览，与会话读到的同一个聚合
pub(crate) async fn summary(
    state: &AppState,
    caller: &ReviewCaller,
) -> Result<ReviewSummary, AppError> {
    authorize_read(state, caller).await?;
    utopia_store::review_summary::summary(&state.pool, caller.kb_id).await
}

/// 经令牌把一对重复项分开。
///
/// 审核行、台账行、审计行由 `keep_duplicate_pair` 在一个事务里写。这里**不调**
/// `governance::after_human_decision`：令牌的决定不是人的先例（0076 决定 9），不排治理
/// 任务，也不替人回答这一对上开着的建议。只推一次刷新，让开着的 Review 页看到队列变了
pub(crate) async fn keep(
    state: &AppState,
    caller: &ReviewCaller,
    review_id: Uuid,
    rationale: Option<&str>,
) -> Result<serde_json::Value, AppError> {
    authorize_write(state, caller).await?;
    // 裁定表在闸门那边。分开没有闸门，所以不带 hold；表给的不是 applied 就不落地
    let outcome = agent_policy(AgentOperation::Keep, None);
    if outcome != AgentOutcome::Applied {
        return Err(not_open());
    }
    let action = utopia_store::token_agent_actions::keep_duplicate_pair(
        &state.pool,
        caller.kb_id,
        review_id,
        caller.user.id,
        caller.token.token_id,
        rationale,
    )
    .await?;
    state.emit_review(caller.kb_id);
    Ok(json!({ "ok": true, "outcome": action.outcome, "action_id": action.id }))
}
