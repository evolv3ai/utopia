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
//! 写的一侧开了三样：「把一对重复项分开」（[`keep`]）；过了服务端闸门的合并——
//! 队列里的一对（[`merge_pair`]）与手动指定源和目标（[`merge_entities`]），闸门留下的
//! 合并什么都不改，只留一条等人答的提议；撤回一次合并（[`revert_merge`]）——令牌 agent
//! 做的合并直接撤，人或治理 agent 做的只留提议。还有成批的两样：一次裁最多 50 对
//! 重复项（[`batch`]），每一对走的就是上面的单笔函数；以及把一批做出的合并整批撤回
//! （[`revert_batch`]）。别的裁决都还关着——读得到「等你点头」不等于能替人点头。
//!
//! 每一笔要落地的写入之前还有两道，都只在 [`guard`] 一处：限速（一枚令牌在一个库里
//! 一小时最多落地 [`TOKEN_WRITES_PER_HOUR`] 笔，过了答 `agent_rate_limited`）与熔断器
//! （它做的合并七天里被人撤回两次，它的合并与撤回就只留提议）。

use serde_json::json;
use utopia_core::models::{ReviewSummary, Role, User};
use utopia_core::AppError;
use utopia_store::execution_gate::{agent_policy, AgentOperation, AgentOutcome, Hold};
use utopia_store::token_agent_actions::{TokenAgentAction, TokenWrite, TokenWriteOutcome};
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

/// 一枚令牌在一个库里、滚动的一小时之内最多落地多少笔写入（0076）。分开、合并、撤回
/// 都算；留成提议的与被拒绝的不算。不是按库可配的——裁定表之外不再多一张表
pub(crate) const TOKEN_WRITES_PER_HOUR: i64 = 200;

/// 限速的那一句拒绝：429，code 是 `agent_rate_limited`
fn rate_limited() -> AppError {
    AppError::RateLimited {
        code: "agent_rate_limited",
        message: format!(
            "This token has finalized {TOKEN_WRITES_PER_HOUR} writes on this knowledge base \
             in the last hour; nothing was changed. Try again later."
        ),
    }
}

