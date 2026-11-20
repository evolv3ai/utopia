//! Review 的 MCP 工具（0076 决定 2）：`review_list`、`review_item`、`review_keep`、
//! `review_merge`、`review_revert`、`review_batch`。
//!
//! 两个读工具管 Review 的十三档：`review_list` 读一档的一页（十一个列表档、总览、决策台账），
//! `review_item` 读某一档里列着的一条，违规一档的条目带上它的证明。写的有两样：
//! `review_keep` 把一对分开；`review_merge` 合并队列里的一对或手动指定的两个实体，
//! 先过服务端的闸门，被留下的不落地、只留一条等人答的提议；`review_revert` 撤回一次
//! 合并——令牌 agent 做的直接撤，人或治理 agent 做的只留提议，给 `batch_id` 就是把一批
//! 做出的合并整批撤回；`review_batch` 一次裁最多 50 对重复项，每一对走的就是单笔的那条路，
//! 服务端发一个 `batch_id`。裁定表里只能提议的操作各有一个工具，`review_defect`（对一处
//! 本体缺陷表态）是第一个：调用什么都不改，只留一条等人答的提议。
//! **没有哪个工具能答提议。**
//!
//! **这里只做两件事：把参数递给 `review_ops`，把它的回答变成工具结果。** 授权、裁定表、
//! 台账、审计都在 `review_ops` 里面，REST 的令牌分支调的是同一批函数——所以同一枚令牌
//! 在两个入口读到的东西相等，被拒绝时拿到的 code 也相等。这里不另做一道检查：
//! 两道检查迟早有一道先变。
//!
//! **不进 `chat::tools_schema`，也不进 `tools::dispatch`。** 那两处与应用内的聊天助手
//! 共用，而助手不该拿到任何 Review 工具。这几个工具的定义与派发只在这个文件里，
//! 由 `mcp.rs` 接进 `tools/list` 与 `tools/call`。
//!
//! 标注（`readOnlyHint` 之类）只是给客户端的提示，服务端没有一条路径读它们（0076 决定 3）。

use serde_json::{json, Value};
use utopia_core::AppError;
use uuid::Uuid;

use super::review_ops::{self, ReviewCaller};
use super::tools::{Step, ToolResult};
use crate::state::AppState;

const LIST: &str = "review_list";
const ITEM: &str = "review_item";
const KEEP: &str = "review_keep";
const MERGE: &str = "review_merge";
const REVERT: &str = "review_revert";
const BATCH: &str = "review_batch";
const DEFECT: &str = "review_defect";

/// 这个名字归不归这里管。归这里的，`mcp.rs` 不再拿它去问 `is_exposed`
pub(super) fn serves(name: &str) -> bool {
    matches!(name, LIST | ITEM | KEEP | MERGE | REVERT | BATCH | DEFECT)
}

/// 写工具：只列给写得了的调用方，标注里 `readOnlyHint` 是 false
fn writes(name: &str) -> bool {
    matches!(name, KEEP | MERGE | REVERT | BATCH | DEFECT)
}

