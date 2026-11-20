//! 编辑者的写令牌撤回一次合并（0076）：令牌 API 的撤回路由与 MCP 工具 `review_revert`。
//!
//! 要钉住的行为：
//!
//! - **令牌 agent 做的合并，令牌直接撤**（SC5 / E13）：回答是 `applied`，台账一行落地的
//!   `merge.revert` 指着这次合并，审计行带 `token_id` 与 `token_name`；再撤一次是
//!   `already_decided`
//! - **撤回之后图回到合并之前**（E12，INTENT C1）：`facts`、`entities`（不含 `updated_at`）、
//!   `fact_evidence` 逐行相等；`typed_fact_sources` 原有的行一行不少，多出来的只有合并
//!   抄到存活事实上的那一条陈述来源。比的是有类型的实体
//! - **哪一枚令牌做的合并都撤得动；认不出的、别的库的合并 id 是「没有」**（E14）
//! - **人做的、治理 agent 做的合并，令牌只留提议**：200、`proposed`、理由，什么都没撤；
//!   再来一次是 `already_proposed`
//! - **`review_revert` 只列给编辑者的写令牌**，说明里写着撤回不收回什么；拒绝带 REST 的 code
//!
//! 全部经路由进去。夹具在 `review_session_unchanged_tests.rs`。种子直接插表或走 store
//! 函数，不走模型任务。没有 `UTOPIA_DATABASE_URL` 时跳过。自建自拆，只碰自己建的库。
use axum::http::StatusCode;
use serde_json::{json, Value};
use utopia_store::names::NameSource;
use utopia_store::token_agent_actions::TokenAgentAction;
use uuid::Uuid;

use super::review_session_unchanged_tests::Fx;

const TOKEN_NAME: &str = "cleanup agent";

// ───────────────────────── 调用 ─────────────────────────

/// 手动合并，经 REST，在指定的库上
async fn rest_manual(
    f: &Fx,
    kb: Uuid,
    bearer: &str,
    source: Uuid,
    target: Uuid,
) -> anyhow::Result<(StatusCode, Value)> {
    f.call(
        bearer,
        "POST",
        &format!("/kbs/{kb}/entities/merge"),
        Some(json!({ "source": source, "target": target })),
    )
    .await
}

/// 撤回，经 REST。`body` 为 None 就是不带请求体
async fn rest_revert(
    f: &Fx,
    bearer: &str,
    merge_id: Uuid,
    body: Option<Value>,
) -> anyhow::Result<(StatusCode, Value)> {
    f.call(
        bearer,
        "POST",
        &format!("/kbs/{}/merges/{merge_id}/revert", f.kb),
        body,
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
    f.call(
        bearer,
        "POST",
        &format!("/kbs/{}/mcp", f.kb),
        Some(json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params })),
    )
    .await
}

/// 一次工具调用，回工具结果。传输层或 JSON-RPC 层出错在这里就是测试失败——
/// 工具自己的拒绝是 `isError: true` 的正常结果
async fn tool(f: &Fx, bearer: &str, name: &str, args: Value) -> anyhow::Result<Value> {
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

async fn mcp_revert(f: &Fx, bearer: &str, args: Value) -> anyhow::Result<Value> {
    tool(f, bearer, "review_revert", args).await
}

/// 成功的工具结果里的 `structuredContent`
fn structured(result: &Value) -> anyhow::Result<Value> {
    anyhow::ensure!(result["isError"] == false, "a tool error: {result}");
    Ok(result["structuredContent"].clone())
}

/// 一个被拒绝的工具结果：`isError`，`structuredContent` 正好是 `{"code","message"}`
fn ensure_refused(result: &Value, code: &str) -> anyhow::Result<()> {
    anyhow::ensure!(result["isError"] == true, "expected {code}: {result}");
    let content = &result["structuredContent"];
    anyhow::ensure!(content["code"] == code, "expected {code}: {result}");
    anyhow::ensure!(
        content["message"].is_string() && content["message"] == result["content"][0]["text"],
        "{result}"
    );
    anyhow::ensure!(keys(content) == ["code", "message"], "{result}");
    Ok(())
}

/// 对象的键，排好序
fn keys(v: &Value) -> Vec<String> {
    let mut k: Vec<String> = v
        .as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    k.sort();
    k
}

fn uuid_at(body: &Value, key: &str) -> anyhow::Result<Uuid> {
    Ok(body[key]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("no {key} in {body}"))?
        .parse()?)
}

/// 落地的回答：正好四个键。回 (台账行, 合并)
fn ensure_applied(body: &Value) -> anyhow::Result<(Uuid, Uuid)> {
    anyhow::ensure!(
        keys(body) == ["action_id", "merge_id", "ok", "outcome"],
        "an applied answer: {body}"
    );
    anyhow::ensure!(body["ok"] == true && body["outcome"] == "applied", "{body}");
    Ok((uuid_at(body, "action_id")?, uuid_at(body, "merge_id")?))
}

/// 留成提议的撤回：`proposed`、理由、一句说明；`repeat` 时多一个 `code: already_proposed`。
/// 回提议（台账行）的 id
fn ensure_proposed(body: &Value, reason: &str, repeat: bool) -> anyhow::Result<Uuid> {
    let expected: &[&str] = if repeat {
        &[
            "action_id",
            "code",
            "explanation",
            "ok",
            "outcome",
            "reason",
        ]
    } else {
        &["action_id", "explanation", "ok", "outcome", "reason"]
    };
    anyhow::ensure!(
        keys(body) == expected,
        "a proposed answer (repeat {repeat}): {body}"
    );
    anyhow::ensure!(
        body["ok"] == true && body["outcome"] == "proposed",
        "{body}"
    );
    anyhow::ensure!(body["reason"] == reason, "expected {reason}: {body}");
    anyhow::ensure!(
        body["explanation"]
            .as_str()
            .is_some_and(|text| text.contains("Nothing was changed")),
        "{body}"
    );
    if repeat {
        anyhow::ensure!(body["code"] == "already_proposed", "{body}");
    }
    uuid_at(body, "action_id")
}