/// 每一次要落地的令牌写入之前的两道（0076）：限速，然后熔断器。**只有这一处**——
/// [`keep`]、[`merge_pair`]、[`merge_entities`]、[`revert_merge`] 都在写的检查之后、
/// store 函数之前调它；成批的每一笔、整批撤回的每一次、MCP 的每个写工具走的都是这四个
/// 函数，所以没有哪个入口绕得过去（一枚被限速或跳了熔断器的令牌换一个入口也一样）。
///
/// - 限速：台账上这枚令牌在这个库里最近一小时落地的行数到了
///   [`TOKEN_WRITES_PER_HOUR`]，这一次答 `agent_rate_limited`（429），什么都不改、
///   一行不写。第 200 笔落地，第 201 笔被拒
/// - 熔断器：回这枚令牌此刻是不是只能提议。只有合并与撤回问它；分开照常落地，
///   所以对分开恒回 false，也不去读库
///
/// 限速是落地之前的一次读，不与落地在同一个事务里：同一枚令牌同时在途的几个调用可能
/// 都读到 199，于是一起落地。多出来的至多是那一刻在途的调用数（成批调用是一笔一笔来的，
/// 只算一个），之后的调用照样被拒
async fn guard(
    state: &AppState,
    caller: &ReviewCaller,
    operation: AgentOperation,
) -> Result<bool, AppError> {
    let since = chrono::Utc::now() - chrono::Duration::hours(1);
    let finalized = utopia_store::token_agent_actions::finalized_since(
        &state.pool,
        caller.kb_id,
        caller.token.token_id,
        since,
    )
    .await?;
    if finalized >= TOKEN_WRITES_PER_HOUR {
        return Err(rate_limited());
    }
    match operation {
        AgentOperation::Merge | AgentOperation::RevertMerge => {
            utopia_store::token_agent_actions::breaker_tripped(
                &state.pool,
                caller.kb_id,
                caller.token.token_id,
            )
            .await
        }
        _ => Ok(false),
    }
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
/// 任务，也不替人回答这一对上开着的建议。只推一次刷新，让开着的 Review 页看到队列变了。
/// `batch_id` 是成批调用的号，单笔为 None
pub(crate) async fn keep(
    state: &AppState,
    caller: &ReviewCaller,
    review_id: Uuid,
    rationale: Option<&str>,
    batch_id: Option<Uuid>,
) -> Result<serde_json::Value, AppError> {
    authorize_write(state, caller).await?;
    // 裁定表在闸门那边。分开没有闸门，所以不带 hold；表给的不是 applied 就不落地
    let outcome = agent_policy(AgentOperation::Keep, None);
    if outcome != AgentOutcome::Applied {
        return Err(not_open());
    }
    // 分开也是落地的写入，同样限速；熔断器不管它（跳着的令牌照样分得开）
    guard(state, caller, AgentOperation::Keep).await?;
    let action = utopia_store::token_agent_actions::keep_duplicate_pair_in_batch(
        &state.pool,
        caller.kb_id,
        review_id,
        caller.user.id,
        caller.token.token_id,
        rationale,
        batch_id,
    )
    .await?;
    state.emit_review(caller.kb_id);
    Ok(json!({ "ok": true, "outcome": action.outcome, "action_id": action.id }))
}

/// 这枚令牌的一次写入，交给 store 的样子。`breaker` 是 [`guard`] 刚问出来的：
/// 跳着的话 store 把这一次合并或撤回留成提议（`breaker`）
fn token_write(
    caller: &ReviewCaller,
    rationale: Option<&str>,
    batch_id: Option<Uuid>,
    breaker: bool,
) -> TokenWrite {
    TokenWrite {
        kb_id: caller.kb_id,
        owner_id: caller.user.id,
        token_id: caller.token.token_id,
        rationale: rationale.map(str::to_string),
        batch_id,
        breaker,
    }
}

/// 闸门当时说的那一句，从台账行的 payload 里读回来。store 把 `Hold` 的 `Display`
/// 存在 `hold` 键下（`contradiction CEO of`、`derived 3`）；这里照原样解回去，
/// 好用 `Hold::explain` 的那一句——解释的话只写在闸门那一处
fn hold_said(action: &TokenAgentAction) -> Option<Hold> {
    let said = action.payload.get("hold")?.as_str()?;
    let counted = |rest: &str| rest.trim().parse::<i64>().ok();
    match said {
        "identity" => Some(Hold::Identity),
        "kept_by_person" => Some(Hold::KeptByPerson),
        "reverted_merge" => Some(Hold::RevertedMerge),
        _ => {
            if let Some(predicate) = said.strip_prefix("contradiction ") {
                Some(Hold::Contradiction(predicate.to_string()))
            } else if let Some(n) = said.strip_prefix("derived ") {
                counted(n).map(Hold::Derived)
            } else if let Some(n) = said.strip_prefix("answered ") {
                counted(n).map(Hold::Answered)
            } else {
                None
            }
        }
    }
}

/// 给对面的 agent 看的一句：为什么没落地、现在怎样了。`reason` 是稳定的 code，
/// 这一句只是它的说明，不是契约
fn explanation(action: &TokenAgentAction) -> String {
    let reason = action.gate_reason.as_deref().unwrap_or_default();
    let why = match reason {
        "breaker" => {
            "this token only proposes merges and reverts on this knowledge base for now".to_string()
        }
        // 撤回的两种「只能提议」不是闸门说的，是「这次合并是谁做的」说的
        "merge_by_person" => {
            "a person made this merge, and a token does not revert a person's merge".to_string()
        }
        "merge_by_governor" => {
            "the governor made this merge, and a token does not revert the governor's merge"
                .to_string()
        }
        _ => match hold_said(action) {
            Some(hold) if hold.code() == reason => hold.explain(),
            _ => format!("the server gate held this call ({reason})"),
        },
    };
    format!(
        "Not applied: {why}. Nothing was changed; the call waits as a proposal that a signed-in \
         editor answers in the Review tab."
    )
}

/// 一次令牌合并或撤回的回答（0076）。落地：`merge_id`（合并：做出的那一次；撤回：撤掉的
/// 那一次），提交之后的某一步失败了再带 `follow_up_error`——合并（撤回）已经在了，所以
/// 仍是 200 与 `applied`，不报成失败。
/// 留成提议：`reason`（稳定的 code）与 `explanation`；撞上的是一条已经开着的提议时带
/// `code: already_proposed`，`action_id` 就是那一条的
fn merge_answer(done: &TokenWriteOutcome) -> serde_json::Value {
    let action = &done.action;
    let mut answer = json!({ "ok": true, "outcome": action.outcome, "action_id": action.id });
    if action.outcome == AgentOutcome::Applied.as_str() {
        answer["merge_id"] = json!(action.merge_id);
        if let Some(error) = &done.follow_up_error {
            answer["follow_up_error"] = json!(error);
        }
    } else {
        answer["reason"] = json!(action.gate_reason);
        answer["explanation"] = json!(explanation(action));
        if done.existing {
            answer["code"] = json!("already_proposed");
        }
    }
    answer
}

/// 经令牌合并队列里的一对重复项。
///
/// **闸门不在这里问，也绕不过去**：`merge_duplicate_pair` 自己先问
/// `hold_for_token_merge` 与裁定表，再决定落地还是留成提议（0076：没有不过闸门的令牌
/// 合并）。这里**不调** `resolution::decide_review`——那是人的路，不问闸门；也**不调**
/// `governance::after_human_decision`：令牌的合并不是人的先例，不排治理任务，不替人回答
/// 这一对上开着的建议。`batch_id` 是成批调用的号，单笔为 None
pub(crate) async fn merge_pair(
    state: &AppState,
    caller: &ReviewCaller,
    review_id: Uuid,
    rationale: Option<&str>,
    batch_id: Option<Uuid>,
) -> Result<serde_json::Value, AppError> {
    authorize_write(state, caller).await?;
    let breaker = guard(state, caller, AgentOperation::Merge).await?;
    let done = utopia_store::token_agent_actions::merge_duplicate_pair(
        &state.pool,
        &token_write(caller, rationale, batch_id, breaker),
        review_id,
    )
    .await?;
    state.emit_review(caller.kb_id);
    Ok(merge_answer(&done))
}

/// 一次拒绝在 REST 上的样子：code 与那一句话，**取自 `ApiErr` 渲染出来的响应本身**。
/// 成批调用里每一条的 `code` / `error`、MCP 工具错误里的 `code` / `message` 都从这里来，
/// 所以三处说的是同一个 code——不在这里另写一张错误对照表，那张表迟早先变（以后多一种
/// 错误，这里不用跟着改）。404 在 REST 上没有 code，这里给它 `not_found`；别的没有 code
/// 的错误（治理 agent 正在裁这一对的那句 409）code 为 None。内部错误的细节只留在日志里，
/// 回出去的是「Internal server error」
pub(crate) async fn refused(error: AppError) -> (Option<String>, String) {
    use axum::response::IntoResponse;
    let response = crate::error::ApiErr(error).into_response();
    let status = response.status();
    let body: serde_json::Value = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(serde_json::Value::Null);
    let message = body["error"]
        .as_str()
        .unwrap_or("Internal server error")
        .to_string();
    let code = body["code"]
        .as_str()
        .map(str::to_string)
        .or_else(|| (status == axum::http::StatusCode::NOT_FOUND).then(|| "not_found".to_string()));
    (code, message)
}

/// 令牌的一批最多多少对（0076）。会话的批量上限是 500，那是人在界面上「全选」的量；
/// 令牌的一批小得多：一次调用里能落地多少笔由它封顶，令牌中途被撤销也只跑完这一批
pub(crate) const TOKEN_BATCH_MAX: usize = 50;

/// 经令牌成批裁重复项：1 到 [`TOKEN_BATCH_MAX`] 对，整批一个动作（`merge` 或 `keep`）。
///
/// **这里没有合并、分开、闸门的逻辑**（0076：成批的一笔拿不到单笔拿不到的东西）：每一对
/// 原样交给 [`merge_pair`] 或 [`keep`]，授权、闸门、裁定表、台账、审计都在它们里面，
/// 每一对各问各的。这里只做三件事：整批的两个拒绝、发一个 `batch_id`、把每一对的回答
/// 或错误收成一条结果。
///
/// - 先过写的检查，再看条数与动作：只读令牌问一批 51 条，答的仍是 `token_read_only`
/// - 0 条或超过上限：`batch_size`；动作不是 merge / keep：会话批量今天的那一句（422，
///   没有 code）。这两种都是**整批拒绝**，一对都没碰，台账一行不写
/// - 之后按请求的次序一对一对来，一对的错误只是它自己的结果，不拖累后面的。同一个 id
///   给了两次，第二次拿到的是它那时的状态给的回答（落地过的是 `already_decided`，
///   留成提议的是同一条提议加 `already_proposed`）
/// - 台账或审计写不进去是那一对的错误：单笔函数的事务整个回滚，这里记成没成，不记成成了
///
/// 回 `{batch_id, decided, outcomes}`；`decided` 数的是落地（`applied`）的条数
pub(crate) async fn batch(
    state: &AppState,
    caller: &ReviewCaller,
    ids: &[Uuid],
    action: &str,
    rationale: Option<&str>,
) -> Result<serde_json::Value, AppError> {
    authorize_write(state, caller).await?;
    if ids.is_empty() || ids.len() > TOKEN_BATCH_MAX {
        return Err(AppError::invalid(
            "batch_size",
            format!("between 1 and {TOKEN_BATCH_MAX} ids per batch"),
        ));
    }
    if action != "merge" && action != "keep" {
        // 与 `resolution::decide_reviews` 同一句：会话的批量对认不出的动作就是这么答的
        return Err(AppError::Validation("action must be merge or keep".into()));
    }
    let batch_id = Uuid::now_v7();
    let mut outcomes = Vec::with_capacity(ids.len());
    let mut decided = 0usize;
    for &id in ids {
        let answered = if action == "merge" {
            merge_pair(state, caller, id, rationale, Some(batch_id)).await
        } else {
            keep(state, caller, id, rationale, Some(batch_id)).await
        };
        outcomes.push(match answered {
            Ok(answer) => {
                if answer["outcome"] == AgentOutcome::Applied.as_str() {
                    decided += 1;
                }
                // 单笔的回答里有什么就带什么，没有的键是 null：七个键每一条都在
                let mut outcome = json!({
                    "id": id,
                    "outcome": answer["outcome"],
                    "action_id": answer["action_id"],
                    "merge_id": answer["merge_id"],
                    "reason": answer["reason"],
                    "code": answer["code"],
                    "error": null,
                });
                // 合并已经在了、之后的某一步失败：仍是 applied，那一句照单笔的样子带上
                if let Some(error) = answer.get("follow_up_error") {
                    outcome["follow_up_error"] = error.clone();
                }
                outcome
            }
            Err(e) => {
                let (code, error) = refused(e).await;
                json!({
                    "id": id, "outcome": null, "action_id": null, "merge_id": null,
                    "reason": null, "code": code, "error": error,
                })
            }
        });
    }
    Ok(json!({ "batch_id": batch_id, "decided": decided, "outcomes": outcomes }))
}

/// 同一个进程里整批撤回一批一批地跑。整批撤回是「列出这一批的合并，一条一条撤」，
/// 撤回自己开事务、自己提交（人的那条路是 `resolution::revert_merge`，不归这里改）；
/// 两个调用同时撤同一批，就会两边各跑一遍同一次合并的撤回。排了队，后到的看见的是
/// 已经撤回的合并，每一条都答 `already_decided`。拿进程内的锁而不是数据库的咨询锁：
/// 后者要占着一条连接等另一条（见 `token_agent_actions::keep_duplicate_pair`）。
/// 它管不到的：多个进程之间，以及同时到的一次**单笔**撤回——那两种与今天一样
static UNIT_REVERTING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// 轮到这一次整批撤回。会话的那一支在 `review_routes` 里跑人的撤回路径，也先拿这一把
pub(crate) async fn unit_revert_turn() -> tokio::sync::MutexGuard<'static, ()> {
    UNIT_REVERTING.lock().await
}

