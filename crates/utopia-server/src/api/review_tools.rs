//! Review 的 MCP 工具（0076 决定 2）：`review_list`、`review_item`、`review_keep`、
//! `review_merge`、`review_revert`。
//!
//! 两个读工具管 Review 的十三档：`review_list` 读一档的一页（十一个列表档、总览、决策台账），
//! `review_item` 读某一档里列着的一条，违规一档的条目带上它的证明。写的有两样：
//! `review_keep` 把一对分开；`review_merge` 合并队列里的一对或手动指定的两个实体，
//! 先过服务端的闸门，被留下的不落地、只留一条等人答的提议；`review_revert` 撤回一次
//! 合并——令牌 agent 做的直接撤，人或治理 agent 做的只留提议。**没有哪个工具能答提议。**
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

use axum::response::IntoResponse;
use serde_json::{json, Value};
use utopia_core::AppError;
use uuid::Uuid;

use super::review_ops::{self, ReviewCaller};
use super::tools::{Step, ToolResult};
use crate::error::ApiErr;
use crate::state::AppState;

const LIST: &str = "review_list";
const ITEM: &str = "review_item";
const KEEP: &str = "review_keep";
const MERGE: &str = "review_merge";
const REVERT: &str = "review_revert";

/// 这个名字归不归这里管。归这里的，`mcp.rs` 不再拿它去问 `is_exposed`
pub(super) fn serves(name: &str) -> bool {
    matches!(name, LIST | ITEM | KEEP | MERGE | REVERT)
}

/// 写工具：只列给写得了的调用方，标注里 `readOnlyHint` 是 false
fn writes(name: &str) -> bool {
    matches!(name, KEEP | MERGE | REVERT)
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
/// 类型，存活的事实留着抄给它的陈述来源。这三句写在说明里，对面的 agent 读得到
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
                    A pair that was already decided is refused with code already_decided.",
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
                    unknown id with code not_found.",
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
                    review_merge, or from review_list with section \"merges\"). A merge that a \
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
                    another knowledge base, with code not_found.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "merge_id": {
                            "type": "string",
                            "format": "uuid",
                            "description": "Merge id (uuid): merge_id in the result of \
                                review_merge, or the id of an item of the \"merges\" section."
                        },
                        "rationale": {
                            "type": "string",
                            "description": "Optional: one sentence on why the merge was wrong."
                        }
                    },
                    "required": ["merge_id"]
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
/// code 的错误不带 `code` 键。内部错误的细节同样只留在日志里
async fn refusal(name: &str, error: AppError) -> ToolResult {
    let response = ApiErr(error).into_response();
    let status = response.status();
    let body: Value = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(Value::Null);
    let message = body["error"]
        .as_str()
        .unwrap_or("Internal server error")
        .to_string();
    let code = body["code"]
        .as_str()
        .map(str::to_string)
        .or_else(|| (status == axum::http::StatusCode::NOT_FOUND).then(|| "not_found".to_string()));
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
            review_ops::keep(state, caller, id, rationale).await
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
            // `format: uuid` 由守卫挡过；这里再解析一次，解析不了同样不执行
            let Some(merge_id) = args["merge_id"]
                .as_str()
                .and_then(|s| s.trim().parse::<Uuid>().ok())
            else {
                return bad_argument(name, "merge_id", "a uuid returned by another review tool");
            };
            let rationale = match optional_string(name, &args, "rationale") {
                Ok(v) => v,
                Err(refused) => return refused,
            };
            // 与 REST 的撤回路由同一个操作：直接撤还是只留提议，由它里面的规则说
            review_ops::revert_merge(state, caller, merge_id, rationale, None).await
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