// ───────────────────────── 读库 ─────────────────────────

/// 某个库的台账，最早的在前
async fn ledger_of(f: &Fx, kb: Uuid) -> anyhow::Result<Vec<TokenAgentAction>> {
    let mut rows = utopia_store::token_agent_actions::list(&f.pool, kb, 500, 0).await?;
    rows.reverse();
    Ok(rows)
}

async fn ledger(f: &Fx) -> anyhow::Result<Vec<TokenAgentAction>> {
    ledger_of(f, f.kb).await
}

async fn ledger_row(f: &Fx, id: Uuid) -> anyhow::Result<TokenAgentAction> {
    ledger(f)
        .await?
        .into_iter()
        .find(|row| row.id == id)
        .ok_or_else(|| anyhow::anyhow!("no ledger row {id}"))
}

/// 这个库里某个动作的审计行：actor、target_kind、target_id、detail，最早的在前
async fn audit_rows(
    f: &Fx,
    action: &str,
) -> anyhow::Result<Vec<(Option<Uuid>, String, Option<Uuid>, Value)>> {
    Ok(sqlx::query_as(
        "SELECT actor_id, target_kind, target_id, detail FROM audit_events
          WHERE kb_id = $1 AND action = $2 ORDER BY created_at, id",
    )
    .bind(f.kb)
    .bind(action)
    .fetch_all(&f.pool)
    .await?)
}

async fn merged_into(f: &Fx, entity: Uuid) -> anyhow::Result<Option<Uuid>> {
    Ok(
        sqlx::query_scalar("SELECT merged_into FROM entities WHERE id = $1")
            .bind(entity)
            .fetch_one(&f.pool)
            .await?,
    )
}

/// 一次合并：源、目标、是否已经撤回
async fn merge_row(f: &Fx, merge_id: Uuid) -> anyhow::Result<(Uuid, Uuid, bool)> {
    Ok(sqlx::query_as(
        "SELECT source_id, target_id, reverted_at IS NOT NULL FROM entity_merges WHERE id = $1",
    )
    .bind(merge_id)
    .fetch_one(&f.pool)
    .await?)
}

/// 「什么都没撤」比的东西：这个库的实体、事实、证据、类型化事实的来源、审核行、
/// 合并日志，每一行的每一列
async fn graph(f: &Fx) -> anyhow::Result<Value> {
    let mut out = serde_json::Map::new();
    for (part, sql) in [
        (
            "entities",
            "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY t.id), '[]'::jsonb)
               FROM entities t WHERE t.kb_id = $1",
        ),
        (
            "facts",
            "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY t.id), '[]'::jsonb)
               FROM facts t WHERE t.kb_id = $1",
        ),
        (
            "fact_evidence",
            "SELECT coalesce(jsonb_agg(to_jsonb(e) ORDER BY e.fact_id, e.chunk_id), '[]'::jsonb)
               FROM fact_evidence e JOIN facts t ON t.id = e.fact_id WHERE t.kb_id = $1",
        ),
        (
            "typed_fact_sources",
            "SELECT coalesce(jsonb_agg(to_jsonb(s) ORDER BY s.fact_id, s.statement_id), '[]'::jsonb)
               FROM typed_fact_sources s JOIN facts t ON t.id = s.fact_id WHERE t.kb_id = $1",
        ),
        (
            "resolution_reviews",
            "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY t.id), '[]'::jsonb)
               FROM resolution_reviews t WHERE t.kb_id = $1",
        ),
        (
            "entity_merges",
            "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY t.id), '[]'::jsonb)
               FROM entity_merges t WHERE t.kb_id = $1",
        ),
    ] {
        let rows: Value = sqlx::query_scalar(sql).bind(f.kb).fetch_one(&f.pool).await?;
        out.insert(part.to_string(), rows);
    }
    Ok(Value::Object(out))
}

/// 两份快照逐部分比，不等时说是哪一部分
fn ensure_same(before: &Value, after: &Value, what: &str) -> anyhow::Result<()> {
    for part in keys(before) {
        anyhow::ensure!(
            before[&part] == after[&part],
            "{what}: {part} changed\nbefore {}\nafter  {}",
            before[&part],
            after[&part]
        );
    }
    anyhow::ensure!(before == after, "{what}: the graph changed");
    Ok(())
}

// ───────────────────────── 种子 ─────────────────────────

async fn relation(f: &Fx, key: &str, label: &str) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO relation_types (id, kb_id, key, label, functional)
         VALUES ($1, $2, $3, $4, false)",
    )
    .bind(id)
    .bind(f.kb)
    .bind(key)
    .bind(label)
    .execute(&f.pool)
    .await?;
    Ok(id)
}

/// 一条实体指向实体的类型化事实
async fn fact(f: &Fx, subject: Uuid, predicate: Uuid, object: Uuid) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO facts (id, kb_id, subject_id, predicate_id, object_id, attested_from)
         VALUES ($1, $2, $3, $4, $5, NULL)",
    )
    .bind(id)
    .bind(f.kb)
    .bind(subject)
    .bind(predicate)
    .bind(object)
    .execute(&f.pool)
    .await?;
    Ok(id)
}

