//! Review 的三个 MCP 工具（0076 决定 2）：`review_list`、`review_item`、`review_keep`。
//!
//! **这里只做两件事：把参数递给 `review_ops`，把它的回答变成工具结果。** 授权、裁定表、
//! 台账、审计都在 `review_ops` 里面，REST 的令牌分支调的是同一批函数——所以同一枚令牌
//! 在两个入口读到的东西相等，被拒绝时拿到的 code 也相等。这里不另做一道检查：
//! 两道检查迟早有一道先变。
//!
//! **不进 `chat::tools_schema`，也不进 `tools::dispatch`。** 那两处与应用内的聊天助手
//! 共用，而助手不该拿到任何 Review 工具。这三个工具的定义与派发只在这个文件里，
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

/// 这个名字归不归这里管。归这里的，`mcp.rs` 不再拿它去问 `is_exposed`
pub(super) fn serves(name: &str) -> bool {
    matches!(name, LIST | ITEM | KEEP)
}

/// 工具定义，OpenAI 的形状——`chat::check_call` 认的就是这个形状，参数守卫与别的工具
/// 共用一份。
///
/// `limit` **不写上下界**：越界的值不是错误，照会话路由的规矩夹到 1 到 200
/// （`review_ops::list`）。写了 `minimum` 的话，守规矩的客户端会在本地就把 0 拦下
fn definitions() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": LIST,
                "description": "Read one section of this knowledge base's Review tab. \
                    section \"duplicates\" returns one page of pending duplicate pairs as \
                    {counts, queue, items}; each item carries both entities with their type, \
                    disambiguator, fact count and top facts, and the governor's open proposal \
                    if there is one. section \"overview\" returns the Review summary (what is \
                    waiting, what was decided, health, the governor's recent rows). No other \
                    section is served yet; asking for one is refused with code unknown_queue.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "section": {
                            "type": "string",
                            "description": "\"duplicates\" or \"overview\"."
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Page size for duplicates (default 10). A value \
                                below 1 is served as 1 and a value above 200 as 200."
                        },
                        "offset": {
                            "type": "integer",
                            "description": "Zero-based start of the duplicates page (default 0)."
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
                "description": "Read the evidence on one pending duplicate pair, by the id \
                    shown in review_list: score, reason, stage, both entities with their type, \
                    disambiguator, fact count and top facts, and the governor's open proposal. \
                    An id that is not a pending duplicate pair of this knowledge base is \
                    refused with code not_found.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "id": {
                            "type": "string",
                            "format": "uuid",
                            "description": "Review id (uuid) of a pair from review_list."
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
        }
    ])
}

/// 提示，不是约束：能不能做由 `review_ops` 说了算
fn annotations(name: &str) -> Value {
    if name == KEEP {
        json!({ "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false })
    } else {
        json!({ "readOnlyHint": true })
    }
}

/// `tools/list` 里的那几项。两个读工具对每一个进得来的调用方都列；`review_keep` 只在
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
            if name == KEEP && !can_write {
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
/// **`review_keep` 没被列出来的调用方照样走到 `review_ops::keep`**：拒绝它的是那里的
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
            // `section` 必填且是字符串，上面的守卫已经看过
            let section = args["section"].as_str().unwrap_or_default().trim();
            if section == "overview" {
                match review_ops::summary(state, caller).await {
                    Ok(summary) => serde_json::to_value(summary)
                        .map_err(|e| AppError::Other(anyhow::Error::new(e))),
                    Err(e) => Err(e),
                }
            } else {
                // 翻页参数先看类型再读库；`duplicates` 以外的档名交给 `review_ops::list`
                // 去拒绝（`unknown_queue`），这里不另判一次
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
        ITEM | KEEP => {
            // `format: uuid` 由守卫挡过；这里再解析一次，解析不了同样不执行
            let Some(id) = args["id"]
                .as_str()
                .and_then(|s| s.trim().parse::<Uuid>().ok())
            else {
                return bad_argument(name, "id", "a uuid returned by review_list");
            };
            if name == ITEM {
                match review_ops::item(state, caller, id).await {
                    Ok(item) => serde_json::to_value(item)
                        .map_err(|e| AppError::Other(anyhow::Error::new(e))),
                    Err(e) => Err(e),
                }
            } else {
                let rationale = match optional_string(name, &args, "rationale") {
                    Ok(v) => v,
                    Err(refused) => return refused,
                };
                review_ops::keep(state, caller, id, rationale).await
            }
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