/// 工具定义，OpenAI 的形状——`chat::check_call` 认的就是这个形状，参数守卫与别的工具
/// 共用一份。
///
/// 翻页参数（`limit`、`offset`、`page`、`per`）**都不写上下界**：越界的值不是错误，照会话
/// 路由的规矩夹（`review_ops::list` 夹到 1 到 200，`review_ops::history` 夹到 1 到 100）。
/// 写了 `minimum` 的话，守规矩的客户端会在本地就把 0 拦下。
///
/// `review_item` 的 `id` **不写 `format: uuid`**：对齐一档的类别词没有 uuid，认的是它的词。
/// `review_keep` 的 `id` 照旧是 uuid——它只分重复项。
///
/// `review_merge` **没有必填参数**：要么给 `id`（队列里的一对），要么给 `source` 与
/// `target`（两个实体）。两种给法是哪一种、给全了没有，由 [`call`] 看——`required`
/// 表达不了「二选一」
///
/// `review_revert` 的说明**不许把撤回说成完全复原**（0076）：它只把图放回去，合并还在时
/// 推出的派生、记下的违规、导出的东西、答出去的话都不收回；没有类型的存活方留着拿到的
/// 类型，存活的事实留着抄给它的陈述来源。这三句写在说明里，对面的 agent 读得到。
/// 它同样**没有必填参数**：`merge_id`（一次合并）与 `batch_id`（一批）正好给一个，
/// 给了几个由 [`call`] 看
///
/// `review_batch` 的 `ids` **不写 `minItems` / `maxItems`**：0 条与 51 条是操作自己的拒绝
/// （`batch_size`，与 REST 同一个 code），不是客户端在本地拦下的参数错误
fn definitions() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": LIST,
                "description": "Read one section of this knowledge base's Review tab; all \
                    thirteen sections are served. The eleven queue sections are \"pending\" \
                    (Awaiting your nod), \"duplicates\", \"conflicts\", \"unconfirmed\", \
                    \"lowconf\" (Low confidence), \"violations\" (Axioms), \"defects\" \
                    (Ontology), \"alignment\", \"errata\", \"agent\" (Agent History) and \
                    \"merges\": each returns one page as {counts, queue, items, total}, paged \
                    with limit and offset; total is the number of items the section lists over \
                    all pages, so the last item is at offset total - 1 (counts holds the tab's \
                    badges, which for some sections is a different number). \"decisions\" \
                    returns one page of the decision history as {events, total}, paged with \
                    page and per. \"overview\" returns the Review summary (what is waiting, \
                    what was decided, health, the governor's recent rows) and takes no paging \
                    argument. A paging argument the section does not take is ignored. Any \
                    other section name is refused with code unknown_queue. Reading is all \
                    these sections offer: nothing listed here is decided by reading it.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "section": {
                            "type": "string",
                            "description": "One of \"overview\", \"pending\", \"duplicates\", \
                                \"conflicts\", \"unconfirmed\", \"lowconf\", \"violations\", \
                                \"defects\", \"alignment\", \"errata\", \"agent\", \
                                \"decisions\", \"merges\"."
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Queue sections only: page size (default 10). A \
                                value below 1 is served as 1 and a value above 200 as 200."
                        },
                        "offset": {
                            "type": "integer",
                            "description": "Queue sections only: zero-based start of the page \
                                (default 0). An offset at or past total returns no items."
                        },
                        "page": {
                            "type": "integer",
                            "description": "\"decisions\" only: zero-based page number \
                                (default 0)."
                        },
                        "per": {
                            "type": "integer",
                            "description": "\"decisions\" only: events per page (default 20). \
                                A value below 1 is served as 1 and a value above 100 as 100."
                        }
                    },
                    "required": ["section"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": ITEM,
                "description": "Read one item that review_list lists, by its section and id. \
                    Returns that item exactly as review_list returns it: for \"duplicates\" \
                    the pair's score, reason, stage, both entities with their type, \
                    disambiguator, fact count and top facts, and the governor's open proposal; \
                    for \"decisions\" the event. For \"violations\" (Axioms) the item also \
                    carries proof: the proof steps when its kind is derived_contradiction, \
                    null for any other kind. section is one of the eleven queue sections \
                    (\"pending\", \"duplicates\", \"conflicts\", \"unconfirmed\", \"lowconf\", \
                    \"violations\", \"defects\", \"alignment\", \"errata\", \"agent\", \
                    \"merges\") or \"decisions\". An id that is not listed in that section of \
                    this knowledge base (unknown, of another knowledge base, or no longer \
                    waiting there) is refused with code not_found, as is any id with section \
                    \"overview\", which lists no items. Any other section name is refused \
                    with code unknown_queue.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "section": {
                            "type": "string",
                            "description": "Section the item is listed in. Default \
                                \"duplicates\"."
                        },
                        "id": {
                            "type": "string",
                            "description": "The item's id from review_list (a uuid; for \
                                \"decisions\" the event's id, for \"agent\" the row's id). An \
                                \"alignment\" item of kind kind_word has no id: pass its \
                                kind_word exactly as listed, same letter case and spacing."
                        }
                    },
                    "required": ["id"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": KEEP,
                "description": "Keep one pending duplicate pair apart: decide that its two \
                    entities are NOT the same thing. Applied at once and recorded under the \
                    token's owner and the token's name. Changes no entity and no fact; the \
                    pair leaves the Duplicates section. Read the pair with review_item first. \
                    A pair that was already decided is refused with code already_decided. A \
                    token finalizes at most 200 writes (keeps, merges, reverts) per knowledge \
                    base in a rolling hour; past that the call is refused with code \
                    agent_rate_limited and nothing is changed.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "id": {
                            "type": "string",
                            "format": "uuid",
                            "description": "Review id (uuid) of a pair from review_list."
                        },
                        "rationale": {
                            "type": "string",
                            "description": "Optional: one sentence on what tells the two apart."
                        }
                    },
                    "required": ["id"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": MERGE,
                "description": "Merge two entities that are the same thing. Pass either id (the \
                    review id of a pending duplicate pair from review_list; the side with more \
                    facts survives, as when a person merges the pair in the Review tab) or \
                    source and target (two entity ids; source is merged into target). The \
                    server's merge gate is asked first. If it passes, the merge is applied at \
                    once and recorded under the token's owner and the token's name, and the \
                    result is {ok, outcome: \"applied\", action_id, merge_id}. If the gate \
                    holds the merge (a source that treats names as identifiers, a one-value \
                    relation the merge would contradict, derived facts resting on one side, an \
                    entity named in an answer, a pair a person kept apart, or a pair whose \
                    merge was reverted), nothing is changed: the call waits as a proposal that \
                    only a signed-in person can answer in the Review tab, and the result is \
                    {ok, outcome: \"held\" or \"proposed\", action_id, reason, explanation}. \
                    Repeating a held call returns the same proposal with code \
                    already_proposed. No tool answers a proposal. A pair that was already \
                    decided, or an entity already merged into another, is refused with code \
                    already_decided; merging an entity into itself with code self_merge; an \
                    unknown id with code not_found. After people have reverted two of this \
                    token's merges on this knowledge base within seven days, its merges are \
                    not applied for the rest of that window: each waits as a proposal with \
                    reason breaker. A token finalizes at most 200 writes per knowledge base in \
                    a rolling hour; past that the call is refused with code agent_rate_limited \
                    and nothing is changed.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "id": {
                            "type": "string",
                            "format": "uuid",
                            "description": "Review id (uuid) of a pending duplicate pair from \
                                review_list. Leave out when source and target are given."
                        },
                        "source": {
                            "type": "string",
                            "format": "uuid",
                            "description": "Entity id (uuid) to merge away. Given together \
                                with target, instead of id."
                        },
                        "target": {
                            "type": "string",
                            "format": "uuid",
                            "description": "Entity id (uuid) that survives. Given together \
                                with source, instead of id."
                        },
                        "rationale": {
                            "type": "string",
                            "description": "Optional: one sentence on what makes the two the \
                                same thing."
                        }
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": REVERT,
                "description": "Revert one merge, named by its merge_id (from the result of \
                    review_merge, or from review_list with section \"merges\"), or every merge \
                    one review_batch call made, named by its batch_id. Pass exactly one of \
                    merge_id and batch_id. With batch_id each merge of the batch that is not \
                    yet reverted goes through the same single revert, best effort; the result \
                    is {batch_id, reverted, outcomes: [{merge_id, outcome, code, error}]}, a \
                    merge that was already reverted is that item's already_decided, pairs the \
                    batch kept apart are left as they are, and a batch_id no action of this \
                    knowledge base carries is refused with code not_found. A merge that a \
                    token agent made on this knowledge base, through this token or another, is \
                    reverted at once and recorded under the token's owner and the token's \
                    name; the result is {ok, outcome: \"applied\", action_id, merge_id}. A \
                    merge that a person or the governor made is not reverted by a token: \
                    nothing is changed, the call waits as a proposal that only a signed-in \
                    person can answer in the Review tab, and the result is {ok, outcome: \
                    \"proposed\", action_id, reason, explanation} with reason merge_by_person, \
                    merge_by_governor or breaker. Repeating such a call returns the same \
                    proposal with code already_proposed. No tool answers a proposal. A revert \
                    restores graph state only and does not recall derivations, violations, \
                    exports or answers drawn while the merge was live. Two differences from \
                    the state before the merge remain: a target that had no type before the \
                    merge keeps the type it took, and a surviving fact keeps the statement \
                    links the merge copied onto it. A merge that was already reverted is \
                    refused with code already_decided; a merge id that is unknown, or of \
                    another knowledge base, with code not_found. Reason breaker means people \
                    reverted two of this token's merges on this knowledge base within seven \
                    days, so for the rest of that window its reverts only propose. A token \
                    finalizes at most 200 writes per knowledge base in a rolling hour; past \
                    that a revert is refused with code agent_rate_limited (with batch_id, as \
                    that item's code) and nothing is changed.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "merge_id": {
                            "type": "string",
                            "format": "uuid",
                            "description": "Merge id (uuid): merge_id in the result of \
                                review_merge, or the id of an item of the \"merges\" section. \
                                Leave out when batch_id is given."
                        },
                        "batch_id": {
                            "type": "string",
                            "format": "uuid",
                            "description": "Batch id (uuid): batch_id in the result of \
                                review_batch. Reverts every merge that batch made. Given \
                                instead of merge_id."
                        },
                        "rationale": {
                            "type": "string",
                            "description": "Optional: one sentence on why the merge was wrong."
                        }
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": BATCH,
                "description": "Decide 1 to 50 pending duplicate pairs in one call, all with \
                    the same action: \"merge\" or \"keep\". Each pair goes through the same \
                    path as review_merge or review_keep, one by one in the order given, so a \
                    batch grants nothing a single call would not: the server's merge gate is \
                    asked for every pair, and a pair it holds changes nothing and waits as a \
                    proposal that only a signed-in person can answer. The server issues one \
                    batch_id, recorded with every action of the batch under the token's owner \
                    and the token's name. The result is {batch_id, decided, outcomes: [{id, \
                    outcome, action_id, merge_id, reason, code, error}]}, one outcome per id in \
                    the order given; outcome is \"applied\", \"held\" or \"proposed\", or null \
                    with code and error set when that pair was refused (not_found for an \
                    unknown id, already_decided for a pair that was already decided, \
                    agent_rate_limited for each pair past the limit of 200 finalized writes \
                    per knowledge base in a rolling hour); decided counts the applied pairs. One refused pair does not stop the others. An id \
                    given twice gets, the second time, the answer its state then gives. An \
                    empty list or more than 50 ids is refused as a whole with code batch_size, \
                    and so is an action other than merge or keep; nothing is changed. To undo \
                    the merges of a batch, call review_revert with its batch_id.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "ids": {
                            "type": "array",
                            "items": { "type": "string", "format": "uuid" },
                            "description": "Review ids (uuids) of 1 to 50 pending duplicate \
                                pairs from review_list, in the order to decide them."
                        },
                        "action": {
                            "type": "string",
                            "description": "\"merge\" or \"keep\", applied to every pair."
                        },
                        "rationale": {
                            "type": "string",
                            "description": "Optional: one sentence that holds for the whole \
                                batch."
                        }
                    },
                    "required": ["ids", "action"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": DEFECT,
                "description": "Propose that a person resolve one open ontology defect, an item \
                    of the \"defects\" (Ontology) section of review_list, as \"fixed\" (the \
                    ontology was corrected) or \"accepted\" (it was looked at and is left as it \
                    is). The call changes nothing: the defect stays open and the call waits as \
                    a proposal, recorded under the token's owner and the token's name, that a \
                    signed-in editor answers in the Review tab; only when that person approves \
                    it is the defect resolved, as that person's decision. The result is {ok, \
                    outcome: \"proposed\", action_id, reason: \"propose_only\", explanation}. \
                    Repeating the call while its proposal is open returns the same proposal \
                    with code already_proposed. A resolution other than fixed or accepted is \
                    refused with code bad_resolution; a defect that is already resolved with \
                    code already_decided; an id that is unknown, or of another knowledge base, \
                    with code not_found. Proposals do not count toward the limit on finalized \
                    writes. No tool answers a proposal.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "id": {
                            "type": "string",
                            "format": "uuid",
                            "description": "Defect id (uuid) of an item of the \"defects\" \
                                section from review_list."
                        },
                        "resolution": {
                            "type": "string",
                            "description": "\"fixed\" or \"accepted\"."
                        }
                    },
                    "required": ["id", "resolution"]
                }
            }
        }
    ])
}