/// 整批撤回里一次合并的结果：`{merge_id, outcome, code, error}`，四个键每一条都在
pub(crate) async fn unit_revert_outcome(
    merge_id: Uuid,
    answered: Result<serde_json::Value, AppError>,
) -> serde_json::Value {
    match answered {
        Ok(answer) => json!({
            "merge_id": merge_id, "outcome": answer["outcome"],
            "code": answer["code"], "error": null,
        }),
        Err(e) => {
            let (code, error) = refused(e).await;
            json!({ "merge_id": merge_id, "outcome": null, "code": code, "error": error })
        }
    }
}

/// 整批撤回的回答：`reverted` 数的是这一次撤掉（`applied`）的合并
pub(crate) fn unit_revert_answer(
    batch_id: Uuid,
    outcomes: Vec<serde_json::Value>,
) -> serde_json::Value {
    let reverted = outcomes
        .iter()
        .filter(|o| o["outcome"] == AgentOutcome::Applied.as_str())
        .count();
    json!({ "batch_id": batch_id, "reverted": reverted, "outcomes": outcomes })
}

/// 经令牌把一批做出的合并整批撤回（0076）。
///
/// 与 [`batch`] 一样，这里**没有撤回的逻辑**：台账上这一批落地的每一次合并原样交给
/// [`revert_merge`]，直接撤还是只留提议、撤没撤过，都由它说。尽力而为：一次合并一条
/// 结果，已经撤回的是那一条的 `already_decided`，不拖累别的；这一批里分开的、留成提议的
/// 不在台账的合并行里，碰不到。撤回的台账行与审计行带着同一个 `batch_id`。
///
/// 这个库的台账里没有哪一行带着这个号：`NotFound`（先过写的检查，再说有没有）
pub(crate) async fn revert_batch(
    state: &AppState,
    caller: &ReviewCaller,
    batch_id: Uuid,
    rationale: Option<&str>,
) -> Result<serde_json::Value, AppError> {
    authorize_write(state, caller).await?;
    let _turn = unit_revert_turn().await;
    let merges =
        utopia_store::token_agent_actions::batch_merges(&state.pool, caller.kb_id, batch_id)
            .await?
            .ok_or(AppError::NotFound)?;
    let mut outcomes = Vec::with_capacity(merges.len());
    for merge_id in merges {
        let answered = revert_merge(state, caller, merge_id, rationale, Some(batch_id)).await;
        outcomes.push(unit_revert_outcome(merge_id, answered).await);
    }
    Ok(unit_revert_answer(batch_id, outcomes))
}