/// 一份文档里的一个分块
async fn chunk(f: &Fx) -> anyhow::Result<Uuid> {
    let (document, chunk) = (Uuid::now_v7(), Uuid::now_v7());
    sqlx::query(
        "INSERT INTO documents (id, kb_id, filename, sha256, source_id)
         VALUES ($1, $2, 'd', $1::text, NULL)",
    )
    .bind(document)
    .bind(f.kb)
    .execute(&f.pool)
    .await?;
    sqlx::query(
        "INSERT INTO chunks (id, kb_id, document_id, seq, text) VALUES ($1, $2, $3, 0, 'acme')",
    )
    .bind(chunk)
    .bind(f.kb)
    .bind(document)
    .execute(&f.pool)
    .await?;
    Ok(chunk)
}

/// 这个库里两样关系与一个共用的宾语
struct Seeds {
    part_of: Uuid,
    based_in: Uuid,
    city: Uuid,
}

async fn seeds(f: &Fx) -> anyhow::Result<Seeds> {
    Ok(Seeds {
        part_of: relation(f, "part_of", "part of").await?,
        based_in: relation(f, "based_in", "based in").await?,
        city: f.entity(f.company_type, "Port Ellen").await?,
    })
}

/// 两个公司，各有一条事实，闸门不拦：回 (左, 右)
async fn passing(f: &Fx, s: &Seeds, name: &str) -> anyhow::Result<(Uuid, Uuid)> {
    let a = f.entity(f.company_type, name).await?;
    let c = f.entity(f.company_type, &format!("{name} Corp")).await?;
    fact(f, a, s.based_in, s.city).await?;
    fact(f, c, s.part_of, s.city).await?;
    Ok((a, c))
}

/// 令牌 agent 的一次合并：闸门不拦的一对，经令牌的手动合并路由落地。回 (合并, 源, 目标)
async fn token_merge(
    f: &Fx,
    s: &Seeds,
    bearer: &str,
    name: &str,
) -> anyhow::Result<(Uuid, Uuid, Uuid)> {
    let (source, target) = passing(f, s, name).await?;
    let (status, body) = rest_manual(f, f.kb, bearer, source, target).await?;
    anyhow::ensure!(status == StatusCode::OK, "seeding a merge: {status} {body}");
    let (_, merge_id) = ensure_applied(&body)?;
    anyhow::ensure!(merged_into(f, source).await? == Some(target));
    Ok((merge_id, source, target))
}

/// 一行落地的 `merge.revert` 台账行该有的样子
fn ensure_revert_row(
    f: &Fx,
    row: &TokenAgentAction,
    token_id: Uuid,
    merge_id: Uuid,
) -> anyhow::Result<()> {
    anyhow::ensure!(row.outcome == "applied", "{row:?}");
    anyhow::ensure!(
        row.operation == "merge.revert" && row.target_kind == "merge" && row.target_id == merge_id,
        "{row:?}"
    );
    anyhow::ensure!(
        row.merge_id == Some(merge_id),
        "a revert row names the merge it targets: {row:?}"
    );
    anyhow::ensure!(
        row.owner_id == f.editor && row.token_id == token_id,
        "{row:?}"
    );
    anyhow::ensure!(row.gate_reason.is_none() && row.state.is_none(), "{row:?}");
    Ok(())
}

/// 这次合并上令牌写下的 `merge.revert` 审计行：正好一行，actor 是主人，detail 带令牌、
/// 台账行与合并
async fn ensure_revert_audit(
    f: &Fx,
    (token_id, token_name): (Uuid, &str),
    (action_id, merge_id): (Uuid, Uuid),
) -> anyhow::Result<()> {
    let rows: Vec<_> = audit_rows(f, "merge.revert")
        .await?
        .into_iter()
        .filter(|row| row.1 == "merge" && row.2 == Some(merge_id))
        .collect();
    anyhow::ensure!(rows.len() == 1, "one merge.revert audit row: {rows:?}");
    let (actor, _, _, detail) = &rows[0];
    anyhow::ensure!(*actor == Some(f.editor), "the owner is the actor: {rows:?}");
    anyhow::ensure!(
        detail["token_id"] == json!(token_id)
            && detail["token_name"] == token_name
            && detail["action_id"] == json!(action_id)
            && detail["merge_id"] == json!(merge_id),
        "{detail}"
    );
    Ok(())
}

// ───────────────────────── SC5 / E13：令牌撤回自己的合并 ─────────────────────────

