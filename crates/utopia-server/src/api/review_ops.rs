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
//! （[`revert_batch`]）。裁定表里「只能提议」的操作走 [`target_open`] 与 [`land_proposal`]：
//! 什么都不改，只留一条等人答的提议——本体缺陷（[`propose_defect`]）是第一种，事实的确认、
//! 拒绝、闭合（[`propose_fact`]）与时态矛盾的裁决（[`propose_conflict`]）照的是它，公理
//! 违规的裁决（[`propose_violation`]）、勘误的回答（[`propose_errata`]）、对齐一档的短语
//! 与规则（[`propose_alignment_phrase`]、[`propose_alignment_rule`]）也是；本体提案不在
//! Review 的列表里，它的读（[`ontology_proposals`]）与拒绝、采纳的提议（[`propose_ontology`]）
//! 走的是它自己的那几条路由。等点头的事实（[`propose_pending`]）与治理 agent 的一笔
//! （[`propose_governor`]）也只留提议，而且不从人的路由进：令牌走它自己的两条 `…/propose`
//! 路由，人的点头与回答治理的路由照旧只收会话。
//! 裁定表里「直接落地」的另外三行也在这里：语义层映射的读（[`mappings`]）与决定
//! （[`decide_mapping`]）、一致性检查与物化推导（[`run_check`]）——都过限速、落地时留一行
//! `applied`；类别词的决定（[`decide_kind_word`]）从前就直接落地，补上了台账行，不限速。
//! 别的裁决都还关着——读得到「等你点头」不等于能替人点头。
//!
//! 每一笔要落地的写入之前还有两道，都只在 [`guard`] 一处：限速（一枚令牌在一个库里
//! 一小时最多落地 [`TOKEN_WRITES_PER_HOUR`] 笔，过了答 `agent_rate_limited`）与熔断器
//! （它做的合并七天里被人撤回两次，它的合并与撤回就只留提议）。

use serde_json::json;
use utopia_core::models::{ReviewSummary, Role, User};
use utopia_core::AppError;
use utopia_store::execution_gate::{agent_policy, AgentOperation, AgentOutcome, Hold};
use utopia_store::review_targets::TargetState;
use utopia_store::token_agent_actions::{
    Proposal, TokenAgentAction, TokenWrite, TokenWriteOutcome,
};
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
/// 映射的决定（[`decide_mapping`]）与两种运行（[`run_check`]）同样先调它再落地；类别词的
/// 决定（[`kind_word_decision`]）按裁定不调，但它的行数在这里的总数里。
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
        // 裁定表里「只能提议」的那些操作：不是闸门拦的，是这种决定本来就归人
        "propose_only" => "a token only proposes this decision; a person makes it".to_string(),
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

// ───────────────────── Review 其余各档：只能提议的操作（0076） ─────────────────────
//
// 裁定表里「只能提议」的每一种操作都是同一个样子：过写的检查、照人的路由校验请求体、
// 问目标还开不开着、留一条提议。后两步是下面两个通用函数；每一档只写自己的那一个
// 函数（校验自己的请求体、说出自己的操作名）。**这里没有一条路径会改图**：留提议只写
// 台账行与 `agent.proposal` 审计行，批准时跑的是人自己的代码，在 `review_routes` 里

/// 目标对这种操作而言还开着才往下走：这个库里没有（别的库的 id 也算）是 `NotFound`，
/// 已经定了是 `already_decided`。读的是带类型的行（`review_targets::target_state`），
/// 不认任何一句错误的原话。`request` 是请求体原样
pub(crate) async fn target_open(
    state: &AppState,
    caller: &ReviewCaller,
    operation: &str,
    target_id: Uuid,
    request: &serde_json::Value,
) -> Result<(), AppError> {
    match utopia_store::review_targets::target_state(
        &state.pool,
        caller.kb_id,
        operation,
        target_id,
        request,
        None,
    )
    .await?
    {
        TargetState::Open => Ok(()),
        TargetState::Missing => Err(AppError::NotFound),
        TargetState::Decided => Err(AppError::CodedConflict {
            code: "already_decided",
            message: "This item was already decided.".into(),
        }),
    }
}

/// 留一条只能提议的提议，回令牌看到的那个回答。
///
/// - **先问裁定表**：`row` 这一行给的不是「只能提议」就不落（`token_operation_not_open`）——
///   表要是哪天改了，这里拒绝，而不是照旧留一条提议
/// - 目标的那句话（`target_label`）与指纹（矛盾、本体提案才有）从 store 取，存进 payload，
///   连同请求体原样（`request`）、动作（`action`）、理由（`why`）
/// - **不调 [`guard`]**：提议不是落地的写入，不算进限速，也不问熔断器（E5）——所以这里
///   永远答不出 `agent_rate_limited`
/// - 同一个库、同一种操作、同一个目标已经有一条开着的：什么都不写，回那一条的 id，
///   带 `code: already_proposed`
///
/// 调用方先过 [`authorize_write`]、校验请求体、问 [`target_open`]，再到这里
#[allow(clippy::too_many_arguments)]
pub(crate) async fn land_proposal(
    state: &AppState,
    caller: &ReviewCaller,
    row: AgentOperation,
    operation: &'static str,
    target_kind: &'static str,
    target_id: Uuid,
    target_key: &str,
    request: serde_json::Value,
    action: Option<&str>,
    rationale: Option<&str>,
) -> Result<serde_json::Value, AppError> {
    if agent_policy(row, None) != AgentOutcome::Proposed {
        return Err(not_open());
    }
    let pool = &state.pool;
    let label = utopia_store::review_targets::target_label(
        pool,
        caller.kb_id,
        operation,
        target_id,
        &request,
    )
    .await?;
    let fingerprint =
        utopia_store::review_targets::fingerprint(pool, caller.kb_id, operation, target_id).await?;
    let why = rationale.map(str::trim).filter(|s| !s.is_empty());
    // 有值的键才出现：没有动作的操作没有 `action`，没给理由就没有 `why`
    let mut payload = json!({ "request": request, "target_label": label });
    if let Some(action) = action {
        payload["action"] = json!(action);
    }
    if let Some(why) = why {
        payload["why"] = json!(why);
    }
    if let Some(fingerprint) = fingerprint {
        payload["fingerprint"] = fingerprint;
    }
    let done = utopia_store::token_agent_actions::propose(
        pool,
        &token_write(caller, why, None, false),
        &Proposal {
            operation,
            target_kind,
            target_id,
            target_key,
            payload,
        },
    )
    .await?;
    state.emit_review(caller.kb_id);
    let action = &done.action;
    let mut answer = json!({
        "ok": true,
        "outcome": action.outcome,
        "action_id": action.id,
        "reason": action.gate_reason,
        "explanation": explanation(action),
    });
    if done.existing {
        answer["code"] = json!("already_proposed");
    }
    Ok(answer)
}