/// 经令牌手动合并两个实体：源并进目标，方向是调用方给的。与 [`merge_pair`] 同一套规矩：
/// 授权在这里，闸门在 `merge_entity_pair` 里面，**不调** `resolution::merge_entities`
/// （人的路，不问闸门），不调 `governance::after_human_decision`
pub(crate) async fn merge_entities(
    state: &AppState,
    caller: &ReviewCaller,
    source: Uuid,
    target: Uuid,
    rationale: Option<&str>,
) -> Result<serde_json::Value, AppError> {
    authorize_write(state, caller).await?;
    let breaker = guard(state, caller, AgentOperation::Merge).await?;
    let done = utopia_store::token_agent_actions::merge_entity_pair(
        &state.pool,
        &token_write(caller, rationale, None, breaker),
        source,
        target,
    )
    .await?;
    state.emit_review(caller.kb_id);
    Ok(merge_answer(&done))
}

/// 经令牌撤回一次合并（0076）。
///
/// 授权在这里；「直接撤还是只留提议」在 `token_agent_actions::revert_merge` 里面：台账上
/// 是令牌 agent 做的合并（哪一枚令牌都行）直接撤，人做的、治理 agent 做的只留一条提议
/// （`merge_by_person` / `merge_by_governor`），图不动。
///
/// **不调 `governance::after_revert`**：那是人撤回治理 agent 的合并时记一笔、看保险丝的
/// 地方。令牌的撤回不动保险丝——它撤得动的只有令牌 agent 自己的合并，治理 agent 的合并
/// 它只能提议。也不调 `resolution::revert_merge`：那是人的路，这里只经 store 的令牌入口。
///
/// 撤回只把图放回去：合并还在的那段时间里据它推出的派生事实、记下的违规、导出去的
/// 东西、答出去的话，都不收回；合并前没有类型的存活方留着它拿到的类型，存活的事实留着
/// 合并抄给它的陈述来源（工具说明里写着同样的话）。`batch_id` 是成批调用的号，单笔为 None
pub(crate) async fn revert_merge(
    state: &AppState,
    caller: &ReviewCaller,
    merge_id: Uuid,
    rationale: Option<&str>,
    batch_id: Option<Uuid>,
) -> Result<serde_json::Value, AppError> {
    authorize_write(state, caller).await?;
    let breaker = guard(state, caller, AgentOperation::RevertMerge).await?;
    let done = utopia_store::token_agent_actions::revert_merge(
        &state.pool,
        &token_write(caller, rationale, batch_id, breaker),
        merge_id,
    )
    .await?;
    state.emit_review(caller.kb_id);
    Ok(merge_answer(&done))
}