#[tokio::test]
async fn a_token_reverts_its_own_merge_over_the_api_and_over_mcp_and_a_second_revert_answers_already_decided(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let s = seeds(&f).await?;
        // 主人正好是 Editor：写的下限
        let (token_id, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;

        // 两次合并：一次经 REST 撤，一次经 MCP 撤
        for over_mcp in [false, true] {
            let name = if over_mcp { "Borealis" } else { "Acme" };
            let (merge_id, source, target) = token_merge(&f, &s, &token, name).await?;
            let rows_before = ledger(&f).await?.len();

            let body = if over_mcp {
                let result = mcp_revert(
                    &f,
                    &token,
                    json!({ "merge_id": merge_id, "rationale": "not the same" }),
                )
                .await?;
                structured(&result)?
            } else {
                let (status, body) = rest_revert(
                    &f,
                    &token,
                    merge_id,
                    Some(json!({ "rationale": "not the same" })),
                )
                .await?;
                anyhow::ensure!(status == StatusCode::OK, "revert: {status} {body}");
                body
            };
            let (action_id, named) = ensure_applied(&body)?;
            anyhow::ensure!(
                named == merge_id,
                "the answer names the merge it undid: {body}"
            );

            // 撤回了：合并带着 reverted_at，被并方回来了
            anyhow::ensure!(
                merge_row(&f, merge_id).await? == (source, target, true),
                "the merge is reverted"
            );
            anyhow::ensure!(merged_into(&f, source).await?.is_none());
            anyhow::ensure!(merged_into(&f, target).await?.is_none());

            // 台账：多了一行落地的 merge.revert，指着这次合并
            let rows = ledger(&f).await?;
            anyhow::ensure!(rows.len() == rows_before + 1, "{rows:?}");
            let row = ledger_row(&f, action_id).await?;
            ensure_revert_row(&f, &row, token_id, merge_id)?;
            anyhow::ensure!(row.token_name == TOKEN_NAME, "{row:?}");
            anyhow::ensure!(row.payload["why"] == "not the same", "{row:?}");
            // 审计：merge.revert，带令牌
            ensure_revert_audit(&f, (token_id, TOKEN_NAME), (action_id, merge_id)).await?;

            // 第二次撤回：两个入口都是 already_decided，什么都不写
            let (status, again) = rest_revert(&f, &token, merge_id, Some(json!({}))).await?;
            anyhow::ensure!(
                status == StatusCode::CONFLICT && again["code"] == "already_decided",
                "a second revert over the API: {status} {again}"
            );
            let again = mcp_revert(&f, &token, json!({ "merge_id": merge_id })).await?;
            ensure_refused(&again, "already_decided")?;
            anyhow::ensure!(ledger(&f).await?.len() == rows_before + 1);
            ensure_revert_audit(&f, (token_id, TOKEN_NAME), (action_id, merge_id)).await?;
            anyhow::ensure!(merge_row(&f, merge_id).await? == (source, target, true));
        }

        // 两次合并、两次撤回：四行都落地，没有一条提议
        let rows = ledger(&f).await?;
        anyhow::ensure!(
            rows.len() == 4 && rows.iter().all(|row| row.outcome == "applied"),
            "{rows:?}"
        );
        anyhow::ensure!(audit_rows(&f, "agent.proposal").await?.is_empty());
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── E12：合了再撤，回到原样 ─────────────────────────

/// E12 比的东西（INTENT C1）：`facts` 的每一行、`entities` 的每一行（不含 `updated_at`）、
/// `fact_evidence` 与 `typed_fact_sources` 的每一行。审核行、记下的签名违规不在里面
const STATE_PARTS: [&str; 4] = ["facts", "entities", "fact_evidence", "typed_fact_sources"];

async fn state(f: &Fx) -> anyhow::Result<Vec<Vec<String>>> {
    let mut out = Vec::new();
    for sql in [
        "SELECT to_jsonb(t)::text FROM facts t WHERE t.kb_id = $1 ORDER BY t.id",
        "SELECT (to_jsonb(e) - 'updated_at')::text FROM entities e
          WHERE e.kb_id = $1 ORDER BY e.id",
        "SELECT to_jsonb(fe)::text FROM fact_evidence fe JOIN facts t ON t.id = fe.fact_id
          WHERE t.kb_id = $1 ORDER BY fe.fact_id, fe.chunk_id",
        "SELECT to_jsonb(ts)::text FROM typed_fact_sources ts JOIN facts t ON t.id = ts.fact_id
          WHERE t.kb_id = $1 ORDER BY ts.fact_id, ts.statement_id",
    ] {
        let rows: Vec<String> = sqlx::query_scalar(sql)
            .bind(f.kb)
            .fetch_all(&f.pool)
            .await?;
        out.push(rows);
    }
    Ok(out)
}

/// E12 的比较（INTENT C1）：`facts`、`entities`（不含 `updated_at`）、`fact_evidence` 逐行
/// 相同；`typed_fact_sources` 合并之前的每一行都还在，多出来的**正好**是 `copied` 这一条——
/// 合并把被去重的那条类型化事实的陈述来源抄到了存活事实上，现有的撤回不删它。多出别的
/// 行、少了原有的行、或者这一条没出现（种子没走到去重），都算失败
fn ensure_restored(
    before: &[Vec<String>],
    after: &[Vec<String>],
    copied: (Uuid, Uuid),
    what: &str,
) -> anyhow::Result<()> {
    anyhow::ensure!(before.len() == 4 && after.len() == 4);
    for ((was, is), part) in before.iter().zip(after).zip(STATE_PARTS).take(3) {
        let gone: Vec<&String> = was.iter().filter(|l| !is.contains(l)).collect();
        let new: Vec<&String> = is.iter().filter(|l| !was.contains(l)).collect();
        anyhow::ensure!(
            was == is,
            "{what}: {part} differs\nonly before: {gone:#?}\nonly after: {new:#?}"
        );
    }
    let (was, is) = (&before[3], &after[3]);
    let missing: Vec<&String> = was.iter().filter(|l| !is.contains(l)).collect();
    anyhow::ensure!(
        missing.is_empty(),
        "{what}: typed_fact_sources lost rows it had before the merge: {missing:#?}"
    );
    let mut added = Vec::new();
    for line in is.iter().filter(|l| !was.contains(l)) {
        added.push(serde_json::from_str::<Value>(line)?);
    }
    anyhow::ensure!(
        added == [json!({ "fact_id": copied.0, "statement_id": copied.1 })],
        "{what}: the only typed_fact_sources row added is the statement link the merge copied \
         onto the surviving fact: {added:?}"
    );
    anyhow::ensure!(is.len() == was.len() + 1, "{what}: no row is duplicated");
    Ok(())
}

/// 一对有血有肉的重复项，两边都有类型（E12 比的是有类型的实体：没类型的存活方会留下
/// 它从被并方拿来的类型，那是现有撤回的行为，不改）：
///
/// - 两边各有名字事实，各带一条证据
/// - 两边各有一条「based in」的类型化事实，各有自己的证据与自己的陈述来源：合并时后记的
///   那条作废，它的证据与来源抄到存活的那条上
/// - 被并方是一条事实的主语、另一条事实的宾语：两条都搬到存活方
/// - 两边之间有一条互指的事实：合并后是自环，作废
/// - 两边的画像不同：合并时加权合起来
struct Rich {
    survivor: Uuid,
    absorbed: Uuid,
    review: Uuid,
    /// 合并会抄到存活事实上的那一条陈述来源：（先记的那条类型化事实，后记那条的陈述）。
    /// 现有的撤回不删它（INTENT C1），所以撤回之后它是 `typed_fact_sources` 里唯一多出来的行
    copied_link: (Uuid, Uuid),
}

async fn rich_pair(f: &Fx, s: &Seeds, name: &str) -> anyhow::Result<Rich> {
    let [survivor, absorbed, city, alice] = std::array::from_fn(|_| Uuid::now_v7());
    let [st_kept, st_dropped, kept, dropped, employs, works_at, cross] =
        std::array::from_fn(|_| Uuid::now_v7());
    let (kb, company, person, based_in, part_of) =
        (f.kb, f.company_type, f.person_type, s.based_in, s.part_of);
    let (c_kept, c_dropped) = (chunk(f).await?, chunk(f).await?);
    // 拼进去的只有夹具自己生成的 UUID 和测试里写死的名字
    sqlx::raw_sql(&format!(
        "INSERT INTO entities (id, kb_id, type_id, canonical_name, profile_embedding, profile_n) VALUES
              ('{survivor}', '{kb}', '{company}', '{name}', '[1,0,0]'::vector, 3),
              ('{absorbed}', '{kb}', '{company}', '{name} Corp', '[0,1,0]'::vector, 1),
              ('{city}', '{kb}', '{company}', '{name} City', NULL, 0),
              ('{alice}', '{kb}', '{person}', '{name} Alice', NULL, 0);
         INSERT INTO facts (id, kb_id, subject_id, object_id, layer, phrase, confidence) VALUES
              ('{st_kept}', '{kb}', '{survivor}', '{city}', 'open', 'is based in', 0.9),
              ('{st_dropped}', '{kb}', '{absorbed}', '{city}', 'open', 'is based in', 0.9);
         INSERT INTO facts (id, kb_id, subject_id, predicate_id, object_id, confidence,
                            from_statement_id, recorded_at) VALUES
              ('{kept}', '{kb}', '{survivor}', '{based_in}', '{city}', 0.9, '{st_kept}', '2024-01-01'),
              ('{dropped}', '{kb}', '{absorbed}', '{based_in}', '{city}', 0.9, '{st_dropped}', '2024-02-01'),
              ('{employs}', '{kb}', '{absorbed}', '{part_of}', '{alice}', 0.8, NULL, '2024-03-01'),
              ('{works_at}', '{kb}', '{alice}', '{part_of}', '{absorbed}', 0.8, NULL, '2024-03-02'),
              ('{cross}', '{kb}', '{survivor}', '{part_of}', '{absorbed}', 0.7, NULL, '2024-03-03');
         INSERT INTO typed_fact_sources (fact_id, statement_id) VALUES
              ('{kept}', '{st_kept}'), ('{dropped}', '{st_dropped}');"
    ))
    .execute(&f.pool)
    .await?;
    for (fact_id, chunk_id, quote) in [
        (kept, c_kept, "based in Port Ellen"),
        (dropped, c_dropped, "headquartered in Port Ellen"),
        (employs, c_dropped, "employs Alice"),
    ] {
        sqlx::query(
            "INSERT INTO fact_evidence (fact_id, chunk_id, quote, document_id)
             SELECT $1, $2, $3, document_id FROM chunks WHERE id = $2",
        )
        .bind(fact_id)
        .bind(chunk_id)
        .bind(quote)
        .execute(&f.pool)
        .await?;
    }
    for (id, called, chunk_id) in [
        (survivor, name.to_string(), c_kept),
        (absorbed, format!("{name} Corp"), c_dropped),
    ] {
        utopia_store::names::record(
            &f.pool,
            kb,
            id,
            &called,
            Some(NameSource {
                chunk_id,
                quote: &called,
            }),
            None,
        )
        .await?;
        // 消歧后缀在实体建好时就算过一遍，这里照做：之后的重算就不是第一次
        utopia_store::resolution::refresh_disambiguators(&f.pool, kb, &called).await?;
    }
    let review = f.pair_of(absorbed, survivor).await?;
    Ok(Rich {
        survivor,
        absorbed,
        review,
        copied_link: (kept, st_dropped),
    })
}

#[tokio::test]
async fn after_a_token_merge_and_its_revert_every_fact_entity_and_evidence_row_is_as_before(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        let s = seeds(&f).await?;
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;

        // 队列里的合并经 REST 撤，手动合并经 MCP 撤：两种合并、两个入口各走一遍
        for manual in [false, true] {
            let name = if manual { "Orion" } else { "Acme" };
            let rich = rich_pair(&f, &s, name).await?;
            let before = state(&f).await?;
            for (part, rows) in STATE_PARTS.iter().zip(&before) {
                anyhow::ensure!(
                    !rows.is_empty(),
                    "{part} must have rows for the comparison to mean anything"
                );
            }
            // 比的是有类型的实体：这个库里没有一个实体是没类型的
            let untyped: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM entities WHERE kb_id = $1 AND type_id IS NULL",
            )
            .bind(kb)
            .fetch_one(&f.pool)
            .await?;
            anyhow::ensure!(untyped == 0, "the comparison is made on typed entities");

            let merged = if manual {
                let result = tool(
                    &f,
                    &token,
                    "review_merge",
                    json!({ "source": rich.absorbed, "target": rich.survivor }),
                )
                .await?;
                structured(&result)?
            } else {
                let (status, body) = f
                    .call(
                        &token,
                        "POST",
                        &format!("/kbs/{kb}/review/{}", rich.review),
                        Some(json!({ "action": "merge", "rationale": "same thing" })),
                    )
                    .await?;
                anyhow::ensure!(status == StatusCode::OK, "merge: {status} {body}");
                body
            };
            let (_, merge_id) = ensure_applied(&merged)?;
            let (source, target, reverted) = merge_row(&f, merge_id).await?;
            anyhow::ensure!(!reverted && merged_into(&f, source).await? == Some(target));
            if manual {
                // 手动合并照调用方说的方向
                anyhow::ensure!((source, target) == (rich.absorbed, rich.survivor));
            }
            // 合并真的动了每一部分：否则下面的「回到原样」什么都证明不了
            let during = state(&f).await?;
            for (part, (was, is)) in STATE_PARTS.iter().zip(before.iter().zip(&during)) {
                anyhow::ensure!(
                    was != is,
                    "the merge did not change {part}: the seed is too thin"
                );
            }

            let reverted = if manual {
                let result = mcp_revert(&f, &token, json!({ "merge_id": merge_id })).await?;
                structured(&result)?
            } else {
                let (status, body) = rest_revert(&f, &token, merge_id, None).await?;
                anyhow::ensure!(status == StatusCode::OK, "revert: {status} {body}");
                body
            };
            let (_, named) = ensure_applied(&reverted)?;
            anyhow::ensure!(named == merge_id, "{reverted}");

            let after = state(&f).await?;
            ensure_restored(
                &before,
                &after,
                rich.copied_link,
                "after a token merge and its token revert",
            )?;

            // 第二次撤回什么都不动
            let (status, again) = rest_revert(&f, &token, merge_id, None).await?;
            anyhow::ensure!(
                status == StatusCode::CONFLICT && again["code"] == "already_decided",
                "{status} {again}"
            );
            ensure_restored(
                &before,
                &state(&f).await?,
                rich.copied_link,
                "after a second revert",
            )?;
        }
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── E14：哪一枚令牌的合并都撤得动；别处的合并没有 ─────────────────────────

#[tokio::test]
async fn a_token_reverts_any_token_agents_merge_of_the_base_and_an_unknown_or_foreign_merge_id_is_not_found(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let s = seeds(&f).await?;
        let (maker_id, maker) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        // 另一枚令牌：撤的不是它自己做的合并
        let (other_id, other) = f.token(f.editor, "second agent", "write", None).await?;
        anyhow::ensure!(maker_id != other_id);

        // 一枚令牌合的，另一枚来撤，两个入口各一次：直接撤，记在撤的那一枚名下
        for over_mcp in [false, true] {
            let name = if over_mcp { "Borealis" } else { "Acme" };
            let (merge_id, source, target) = token_merge(&f, &s, &maker, name).await?;
            let body = if over_mcp {
                structured(&mcp_revert(&f, &other, json!({ "merge_id": merge_id })).await?)?
            } else {
                // 不带请求体也行
                let (status, body) = rest_revert(&f, &other, merge_id, None).await?;
                anyhow::ensure!(status == StatusCode::OK, "revert: {status} {body}");
                body
            };
            let (action_id, named) = ensure_applied(&body)?;
            anyhow::ensure!(named == merge_id, "{body}");
            anyhow::ensure!(merge_row(&f, merge_id).await? == (source, target, true));
            anyhow::ensure!(merged_into(&f, source).await?.is_none());
            let row = ledger_row(&f, action_id).await?;
            ensure_revert_row(&f, &row, other_id, merge_id)?;
            anyhow::ensure!(row.token_name == "second agent", "{row:?}");
            ensure_revert_audit(&f, (other_id, "second agent"), (action_id, merge_id)).await?;
        }

        // 别的库里令牌 agent 做的一次合并：在这个库的路由上它不存在
        let foreign_source = Uuid::now_v7();
        let foreign_target = Uuid::now_v7();
        for (id, name) in [
            (foreign_source, "Elsewhere"),
            (foreign_target, "Elsewhere Corp"),
        ] {
            sqlx::query("INSERT INTO entities (id, kb_id, canonical_name) VALUES ($1, $2, $3)")
                .bind(id)
                .bind(f.other_kb)
                .bind(name)
                .execute(&f.pool)
                .await?;
        }
        let (status, body) =
            rest_manual(&f, f.other_kb, &maker, foreign_source, foreign_target).await?;
        anyhow::ensure!(
            status == StatusCode::OK,
            "a merge on the other base: {status} {body}"
        );
        let (_, foreign) = ensure_applied(&body)?;
        // 还没撤回、令牌 agent 做的、可以撤的一次合并，留给下面的「没有」对照
        let (live, live_source, live_target) = token_merge(&f, &s, &maker, "Cygnus").await?;

        let before = graph(&f).await?;
        let rows_before = ledger(&f).await?.len();
        let other_rows_before = ledger_of(&f, f.other_kb).await?.len();
        for (what, merge_id) in [
            ("an unknown id", Uuid::now_v7()),
            ("another base's id", foreign),
        ] {
            let (status, body) = rest_revert(&f, &other, merge_id, None).await?;
            anyhow::ensure!(
                status == StatusCode::NOT_FOUND,
                "{what} over the API: {status} {body}"
            );
            let result = mcp_revert(&f, &other, json!({ "merge_id": merge_id })).await?;
            ensure_refused(&result, "not_found")?;
        }
        // 什么都没动，什么都没记：这个库的图与台账、另一个库的那次合并与台账
        ensure_same(&before, &graph(&f).await?, "a revert of a merge not found")?;
        anyhow::ensure!(ledger(&f).await?.len() == rows_before);
        anyhow::ensure!(ledger_of(&f, f.other_kb).await?.len() == other_rows_before);
        anyhow::ensure!(
            merge_row(&f, foreign).await? == (foreign_source, foreign_target, false),
            "the other base's merge stands"
        );
        anyhow::ensure!(merge_row(&f, live).await? == (live_source, live_target, false));
        anyhow::ensure!(audit_rows(&f, "agent.proposal").await?.is_empty());

        // 会话编辑者撤回照旧：同一条路由，从前的回答，审计行里没有令牌；台账一行不多。
        // 之后令牌再撤这一次，是 already_decided——只撤得动还没撤回的
        let (status, body) = rest_revert(&f, &f.editor_session, live, None).await?;
        anyhow::ensure!(
            status == StatusCode::OK && body == json!({ "ok": true }),
            "a person's revert answers as before: {status} {body}"
        );
        anyhow::ensure!(merge_row(&f, live).await? == (live_source, live_target, true));
        let by_person: Vec<_> = audit_rows(&f, "merge.revert")
            .await?
            .into_iter()
            .filter(|row| row.2 == Some(live))
            .collect();
        anyhow::ensure!(
            by_person.len() == 1
                && by_person[0].0 == Some(f.editor)
                && keys(&by_person[0].3) == ["source", "target"],
            "{by_person:?}"
        );
        anyhow::ensure!(ledger(&f).await?.len() == rows_before);
        let (status, body) = rest_revert(&f, &other, live, None).await?;
        anyhow::ensure!(
            status == StatusCode::CONFLICT && body["code"] == "already_decided",
            "a token revert of a merge already reverted: {status} {body}"
        );
        anyhow::ensure!(ledger(&f).await?.len() == rows_before);
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── 人或治理 agent 的合并：只留提议 ─────────────────────────

#[tokio::test]
async fn a_token_revert_of_a_persons_or_the_governors_merge_lands_a_proposal_and_reverts_nothing(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        let s = seeds(&f).await?;
        let (token_id, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;

        // 人做的合并：会话编辑者在队列里点的
        let (pa, pc) = passing(&f, &s, "Acme").await?;
        let review = f.pair_of(pa, pc).await?;
        let (status, body) = f
            .call(
                &f.editor_session,
                "POST",
                &format!("/kbs/{kb}/review/{review}"),
                Some(json!({ "action": "merge" })),
            )
            .await?;
        anyhow::ensure!(
            status == StatusCode::OK,
            "a person's merge: {status} {body}"
        );
        let by_person: Uuid =
            sqlx::query_scalar("SELECT id FROM entity_merges WHERE kb_id = $1 AND merged_by = $2")
                .bind(kb)
                .bind(f.editor)
                .fetch_one(&f.pool)
                .await?;
        // 治理 agent 做的合并：没有 `merged_by`（它的落地步骤就是这样调的，不走模型任务）
        let (ga, gc) = passing(&f, &s, "Borealis").await?;
        let by_governor =
            utopia_store::resolution::merge_entities(&f.pool, kb, ga, gc, None, "governor").await?;

        let before = graph(&f).await?;
        let mut proposals = Vec::new();
        // 人的那一次先经 REST、再经 MCP 重复；治理 agent 的那一次反过来
        for (merge_id, reason, rest_first, who) in [
            (by_person, "merge_by_person", true, "person"),
            (by_governor, "merge_by_governor", false, "governor"),
        ] {
            let over_rest = |repeat: bool| {
                let (f, token) = (&f, &token);
                async move {
                    let (status, body) = rest_revert(
                        f,
                        token,
                        merge_id,
                        Some(json!({ "rationale": "looks wrong" })),
                    )
                    .await?;
                    anyhow::ensure!(
                        status == StatusCode::OK,
                        "a propose-only revert is not an error (repeat {repeat}): {status} {body}"
                    );
                    Ok::<_, anyhow::Error>(body)
                }
            };
            let over_mcp = || {
                let (f, token) = (&f, &token);
                async move {
                    structured(
                        &mcp_revert(
                            f,
                            token,
                            json!({ "merge_id": merge_id, "rationale": "looks wrong" }),
                        )
                        .await?,
                    )
                }
            };
            let first = if rest_first {
                over_rest(false).await?
            } else {
                over_mcp().await?
            };
            let proposal = ensure_proposed(&first, reason, false)?;
            anyhow::ensure!(
                first["explanation"]
                    .as_str()
                    .is_some_and(|text| text.contains(who)),
                "the explanation says whose merge it is: {first}"
            );
            ensure_same(&before, &graph(&f).await?, "a propose-only revert")?;

            // 再来一次，换一个入口：同一条提议，already_proposed，没有第二行
            let second = if rest_first {
                over_mcp().await?
            } else {
                over_rest(true).await?
            };
            anyhow::ensure!(
                ensure_proposed(&second, reason, true)? == proposal,
                "a repeat returns the open proposal: {second}"
            );
            ensure_same(&before, &graph(&f).await?, "a repeated propose-only revert")?;

            let row = ledger_row(&f, proposal).await?;
            anyhow::ensure!(
                row.outcome == "proposed" && row.gate_reason.as_deref() == Some(reason),
                "{row:?}"
            );
            anyhow::ensure!(
                row.operation == "merge.revert"
                    && row.target_kind == "merge"
                    && row.target_id == merge_id
                    && row.merge_id == Some(merge_id),
                "{row:?}"
            );
            anyhow::ensure!(row.state.is_none() && row.state_at.is_none(), "{row:?}");
            anyhow::ensure!(
                row.owner_id == f.editor && row.token_id == token_id,
                "{row:?}"
            );
            anyhow::ensure!(
                row.payload["merge_id"] == json!(merge_id) && row.payload["why"] == "looks wrong",
                "the payload holds the request: {row:?}"
            );
            proposals.push(proposal);
        }

        // 两条提议，各一行台账、各一条 agent.proposal 审计行；没有一次撤回
        let rows = ledger(&f).await?;
        anyhow::ensure!(
            rows.len() == 2 && rows.iter().all(|row| row.outcome == "proposed"),
            "{rows:?}"
        );
        let audits = audit_rows(&f, "agent.proposal").await?;
        anyhow::ensure!(audits.len() == 2, "{audits:?}");
        for ((actor, target_kind, target_id, detail), proposal) in audits.iter().zip(&proposals) {
            anyhow::ensure!(
                *actor == Some(f.editor)
                    && target_kind == "agent_action"
                    && *target_id == Some(*proposal),
                "{audits:?}"
            );
            anyhow::ensure!(
                detail["operation"] == "merge.revert"
                    && detail["outcome"] == "proposed"
                    && detail["token_id"] == json!(token_id)
                    && detail["token_name"] == TOKEN_NAME,
                "{detail}"
            );
        }
        anyhow::ensure!(audit_rows(&f, "merge.revert").await?.is_empty());
        for merge_id in [by_person, by_governor] {
            let (source, target, reverted) = merge_row(&f, merge_id).await?;
            anyhow::ensure!(!reverted, "nothing was reverted");
            anyhow::ensure!(merged_into(&f, source).await? == Some(target));
        }
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── MCP：谁看得到 `review_revert`，它说了什么 ─────────────────────────

#[tokio::test]
async fn review_revert_is_listed_only_to_a_write_token_of_an_editor_and_says_what_a_revert_does_not_recall(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let s = seeds(&f).await?;
        let (_, editor_write) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let (_, editor_read) = f.token(f.editor, "editor reader", "read", None).await?;
        let (_, viewer_read) = f.token(f.viewer, "viewer reader", "read", None).await?;
        let (_, viewer_write) = f.token(f.viewer, "viewer writer", "write", None).await?;

        let mut described = None;
        for (who, token, listed) in [
            ("write token of an editor", &editor_write, true),
            ("read token of an editor", &editor_read, false),
            ("read token of a viewer", &viewer_read, false),
            ("write token of a viewer", &viewer_write, false),
        ] {
            let (status, body) = rpc(&f, token, "tools/list", json!({})).await?;
            anyhow::ensure!(status == StatusCode::OK, "{who}: {status} {body}");
            let tools = body["result"]["tools"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let revert = tools.iter().find(|t| t["name"] == "review_revert").cloned();
            anyhow::ensure!(
                revert.is_some() == listed,
                "{who}: review_revert listed = {}",
                revert.is_some()
            );
            // 读工具对谁都列：没列出撤回不是因为整个清单空了
            anyhow::ensure!(
                tools.iter().any(|t| t["name"] == "review_list"),
                "{who}: {tools:?}"
            );
            if listed {
                described = revert;
            }
        }
        let revert = described.ok_or_else(|| anyhow::anyhow!("review_revert is not listed"))?;
        anyhow::ensure!(
            revert["annotations"]["readOnlyHint"] == false,
            "a write tool is not read-only: {revert}"
        );
        // 一个 uuid 的 `merge_id`，一句可省的理由
        let schema = &revert["inputSchema"];
        anyhow::ensure!(
            schema["required"] == json!(["merge_id"])
                && keys(&schema["properties"]) == ["merge_id", "rationale"]
                && schema["properties"]["merge_id"]["format"] == "uuid",
            "{schema}"
        );
        // 说明不把撤回说成完全复原（N6）：不收回什么、没类型的存活方、抄过去的陈述来源
        let description = revert["description"].as_str().unwrap_or_default();
        for said in [
            "restores graph state only",
            "does not recall derivations, violations, exports or answers drawn while the merge \
             was live",
            "a target that had no type before the merge keeps the type it took",
            "a surviving fact keeps the statement links the merge copied onto it",
            "already_decided",
            "not_found",
        ] {
            anyhow::ensure!(
                description.contains(said),
                "review_revert does not say \"{said}\": {description}"
            );
        }
        for word in ["full undo", "fully", "completely"] {
            anyhow::ensure!(!description.contains(word), "{word}: {description}");
        }

        // 没被列出来的调用方照样调得到这个名字：回来的是带 REST code 的工具错误，什么都没撤
        let (merge_id, source, target) = token_merge(&f, &s, &editor_write, "Acme").await?;
        let rows_before = ledger(&f).await?.len();
        for (who, token, code) in [
            ("read token of an editor", &editor_read, "token_read_only"),
            ("read token of a viewer", &viewer_read, "token_read_only"),
            (
                "write token of a viewer",
                &viewer_write,
                "token_owner_below_editor",
            ),
        ] {
            let result = mcp_revert(&f, token, json!({ "merge_id": merge_id })).await?;
            ensure_refused(&result, code)?;
            // REST 上是同一个 code
            let (status, body) = rest_revert(&f, token, merge_id, None).await?;
            anyhow::ensure!(
                status == StatusCode::FORBIDDEN && body["code"] == code,
                "{who} over the API: {status} {body}"
            );
            anyhow::ensure!(
                result["structuredContent"]["message"] == body["error"],
                "{who}: the tool error carries the REST message\n{result}\n{body}"
            );
        }
        anyhow::ensure!(merge_row(&f, merge_id).await? == (source, target, false));
        anyhow::ensure!(merged_into(&f, source).await? == Some(target));
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before,
            "a refusal writes no row"
        );
        anyhow::ensure!(audit_rows(&f, "merge.revert").await?.is_empty());

        // 写得了的那一枚调同一个工具：撤了
        let done =
            structured(&mcp_revert(&f, &editor_write, json!({ "merge_id": merge_id })).await?)?;
        let (_, named) = ensure_applied(&done)?;
        anyhow::ensure!(named == merge_id, "{done}");
        anyhow::ensure!(merge_row(&f, merge_id).await? == (source, target, true));
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}