/// 经令牌对一处本体缺陷表态：**只留提议**（裁定表的 Defect 一行）。缺陷行不动，
/// 等登录的人在 Review 里答；批准时跑的是人自己的那段代码（`review_routes`）。
///
/// 次序是契约：写的检查；请求体（`resolution` 只能是 fixed / accepted，否则
/// `bad_resolution`，与人的路由同一个 code）；目标（没有是 404，已经裁过是
/// `already_decided`）；留提议。前三步哪一步没过都一行不写
pub(crate) async fn propose_defect(
    state: &AppState,
    caller: &ReviewCaller,
    defect_id: Uuid,
    resolution: &str,
) -> Result<serde_json::Value, AppError> {
    authorize_write(state, caller).await?;
    if !matches!(resolution, "fixed" | "accepted") {
        return Err(AppError::invalid(
            "bad_resolution",
            "resolution must be fixed or accepted",
        ));
    }
    let request = json!({ "resolution": resolution });
    target_open(state, caller, "defect.decide", defect_id, &request).await?;
    land_proposal(
        state,
        caller,
        AgentOperation::Defect,
        "defect.decide",
        "ontology_defect",
        defect_id,
        &defect_id.to_string(),
        request,
        Some(resolution),
        None,
    )
    .await
}

/// 「终点早于起点」的那一句拒绝，与人闭合一条事实时 store 给的同一个 code、同一句话
fn interval_inverted() -> AppError {
    AppError::invalid(
        "interval_inverted",
        "The start of the interval is after its end.",
    )
}

/// 把一条事实关在 `at`（精度 `precision`）会不会关在它的起点之前。两端各按各的精度截断，
/// 与 `temporal::close_superseded` 同一个比法——这里只读，不改
async fn closes_before_start(
    state: &AppState,
    kb_id: Uuid,
    fact_id: Uuid,
    at: chrono::DateTime<chrono::Utc>,
    precision: &str,
) -> Result<bool, AppError> {
    let start: Option<(Option<chrono::DateTime<chrono::Utc>>, Option<String>)> = sqlx::query_as(
        "SELECT valid_from, valid_from_precision FROM facts WHERE id = $1 AND kb_id = $2",
    )
    .bind(fact_id)
    .bind(kb_id)
    .fetch_optional(&state.pool)
    .await?;
    let at = utopia_store::graph::truncate_to(at, Some(precision));
    Ok(match start {
        Some((Some(from), from_precision)) => {
            at < utopia_store::graph::truncate_to(from, from_precision.as_deref())
        }
        _ => false,
    })
}

/// 经令牌确认、拒绝或闭合一条事实：**只留提议**（裁定表的 ConfirmFact / RejectFact /
/// CloseFact 三行）。事实不动，等登录的人在 Review 里答；批准时跑的是人自己的那段代码
/// （`review_routes`）。`action` 是 confirm / reject / close；`request` 是闭合的请求体，
/// 确认与拒绝没有请求体。
///
/// 次序是契约：写的检查；请求体（闭合日期的精度，`bad_precision`，与人的路由同一个
/// code）；目标（没有是 404，作废了的事实对拒绝与闭合是 `already_decided`）；要读目标才
/// 知道的检查（闭合点早于起点，`interval_inverted`）；留提议。哪一步没过都一行不写。
/// 三种操作各是各的提议：同一条事实上确认与拒绝可以同时开着
pub(crate) async fn propose_fact(
    state: &AppState,
    caller: &ReviewCaller,
    fact_id: Uuid,
    action: &str,
    request: Option<&super::review_routes::CloseFactBody>,
) -> Result<serde_json::Value, AppError> {
    authorize_write(state, caller).await?;
    let (row, operation, action): (AgentOperation, &'static str, &'static str) = match action {
        "confirm" => (AgentOperation::ConfirmFact, "fact.confirm", "confirm"),
        "reject" => (AgentOperation::RejectFact, "fact.reject", "reject"),
        "close" => (AgentOperation::CloseFact, "fact.close", "close"),
        // REST 的三条路由各给各的动作，到不了这里；MCP 工具的 `action` 是调用方写的
        other => {
            return Err(AppError::invalid(
                "unknown_action",
                format!("action must be confirm, reject or close, got {other}"),
            ))
        }
    };
    // 闭合才有请求体。存下的就是那条路由的请求体原样：没给精度就没有那个键
    let closing = match (operation, request) {
        ("fact.close", Some(body)) => {
            let precision =
                super::review_routes::world_precision(body.valid_to_precision.as_deref())?;
            Some((body, precision))
        }
        ("fact.close", None) => {
            return Err(AppError::Validation(
                "valid_to is required to close a fact".into(),
            ))
        }
        _ => None,
    };
    let mut stored = json!({});
    if let Some((body, _)) = closing {
        stored["valid_to"] = json!(body.valid_to);
        if let Some(precision) = &body.valid_to_precision {
            stored["valid_to_precision"] = json!(precision);
        }
    }
    target_open(state, caller, operation, fact_id, &stored).await?;
    if let Some((body, precision)) = closing {
        if closes_before_start(state, caller.kb_id, fact_id, body.valid_to, precision).await? {
            return Err(interval_inverted());
        }
    }
    land_proposal(
        state,
        caller,
        row,
        operation,
        "fact",
        fact_id,
        &fact_id.to_string(),
        stored,
        Some(action),
        None,
    )
    .await
}