/// 提示，不是约束：能不能做由 `review_ops` 说了算
fn annotations(name: &str) -> Value {
    match name {
        KEEP => json!({ "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false }),
        // 合并改事实与实体：落地的那一次不是只添不改
        MERGE => json!({ "readOnlyHint": false, "destructiveHint": true, "idempotentHint": false }),
        // 撤回同样改事实与实体；第二次是 `already_decided`，不是同一个结果再来一遍
        REVERT => {
            json!({ "readOnlyHint": false, "destructiveHint": true, "idempotentHint": false })
        }
        // 一批里落地的合并同样改事实与实体
        BATCH => {
            json!({ "readOnlyHint": false, "destructiveHint": true, "idempotentHint": false })
        }
        // 只能提议的工具：这一次调用什么都不改，只留一条提议——不是只读（台账多一行），
        // 也不破坏什么
        DEFECT => {
            json!({ "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false })
        }
        _ => json!({ "readOnlyHint": true }),
    }
}

/// `tools/list` 里的那几项。两个读工具对每一个进得来的调用方都列；写工具（[`writes`]）只在
/// `can_write`（令牌 scope 是 write 且主人至少是 Editor）时列——列出一个注定被拒绝的
/// 工具，等于让对面的 agent 反复去试
pub(super) fn listed(can_write: bool) -> Vec<Value> {
    definitions()
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|tool| {
            let f = tool.get("function")?;
            let name = f.get("name")?.as_str()?;
            if writes(name) && !can_write {
                return None;
            }
            Some(json!({
                "name": name,
                "description": f.get("description").and_then(|d| d.as_str()).unwrap_or(""),
                "inputSchema": f.get("parameters").cloned().unwrap_or(json!({
                    "type": "object", "properties": {}
                })),
                "annotations": annotations(name),
            }))
        })
        .collect()
}

/// 参数没说清：这次调用没有执行。与别的工具一样是纯文字的工具错误，不带 code——
/// code 留给操作本身的拒绝
fn bad_argument(name: &str, key: &str, expected: &str) -> ToolResult {
    ToolResult::new(
        format!(
            "{name} needs `{key}` to be {expected}, and it was not, so the call was not run. \
            Call it again with `{key}` corrected, or leave it out."
        ),
        Step::new("tool", name, format!("invalid {key}"))
            .invalid(key)
            .json(),
    )
    .error()
}

/// 可选的整数：没给或 null 是「没给」，给了别的类型是参数错误——不猜
fn optional_integer(name: &str, args: &Value, key: &str) -> Result<Option<i64>, ToolResult> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_i64()
            .map(Some)
            .ok_or_else(|| bad_argument(name, key, "an integer")),
    }
}

/// 可选的 uuid：没给或 null 是「没给」，给了不是 uuid 的东西是参数错误
fn optional_uuid(name: &str, args: &Value, key: &str) -> Result<Option<Uuid>, ToolResult> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_str()
            .and_then(|s| s.trim().parse::<Uuid>().ok())
            .map(Some)
            .ok_or_else(|| bad_argument(name, key, "a uuid returned by another review tool")),
    }
}

