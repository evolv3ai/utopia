//! 令牌走 Review 的共享操作层（0076）：一个操作一个函数，REST 的令牌分支与 MCP 工具
//! 调的是同一个。
//!
//! 每个函数拿到的是已经解出来的调用方（[`ReviewCaller`]：令牌的主人、令牌、哪个库），
//! **授权在函数里面做**，不靠调用它的那一层先问过——两个入口各问一遍，迟早有一边漏。
//! 授权、裁定表、台账、审计都只在这里出现一次。
//!
//! 会话不走这里：`review_routes` 里会话分支原样跑从前的代码（0076 决定 2）。
//!
//! 这一刀开给令牌的只有三样：读重复项、读总览、把一对重复项分开。别的档、合并、撤销、
//! 批量都还关着。

use serde_json::json;
use utopia_core::models::{ReviewSummary, Role, User};
use utopia_core::AppError;
use utopia_store::execution_gate::{agent_policy, AgentOperation, AgentOutcome};
use uuid::Uuid;

use super::review_routes::REVIEW_PAGE;
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

/// 一档的一页。这一刀只有 `duplicates`：回的东西与会话拿到的一样——同一组 store 函数、
/// 同一个 clamp（缺省 10，1 到 200；offset 不为负）。别的档答 `unknown_queue`，不回空列表
pub(crate) async fn list(
    state: &AppState,
    caller: &ReviewCaller,
    section: &str,
    types: Option<&str>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<serde_json::Value, AppError> {
    authorize_read(state, caller).await?;
    if section != "duplicates" {
        return Err(AppError::invalid(
            "unknown_queue",
            format!("no review queue named {section}"),
        ));
    }
    let kb_id = caller.kb_id;
    let counts = utopia_store::review::counts(&state.pool, kb_id).await?;
    let limit = limit.unwrap_or(REVIEW_PAGE).clamp(1, 200);
    let offset = offset.unwrap_or(0).max(0);
    let items = utopia_store::resolution::list_reviews(
        &state.pool,
        kb_id,
        utopia_store::resolution::TypeFilter::parse(types)?,
        limit,
        offset,
    )
    .await?;
    Ok(json!({
        "counts": counts,
        "queue": section,
        "items": items,
    }))
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