/// 经令牌裁一处时态矛盾：**只留提议**（裁定表的 ResolveConflict 一行）。矛盾行与两条事实
/// 都不动，等登录的人在 Review 里答；批准时跑的是人自己的那段代码（`review_routes`）。
///
/// 次序是契约：写的检查；请求体——两个精度（`bad_precision`）、动作（认不出的动作是
/// store 给人的那一句 422）、重定时没给新起点（`new_from_required`），都与人的路由同一个
/// code；目标（没有是 404，已经裁过、或有一条事实已作废是 `already_decided`）；要读目标
/// 才知道的检查——闭合旧事实而新事实没有起点、也没给日期（`close_at_required`），闭合点
/// 早于旧事实的起点或新起点晚于新事实写明的终点（`interval_inverted`），重定时的不是一段
/// 状态（`not_a_state`）；留提议。哪一步没过都一行不写。
///
/// 提议存下矛盾此刻指着的两条事实（指纹，`land_proposal` 从 store 取）：批准时它们换了，
/// 提议就过时了
pub(crate) async fn propose_conflict(
    state: &AppState,
    caller: &ReviewCaller,
    conflict_id: Uuid,
    request: &super::review_routes::ConflictBody,
) -> Result<serde_json::Value, AppError> {
    authorize_write(state, caller).await?;
    let close_precision =
        super::review_routes::world_precision(request.close_at_precision.as_deref())?;
    let new_precision =
        super::review_routes::world_precision(request.new_from_precision.as_deref())?;
    let action: &'static str = match request.action.as_str() {
        "close" => "close",
        "retime_new" => "retime_new",
        "keep" => "keep",
        "reject_new" => "reject_new",
        // 与 `temporal::resolve_review_conflict` 对认不出的动作说的同一句
        other => return Err(AppError::Validation(format!("Unknown resolution: {other}"))),
    };
    if action == "retime_new" && request.new_from.is_none() {
        return Err(AppError::invalid(
            "new_from_required",
            "A new start is required when retiming a fact.",
        ));
    }
    // 存下的就是请求体原样：没给的键不出现
    let mut stored = json!({ "action": request.action });
    if let Some(at) = request.close_at {
        stored["close_at"] = json!(at);
    }
    if let Some(precision) = &request.close_at_precision {
        stored["close_at_precision"] = json!(precision);
    }
    if let Some(at) = request.new_from {
        stored["new_from"] = json!(at);
    }
    if let Some(precision) = &request.new_from_precision {
        stored["new_from_precision"] = json!(precision);
    }
    target_open(state, caller, "conflict.resolve", conflict_id, &stored).await?;

    // 目标开着：矛盾在这个库里，两条事实都活着。下面的检查读它们，不改
    let (old, new): (Uuid, Uuid) = sqlx::query_as(
        "SELECT old_fact_id, new_fact_id FROM fact_conflicts WHERE id = $1 AND kb_id = $2",
    )
    .bind(conflict_id)
    .bind(caller.kb_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    #[allow(clippy::type_complexity)]
    let (new_from, new_from_precision, new_to, new_to_precision, end_derived, predicate): (
        Option<chrono::DateTime<chrono::Utc>>,
        Option<String>,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<String>,
        bool,
        Option<Uuid>,
    ) = sqlx::query_as(
        "SELECT valid_from, valid_from_precision, valid_to, valid_to_precision, end_derived,
                predicate_id
           FROM facts WHERE id = $1 AND kb_id = $2",
    )
    .bind(new)
    .bind(caller.kb_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    match action {
        "close" => {
            // 人给了日期就用人给的；没给就闭合在新事实的起点，带着那个起点的精度
            let (at, precision) = match request.close_at {
                Some(at) => (at, close_precision),
                None => (
                    new_from.ok_or_else(|| {
                        AppError::invalid(
                            "close_at_required",
                            "close_at is required when the new fact has no start time",
                        )
                    })?,
                    new_from_precision.as_deref().unwrap_or("day"),
                ),
            };
            if closes_before_start(state, caller.kb_id, old, at, precision).await? {
                return Err(interval_inverted());
            }
        }
        "retime_new" => {
            if utopia_store::graph::predicate_temporal(&state.pool, predicate).await?
                != utopia_store::graph::Temporal::State
            {
                return Err(AppError::invalid(
                    "not_a_state",
                    "Only a state has a start to retime.",
                ));
            }
            // 引擎画的终点会跟着重画，只有写明的终点拦得住新起点
            if let (false, Some(at), Some(end)) = (end_derived, request.new_from, new_to) {
                let from = utopia_store::graph::truncate_to(at, Some(new_precision));
                if from > utopia_store::graph::truncate_to(end, new_to_precision.as_deref()) {
                    return Err(interval_inverted());
                }
            }
        }
        _ => {}
    }
    land_proposal(
        state,
        caller,
        AgentOperation::ResolveConflict,
        "conflict.resolve",
        "conflict",
        conflict_id,
        &conflict_id.to_string(),
        stored,
        Some(action),
        None,
    )
    .await
}

/// 经令牌裁一处公理违规：**只留提议**（裁定表的 Violation 一行）。违规行与事实都不动，等
/// 登录的人在 Review 里答；批准时跑的是人自己的那段代码（`review_routes`）。
///
/// 次序是契约：写的检查；请求体（`resolution` 只能是那四种，否则 `bad_resolution`，与人的
/// 路由同一个 code）；目标（没有是 404，已经裁过、或请求指着的那条事实已经撤了、闭合了是
/// `already_decided`）；要读违规才知道的检查，照人的路由的次序——撤事实而说不出撤哪一条
/// （`fact_required`），闭合而没给日期（`close_at_required`）、精度认不出（`bad_precision`）、
/// 闭合点早于那条断言的起点（`interval_inverted`），闭合用在不是派生矛盾的违规上
/// （`bad_resolution`）；留提议。哪一步没过都一行不写
pub(crate) async fn propose_violation(
    state: &AppState,
    caller: &ReviewCaller,
    violation_id: Uuid,
    request: &super::review_routes::DecideViolationReq,
) -> Result<serde_json::Value, AppError> {
    authorize_write(state, caller).await?;
    let resolution: &'static str = match request.resolution.as_str() {
        "fact_retracted" => "fact_retracted",
        "fact_closed" => "fact_closed",
        "axiom_relaxed" => "axiom_relaxed",
        "accepted" => "accepted",
        _ => {
            return Err(AppError::invalid(
                "bad_resolution",
                "resolution must be fact_retracted, fact_closed, axiom_relaxed or accepted",
            ))
        }
    };
    // 存下的就是请求体原样：没给的键不出现
    let mut stored = json!({ "resolution": resolution });
    if let Some(at) = request.close_at {
        stored["close_at"] = json!(at);
    }
    if let Some(precision) = &request.close_at_precision {
        stored["close_at_precision"] = json!(precision);
    }
    if let Some(fact_id) = request.fact_id {
        stored["fact_id"] = json!(fact_id);
    }
    target_open(state, caller, "violation.decide", violation_id, &stored).await?;

    // 目标开着：违规在这个库里、还没人裁。下面的检查读它，不改
    let (kind, left, right, path): (String, Uuid, Uuid, Vec<Uuid>) = sqlx::query_as(
        "SELECT kind, left_fact, right_fact, path FROM axiom_violations
          WHERE id = $1 AND kb_id = $2",
    )
    .bind(violation_id)
    .bind(caller.kb_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    match (kind == "derived_contradiction", resolution) {
        (_, "fact_retracted") => {
            if utopia_store::reasoning::pick_retraction(left, right, &path, request.fact_id)
                .is_none()
            {
                return Err(AppError::invalid(
                    "fact_required",
                    "This violation involves more than one fact: fact_id must name the one to \
                     retract, and it must be one of the facts the violation lists.",
                ));
            }
        }
        (true, "fact_closed") => {
            let Some(at) = request.close_at else {
                return Err(AppError::invalid(
                    "close_at_required",
                    "fact_closed needs close_at, the date the assertion ended.",
                ));
            };
            let precision =
                super::review_routes::world_precision(request.close_at_precision.as_deref())?;
            // 闭的是违规的左事实，与人的路由同一个选法
            if closes_before_start(state, caller.kb_id, left, at, precision).await? {
                return Err(interval_inverted());
            }
        }
        (false, "fact_closed") => {
            return Err(AppError::invalid(
                "bad_resolution",
                "fact_closed applies only to a derived_contradiction.",
            ));
        }
        _ => {}
    }
    land_proposal(
        state,
        caller,
        AgentOperation::Violation,
        "violation.decide",
        "axiom_violation",
        violation_id,
        &violation_id.to_string(),
        stored,
        Some(resolution),
        None,
    )
    .await
}

/// 经令牌答勘误 agent 留给人的一笔：**只留提议**（裁定表的 Errata 一行）。那一笔仍然留着、
/// 事实不动，等登录的人在 Review 里答；批准时跑的是人自己的那段代码（`review_routes`）。
///
/// 次序是契约：写的检查；目标（没有是 404，已经答过是 `already_decided`）；留提议。请求体
/// 只有一个布尔值，解不出来的在提取器那里就答了
pub(crate) async fn propose_errata(
    state: &AppState,
    caller: &ReviewCaller,
    action_id: Uuid,
    approve: bool,
) -> Result<serde_json::Value, AppError> {
    authorize_write(state, caller).await?;
    let request = json!({ "approve": approve });
    target_open(state, caller, "errata.decide", action_id, &request).await?;
    land_proposal(
        state,
        caller,
        AgentOperation::Errata,
        "errata.decide",
        "errata_action",
        action_id,
        &action_id.to_string(),
        request,
        Some(if approve { "approve" } else { "reject" }),
        None,
    )
    .await
}

/// 经令牌定一条短语签名绑到哪个属性：**只留提议**（裁定表的 AlignmentPhrase 一行）。绑定行
/// 不动、**不排任务**，等登录的人在 Review 里答；批准时跑的是人自己的那段代码
/// （`review_routes`），那时判定连同它的投递任务一次提交——改动是排着，不是已经落地。
///
/// 次序是契约：写的检查；目标（这个库里没有这条签名是 404；短语没有「定了」这一态，行在
/// 就能提）；请求体——它的每一项都要读本体才校验得了，所以排在目标之后，跑的就是人的路由
/// 的那几句（`review_routes::phrase_choice`）：`unknown_property`、`unknown_marks`、
/// `marks_needs_state`、`direction_required`，同一个 code、同一句话；留提议
pub(crate) async fn propose_alignment_phrase(
    state: &AppState,
    caller: &ReviewCaller,
    binding_id: Uuid,
    request: &super::review_routes::DecideAlignmentPhraseReq,
) -> Result<serde_json::Value, AppError> {
    authorize_write(state, caller).await?;
    // 存下的就是请求体原样：没给的键不出现
    let mut stored = json!({});
    if let Some(property) = &request.property {
        stored["property"] = json!(property);
    }
    if let Some(direction) = &request.direction {
        stored["direction"] = json!(direction);
    }
    if let Some(marks) = &request.marks {
        stored["marks"] = json!(marks);
    }
    target_open(state, caller, "alignment.phrase", binding_id, &stored).await?;
    let (property, _, _) =
        super::review_routes::phrase_choice(state, caller.kb_id, request).await?;
    // 动作只有两种：绑到一个属性，或者说没有属性对得上
    let action = if property.is_some() { "bind" } else { "none" };
    land_proposal(
        state,
        caller,
        AgentOperation::AlignmentPhrase,
        "alignment.phrase",
        "phrase_binding",
        binding_id,
        &binding_id.to_string(),
        stored,
        Some(action),
        None,
    )
    .await
}

/// 经令牌批或驳一条蕴含规则：**只留提议**（裁定表的 AlignmentRule 一行）。规则行不动、
/// **不排任务**，等登录的人在 Review 里答；批准时判定连同它的投递任务一次提交——改动是
/// 排着，不是已经落地。
///
/// 次序是契约：写的检查；目标（这个库里没有这条规则是 404；规则没有「定了」这一态，行在
/// 就能提）；留提议
pub(crate) async fn propose_alignment_rule(
    state: &AppState,
    caller: &ReviewCaller,
    rule_id: Uuid,
    approve: bool,
) -> Result<serde_json::Value, AppError> {
    authorize_write(state, caller).await?;
    let request = json!({ "approve": approve });
    target_open(state, caller, "alignment.rule", rule_id, &request).await?;
    land_proposal(
        state,
        caller,
        AgentOperation::AlignmentRule,
        "alignment.rule",
        "implication_rule",
        rule_id,
        &rule_id.to_string(),
        request,
        Some(if approve { "approve" } else { "reject" }),
        None,
    )
    .await
}

// ───────────────────── 令牌自己的两个入口：等点头的事实、治理的一笔（0076） ─────────────────────
//
// 这两种提议**不从人的路由进**。替一条等点头的事实点头是宪章否决的（令牌不许经任何一条
// 路径确认或否决它），治理的那条回答路由又是「以人的身份合并、分开、撤回一对」的第二条
// 路——所以人的三条路由（点头、按句取、答治理）照旧只收会话，令牌走它自己的两条 `…/propose`
// 路由，会话调这两条拿到的是 [`token_required`]。下面两个函数与别的「只能提议」的函数
// 一个样：什么都不改，只留一条提议；批准时跑的是人自己的那段代码（`review_routes`）

/// 这两条路由只收个人令牌：会话来的（没有令牌的调用方）答 403 `token_required`，什么都
/// 不做。登录的人点头、答治理走的是他自己的路由
pub(crate) fn token_required() -> AppError {
    AppError::CodedForbidden {
        code: "token_required",
        message: "This route takes a personal token: it only leaves a proposal. A signed-in \
                  person decides on the item's own route."
            .into(),
    }
}

/// 经令牌提议对一条等点头的事实点头或摇头：**只留提议**（裁定表的 PendingNod 一行）。
/// `pending_facts` 那一行一个字节都不动，不落事实、不记拒绝、不排任务；等登录的人在 Review
/// 里答，批准时那一次点头是这个人的（`review_routes`）。**这里没有、也不许有一条路径替令牌
/// 点头**（宪章否决，N4）。
///
/// 次序是契约：写的检查；请求体（`action` 只能是 confirm / reject，否则 `unknown_action`，
/// 与人的点头路由同一个 code、同一句话）；目标（这个库里没有这一条——认不出的 id、别的库
/// 的、已经点过头的——是 404；等点头的事实没有「定了」这一态，点过头行就没了）；留提议。
/// 哪一步没过都一行不写。一条等点头的事实上只有一条开着的提议，确认还是否决都算它：
/// 第二次调用拿到的是那一条加 `already_proposed`
pub(crate) async fn propose_pending(
    state: &AppState,
    caller: &ReviewCaller,
    pending_id: Uuid,
    action: &str,
) -> Result<serde_json::Value, AppError> {
    authorize_write(state, caller).await?;
    let action: &'static str = match action {
        "confirm" => "confirm",
        "reject" => "reject",
        other => {
            return Err(AppError::invalid(
                "unknown_action",
                format!("action must be confirm or reject, got {other}"),
            ))
        }
    };
    let request = json!({ "action": action });
    target_open(state, caller, "pending.nod", pending_id, &request).await?;
    land_proposal(
        state,
        caller,
        AgentOperation::PendingNod,
        "pending.nod",
        "pending_fact",
        pending_id,
        &pending_id.to_string(),
        request,
        Some(action),
        None,
    )
    .await
}