fn optional_string<'a>(
    name: &str,
    args: &'a Value,
    key: &str,
) -> Result<Option<&'a str>, ToolResult> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(_) => Err(bad_argument(name, key, "a string")),
    }
}

/// 成功：`structuredContent` 就是令牌 API 回的那个 JSON；文字那一份是同一个 JSON 的
/// 序列化，给不读结构化结果的客户端
fn success(name: &str, content: Value) -> ToolResult {
    ToolResult::new(content.to_string(), Step::new("tool", name, "ok").json()).structured(content)
}

/// 操作的拒绝变成工具错误，`structuredContent` 是 `{"code", "message"}`。
///
/// **code 与 message 取自 REST 的错误响应本身**（`ApiErr` 渲染出来的那个 body），
/// 而不是在这里再写一张对照表：两张表迟早有一张先变，而「两个入口同一个 code」
/// 正是这一层要保证的。404 在 REST 那边没有 code，这里给它 `not_found`；别的没有
/// code 的错误不带 `code` 键。内部错误的细节同样只留在日志里。
/// 限速的 429（`AppError::RateLimited`）走的也是这一条：REST 的 body 里是
/// `agent_rate_limited`，工具错误里就是 `agent_rate_limited`——四个写工具都一样，
/// 成批的两个工具里它是那一条结果的 code
async fn refusal(name: &str, error: AppError) -> ToolResult {
    // 渲染在 `review_ops::refused`：成批调用里每一条的 code 读的也是它
    let (code, message) = review_ops::refused(error).await;
    let mut content = json!({ "message": message });
    if let Some(code) = &code {
        content["code"] = json!(code);
    }
    let detail = code.unwrap_or_else(|| "failed".to_string());
    ToolResult::new(
        message,
        Step::new("tool", name, detail).status("failed").json(),
    )
    .structured(content)
    .error()
}

