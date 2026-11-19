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
//! 每一档里列着的一条也能单独取（[`item`]），取到的就是列表里那个对象。
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

/// 不在开给令牌的档里的名字：与列表路由同一个 code、同一句话
fn unknown_queue(section: &str) -> AppError {
    AppError::invalid("unknown_queue", format!("no review queue named {section}"))
}

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
        return Err(unknown_queue(section));
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

/// 找一条时每次向列表要的行数：列表档自己的上限，决策台账自己的上限
const ITEM_SCAN_QUEUE: i64 = 200;
const ITEM_SCAN_HISTORY: i64 = 100;

/// 这一条是不是要找的那一条。短语、规则和别的档的条目认 `id`；对齐一档的类别词没有
/// `id`，认它的词——**原样比**，不去空白、不折大小写：库里存的是什么，认的就是什么
fn is_the_item(section: &str, item: &serde_json::Value, id: &str, uuid: Option<Uuid>) -> bool {
    if let Some(uuid) = uuid {
        if item["id"].as_str() == Some(uuid.to_string().as_str()) {
            return true;
        }
    }
    section == "alignment" && item["kind"] == "kind_word" && item["kind_word"].as_str() == Some(id)
}

/// 某一档里列着的一条，就是列表里的那个对象（0076）。
///
/// **不另写一条按 id 取的查询，而是到这一档自己的列表里去找**：走的是 [`list`] 走的那个
/// `queue_page`（决策台账走 [`history`] 走的那个 `audit::review_history`），一页一页翻到
/// 找着或翻完。这样回来的对象与列表里那一条逐字段相等是构造出来的，不是两处 SQL 碰巧
/// 写得一样；列表以后多一个字段、换一个判据，这里跟着变。代价是排在后面的条目要多翻
/// 几页——一档等人的东西是几十上百条，不是几十万条。
///
/// 读的检查与 `list` 同一个，**在取任何一行之前**（N3）；回的字段只来自列表，外加违规
/// 一档的 `proof`——那是证明路由（[`violation_proof`]）本来就回的 `steps`：
/// `derived_contradiction` 展得开，别的种类是 `null`，不是错误。别的档一个键也不加。
///
/// 「没有」只有一种回答：认不出的 id、别的库的 id、已经不在这一档里等着的，都是同一个
/// `NotFound`，不带证据。`overview` 没有条目，问它要哪一条都是 `NotFound`。十三档以外的
/// 名字（`mappings` 也算）是 `unknown_queue`。不是 uuid 的 id 只可能是对齐一档的类别词，
/// 在别的档直接是 `NotFound`，不去读库
pub(crate) async fn item(
    state: &AppState,
    caller: &ReviewCaller,
    section: &str,
    id: &str,
) -> Result<serde_json::Value, AppError> {
    authorize_read(state, caller).await?;
    let uuid = id.trim().parse::<Uuid>().ok();
    if section == "overview" {
        return Err(AppError::NotFound);
    }
    if section == "decisions" {
        if uuid.is_none() {
            return Err(AppError::NotFound);
        }
        let mut offset = 0;
        loop {
            let (events, _) = utopia_store::audit::review_history(
                &state.pool,
                caller.kb_id,
                ITEM_SCAN_HISTORY,
                offset,
            )
            .await?;
            let seen = events.len() as i64;
            // 与 `history` 同一种序列化：先整页变成 JSON，再从里面挑
            let mut page = json!(events);
            let found = page.as_array_mut().and_then(|rows| {
                rows.iter_mut()
                    .find(|row| is_the_item(section, row, id, uuid))
                    .map(serde_json::Value::take)
            });
            if let Some(event) = found {
                return Ok(event);
            }
            if seen < ITEM_SCAN_HISTORY {
                return Err(AppError::NotFound);
            }
            offset += ITEM_SCAN_HISTORY;
        }
    }
    if !TOKEN_SECTIONS.contains(&section) {
        return Err(unknown_queue(section));
    }
    if uuid.is_none() && section != "alignment" {
        return Err(AppError::NotFound);
    }
    let mut offset = 0;
    let mut found = loop {
        let mut page = super::review_routes::queue_page(
            state,
            caller.kb_id,
            section,
            None,
            Some(ITEM_SCAN_QUEUE),
            Some(offset),
        )
        .await?;
        let rows = page["items"].as_array_mut();
        let seen = rows.as_ref().map_or(0, |rows| rows.len()) as i64;
        let hit = rows.and_then(|rows| {
            rows.iter_mut()
                .find(|row| is_the_item(section, row, id, uuid))
                .map(serde_json::Value::take)
        });
        if let Some(hit) = hit {
            break hit;
        }
        if seen < ITEM_SCAN_QUEUE {
            return Err(AppError::NotFound);
        }
        offset += ITEM_SCAN_QUEUE;
    };
    if section == "violations" {
        // 违规的 id 一定是 uuid：上面是按它找着的
        let proof = match uuid {
            Some(violation_id) if found["kind"] == "derived_contradiction" => json!(
                utopia_store::reasoning::blocked_proof(&state.pool, caller.kb_id, violation_id)
                    .await?
            ),
            _ => serde_json::Value::Null,
        };
        found["proof"] = proof;
    }
    Ok(found)
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