/// 经令牌提议怎么回答治理 agent 的一笔：**只留提议**（裁定表的 AnswerGovernor 一行）。
/// 治理的那一行、它指着的那一对、保险丝都不动，不排治理任务；等登录的人在 Review 里答，
/// 批准时跑的是人回答治理的那段代码（`review_routes`），以这个人的身份、不带令牌的理由。
/// `action` 是 merge / keep / revert。
///
/// 次序是契约：写的检查；目标（这个库里没有这一笔是 404；已经有人答过，或者建议还开着
/// 而它指着的那一对已经不等人了，是 `already_decided`）；要读这一笔才知道的检查，与人的
/// 路由同一组判据、同一个 code——动作对不上这一笔的状态是 `agent_answer`，撤一笔没有
/// 合并可撤的是 `not_a_merge`；留提议。哪一步没过都一行不写。
///
/// 存下的请求体只有 `action`：令牌写的理由留在提议上（`why`），**不进请求体**——批准时
/// 解回来的请求体里就没有理由可传，它成不了这个人的话
pub(crate) async fn propose_governor(
    state: &AppState,
    caller: &ReviewCaller,
    decision_id: Uuid,
    action: &str,
    rationale: Option<&str>,
) -> Result<serde_json::Value, AppError> {
    authorize_write(state, caller).await?;
    let request = json!({ "action": action });
    target_open(state, caller, "governor.answer", decision_id, &request).await?;
    // 目标开着：这一笔在这个库里。下面只读它，不改
    let (status, decided, merge_id): (String, String, Option<Uuid>) = sqlx::query_as(
        "SELECT status, action, merge_id FROM agent_decisions WHERE id = $1 AND kb_id = $2",
    )
    .bind(decision_id)
    .bind(caller.kb_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    // 与 `review_routes::governor_answer` 的那个 `match` 同一组情形
    let action: &'static str = match (status.as_str(), action) {
        ("proposed", "merge") => "merge",
        ("proposed", "keep") => "keep",
        ("applied", "revert") => {
            if merge_id.is_none() {
                return Err(AppError::invalid(
                    "not_a_merge",
                    "Only an applied merge can be reverted; a pair kept apart can be merged instead.",
                ));
            }
            "revert"
        }
        ("applied", "merge") if decided == "keep" => "merge",
        (status, action) => {
            return Err(AppError::invalid(
                "agent_answer",
                format!("cannot {action} an agent decision that is {status}"),
            ))
        }
    };
    land_proposal(
        state,
        caller,
        AgentOperation::AnswerGovernor,
        "governor.answer",
        "agent_decision",
        decision_id,
        &decision_id.to_string(),
        request,
        Some(action),
        rationale,
    )
    .await
}

