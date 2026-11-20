//! Review 其余各档的令牌写入测试共用的夹具（0076）。
//!
//! 每一档一个测试模块（`review_token_defect_tests`、`review_token_fact_tests`……），要钉的
//! 是同一组事情：令牌的调用只留一条提议、什么都不改；人批准之后才照人自己的决定落地；
//! MCP 工具与 REST 是同一个回答。这里是它们都要用的那几样：
//!
//! - 调用：带 Bearer 的一次 REST 调用（[`post`]）、答一条提议（[`answer`]）、MCP 的
//!   `tools/list`（[`tools_list`]）与 `tools/call`（[`tool_call`]）
//! - 判断：一个工具结果是不是带着这个 code 的拒绝（[`ensure_tool_refused`]）
//! - 读库：「什么都没动」比的那一份快照（[`snapshot`]）、目标自己的那一行（[`row`]）、
//!   这个库的台账行（[`ledger`]）与开着的提议（[`open_proposals`]）、审计行（[`audits`]）
//!
//! 夹具是 `review_session_unchanged_tests` 的 [`Fx`]，种子是 `review_totals_tests` 的
//! [`seed_every_section`]（[`seeded`] 把两样合在一起）。后来的测试模块只引用这里，不改这里。
//! 没有 `UTOPIA_DATABASE_URL` 时 [`seeded`] 回 `None`，测试跳过。
use axum::http::StatusCode;
use serde_json::{json, Value};
use utopia_store::token_agent_actions::TokenAgentAction;
use uuid::Uuid;

pub(super) use super::review_session_unchanged_tests::Fx;
use super::review_totals_tests::seed_every_section;

/// 一个十三档都有东西的库。没有数据库时是 `None`
pub(super) async fn seeded() -> anyhow::Result<Option<Fx>> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(None);
    };
    if let Err(e) = seed_every_section(&f).await {
        f.cleanup().await?;
        return Err(e);
    }
    Ok(Some(f))
}

// ───────────────────────── 调用 ─────────────────────────

/// 带 Bearer 的一次 POST。`bearer` 是令牌明文或会话 JWT，`tail` 接在 `/kbs/{f.kb}/` 后面
pub(super) async fn post(
    f: &Fx,
    bearer: &str,
    tail: &str,
    body: Value,
) -> anyhow::Result<(StatusCode, Value)> {
    f.call(bearer, "POST", &format!("/kbs/{}/{tail}", f.kb), Some(body))
        .await
}

/// 答一条提议（单条的那条路由）：`action` 是 approve 或 decline
pub(super) async fn answer(
    f: &Fx,
    bearer: &str,
    proposal: Uuid,
    action: &str,
) -> anyhow::Result<(StatusCode, Value)> {
    post(
        f,
        bearer,
        &format!("review/agent-proposals/{proposal}"),
        json!({ "action": action }),
    )
    .await
}

/// 一次 JSON-RPC 调用，回 HTTP 状态与整个响应体
async fn rpc(
    f: &Fx,
    bearer: &str,
    method: &str,
    params: Value,
) -> anyhow::Result<(StatusCode, Value)> {
    post(
        f,
        bearer,
        "mcp",
        json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }),
    )
    .await
}

/// 这个调用方在 `tools/list` 里看到的工具
pub(super) async fn tools_list(f: &Fx, bearer: &str) -> anyhow::Result<Vec<Value>> {
    let (status, body) = rpc(f, bearer, "tools/list", json!({})).await?;
    anyhow::ensure!(status == StatusCode::OK, "tools/list: {status} {body}");
    Ok(body["result"]["tools"]
        .as_array()
        .cloned()
        .unwrap_or_default())
}

/// 一次工具调用的 `result`。传输层或 JSON-RPC 层出错在这里就是测试失败——工具自己的
/// 拒绝是 `isError: true` 的正常结果，不走那条路
pub(super) async fn tool_call(
    f: &Fx,
    bearer: &str,
    name: &str,
    args: Value,
) -> anyhow::Result<Value> {
    let (status, body) = rpc(
        f,
        bearer,
        "tools/call",
        json!({ "name": name, "arguments": args }),
    )
    .await?;
    anyhow::ensure!(status == StatusCode::OK, "{name} {args}: {status} {body}");
    anyhow::ensure!(body.get("error").is_none(), "{name} {args}: {body}");
    anyhow::ensure!(body["result"].is_object(), "{name} {args}: {body}");
    Ok(body["result"].clone())
}