/// 执行一次调用。`name` 必须是 [`serves`] 认的名字。
///
/// **写工具没被列出来的调用方照样走到 `review_ops` 的那个函数**：拒绝它的是那里的
/// 检查，回来的是带 code 的工具错误（`token_read_only`、`token_owner_below_editor`），
/// 与 REST 同一个 code——对面的 agent 由此知道是换一枚令牌，还是换令牌也没用
pub(super) async fn call(
    state: &AppState,
    caller: &ReviewCaller,
    name: &str,
    args: &Value,
) -> ToolResult {
    let args = match super::chat::check_call(&definitions(), name, &args.to_string()) {
        Ok(args) => args,
        Err((text, step)) => return ToolResult::new(text, step).error(),
    };
    let outcome = match name {
        LIST => {
            // `section` 必填且是字符串，上面的守卫已经看过。这里只按档名挑操作，不判它
            // 认不认得：认不出的名字落到 `review_ops::list`，由它拒绝（`unknown_queue`）
            let section = args["section"].as_str().unwrap_or_default().trim();
            // 翻页参数只读这一档用的那一组，先看类型再读库。这一档不用的参数不读也不查：
            // 总览从来不看 `limit`，决策台账同理不看它，列表档不看 `page` / `per`
            match section {
                "overview" => match review_ops::summary(state, caller).await {
                    Ok(summary) => serde_json::to_value(summary)
                        .map_err(|e| AppError::Other(anyhow::Error::new(e))),
                    Err(e) => Err(e),
                },
                // 决策台账有它自己的 `page` / `per`，回它自己的 `{events, total}`
                "decisions" => {
                    let page = match optional_integer(name, &args, "page") {
                        Ok(v) => v,
                        Err(refused) => return refused,
                    };
                    let per = match optional_integer(name, &args, "per") {
                        Ok(v) => v,
                        Err(refused) => return refused,
                    };
                    review_ops::history(state, caller, page, per).await
                }
                _ => {
                    let limit = match optional_integer(name, &args, "limit") {
                        Ok(v) => v,
                        Err(refused) => return refused,
                    };
                    let offset = match optional_integer(name, &args, "offset") {
                        Ok(v) => v,
                        Err(refused) => return refused,
                    };
                    review_ops::list(state, caller, section, None, limit, offset).await
                }
            }
        }
        ITEM => {
            // 不给 `section` 就是重复项，与只有这一档时的调用同一个意思
            let section = match optional_string(name, &args, "section") {
                Ok(v) => v.map_or("duplicates", str::trim),
                Err(refused) => return refused,
            };
            // `id` 必填且是字符串，守卫看过；**原样递下去**——类别词按存着的样子比，
            // 是不是 uuid、在不在这一档里，都由 `review_ops::item` 说
            let id = args["id"].as_str().unwrap_or_default();
            review_ops::item(state, caller, section, id).await
        }
        KEEP => {
            // `format: uuid` 由守卫挡过；这里再解析一次，解析不了同样不执行
            let Some(id) = args["id"]
                .as_str()
                .and_then(|s| s.trim().parse::<Uuid>().ok())
            else {
                return bad_argument(name, "id", "a uuid returned by review_list");
            };
            let rationale = match optional_string(name, &args, "rationale") {
                Ok(v) => v,
                Err(refused) => return refused,
            };
            review_ops::keep(state, caller, id, rationale, None).await
        }
        MERGE => {
            let (id, source, target) = match (
                optional_uuid(name, &args, "id"),
                optional_uuid(name, &args, "source"),
                optional_uuid(name, &args, "target"),
            ) {
                (Ok(id), Ok(source), Ok(target)) => (id, source, target),
                (Err(refused), _, _) | (_, Err(refused), _) | (_, _, Err(refused)) => {
                    return refused
                }
            };
            let rationale = match optional_string(name, &args, "rationale") {
                Ok(v) => v,
                Err(refused) => return refused,
            };
            // 两种给法各走各的操作，与 REST 的两条路由一一对应；闸门在操作里面。
            // 给法说不清（两种都给、只给一半、都没给）就不执行，不猜是哪一种
            match (id, source, target) {
                (Some(id), None, None) => {
                    review_ops::merge_pair(state, caller, id, rationale, None).await
                }
                (None, Some(source), Some(target)) => {
                    review_ops::merge_entities(state, caller, source, target, rationale).await
                }
                _ => {
                    return ToolResult::new(
                        format!(
                            "{name} needs either `id` (a pending duplicate pair) or both \
                            `source` and `target` (two entity ids), and not both forms, so \
                            the call was not run. Call it again with one of the two."
                        ),
                        Step::new("tool", name, "invalid arguments")
                            .invalid("id")
                            .json(),
                    )
                    .error()
                }
            }
        }
        REVERT => {
            // 两个 id 都不是必填，守卫不看它们的格式：这里解析，解析不了就不执行
            let (merge_id, batch_id) = match (
                optional_uuid(name, &args, "merge_id"),
                optional_uuid(name, &args, "batch_id"),
            ) {
                (Ok(merge_id), Ok(batch_id)) => (merge_id, batch_id),
                (Err(refused), _) | (_, Err(refused)) => return refused,
            };
            let rationale = match optional_string(name, &args, "rationale") {
                Ok(v) => v,
                Err(refused) => return refused,
            };
            // 两种给法各走各的操作，与 REST 的两条路由一一对应：一次合并的撤回（直接撤还是
            // 只留提议，由它里面的规则说），一批的整批撤回。两个都给、都没给就不执行，不猜
            match (merge_id, batch_id) {
                (Some(merge_id), None) => {
                    review_ops::revert_merge(state, caller, merge_id, rationale, None).await
                }
                (None, Some(batch_id)) => {
                    review_ops::revert_batch(state, caller, batch_id, rationale).await
                }
                _ => {
                    return ToolResult::new(
                        format!(
                            "{name} needs exactly one of `merge_id` (one merge) and `batch_id` \
                            (every merge of one batch), so the call was not run. Call it again \
                            with one of the two."
                        ),
                        Step::new("tool", name, "invalid arguments")
                            .invalid("merge_id")
                            .json(),
                    )
                    .error()
                }
            }
        }
        BATCH => {
            // `ids` 与 `action` 必填，守卫看过在不在；`ids` 里每一个都得是 uuid——有一个
            // 不是，整批不执行（REST 上同样是整个请求被拒）。条数不在这里看：0 条与 51 条
            // 由 `review_ops::batch` 拒绝，带的是 REST 的 code
            let ids: Option<Vec<Uuid>> = args["ids"].as_array().and_then(|ids| {
                ids.iter()
                    .map(|id| id.as_str().and_then(|s| s.trim().parse::<Uuid>().ok()))
                    .collect()
            });
            let Some(ids) = ids else {
                return bad_argument(name, "ids", "a list of uuids returned by review_list");
            };
            let Some(action) = args["action"].as_str() else {
                return bad_argument(name, "action", "\"merge\" or \"keep\"");
            };
            let rationale = match optional_string(name, &args, "rationale") {
                Ok(v) => v,
                Err(refused) => return refused,
            };
            review_ops::batch(state, caller, &ids, action.trim(), rationale).await
        }
        DEFECT => {
            // 参数就是那条路由的 id 与请求体的字段；`resolution` 认不认得由操作说
            // （`bad_resolution`，与 REST 同一个 code），这里原样递下去
            let Some(id) = args["id"]
                .as_str()
                .and_then(|s| s.trim().parse::<Uuid>().ok())
            else {
                return bad_argument(name, "id", "a uuid returned by review_list");
            };
            let Some(resolution) = args["resolution"].as_str() else {
                return bad_argument(name, "resolution", "\"fixed\" or \"accepted\"");
            };
            review_ops::propose_defect(state, caller, id, resolution).await
        }
        // `serves` 之外的名字到不了这里；到了也只说没有这个工具，不去碰库
        _ => {
            return ToolResult::new(
                format!("Unknown tool: {name}"),
                Step::new("tool", name, "unknown").status("failed").json(),
            )
            .error()
        }
    };
    match outcome {
        Ok(content) => success(name, content),
        Err(error) => refusal(name, error).await,
    }
}