// ───────────────────── 本体提案：读，以及拒绝与采纳的提议（0076） ─────────────────────
//
// 本体提案不在 Review 的列表里：它有自己的路由，按 section 与 key 认，人在本体页上定它。
// 令牌读的是那条路由回给会话的同一份；拒绝与采纳都只留提议，台账里的目标是
// `ontology_proposals` 那一行的 id

/// 还开着的本体提案，与会话从 `GET /ontology/proposals` 读到的同一份（一条提案上有开着的
/// 令牌提议时，那一项带 `agent_proposals`）。读的检查与 [`list`] 同一个：主人至少是
/// Viewer、令牌的 `kb_ids` 够得着这个库，否则 404
pub(crate) async fn ontology_proposals(
    state: &AppState,
    caller: &ReviewCaller,
) -> Result<serde_json::Value, AppError> {
    authorize_read(state, caller).await?;
    super::ontology_routes::open_proposals_body(state, caller.kb_id).await
}

/// 这个库里还开着的本体提案上，令牌 agent 开着的提议：按（section, key）找，值就是要挂
/// 上去的那一列（与 [`attach_agent_proposals`] 挂的同一个形状，最早提的在前）。没有开着
/// 的提议的提案不在里面。
///
/// 本体提案的项没有 `id`（它按 section 与 key 认，响应里也不为这个多加一个键），所以不走
/// `attach_agent_proposals`：这里自己把行的 id 对回 section 与 key
pub(crate) async fn open_ontology_proposals(
    state: &AppState,
    kb_id: Uuid,
) -> Result<std::collections::HashMap<(String, String), serde_json::Value>, AppError> {
    let rows: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT id, section, key FROM ontology_proposals WHERE kb_id = $1 AND status = 'open'",
    )
    .bind(kb_id)
    .fetch_all(&state.pool)
    .await?;
    let ids: Vec<Uuid> = rows.iter().map(|(id, _, _)| *id).collect();
    let mut open = utopia_store::token_agent_actions::open_proposals_for(
        &state.pool,
        kb_id,
        "ontology_proposal",
        &ids,
    )
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(id, section, key)| Some(((section, key), json!(open.remove(&id)?))))
        .collect())
}