/// 对象的键，排好序
pub(super) fn keys(v: &Value) -> Vec<String> {
    let mut k: Vec<String> = v
        .as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    k.sort();
    k
}

/// 一个被拒绝的工具结果：`isError`，带这个 code，文字就是那句 message，
/// `structuredContent` 里除了这两样什么都没有
pub(super) fn ensure_tool_refused(result: &Value, code: &str) -> anyhow::Result<()> {
    anyhow::ensure!(result["isError"] == true, "expected {code}: {result}");
    let content = &result["structuredContent"];
    anyhow::ensure!(content["code"] == code, "expected {code}: {result}");
    anyhow::ensure!(
        content["message"].is_string() && content["message"] == result["content"][0]["text"],
        "{result}"
    );
    anyhow::ensure!(
        keys(content) == ["code", "message"],
        "a refusal carries a code and a message and nothing else: {result}"
    );
    Ok(())
}

// ───────────────────────── 读库 ─────────────────────────

/// 「什么都没动」比的那一份（E1）：这个库的每一条事实、每一个实体、每一行事实证据的
/// 每一列，加上排着的任务数。两次快照相等，就是图没动、也没排下任务
pub(super) async fn snapshot(f: &Fx) -> anyhow::Result<Value> {
    Ok(sqlx::query_scalar(
        "SELECT jsonb_build_object(
            'facts', (SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY t.id), '[]'::jsonb)
                        FROM facts t WHERE t.kb_id = $1),
            'entities', (SELECT coalesce(jsonb_agg(to_jsonb(e) ORDER BY e.id), '[]'::jsonb)
                           FROM entities e WHERE e.kb_id = $1),
            'fact_evidence', (SELECT coalesce(jsonb_agg(to_jsonb(v)
                                                        ORDER BY v.fact_id, v.chunk_id),
                                              '[]'::jsonb)
                                FROM fact_evidence v JOIN facts t ON t.id = v.fact_id
                               WHERE t.kb_id = $1),
            'queued_jobs', (SELECT count(*) FROM jobs
                             WHERE status = 'queued' AND payload ->> 'kb_id' = $1::text))",
    )
    .bind(f.kb)
    .fetch_one(&f.pool)
    .await?)
}

/// 目标自己的那一行，每一列。`table` 是测试里写死的表名；行不在就是 `null`
pub(super) async fn row(f: &Fx, table: &'static str, id: Uuid) -> anyhow::Result<Value> {
    let found: Option<Value> = sqlx::query_scalar(&format!(
        "SELECT to_jsonb(t) FROM {table} t WHERE t.id = $1 AND t.kb_id = $2"
    ))
    .bind(id)
    .bind(f.kb)
    .fetch_optional(&f.pool)
    .await?;
    Ok(found.unwrap_or(Value::Null))
}

/// 这个库的台账行，最早的在前
pub(super) async fn ledger(f: &Fx) -> anyhow::Result<Vec<TokenAgentAction>> {
    let mut rows = utopia_store::token_agent_actions::list(&f.pool, f.kb, 2000, 0).await?;
    rows.reverse();
    Ok(rows)
}

/// 这个库里开着的提议（`held` / `proposed`，还没人答），最早的在前
pub(super) async fn open_proposals(f: &Fx) -> anyhow::Result<Vec<TokenAgentAction>> {
    Ok(ledger(f)
        .await?
        .into_iter()
        .filter(|row| (row.outcome == "held" || row.outcome == "proposed") && row.state.is_none())
        .collect())
}

/// 一条审计行里测试要看的那几样
#[derive(Debug, Clone)]
pub(super) struct Audit {
    pub action: String,
    pub actor: Option<Uuid>,
    pub target_kind: String,
    pub target_id: Option<Uuid>,
    pub detail: Value,
}

/// 这个库的审计行，最早的在前
pub(super) async fn audits(f: &Fx) -> anyhow::Result<Vec<Audit>> {
    #[allow(clippy::type_complexity)]
    let rows: Vec<(String, Option<Uuid>, String, Option<Uuid>, Value)> = sqlx::query_as(
        "SELECT action, actor_id, target_kind, target_id, detail FROM audit_events
          WHERE kb_id = $1 ORDER BY created_at, id",
    )
    .bind(f.kb)
    .fetch_all(&f.pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(action, actor, target_kind, target_id, detail)| Audit {
            action,
            actor,
            target_kind,
            target_id,
            detail,
        })
        .collect())
}