/// 请求里点了名的类这个库里有没有：没有的那一个是 `unknown_class`，与人采纳时
/// `ontology_agent` 给的同一个 code、同一句话。这里只读，不锁
async fn classes_exist(state: &AppState, kb_id: Uuid, keys: &[String]) -> Result<(), AppError> {
    if keys.is_empty() {
        return Ok(());
    }
    let found: Vec<String> =
        sqlx::query_scalar("SELECT key FROM entity_types WHERE kb_id = $1 AND key = ANY($2)")
            .bind(kb_id)
            .bind(keys)
            .fetch_all(&state.pool)
            .await?;
    match keys.iter().find(|k| !found.contains(k)) {
        Some(k) => Err(AppError::invalid(
            "unknown_class",
            format!("class `{k}` does not exist"),
        )),
        None => Ok(()),
    }
}

/// 一列类的键：请求里给了就用请求的，没给就是提案自己的那一列
fn class_keys(request: &serde_json::Value, payload: &serde_json::Value, name: &str) -> Vec<String> {
    let listed = match request.get(name) {
        Some(keys) if !keys.is_null() => keys,
        _ => &payload[name],
    };
    listed
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|k| k.as_str().map(str::to_string))
        .collect()
}

/// 经令牌拒绝或采纳一条本体提案：**只留提议**（裁定表的 OntologyProposal 一行）。提案行与
/// 本体都不动、不排任务，等登录的人答；批准时跑的是人自己的那段代码（`review_routes`）。
/// `action` 是 reject / adopt；`request` 是那条路由的请求体原样。
///
/// 次序是契约：写的检查；请求体——拒绝的 `status`（`adoption_endpoint`、`bad_status`，与
/// 人的路由同一个 code）；目标——这个 section 与 key 没有存着的提案、或者它不是本体代理
/// 提的（人的采纳路由对它同样答 not found，列表里也不列它）是 404，已经定了是
/// `already_decided`；要读本体才知道的检查——采纳时点了名的类不存在（`unknown_class`）；
/// 留提议。哪一步没过都一行不写。
///
/// 一条本体提案上只有一条开着的提议，拒绝还是采纳都算它：第二次调用拿到的是那一条加
/// `already_proposed`。提议存下提案此刻的 payload（指纹，`land_proposal` 从 store 取）：
/// 批准时代理把它刷成了别的样子，提议就过时了。拒绝的理由是令牌写的，留在提议上（`why`）
pub(crate) async fn propose_ontology(
    state: &AppState,
    caller: &ReviewCaller,
    section: &str,
    key: &str,
    action: &str,
    request: serde_json::Value,
) -> Result<serde_json::Value, AppError> {
    authorize_write(state, caller).await?;
    let action: &'static str = match action {
        "reject" => "reject",
        "adopt" => "adopt",
        // REST 的两条路由各给各的动作，到不了这里；MCP 工具的 `action` 是调用方写的
        other => {
            return Err(AppError::invalid(
                "unknown_action",
                format!("action must be reject or adopt, got {other}"),
            ))
        }
    };
    if action == "reject" {
        super::ontology_routes::rejection_status(request["status"].as_str().unwrap_or_default())?;
    }
    let target_id =
        utopia_store::review_targets::ontology_proposal_id(&state.pool, caller.kb_id, section, key)
            .await?
            .ok_or(AppError::NotFound)?;
    target_open(state, caller, "ontology.proposal", target_id, &request).await?;
    if action == "adopt" {
        // 目标开着：提案在这个库里。新类要它的父类在，新属性要它的定义域（关系还要值域）在
        let payload: serde_json::Value = sqlx::query_scalar(
            "SELECT payload FROM ontology_proposals WHERE id = $1 AND kb_id = $2",
        )
        .bind(target_id)
        .bind(caller.kb_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;
        let named: Vec<String> = match section {
            "entity_types" => class_keys(&request, &payload, "parents"),
            "relation_types" => [
                class_keys(&request, &payload, "domains"),
                class_keys(&request, &payload, "ranges"),
            ]
            .concat(),
            "attribute_types" => class_keys(&request, &payload, "domains"),
            _ => Vec::new(),
        };
        classes_exist(state, caller.kb_id, &named).await?;
    }
    let why = if action == "reject" {
        request["reason"].as_str().map(str::to_string)
    } else {
        None
    };
    land_proposal(
        state,
        caller,
        AgentOperation::OntologyProposal,
        "ontology.proposal",
        "ontology_proposal",
        target_id,
        &format!("{section}:{key}"),
        request,
        Some(action),
        why.as_deref(),
    )
    .await
}

// ───────────────────── 直接落地的三行：映射、两种运行、类别词（0076） ─────────────────────
//
// 裁定表里这三行是「直接落地」。映射的决定与两种运行（一致性检查、物化推导）先问裁定表、
// 再过 [`guard`]（限速），落地时台账里各留一行 `applied`；类别词的决定从前就对写令牌
// 直接落地，这里只给它补上台账行与令牌的名字，**不限速**（它的行照样数进那一小时的总数）。
// 回答都是人拿到的那一个 body

/// 一页语义层映射，与会话从 `GET /mappings` 读到的同一个 `{items, total, counts,
/// last_run}`（`mapping_routes::page_body`）。读的检查与 [`list`] 同一个：主人至少是
/// Viewer、令牌的 `kb_ids` 够得着这个库，否则 404。Review 列表里 `mappings` 仍不是开给
/// 令牌的档（[`list`] 照旧答 `unknown_queue`）——令牌读映射走的是这一条
pub(crate) async fn mappings(
    state: &AppState,
    caller: &ReviewCaller,
    status: Option<&str>,
    q: Option<&str>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<serde_json::Value, AppError> {
    authorize_read(state, caller).await?;
    super::mapping_routes::page_body(state, caller.kb_id, status, q, limit, offset).await
}

/// 经令牌对一条语义层映射表态：**直接落地**（裁定表的 Mapping 一行）。
///
/// 次序是契约：写的检查；请求体（`status` 只能是 confirmed / rejected，否则 `bad_status`，
/// 与人的路由同一个 code、同一句话）；目标（这个库里没有是 404；映射没有「定了」这一态，
/// 状态已经是请求要的那个就是 `already_decided`，一行不写）；裁定表（给的不是「落地」就
/// 拒绝）；[`guard`]（限速：`agent_rate_limited`，什么都不改、一行不写）；store。
///
/// 新状态、台账行（`mapping.decide`，`applied`）、审计行（`mapping.decided`，actor 是令牌的
/// 主人，detail 里带 `token_id`、`token_name`、`action_id`）由
/// `token_agent_actions::decide_mapping` 在一个事务里写。回答是人拿到的那一个
pub(crate) async fn decide_mapping(
    state: &AppState,
    caller: &ReviewCaller,
    mapping_id: Uuid,
    status: &str,
) -> Result<serde_json::Value, AppError> {
    authorize_write(state, caller).await?;
    if !matches!(status, "confirmed" | "rejected") {
        return Err(AppError::invalid(
            "bad_status",
            "status must be confirmed or rejected",
        ));
    }
    let request = json!({ "status": status });
    target_open(state, caller, "mapping.decide", mapping_id, &request).await?;
    if agent_policy(AgentOperation::Mapping, None) != AgentOutcome::Applied {
        return Err(not_open());
    }
    guard(state, caller, AgentOperation::Mapping).await?;
    utopia_store::token_agent_actions::decide_mapping(
        &state.pool,
        &token_write(caller, None, None, false),
        mapping_id,
        status,
    )
    .await?;
    state.emit_review(caller.kb_id);
    Ok(json!({ "ok": true }))
}

/// 经令牌跑一遍一致性检查（`run` 是 `consistency`）或物化推导（`inference`）：**直接落地**
/// （裁定表的 ConsistencyRun 一行管这两种）。
///
/// 次序是契约：写的检查；`run` 认不认得（`unknown_run`——REST 的两条路由各给各的，到不了
/// 这里，MCP 工具的 `run` 是调用方写的）；推导的开关（关着是 `inference_off`，与人的路由
/// 同一个 code，一行不写）；裁定表；[`guard`]（限速：`agent_rate_limited`，不跑、一行不写）；
/// 跑——就是人跑的那一段（`review_routes::consistency_run` / `inference_run`），没有新加
/// 的锁；跑成了**紧接着**在一个事务里写台账行（`consistency.check` / `inference.run`，
/// `applied`，目标是库本身）与审计行（`consistency.checked` / `inference.materialized`，
/// actor 是令牌的主人，detail 是人的那一份加 `token_id`、`token_name`、`action_id`）。
/// 这两行写不进去，这一次调用就是失败——不回一份没有记下来的报告。回答是人拿到的报告
pub(crate) async fn run_check(
    state: &AppState,
    caller: &ReviewCaller,
    run: &str,
) -> Result<serde_json::Value, AppError> {
    authorize_write(state, caller).await?;
    let (operation, audited, inference): (&'static str, &'static str, bool) = match run {
        "consistency" => ("consistency.check", "consistency.checked", false),
        "inference" => ("inference.run", "inference.materialized", true),
        other => {
            return Err(AppError::invalid(
                "unknown_run",
                format!("run must be consistency or inference, got {other}"),
            ))
        }
    };
    if inference {
        super::review_routes::inference_on(state, caller.kb_id).await?;
    }
    if agent_policy(AgentOperation::ConsistencyRun, None) != AgentOutcome::Applied {
        return Err(not_open());
    }
    guard(state, caller, AgentOperation::ConsistencyRun).await?;
    let pool = &state.pool;
    // 两条路由都没有请求体。目标的那句话（库的名字）在跑之前读：读不到就不跑
    let request = json!({});
    let label = utopia_store::review_targets::target_label(
        pool,
        caller.kb_id,
        operation,
        caller.kb_id,
        &request,
    )
    .await?;
    let (detail, body) = if inference {
        super::review_routes::inference_run(state, caller.kb_id).await?
    } else {
        super::review_routes::consistency_run(state, caller.kb_id).await?
    };
    let mut tx = pool.begin().await?;
    let action = utopia_store::token_agent_actions::record_applied(
        &mut tx,
        &token_write(caller, None, None, false),
        &Proposal {
            operation,
            target_kind: "knowledge_base",
            target_id: caller.kb_id,
            target_key: &caller.kb_id.to_string(),
            payload: json!({ "request": request, "target_label": label }),
        },
    )
    .await?;
    utopia_store::audit::record_on(
        &mut tx,
        Some(caller.kb_id),
        Some(caller.user.id),
        audited,
        "knowledge_base",
        Some(caller.kb_id),
        utopia_store::token_agent_actions::token_audit_detail(detail, &action),
    )
    .await?;
    tx.commit().await?;
    // 与人的两条路由各推的那一个事件相同：检查动的是 Review 的队列，推导动的是图
    if inference {
        state.emit_graph(caller.kb_id);
    } else {
        state.emit_review(caller.kb_id);
    }
    Ok(body)
}

/// 经令牌定一个类别词绑到哪个类，MCP 的 `review_kind_word` 走的入口：先过
/// [`authorize_write`]——所以被拒绝的调用方拿到的是带 code 的工具错误（`token_read_only`、
/// `token_kb_out_of_scope`、`token_owner_below_editor`），再到 [`kind_word_decision`]。
/// REST 的那条路由不走这里：它留着自己从前的两道检查与那句不带 code 的 403
pub(crate) async fn decide_kind_word(
    state: &AppState,
    caller: &ReviewCaller,
    kind_word: &str,
    class: Option<&str>,
) -> Result<serde_json::Value, AppError> {
    authorize_write(state, caller).await?;
    kind_word_decision(state, caller, kind_word, class).await
}

/// 类别词的决定在写的检查之后的那一段：**直接落地**（裁定表的 AlignmentKindWord 一行），
/// REST 的令牌分支与 [`decide_kind_word`] 共用。**这里不做授权**——调用方已经做过。
///
/// - 类的键认不出：`unknown_class`；锁等不到：`alignment_busy`；store 说没写下判定：404。
///   都与从前同一个回答
/// - 判定与一行台账（`alignment.kind_word`，`applied`）由
///   `token_agent_actions::decide_kind_word` 在一个事务里写
/// - 审计行 `alignment.kind_word_decided` 照旧在判定之后尽力而为地写；detail 除了从前的
///   `kind_word`、`class`、`token_id`，多了 `token_name` 与 `action_id`
/// - **不调 [`guard`]**：这条路由从前对写令牌就不限速，0076 不为令牌收窄它。它的台账行
///   是 `applied`，所以照样数进这枚令牌那一小时的总数
pub(crate) async fn kind_word_decision(
    state: &AppState,
    caller: &ReviewCaller,
    kind_word: &str,
    class: Option<&str>,
) -> Result<serde_json::Value, AppError> {
    if agent_policy(AgentOperation::AlignmentKindWord, None) != AgentOutcome::Applied {
        return Err(not_open());
    }
    let type_id = super::review_routes::kind_word_class(state, caller.kb_id, class).await?;
    // 人的那一票：请求里的 `class` 原样，与会话记下的同一个形状
    let votes = json!({ "person": class });
    let action = utopia_store::token_agent_actions::decide_kind_word(
        &state.pool,
        &token_write(caller, None, None, false),
        kind_word,
        type_id,
        &votes,
    )
    .await?
    .ok_or(AppError::NotFound)?;
    let _ = utopia_store::audit::record(
        &state.pool,
        Some(caller.kb_id),
        caller.user.id,
        "alignment.kind_word_decided",
        "type_binding",
        None,
        utopia_store::token_agent_actions::token_audit_detail(
            json!({ "kind_word": kind_word, "class": class }),
            &action,
        ),
    )
    .await;
    state.emit_review(caller.kb_id);
    state.emit_graph(caller.kb_id);
    Ok(json!({ "ok": true }))
}

// ───────────────────── 开着的提议挂在它指着的那一条上（0076） ─────────────────────

/// 把开着的令牌提议挂到这些条目上：键是 `agent_proposals`，一列，最早提的在前；每一项是
/// `{id, operation, reason, owner_name, token_name, created_at}`，提议有动作时再带 `action`。
///
/// - **只在有开着的提议时才出这个键**：没有提议的条目一个键也不多，与从前逐字相等
/// - 挂在 JSON 上，不往哪个条目的结构体里加字段；认的是条目的 `id`，没有 `id`（或不是
///   uuid）的条目原样不动
/// - `target_kind` 是台账的目标种类（`fact`、`conflict`、`ontology_defect`……）：同一个 id
///   只在它自己的种类里找
///
/// 重复项与合并历史不走这里：它们带的是 M003 的 `agent_proposal`（一条，不是一列），不动
pub(crate) async fn attach_agent_proposals(
    state: &AppState,
    kb_id: Uuid,
    target_kind: &str,
    items: &mut [serde_json::Value],
) -> Result<(), AppError> {
    let id_of = |item: &serde_json::Value| item["id"].as_str().and_then(|s| s.parse::<Uuid>().ok());
    let ids: Vec<Uuid> = items.iter().filter_map(id_of).collect();
    let open = utopia_store::token_agent_actions::open_proposals_for(
        &state.pool,
        kb_id,
        target_kind,
        &ids,
    )
    .await?;
    if open.is_empty() {
        return Ok(());
    }
    for item in items.iter_mut() {
        let Some(found) = id_of(item).and_then(|id| open.get(&id)) else {
            continue;
        };
        if let Some(fields) = item.as_object_mut() {
            fields.insert("agent_proposals".into(), json!(found));
        }
    }
    Ok(())
}

/// 一页里只有一部分条目归这个目标种类时（对齐一档的短语与规则、Agent 一档的治理行）：
/// 挑出来挂，再原位放回去，次序不变
async fn attach_to_some(
    state: &AppState,
    kb_id: Uuid,
    target_kind: &str,
    items: &mut [serde_json::Value],
    belongs: impl Fn(&serde_json::Value) -> bool,
) -> Result<(), AppError> {
    let at: Vec<usize> = (0..items.len()).filter(|&i| belongs(&items[i])).collect();
    let mut picked: Vec<serde_json::Value> = at.iter().map(|&i| items[i].take()).collect();
    let attached = attach_agent_proposals(state, kb_id, target_kind, &mut picked).await;
    // 先放回去再报错：出错也不留下被掏空的条目
    for (i, item) in at.into_iter().zip(picked) {
        items[i] = item;
    }
    attached
}

/// 一档的一页上，哪些条目带开着的提议、按哪个目标种类找（0076）。**只写这一处**：会话的
/// 列表、令牌的列表、MCP 的 `review_list` 与单条读（[`item`]）拿的都是 `queue_page` 的
/// 那一页，`queue_page` 调的是这里，所以四个入口看到的是同一个键。
///
/// - 等你点头、矛盾、未确认、低置信、公理、本体缺陷、勘误：整页一个种类
/// - 对齐：短语与规则各一个种类；**类别词不带**（它没有 `id`，决定直接落地）
/// - Agent：治理 agent 的行（没有 `source` 键）带人对它的回答的提议；令牌台账的行不带——
///   它自己就是提议
/// - 重复项与合并历史带的是 M003 的 `agent_proposal`，在 store 里挂，这里不碰；`mappings`
///   的决定直接落地，没有提议
pub(crate) async fn attach_section_proposals(
    state: &AppState,
    kb_id: Uuid,
    section: &str,
    items: &mut serde_json::Value,
) -> Result<(), AppError> {
    let Some(items) = items.as_array_mut() else {
        return Ok(());
    };
    let whole = match section {
        "pending" => "pending_fact",
        "conflicts" => "conflict",
        "unconfirmed" | "lowconf" => "fact",
        "violations" => "axiom_violation",
        "defects" => "ontology_defect",
        "errata" => "errata_action",
        "alignment" => {
            attach_to_some(state, kb_id, "phrase_binding", items, |item| {
                item["kind"] == "phrase"
            })
            .await?;
            return attach_to_some(state, kb_id, "implication_rule", items, |item| {
                item["kind"] == "rule"
            })
            .await;
        }
        "agent" => {
            return attach_to_some(state, kb_id, "agent_decision", items, |item| {
                item.get("source").is_none()
            })
            .await;
        }
        _ => return Ok(()),
    };
    attach_agent_proposals(state, kb_id, whole, items).await
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
