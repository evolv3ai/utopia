//! 编辑者的写令牌合并一对重复项或两个实体（0076）：令牌 API 的两条路由与 MCP 工具
//! `review_merge`，都先过服务端的闸门。
//!
//! 要钉住的行为：
//!
//! - **闸门放行就落地**（SC1）：实体合了，回答里有 `merge_id`，台账一行 `applied` 指着
//!   这次合并，审计行带 `token_id` 与 `token_name`
//! - **闸门留下就什么都不改**（SC2 / E7）：200、`held`、闸门的理由，一条开着的提议；
//!   事实、实体、证据、审核行前后相等；再来一次是 `already_proposed`
//! - **已经定了的一对**（E1）：谁定的都一样，`already_decided`，没有第二次合并
//! - **人、治理 agent、令牌同时裁一对**（E2）：正好合一次，输了的令牌听到的是
//!   `already_decided`；治理轮次正锁着的一对，合并听到的就是分开听到的那个 409
//! - **手动合并的三种不行**（E3）：自己并自己、已经并走的、认不出的 id
//! - **方向**（E4）：队列里的合并与人点合并时同一个方向；手动合并照调用方给的，
//!   并且把这一对自己那条等着的审核行关成 merged、记上令牌
//! - **六种留下的理由**（E5）与**一个目标一条开着的提议**（E6）
//! - **`review_merge` 只列给编辑者的写令牌**，拒绝带 REST 的 code
//! - **令牌的合并不排治理轮次；会话的合并照旧不问闸门**
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

/// 队列里的合并，经 REST
async fn rest_merge(f: &Fx, bearer: &str, review: Uuid) -> anyhow::Result<(StatusCode, Value)> {
    f.call(
        bearer,
        "POST",
        &format!("/kbs/{}/review/{review}", f.kb),
        Some(json!({ "action": "merge", "rationale": "same thing" })),
    )
    .await
}

/// 手动合并，经 REST
async fn rest_manual(
    f: &Fx,
    bearer: &str,
    source: Uuid,
    target: Uuid,
) -> anyhow::Result<(StatusCode, Value)> {
    f.call(
        bearer,
        "POST",
        &format!("/kbs/{}/entities/merge", f.kb),
        Some(json!({ "source": source, "target": target })),
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

/// `review_merge` 的一次调用，回工具结果。传输层或 JSON-RPC 层出错在这里就是测试失败——
/// 工具自己的拒绝是 `isError: true` 的正常结果
async fn mcp_merge(f: &Fx, bearer: &str, args: Value) -> anyhow::Result<Value> {
    let (status, body) = rpc(
        f,
        bearer,
        "tools/call",
        json!({ "name": "review_merge", "arguments": args }),
    )
    .await?;
    anyhow::ensure!(
        status == StatusCode::OK,
        "review_merge {args}: {status} {body}"
    );
    anyhow::ensure!(body.get("error").is_none(), "review_merge {args}: {body}");
    anyhow::ensure!(body["result"].is_object(), "review_merge {args}: {body}");
    Ok(body["result"].clone())
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

/// 留成提议的回答：`held`、理由、一句说明；`repeat` 时多一个 `code: already_proposed`。
/// 回提议（台账行）的 id
fn ensure_held(body: &Value, reason: &str, repeat: bool) -> anyhow::Result<Uuid> {
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
        "a held answer (repeat {repeat}): {body}"
    );
    anyhow::ensure!(body["ok"] == true && body["outcome"] == "held", "{body}");
    anyhow::ensure!(body["reason"] == reason, "expected {reason}: {body}");
    anyhow::ensure!(
        body["explanation"].as_str().is_some_and(|s| !s.is_empty()),
        "{body}"
    );
    if repeat {
        anyhow::ensure!(body["code"] == "already_proposed", "{body}");
    }
    uuid_at(body, "action_id")
}

// ───────────────────────── 读库 ─────────────────────────

/// 「图一行不动」比的东西（E7）：这个库的实体、事实、证据、类型化事实的来源、审核行、
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

/// 这个库的台账，最早的在前
async fn ledger(f: &Fx) -> anyhow::Result<Vec<TokenAgentAction>> {
    let mut rows = utopia_store::token_agent_actions::list(&f.pool, f.kb, 500, 0).await?;
    rows.reverse();
    Ok(rows)
}

async fn ledger_row(f: &Fx, id: Uuid) -> anyhow::Result<TokenAgentAction> {
    ledger(f)
        .await?
        .into_iter()
        .find(|row| row.id == id)
        .ok_or_else(|| anyhow::anyhow!("no ledger row {id}"))
}

/// 开着的提议：id、操作、目标的钥匙（`TokenAgentAction` 不带 `target_key`，直接读表）
async fn open_proposals(f: &Fx) -> anyhow::Result<Vec<(Uuid, String, Option<String>)>> {
    Ok(sqlx::query_as(
        "SELECT id, operation, target_key FROM token_agent_actions
          WHERE kb_id = $1 AND outcome IN ('held', 'proposed') AND state IS NULL
          ORDER BY created_at, id",
    )
    .bind(f.kb)
    .fetch_all(&f.pool)
    .await?)
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

/// 这两个实体之间的合并（哪个方向都算）：id、源、目标、谁合的
async fn merges_between(
    f: &Fx,
    a: Uuid,
    c: Uuid,
) -> anyhow::Result<Vec<(Uuid, Uuid, Uuid, Option<Uuid>)>> {
    Ok(sqlx::query_as(
        "SELECT id, source_id, target_id, merged_by FROM entity_merges
          WHERE kb_id = $1
            AND ((source_id = $2 AND target_id = $3) OR (source_id = $3 AND target_id = $2))
          ORDER BY created_at, id",
    )
    .bind(f.kb)
    .bind(a)
    .bind(c)
    .fetch_all(&f.pool)
    .await?)
}

async fn merge_count(f: &Fx) -> anyhow::Result<i64> {
    Ok(
        sqlx::query_scalar("SELECT count(*) FROM entity_merges WHERE kb_id = $1")
            .bind(f.kb)
            .fetch_one(&f.pool)
            .await?,
    )
}

async fn govern_jobs(f: &Fx) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "SELECT count(*) FROM jobs WHERE kind = 'govern' AND payload->>'kb_id' = $1",
    )
    .bind(f.kb.to_string())
    .fetch_one(&f.pool)
    .await?)
}

// ───────────────────────── 种子 ─────────────────────────

async fn relation(f: &Fx, key: &str, label: &str, functional: bool) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO relation_types (id, kb_id, key, label, functional)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(id)
    .bind(f.kb)
    .bind(key)
    .bind(label)
    .bind(functional)
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

/// 一份文档里的一个分块。`config` 给了就是一个 statements 来源下的文档
async fn chunk(f: &Fx, config: Option<Value>) -> anyhow::Result<Uuid> {
    let (source, document, chunk) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    if let Some(config) = &config {
        sqlx::query(
            "INSERT INTO sources (id, kb_id, kind, name, config)
             VALUES ($1, $2, 'statements', 's', $3)",
        )
        .bind(source)
        .bind(f.kb)
        .bind(config)
        .execute(&f.pool)
        .await?;
    }
    sqlx::query(
        "INSERT INTO documents (id, kb_id, filename, sha256, source_id)
         VALUES ($1, $2, 'd', $1::text, $3)",
    )
    .bind(document)
    .bind(f.kb)
    .bind(config.map(|_| source))
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

/// 给一条事实挂一条证据
async fn evidence(f: &Fx, fact_id: Uuid, chunk_id: Uuid, quote: &str) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO fact_evidence (fact_id, chunk_id, quote, document_id)
         SELECT $1, $2, $3, document_id FROM chunks WHERE id = $2",
    )
    .bind(fact_id)
    .bind(chunk_id)
    .bind(quote)
    .execute(&f.pool)
    .await?;
    Ok(())
}

/// 这个库里几样关系与一个共用的宾语，够种出闸门的各种留下
struct Seeds {
    /// 只许一个值的关系：两边各指一个不同的人就是矛盾
    ceo_of: Uuid,
    part_of: Uuid,
    based_in: Uuid,
    city: Uuid,
}

async fn seeds(f: &Fx) -> anyhow::Result<Seeds> {
    Ok(Seeds {
        ceo_of: relation(f, "ceo_of", "CEO of", true).await?,
        part_of: relation(f, "part_of", "part of", false).await?,
        based_in: relation(f, "based_in", "based in", false).await?,
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

/// 让闸门留下 `entity` 的任何合并：它身上有一条还成立的派生事实（`derived`）
async fn derived_on(f: &Fx, s: &Seeds, entity: Uuid) -> anyhow::Result<()> {
    let rule: Option<Uuid> = sqlx::query_scalar("SELECT id FROM rules WHERE kb_id = $1 LIMIT 1")
        .bind(f.kb)
        .fetch_optional(&f.pool)
        .await?;
    let rule = match rule {
        Some(rule) => rule,
        None => {
            let rule = Uuid::now_v7();
            sqlx::query(
                "INSERT INTO rules (id, kb_id, predicate_id, kind)
                 VALUES ($1, $2, $3, 'transitive')",
            )
            .bind(rule)
            .bind(f.kb)
            .bind(s.part_of)
            .execute(&f.pool)
            .await?;
            rule
        }
    };
    sqlx::query(
        "INSERT INTO derived_facts (id, kb_id, subject_id, predicate_id, object_id, rule_id)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(Uuid::now_v7())
    .bind(f.kb)
    .bind(entity)
    .bind(s.part_of)
    .bind(s.city)
    .bind(rule)
    .execute(&f.pool)
    .await?;
    Ok(())
}

/// 闸门会留下的一对（`derived`），两边各有事实：回 (左, 右)
async fn held(f: &Fx, s: &Seeds, name: &str) -> anyhow::Result<(Uuid, Uuid)> {
    let (a, c) = passing(f, s, name).await?;
    derived_on(f, s, a).await?;
    Ok((a, c))
}

/// 一行落地的台账行该有的样子
fn ensure_applied_row(
    f: &Fx,
    row: &TokenAgentAction,
    token_id: Uuid,
    (operation, target_kind, target_id): (&str, &str, Uuid),
    merge_id: Uuid,
) -> anyhow::Result<()> {
    anyhow::ensure!(row.outcome == "applied", "{row:?}");
    anyhow::ensure!(
        row.operation == operation && row.target_kind == target_kind && row.target_id == target_id,
        "{row:?}"
    );
    anyhow::ensure!(row.merge_id == Some(merge_id), "{row:?}");
    anyhow::ensure!(
        row.owner_id == f.editor && row.token_id == token_id,
        "{row:?}"
    );
    anyhow::ensure!(row.token_name == TOKEN_NAME, "{row:?}");
    anyhow::ensure!(row.gate_reason.is_none() && row.state.is_none(), "{row:?}");
    Ok(())
}

/// 令牌写入的审计行：这个动作、这个目标上正好一行，actor 是主人，detail 带令牌、
/// 台账行与合并
async fn ensure_token_audit(
    f: &Fx,
    token_id: Uuid,
    (action, target_kind, target_id): (&str, &str, Uuid),
    (action_id, merge_id): (Uuid, Uuid),
) -> anyhow::Result<()> {
    let rows: Vec<_> = audit_rows(f, action)
        .await?
        .into_iter()
        .filter(|row| row.1 == target_kind && row.2 == Some(target_id))
        .collect();
    anyhow::ensure!(rows.len() == 1, "one {action} audit row: {rows:?}");
    let (actor, _, _, detail) = &rows[0];
    anyhow::ensure!(*actor == Some(f.editor), "the owner is the actor: {rows:?}");
    anyhow::ensure!(
        detail["token_id"] == json!(token_id)
            && detail["token_name"] == TOKEN_NAME
            && detail["action_id"] == json!(action_id)
            && detail["merge_id"] == json!(merge_id),
        "{detail}"
    );
    Ok(())
}

// ───────────────────────── SC1：闸门放行就落地 ─────────────────────────

#[tokio::test]
async fn a_write_token_of_an_editor_merges_a_passed_pair_over_the_api_and_over_mcp(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let s = seeds(&f).await?;
        // 主人正好是 Editor：写的下限
        let (token_id, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;

        // 队列里的两对：一对经 REST，一对经 MCP
        for over_mcp in [false, true] {
            let (a, c) = passing(&f, &s, if over_mcp { "Borealis" } else { "Acme" }).await?;
            let review = f.pair_of(a, c).await?;
            let body = if over_mcp {
                let result = mcp_merge(
                    &f,
                    &token,
                    json!({ "id": review, "rationale": "same thing" }),
                )
                .await?;
                structured(&result)?
            } else {
                let (status, body) = rest_merge(&f, &token, review).await?;
                anyhow::ensure!(status == StatusCode::OK, "merge: {status} {body}");
                body
            };
            let (action_id, merge_id) = ensure_applied(&body)?;

            // 实体合了：正好一次合并，回答里的 `merge_id` 就是它，被并方指着存活方
            let merges = merges_between(&f, a, c).await?;
            anyhow::ensure!(merges.len() == 1 && merges[0].0 == merge_id, "{merges:?}");
            let (_, source, target, merged_by) = merges[0];
            anyhow::ensure!(merged_by == Some(f.editor), "the owner merged: {merges:?}");
            anyhow::ensure!(merged_into(&f, source).await? == Some(target));
            anyhow::ensure!(merged_into(&f, target).await?.is_none());
            anyhow::ensure!(
                f.review_row(review).await?
                    == ("merged".to_string(), Some(f.editor), Some(token_id)),
                "decided by the owner, through the token"
            );

            let row = ledger_row(&f, action_id).await?;
            ensure_applied_row(
                &f,
                &row,
                token_id,
                ("review.merge", "review", review),
                merge_id,
            )?;
            anyhow::ensure!(row.payload["why"] == "same thing", "{row:?}");
            ensure_token_audit(
                &f,
                token_id,
                ("review.merge", "review", review),
                (action_id, merge_id),
            )
            .await?;
        }

        // 手动合并：源并进目标，一次经 REST，一次经 MCP
        for over_mcp in [false, true] {
            let (source, target) =
                passing(&f, &s, if over_mcp { "Draco" } else { "Cygnus" }).await?;
            let body = if over_mcp {
                let result =
                    mcp_merge(&f, &token, json!({ "source": source, "target": target })).await?;
                structured(&result)?
            } else {
                let (status, body) = rest_manual(&f, &token, source, target).await?;
                anyhow::ensure!(status == StatusCode::OK, "manual merge: {status} {body}");
                body
            };
            let (action_id, merge_id) = ensure_applied(&body)?;
            let merges = merges_between(&f, source, target).await?;
            anyhow::ensure!(
                merges == [(merge_id, source, target, Some(f.editor))],
                "{merges:?}"
            );
            anyhow::ensure!(merged_into(&f, source).await? == Some(target));
            let row = ledger_row(&f, action_id).await?;
            ensure_applied_row(
                &f,
                &row,
                token_id,
                ("merge.manual", "entity", target),
                merge_id,
            )?;
            ensure_token_audit(
                &f,
                token_id,
                ("merge.manual", "entity", target),
                (action_id, merge_id),
            )
            .await?;
        }

        let rows = ledger(&f).await?;
        anyhow::ensure!(
            rows.len() == 4 && rows.iter().all(|row| row.outcome == "applied"),
            "four calls, four applied ledger rows: {rows:?}"
        );
        anyhow::ensure!(merge_count(&f).await? == 4);
        anyhow::ensure!(open_proposals(&f).await?.is_empty());
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── SC2 / E7：闸门留下就什么都不改 ─────────────────────────

#[tokio::test]
async fn a_held_merge_changes_no_fact_entity_evidence_or_review_row_and_lands_one_proposal(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        let s = seeds(&f).await?;
        let (a, c) = held(&f, &s, "Acme").await?;
        // 两边的事实各带一条证据，一条类型化事实带着它的陈述来源：合并真落地的话都会动
        let (ca, cc) = (chunk(&f, None).await?, chunk(&f, None).await?);
        let fa = fact(&f, a, s.part_of, s.city).await?;
        let fc = fact(&f, c, s.based_in, s.city).await?;
        evidence(&f, fa, ca, "part of Port Ellen").await?;
        evidence(&f, fc, cc, "based in Port Ellen").await?;
        let statement = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO facts (id, kb_id, subject_id, object_id, layer, phrase)
             VALUES ($1, $2, $3, $4, 'open', 'is based in')",
        )
        .bind(statement)
        .bind(kb)
        .bind(c)
        .bind(s.city)
        .execute(&f.pool)
        .await?;
        sqlx::query("INSERT INTO typed_fact_sources (fact_id, statement_id) VALUES ($1, $2)")
            .bind(fc)
            .bind(statement)
            .execute(&f.pool)
            .await?;
        let review = f.pair_of(a, c).await?;
        let (other_a, other_c) = held(&f, &s, "Borealis").await?;
        let (token_id, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;

        let before = graph(&f).await?;
        for part in [
            "entities",
            "facts",
            "fact_evidence",
            "typed_fact_sources",
            "resolution_reviews",
        ] {
            anyhow::ensure!(
                before[part].as_array().is_some_and(|rows| !rows.is_empty()),
                "{part} must have rows for the comparison to mean anything"
            );
        }

        // 同一个调用，这一对被闸门留下：200、held、闸门的理由
        let (status, body) = rest_merge(&f, &token, review).await?;
        anyhow::ensure!(
            status == StatusCode::OK,
            "a hold is not an error: {status} {body}"
        );
        let proposal = ensure_held(&body, "derived", false)?;
        anyhow::ensure!(
            body["explanation"].as_str().is_some_and(
                |text| text.contains("derived fact") && text.contains("Nothing was changed")
            ),
            "{body}"
        );
        ensure_same(&before, &graph(&f).await?, "a held queue merge")?;
        anyhow::ensure!(f.review_row(review).await? == ("pending".to_string(), None, None));

        // 一条开着的提议：台账行 held、理由、整个请求；一条 agent.proposal 审计行
        let open = open_proposals(&f).await?;
        anyhow::ensure!(
            open == [(
                proposal,
                "review.merge".to_string(),
                Some(review.to_string())
            )],
            "{open:?}"
        );
        let row = ledger_row(&f, proposal).await?;
        anyhow::ensure!(
            row.outcome == "held" && row.gate_reason.as_deref() == Some("derived"),
            "{row:?}"
        );
        anyhow::ensure!(
            row.merge_id.is_none() && row.state.is_none() && row.state_at.is_none(),
            "{row:?}"
        );
        anyhow::ensure!(
            row.owner_id == f.editor && row.token_id == token_id,
            "{row:?}"
        );
        anyhow::ensure!(row.token_name == TOKEN_NAME, "{row:?}");
        anyhow::ensure!(
            row.payload["review_id"] == json!(review) && row.payload["why"] == "same thing",
            "the payload holds the request: {row:?}"
        );
        let audits = audit_rows(&f, "agent.proposal").await?;
        anyhow::ensure!(audits.len() == 1, "{audits:?}");
        anyhow::ensure!(
            audits[0].0 == Some(f.editor)
                && audits[0].1 == "agent_action"
                && audits[0].2 == Some(proposal),
            "{audits:?}"
        );
        anyhow::ensure!(
            audits[0].3
                == json!({ "operation": "review.merge", "outcome": "held", "reason": "derived",
                           "token_id": token_id, "token_name": TOKEN_NAME,
                           "action_id": proposal }),
            "{}",
            audits[0].3
        );
        anyhow::ensure!(audit_rows(&f, "review.merge").await?.is_empty());

        // 再来一次：同一条提议，already_proposed，什么都没多写。REST 与 MCP 回的是同一个
        let (status, again) = rest_merge(&f, &token, review).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {again}");
        anyhow::ensure!(ensure_held(&again, "derived", true)? == proposal, "{again}");
        let over_mcp = structured(&mcp_merge(&f, &token, json!({ "id": review })).await?)?;
        anyhow::ensure!(over_mcp == again, "MCP {over_mcp}\nREST {again}");
        anyhow::ensure!(ledger(&f).await?.len() == 1, "a repeat writes no row");
        anyhow::ensure!(audit_rows(&f, "agent.proposal").await?.len() == 1);
        ensure_same(&before, &graph(&f).await?, "a repeated held merge")?;

        // 手动合并同一类被留下的一对，经 MCP：另一条提议，图仍一行不动
        let result = mcp_merge(
            &f,
            &token,
            json!({ "source": other_a, "target": other_c, "rationale": "same thing" }),
        )
        .await?;
        let manual = ensure_held(&structured(&result)?, "derived", false)?;
        anyhow::ensure!(manual != proposal);
        ensure_same(&before, &graph(&f).await?, "a held manual merge")?;
        anyhow::ensure!(merge_count(&f).await? == 0, "no held call merged anything");
        anyhow::ensure!(merged_into(&f, a).await?.is_none() && merged_into(&f, c).await?.is_none());

        // 这一对还在队列里等人；提议在 Agent 一档里，带着理由，没有后来的状态
        let (status, queue) = f
            .call(
                &token,
                "GET",
                &format!("/kbs/{kb}/review?queue=duplicates"),
                None,
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "{status}");
        anyhow::ensure!(
            queue["items"]
                .as_array()
                .is_some_and(|items| items.iter().any(|i| i["id"] == json!(review))),
            "{queue}"
        );
        let (status, agent) = f
            .call(
                &token,
                "GET",
                &format!("/kbs/{kb}/review?queue=agent"),
                None,
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "{status}");
        let listed = agent["items"]
            .as_array()
            .and_then(|items| items.iter().find(|i| i["id"] == json!(proposal)))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("the proposal is not in Agent History: {agent}"))?;
        anyhow::ensure!(
            listed["outcome"] == "held"
                && listed["reason"] == "derived"
                && listed["token_name"] == TOKEN_NAME
                && listed.get("state").is_none()
                && listed.get("merge_id").is_none(),
            "{listed}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── E1：已经定了的一对 ─────────────────────────

#[tokio::test]
async fn a_token_merge_of_a_pair_that_is_already_decided_answers_already_decided_and_writes_no_second_row(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        let s = seeds(&f).await?;
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let (_, other_token) = f.token(f.editor, "second agent", "write", None).await?;

        // 五种「已经定了」：人分开的、人合的、令牌分开的、令牌合的、治理 agent 关掉的
        let mut decided: Vec<(&str, Uuid, Uuid, Uuid)> = Vec::new();
        for (who, name) in [
            ("a person kept it", "Acme"),
            ("a person merged it", "Borealis"),
            ("a token kept it", "Cygnus"),
            ("a token merged it", "Draco"),
            ("the governor closed it", "Eridanus"),
        ] {
            let (a, c) = passing(&f, &s, name).await?;
            let review = f.pair_of(a, c).await?;
            let path = format!("/kbs/{kb}/review/{review}");
            let (bearer, action) = match who {
                "a person kept it" => (Some(&f.editor_session), "keep"),
                "a person merged it" => (Some(&f.editor_session), "merge"),
                "a token kept it" => (Some(&other_token), "keep"),
                "a token merged it" => (Some(&other_token), "merge"),
                _ => (None, "keep"),
            };
            match bearer {
                Some(bearer) => {
                    let (status, body) = f
                        .call(bearer, "POST", &path, Some(json!({ "action": action })))
                        .await?;
                    anyhow::ensure!(status == StatusCode::OK, "{who}: {status} {body}");
                }
                // 治理 agent 落地一次分开，就是这一句（`governance::apply`）
                None => {
                    let closed = utopia_store::resolution::close_review_auto(
                        &f.pool,
                        review,
                        "kept",
                        "governed|0.99",
                    )
                    .await?;
                    anyhow::ensure!(closed == 1, "{who}");
                }
            }
            decided.push((who, review, a, c));
        }

        let before = graph(&f).await?;
        let rows_before = ledger(&f).await?;
        let applied_before = rows_before
            .iter()
            .filter(|r| r.outcome == "applied")
            .count();
        anyhow::ensure!(
            applied_before == 2,
            "the two token decisions above: {rows_before:?}"
        );
        let merges_before = merge_count(&f).await?;
        anyhow::ensure!(
            merges_before == 2,
            "one merge by the person, one by the token"
        );

        for (who, review, a, c) in &decided {
            let (status, body) = rest_merge(&f, &token, *review).await?;
            anyhow::ensure!(status == StatusCode::CONFLICT, "{who}: {status} {body}");
            anyhow::ensure!(body["code"] == "already_decided", "{who}: {body}");
            let result = mcp_merge(&f, &token, json!({ "id": review })).await?;
            ensure_refused(&result, "already_decided")?;
            // 这一对的两个实体已经合了的话，手动再合也是 already_decided
            if merged_into(&f, *a).await?.is_some() || merged_into(&f, *c).await?.is_some() {
                for (source, target) in [(a, c), (c, a)] {
                    let (status, body) = rest_manual(&f, &token, *source, *target).await?;
                    anyhow::ensure!(status == StatusCode::CONFLICT, "{who}: {status} {body}");
                    anyhow::ensure!(body["code"] == "already_decided", "{who}: {body}");
                }
            }
            ensure_same(&before, &graph(&f).await?, who)?;
        }
        anyhow::ensure!(merge_count(&f).await? == merges_before, "no second merge");
        let rows = ledger(&f).await?;
        anyhow::ensure!(
            rows.len() == rows_before.len(),
            "a refused merge writes no ledger row: {rows:?}"
        );
        anyhow::ensure!(
            open_proposals(&f).await?.is_empty(),
            "and lands no proposal"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── E2：人、治理 agent、令牌同时裁一对 ─────────────────────────

/// 治理 agent 落地一次合并的那几步，照 `crate::governance::apply` 的 `Gate::Apply` 分支：
/// 走到活着的两边，两边已经是同一个就只关审核行；否则按 `merge_direction` 合（`merged_by`
/// 为空），合成了再关审核行；一侧已经被别人并走就转回给人。`Ok(true)` = 它合了
async fn governor_merges(
    f: &Fx,
    review: Uuid,
    left: Uuid,
    right: Uuid,
) -> utopia_core::AppResult<bool> {
    use utopia_store::resolution;
    let reason = "governed|0.99";
    let (l, r) = (
        resolution::survivor(&f.pool, f.kb, left).await?,
        resolution::survivor(&f.pool, f.kb, right).await?,
    );
    if l == r {
        resolution::close_review_auto(&f.pool, review, "merged", reason).await?;
        return Ok(false);
    }
    let (target, source) = resolution::merge_direction(&f.pool, l, r).await?;
    match resolution::merge_entities(&f.pool, f.kb, source, target, None, reason).await {
        Ok(_) => {
            resolution::close_review_auto(&f.pool, review, "merged", reason).await?;
            Ok(true)
        }
        Err(utopia_core::AppError::Conflict(_)) | Err(utopia_core::AppError::NotFound) => {
            resolution::escalate_review(&f.pool, review, "escalate_entity_changed").await?;
            Ok(false)
        }
        Err(e) => Err(e),
    }
}

/// 人与治理 agent 各比令牌晚几毫秒出发。三方同时出发的话总是治理 agent 先到（它那几步
/// 不过 HTTP、不验身份），其次是人；令牌的路最长——验令牌、授权、问闸门。错开之后三方
/// 才真的撞在合并的事务上：这张表从「一起出发」排到「令牌先走一大步」，中间哪几轮撞上
/// 随机器而定，所以下面每一轮只断言不变的东西，不断言谁赢
const STAGGER: [(u64, u64); 16] = [
    (0, 0),
    (0, 4),
    (0, 8),
    (0, 12),
    (10, 14),
    (20, 24),
    (25, 30),
    (30, 34),
    (35, 40),
    (40, 44),
    (45, 50),
    (50, 54),
    (60, 65),
    (70, 75),
    (80, 85),
    (100, 105),
];

async fn after<T>(ms: u64, call: impl std::future::Future<Output = T>) -> T {
    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
    call.await
}

#[tokio::test]
async fn of_a_person_the_governor_and_a_token_deciding_one_pair_exactly_one_is_applied_and_the_losing_token_gets_already_decided(
) -> anyhow::Result<()> {
    // 治理开着的库：后半段要一个跑着的治理轮次锁住一对
    let Some(f) = Fx::new(true).await? else {
        return Ok(());
    };
    let result = async {
        let s = seeds(&f).await?;
        let (token_id, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let mut token_wins = 0;
        for (round, (person_ms, governor_ms)) in STAGGER.into_iter().enumerate() {
            let (a, c) = passing(&f, &s, &format!("Acme {round}")).await?;
            let review = f.pair_of(a, c).await?;
            // 三方同时裁同一对：会话的合并、治理 agent 的合并、令牌的合并
            let (person, governor, agent) = tokio::join!(
                after(person_ms, rest_merge(&f, &f.editor_session, review)),
                after(governor_ms, governor_merges(&f, review, a, c)),
                rest_merge(&f, &token, review)
            );
            // 人与治理 agent 走的是没改过的路：它们各自听到什么不归这里断言，只记进失败信息
            let (person_status, _) = person?;
            let (status, body) = agent?;

            // 正好合一次
            let merges = merges_between(&f, a, c).await?;
            anyhow::ensure!(
                merges.len() == 1,
                "round {round}: exactly one merge exists, got {merges:?} \
                 (person {person_status}, governor {governor:?}, token {status} {body})"
            );
            anyhow::ensure!(f.review_row(review).await?.0 == "merged", "round {round}");
            anyhow::ensure!(
                merged_into(&f, a).await?.is_some() != merged_into(&f, c).await?.is_some(),
                "round {round}: one side was merged into the other"
            );

            // 令牌要么落地了那一次合并，要么输了、听到 already_decided——不是 404，
            // 也不是不带 code 的 409
            if status == StatusCode::OK {
                let (action_id, merge_id) = ensure_applied(&body)?;
                anyhow::ensure!(merges[0].0 == merge_id, "round {round}: {merges:?} {body}");
                let row = ledger_row(&f, action_id).await?;
                ensure_applied_row(
                    &f,
                    &row,
                    token_id,
                    ("review.merge", "review", review),
                    merge_id,
                )?;
                anyhow::ensure!(
                    f.review_row(review).await?.2 == Some(token_id),
                    "round {round}: the token's decision keeps its token"
                );
                token_wins += 1;
            } else {
                anyhow::ensure!(
                    status == StatusCode::CONFLICT && body["code"] == "already_decided",
                    "round {round}: a losing token call gets already_decided, got {status} {body}"
                );
                anyhow::ensure!(
                    f.review_row(review).await?.2.is_none(),
                    "round {round}: a losing token leaves no mark on the pair"
                );
            }
            let rows = ledger(&f).await?;
            anyhow::ensure!(
                rows.len() == token_wins && rows.iter().all(|row| row.outcome == "applied"),
                "round {round}: one applied row per token win and nothing else: {rows:?}"
            );
        }
        // 每一行落地的台账行都有它的审计行，一行不多
        let audited = audit_rows(&f, "review.merge")
            .await?
            .into_iter()
            .filter(|row| row.3.get("token_id").is_some())
            .count();
        anyhow::ensure!(
            audited == token_wins,
            "{audited} audit rows, {token_wins} wins"
        );

        // 治理轮次正锁着的一对：合并听到的就是分开今天听到的那个 409——不带 code
        let (a, c) = passing(&f, &s, "Borealis").await?;
        let review = f.pair_of(a, c).await?;
        sqlx::query("DELETE FROM jobs WHERE kind = 'govern' AND payload->>'kb_id' = $1")
            .bind(f.kb.to_string())
            .execute(&f.pool)
            .await?;
        sqlx::query("INSERT INTO jobs (kind, payload, status) VALUES ('govern', $1, 'running')")
            .bind(json!({ "kb_id": f.kb }))
            .execute(&f.pool)
            .await?;
        utopia_store::governance::lock(&f.pool, f.kb, &[review]).await?;
        anyhow::ensure!(utopia_store::governance::locked_by_agent(&f.pool, f.kb, review).await?);
        let before = graph(&f).await?;
        let rows_before = ledger(&f).await?.len();

        let (keep_status, keep_body) = f
            .call(
                &token,
                "POST",
                &format!("/kbs/{}/review/{review}", f.kb),
                Some(json!({ "action": "keep" })),
            )
            .await?;
        anyhow::ensure!(
            keep_status == StatusCode::CONFLICT && keys(&keep_body) == ["error"],
            "the keep's refusal for a locked pair: {keep_status} {keep_body}"
        );
        for (what, answer) in [
            ("queue merge", rest_merge(&f, &token, review).await?),
            ("manual merge", rest_manual(&f, &token, a, c).await?),
        ] {
            anyhow::ensure!(
                answer == (keep_status, keep_body.clone()),
                "{what} of a locked pair must answer as the keep does: {answer:?}"
            );
        }
        for args in [json!({ "id": review }), json!({ "source": a, "target": c })] {
            let result = mcp_merge(&f, &token, args).await?;
            anyhow::ensure!(
                result["isError"] == true
                    && result["structuredContent"] == json!({ "message": keep_body["error"] }),
                "{result}"
            );
        }
        ensure_same(&before, &graph(&f).await?, "a locked pair")?;
        anyhow::ensure!(ledger(&f).await?.len() == rows_before);
        anyhow::ensure!(
            open_proposals(&f).await?.is_empty(),
            "a locked pair lands no proposal"
        );

        // 轮次一停，令牌照常能合：挡住它的只是那把锁
        sqlx::query("DELETE FROM jobs WHERE kind = 'govern' AND payload->>'kb_id' = $1")
            .bind(f.kb.to_string())
            .execute(&f.pool)
            .await?;
        let (status, body) = rest_merge(&f, &token, review).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        ensure_applied(&body)?;
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── E3：手动合并的三种不行 ─────────────────────────

#[tokio::test]
async fn a_token_manual_merge_of_an_entity_with_itself_a_merged_one_or_an_unknown_one_is_refused_and_lands_nothing(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let s = seeds(&f).await?;
        let (alive, other) = passing(&f, &s, "Acme").await?;
        // 一个已经并进别人的实体：人手动合的
        let (gone, into) = passing(&f, &s, "Borealis").await?;
        let (status, body) = rest_manual(&f, &f.editor_session, gone, into).await?;
        anyhow::ensure!(status == StatusCode::OK, "seeding a merge: {status} {body}");
        // 别的库里的一个实体，和一个哪儿都没有的 id
        let elsewhere = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO entities (id, kb_id, canonical_name) VALUES ($1, $2, 'Elsewhere')",
        )
        .bind(elsewhere)
        .bind(f.other_kb)
        .execute(&f.pool)
        .await?;
        let nowhere = Uuid::now_v7();
        // 这枚令牌两个库都够得着：别的库的实体仍然不是这个库的
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;

        let before = graph(&f).await?;
        // (说明, 源, 目标, HTTP 状态, REST 的 code, MCP 的 code)
        let cases = [
            (
                "itself",
                alive,
                alive,
                StatusCode::UNPROCESSABLE_ENTITY,
                Some("self_merge"),
                "self_merge",
            ),
            (
                "a merged source",
                gone,
                alive,
                StatusCode::CONFLICT,
                Some("already_decided"),
                "already_decided",
            ),
            (
                "a merged target",
                alive,
                gone,
                StatusCode::CONFLICT,
                Some("already_decided"),
                "already_decided",
            ),
            (
                "its own survivor again",
                gone,
                into,
                StatusCode::CONFLICT,
                Some("already_decided"),
                "already_decided",
            ),
            (
                "an unknown source",
                nowhere,
                alive,
                StatusCode::NOT_FOUND,
                None,
                "not_found",
            ),
            (
                "an unknown target",
                alive,
                nowhere,
                StatusCode::NOT_FOUND,
                None,
                "not_found",
            ),
            (
                "a source of another base",
                elsewhere,
                alive,
                StatusCode::NOT_FOUND,
                None,
                "not_found",
            ),
            (
                "a target of another base",
                other,
                elsewhere,
                StatusCode::NOT_FOUND,
                None,
                "not_found",
            ),
        ];
        for (what, source, target, expected, rest_code, mcp_code) in cases {
            let (status, body) = rest_manual(&f, &token, source, target).await?;
            anyhow::ensure!(status == expected, "{what}: {status} {body}");
            anyhow::ensure!(
                body.get("code").and_then(Value::as_str) == rest_code,
                "{what}: {body}"
            );
            let result =
                mcp_merge(&f, &token, json!({ "source": source, "target": target })).await?;
            ensure_refused(&result, mcp_code)?;
            ensure_same(&before, &graph(&f).await?, what)?;
        }
        // 自己并自己：就是会话今天听到的那个拒绝，一个字不差
        let (status, mine) = rest_manual(&f, &token, alive, alive).await?;
        let (session_status, theirs) = rest_manual(&f, &f.editor_session, alive, alive).await?;
        anyhow::ensure!(
            status == session_status && mine == theirs,
            "the existing self_merge refusal: {mine} vs {theirs}"
        );

        anyhow::ensure!(
            ledger(&f).await?.is_empty(),
            "no refusal writes a ledger row"
        );
        anyhow::ensure!(
            open_proposals(&f).await?.is_empty(),
            "none lands a proposal"
        );
        anyhow::ensure!(audit_rows(&f, "agent.proposal").await?.is_empty());
        anyhow::ensure!(merge_count(&f).await? == 1, "only the seeded merge exists");
        ensure_same(&before, &graph(&f).await?, "after every refusal")?;
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── E4：方向 ─────────────────────────

#[tokio::test]
async fn a_token_queue_merge_takes_the_persons_direction_and_a_manual_merge_takes_the_callers_and_closes_the_pairs_review(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let s = seeds(&f).await?;
        let (token_id, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        // 一边有两条事实、一边没有的一对；`heavy_left` 说事实多的是不是左边。回 (审核行, 左, 右)
        let lopsided = |name: &'static str, heavy_left: bool| {
            let (f, s) = (&f, &s);
            async move {
                let left = f.entity(f.company_type, name).await?;
                let right = f.entity(f.company_type, &format!("{name} Corp")).await?;
                let heavy = if heavy_left { left } else { right };
                fact(f, heavy, s.based_in, s.city).await?;
                fact(f, heavy, s.part_of, s.city).await?;
                anyhow::Ok((f.pair_of(left, right).await?, left, right))
            }
        };

        // 队列里的合并：两对一样的，一对人合、一对令牌合，活下来的是同一边
        for heavy_left in [false, true] {
            let (by_person, pl, pr) = lopsided("Acme", heavy_left).await?;
            let (by_token, tl, tr) = lopsided("Borealis", heavy_left).await?;
            let (status, body) = rest_merge(&f, &f.editor_session, by_person).await?;
            anyhow::ensure!(status == StatusCode::OK, "session merge: {status} {body}");
            let (status, body) = rest_merge(&f, &token, by_token).await?;
            anyhow::ensure!(status == StatusCode::OK, "token merge: {status} {body}");
            ensure_applied(&body)?;
            let person_kept_left = merged_into(&f, pr).await? == Some(pl);
            let token_kept_left = merged_into(&f, tr).await? == Some(tl);
            anyhow::ensure!(
                person_kept_left == heavy_left
                    && merged_into(&f, pl).await?.is_some() != heavy_left,
                "heavy_left {heavy_left}: the person's merge keeps the side with more facts"
            );
            anyhow::ensure!(
                token_kept_left == person_kept_left
                    && merged_into(&f, tl).await?.is_some() == merged_into(&f, pl).await?.is_some(),
                "heavy_left {heavy_left}: the token's queue merge takes the person's direction"
            );
        }

        // 手动合并：照调用方给的方向——哪怕队列会反过来合（事实多的一边会活下来）
        let (review, target, source) = lopsided("Cygnus", false).await?;
        let queue_would =
            utopia_store::resolution::merge_direction(&f.pool, target, source).await?;
        anyhow::ensure!(
            queue_would == (source, target),
            "the queue would keep the side with more facts: {queue_would:?}"
        );
        anyhow::ensure!(f.review_row(review).await? == ("pending".to_string(), None, None));
        let (status, body) = rest_manual(&f, &token, source, target).await?;
        anyhow::ensure!(status == StatusCode::OK, "manual merge: {status} {body}");
        let (action_id, merge_id) = ensure_applied(&body)?;
        anyhow::ensure!(
            merges_between(&f, source, target).await?
                == [(merge_id, source, target, Some(f.editor))],
            "the caller's source was merged into the caller's target"
        );
        anyhow::ensure!(merged_into(&f, source).await? == Some(target));
        anyhow::ensure!(merged_into(&f, target).await?.is_none());
        // 这一对自己那条等着的审核行关成了 merged，记着令牌
        anyhow::ensure!(
            f.review_row(review).await? == ("merged".to_string(), Some(f.editor), Some(token_id)),
            "the pair's own pending review is closed as merged with the token recorded"
        );
        let row = ledger_row(&f, action_id).await?;
        ensure_applied_row(
            &f,
            &row,
            token_id,
            ("merge.manual", "entity", target),
            merge_id,
        )?;
        anyhow::ensure!(row.payload["review_id"] == json!(review), "{row:?}");
        let audits: Vec<_> = audit_rows(&f, "merge.manual")
            .await?
            .into_iter()
            .filter(|a| a.3.get("token_id").is_some())
            .collect();
        anyhow::ensure!(
            audits.len() == 1 && audits[0].3["review_id"] == json!(review),
            "{audits:?}"
        );
        // 关掉的审核行不在队列里了
        let (_, queue) = f
            .call(
                &token,
                "GET",
                &format!("/kbs/{}/review?queue=duplicates", f.kb),
                None,
            )
            .await?;
        anyhow::ensure!(queue["counts"]["duplicates"] == 0, "{queue}");

        // 经 MCP 也一样：方向是调用方的，审核行（这一次左是源、右是目标，与上面相反）一并关上
        let (review, source, target) = lopsided("Draco", true).await?;
        let result = mcp_merge(&f, &token, json!({ "source": source, "target": target })).await?;
        let (_, merge_id) = ensure_applied(&structured(&result)?)?;
        anyhow::ensure!(
            merges_between(&f, source, target).await?
                == [(merge_id, source, target, Some(f.editor))]
        );
        anyhow::ensure!(
            f.review_row(review).await? == ("merged".to_string(), Some(f.editor), Some(token_id))
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── E5：六种留下的理由 ─────────────────────────

#[tokio::test]
async fn each_of_the_six_hold_reasons_lands_a_proposal_that_names_it_for_the_queue_merge_and_the_manual_merge(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        let s = seeds(&f).await?;
        let alice = f.entity(f.person_type, "Alice").await?;
        let bob = f.entity(f.person_type, "Bob").await?;
        let (token_id, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let company = |name: &'static str| f.entity(f.company_type, name);
        let mut holds: Vec<(&str, Uuid, Uuid)> = Vec::new();

        // 身份来源：a 的名字是一个把名字当身份标识的来源说的
        let (a, c) = (company("acme/a.ts").await?, company("acme/a").await?);
        let identity = chunk(&f, Some(json!({ "names": "identity" }))).await?;
        utopia_store::names::record(
            &f.pool,
            kb,
            a,
            "acme/a.ts",
            Some(NameSource {
                chunk_id: identity,
                quote: "acme/a.ts",
            }),
            None,
        )
        .await?;
        holds.push(("identity", a, c));

        // 矛盾：两边各有一个不同的 CEO
        let (a, c) = (company("Borealis").await?, company("Borealis Corp").await?);
        fact(&f, a, s.ceo_of, alice).await?;
        fact(&f, c, s.ceo_of, bob).await?;
        holds.push(("contradiction", a, c));

        // 派生：一条靠着 a 推出来的、还成立的派生事实
        let (a, c) = (company("Cygnus").await?, company("Cygnus Ltd").await?);
        derived_on(&f, &s, a).await?;
        holds.push(("derived", a, c));

        // 回答：c 在一轮回答里被认过
        let (a, c) = (company("Draco").await?, company("Draco Group").await?);
        let conversation = Uuid::now_v7();
        sqlx::query("INSERT INTO conversations (id, kb_id, user_id) VALUES ($1, $2, $3)")
            .bind(conversation)
            .bind(kb)
            .bind(f.editor)
            .execute(&f.pool)
            .await?;
        sqlx::query(
            "INSERT INTO conversation_messages (id, conversation_id, role, content, resolved)
             VALUES ($1, $2, 'assistant', 'hi', $3)",
        )
        .bind(Uuid::now_v7())
        .bind(conversation)
        .bind(json!([{ "id": c.to_string(), "name": "x", "type": null }]))
        .execute(&f.pool)
        .await?;
        holds.push(("answered", a, c));

        // 人分开过的一对：人在 Review 里裁的是 (c, a)，令牌问的是 (a, c)——按实体 id、不分左右
        let (a, c) = (company("Eridanus").await?, company("Eridanus Inc.").await?);
        let earlier = f.pair_of(c, a).await?;
        let (status, body) = f
            .call(
                &f.editor_session,
                "POST",
                &format!("/kbs/{kb}/review/{earlier}"),
                Some(json!({ "action": "keep", "rationale": "two companies" })),
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "a person's keep: {status} {body}");
        holds.push(("kept_by_person", a, c));

        // 合过又撤回的一对：人把 c 并进 a 又撤回，令牌问的是 (a, c)
        let (a, c) = (company("Fornax").await?, company("Fornax Labs").await?);
        let (status, body) = rest_manual(&f, &f.editor_session, c, a).await?;
        anyhow::ensure!(
            status == StatusCode::OK,
            "a person's merge: {status} {body}"
        );
        let undone = uuid_at(&body, "merge_id")?;
        let (status, body) = f
            .call(
                &f.editor_session,
                "POST",
                &format!("/kbs/{kb}/merges/{undone}/revert"),
                Some(json!({})),
            )
            .await?;
        anyhow::ensure!(
            status == StatusCode::OK,
            "a person's revert: {status} {body}"
        );
        holds.push(("reverted_merge", a, c));

        let merges_seeded = merge_count(&f).await?;
        let mut expected_rows = 0;
        for (code, a, c) in holds {
            // 队列合并：一条提议，理由点名，图一行不动
            let review = f.pair_of(a, c).await?;
            let before = graph(&f).await?;
            let (status, body) = rest_merge(&f, &token, review).await?;
            anyhow::ensure!(status == StatusCode::OK, "{code}: {status} {body}");
            let queue_proposal = ensure_held(&body, code, false)?;
            let row = ledger_row(&f, queue_proposal).await?;
            anyhow::ensure!(
                row.outcome == "held"
                    && row.gate_reason.as_deref() == Some(code)
                    && row.operation == "review.merge"
                    && row.target_kind == "review"
                    && row.target_id == review
                    && row.token_id == token_id,
                "{code}: {row:?}"
            );
            ensure_same(&before, &graph(&f).await?, code)?;
            anyhow::ensure!(f.review_row(review).await?.0 == "pending", "{code}");

            // 手动合并，经 MCP：同一种理由的另一条提议，图仍一行不动
            let result = mcp_merge(&f, &token, json!({ "source": a, "target": c })).await?;
            let manual_proposal = ensure_held(&structured(&result)?, code, false)?;
            let row = ledger_row(&f, manual_proposal).await?;
            anyhow::ensure!(
                row.outcome == "held"
                    && row.gate_reason.as_deref() == Some(code)
                    && row.operation == "merge.manual"
                    && row.target_kind == "entity"
                    && row.target_id == c,
                "{code}: {row:?}"
            );
            // 方向反过来给，经 REST：还是被留下，同一个理由，而且就是那同一条提议
            let (status, body) = rest_manual(&f, &token, c, a).await?;
            anyhow::ensure!(status == StatusCode::OK, "{code} swapped: {status} {body}");
            anyhow::ensure!(
                ensure_held(&body, code, true)? == manual_proposal,
                "{code}: either order of the two entities is one target"
            );
            ensure_same(&before, &graph(&f).await?, code)?;

            expected_rows += 2;
            anyhow::ensure!(ledger(&f).await?.len() == expected_rows, "{code}");
            // 两条提议的理由也在 agent.proposal 审计行里
            let reasons: Vec<Value> = audit_rows(&f, "agent.proposal")
                .await?
                .into_iter()
                .filter(|row| row.2 == Some(queue_proposal) || row.2 == Some(manual_proposal))
                .map(|row| row.3["reason"].clone())
                .collect();
            anyhow::ensure!(reasons == [json!(code), json!(code)], "{code}: {reasons:?}");
        }
        anyhow::ensure!(
            open_proposals(&f).await?.len() == 12,
            "six holds, two entries each"
        );
        anyhow::ensure!(
            merge_count(&f).await? == merges_seeded,
            "no held call merged anything"
        );
        anyhow::ensure!(
            ledger(&f).await?.iter().all(|row| row.outcome == "held"),
            "nothing was applied"
        );

        // 对照：令牌分开过的一对不算「人分开的」，闸门不留，合并落地
        let (a, c) = (company("Gemini").await?, company("Gemini Co").await?);
        let by_token = f.pair_of(c, a).await?;
        let (status, body) = f
            .call(
                &token,
                "POST",
                &format!("/kbs/{kb}/review/{by_token}"),
                Some(json!({ "action": "keep" })),
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "a token's keep: {status} {body}");
        let review = f.pair_of(a, c).await?;
        let (status, body) = rest_merge(&f, &token, review).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        ensure_applied(&body)?;
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── E6：一个目标一条开着的提议 ─────────────────────────

#[tokio::test]
async fn a_repeated_or_simultaneous_held_merge_returns_the_one_open_proposal_with_already_proposed(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let s = seeds(&f).await?;
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        // 另一枚令牌问同一个目标，撞上的也是那一条：一个库、一种操作、一个目标
        let (_, second) = f.token(f.editor, "second agent", "write", None).await?;
        let (a, c) = held(&f, &s, "Acme").await?;
        let review = f.pair_of(a, c).await?;
        let (b1, b2) = held(&f, &s, "Borealis").await?;
        let racing = f.pair_of(b1, b2).await?;
        let before = graph(&f).await?;
        let key = |x: Uuid, y: Uuid| format!("{}|{}", x.min(y), x.max(y));

        // 一个接一个：队列合并的目标是审核行
        let (status, body) = rest_merge(&f, &token, review).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let queue_proposal = ensure_held(&body, "derived", false)?;
        for bearer in [&token, &second] {
            let (status, body) = rest_merge(&f, bearer, review).await?;
            anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
            anyhow::ensure!(
                ensure_held(&body, "derived", true)? == queue_proposal,
                "{body}"
            );
        }
        let again = structured(&mcp_merge(&f, &token, json!({ "id": review })).await?)?;
        anyhow::ensure!(
            ensure_held(&again, "derived", true)? == queue_proposal,
            "{again}"
        );

        // 手动合并的目标是这两个实体，不分谁并进谁：是另一条提议（操作不同），
        // 反过来给还是它
        let (status, body) = rest_manual(&f, &token, a, c).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let manual_proposal = ensure_held(&body, "derived", false)?;
        anyhow::ensure!(manual_proposal != queue_proposal);
        for (source, target) in [(a, c), (c, a)] {
            let (status, body) = rest_manual(&f, &second, source, target).await?;
            anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
            anyhow::ensure!(
                ensure_held(&body, "derived", true)? == manual_proposal,
                "{body}"
            );
            let again = structured(
                &mcp_merge(&f, &token, json!({ "source": source, "target": target })).await?,
            )?;
            anyhow::ensure!(
                ensure_held(&again, "derived", true)? == manual_proposal,
                "{again}"
            );
        }

        // 同一刻到的两个调用：一条提议，两边拿到同一个 id，正好一边是 already_proposed
        let (one, two) = tokio::join!(
            rest_merge(&f, &token, racing),
            mcp_merge(&f, &second, json!({ "id": racing }))
        );
        let ((status, one), two) = (one?, structured(&two?)?);
        anyhow::ensure!(status == StatusCode::OK, "{status} {one}");
        let racing_queue = uuid_at(&one, "action_id")?;
        anyhow::ensure!(uuid_at(&two, "action_id")? == racing_queue, "{one}\n{two}");
        let repeats = [&one, &two]
            .iter()
            .filter(|body| body["code"] == "already_proposed")
            .count();
        anyhow::ensure!(
            repeats == 1,
            "exactly one of the two met the proposal: {one}\n{two}"
        );
        for body in [&one, &two] {
            ensure_held(body, "derived", body.get("code").is_some())?;
        }
        // 手动合并同一刻到，方向相反
        let (one, two) = tokio::join!(
            rest_manual(&f, &token, b1, b2),
            rest_manual(&f, &second, b2, b1)
        );
        let ((status_one, one), (status_two, two)) = (one?, two?);
        anyhow::ensure!(
            status_one == StatusCode::OK && status_two == StatusCode::OK,
            "{one} {two}"
        );
        let racing_manual = uuid_at(&one, "action_id")?;
        anyhow::ensure!(uuid_at(&two, "action_id")? == racing_manual, "{one}\n{two}");
        let repeats = [&one, &two]
            .iter()
            .filter(|body| body["code"] == "already_proposed")
            .count();
        anyhow::ensure!(repeats == 1, "{one}\n{two}");

        // 四个目标，四条开着的提议，四条审计行；第二枚令牌一行也没写
        let open = open_proposals(&f).await?;
        let mut expected = vec![
            (
                queue_proposal,
                "review.merge".to_string(),
                Some(review.to_string()),
            ),
            (manual_proposal, "merge.manual".to_string(), Some(key(a, c))),
            (
                racing_queue,
                "review.merge".to_string(),
                Some(racing.to_string()),
            ),
            (racing_manual, "merge.manual".to_string(), Some(key(b1, b2))),
        ];
        let mut listed = open.clone();
        expected.sort();
        listed.sort();
        anyhow::ensure!(listed == expected, "{open:?}");
        anyhow::ensure!(ledger(&f).await?.len() == 4, "no second row for any target");
        anyhow::ensure!(audit_rows(&f, "agent.proposal").await?.len() == 4);
        ensure_same(
            &before,
            &graph(&f).await?,
            "repeated and simultaneous held merges",
        )?;
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── MCP：谁看得见、被拒绝时拿到什么 ─────────────────────────

#[tokio::test]
async fn review_merge_is_listed_only_to_a_write_token_of_an_editor_and_a_refusal_carries_the_rest_code(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let s = seeds(&f).await?;
        let (a, c) = passing(&f, &s, "Acme").await?;
        let review = f.pair_of(a, c).await?;
        let (_, editor_read) = f.token(f.editor, "reader", "read", None).await?;
        let (_, viewer_write) = f.token(f.viewer, "viewer writer", "write", None).await?;
        // 只读、主人又是 Viewer：scope 先于角色
        let (_, viewer_read) = f.token(f.viewer, "viewer reader", "read", None).await?;
        let (_, editor_write) = f.token(f.editor, TOKEN_NAME, "write", None).await?;

        for (who, token, writes) in [
            ("read token of an editor", &editor_read, false),
            ("write token of a viewer", &viewer_write, false),
            ("read token of a viewer", &viewer_read, false),
            ("write token of an editor", &editor_write, true),
        ] {
            let (status, body) = rpc(&f, token, "tools/list", json!({})).await?;
            anyhow::ensure!(status == StatusCode::OK, "{who}: {status} {body}");
            let tools = body["result"]["tools"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let merge = tools.iter().find(|t| t["name"] == "review_merge");
            anyhow::ensure!(
                merge.is_some() == writes,
                "{who}: review_merge listed = {}",
                merge.is_some()
            );
            anyhow::ensure!(
                tools.iter().any(|t| t["name"] == "review_list"),
                "{who}: the read tools stay listed"
            );
            if let Some(merge) = merge {
                anyhow::ensure!(merge["annotations"]["readOnlyHint"] == false, "{merge}");
                anyhow::ensure!(
                    keys(&merge["inputSchema"]["properties"])
                        == ["id", "rationale", "source", "target"],
                    "{merge}"
                );
                let text = merge["description"].as_str().unwrap_or_default();
                for word in [
                    "gate",
                    "proposal",
                    "already_proposed",
                    "already_decided",
                    "merge_id",
                ] {
                    anyhow::ensure!(
                        text.contains(word),
                        "review_merge does not say {word}: {text}"
                    );
                }
            }
        }

        // 没列出来的调用方照样调得到：拿回的是带 code 的工具错误，code 就是 REST 的那个
        let before = graph(&f).await?;
        for (token, code) in [
            (&editor_read, "token_read_only"),
            (&viewer_write, "token_owner_below_editor"),
            (&viewer_read, "token_read_only"),
        ] {
            for args in [json!({ "id": review }), json!({ "source": a, "target": c })] {
                let result = mcp_merge(&f, token, args).await?;
                ensure_refused(&result, code)?;
            }
            for (status, body) in [
                rest_merge(&f, token, review).await?,
                rest_manual(&f, token, a, c).await?,
            ] {
                anyhow::ensure!(
                    status == StatusCode::FORBIDDEN && body["code"] == code,
                    "{code}: {status} {body}"
                );
            }
        }
        // 给法说不清的调用不执行：两种都给、只给一半、都没给、不是 uuid。纯文字的工具错误，
        // 不带 code——code 留给操作本身的拒绝
        for args in [
            json!({}),
            json!({ "id": review, "source": a, "target": c }),
            json!({ "source": a }),
            json!({ "target": c }),
            json!({ "id": "not-a-uuid" }),
            json!({ "source": a, "target": 7 }),
        ] {
            let result = mcp_merge(&f, &editor_write, args.clone()).await?;
            anyhow::ensure!(
                result["isError"] == true && result["structuredContent"].get("code").is_none(),
                "{args}: {result}"
            );
        }
        ensure_same(&before, &graph(&f).await?, "refused and malformed calls")?;
        anyhow::ensure!(f.review_row(review).await? == ("pending".to_string(), None, None));
        anyhow::ensure!(
            ledger(&f).await?.is_empty(),
            "no refusal writes a ledger row"
        );

        // 没有哪个工具能替人点头或答提议：这些名字不存在，是 JSON-RPC 的 -32601
        for name in ["review_decide", "review_confirm", "review_approve"] {
            let (status, body) = rpc(
                &f,
                &editor_write,
                "tools/call",
                json!({ "name": name, "arguments": { "id": review } }),
            )
            .await?;
            anyhow::ensure!(status == StatusCode::OK, "{name}: {status}");
            anyhow::ensure!(
                body["error"]["code"] == -32601 && body.get("result").is_none(),
                "{name} must not exist: {body}"
            );
        }
        // 队列的决定路由上，分开与合并之外的动作仍然没开
        let (status, body) = f
            .call(
                &editor_write,
                "POST",
                &format!("/kbs/{}/review/{review}", f.kb),
                Some(json!({ "action": "approve" })),
            )
            .await?;
        anyhow::ensure!(
            status == StatusCode::FORBIDDEN && body["code"] == "token_operation_not_open",
            "{status} {body}"
        );
        anyhow::ensure!(f.review_row(review).await? == ("pending".to_string(), None, None));
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── 不排治理轮次；会话照旧不问闸门 ─────────────────────────

#[tokio::test]
async fn a_token_merge_starts_no_govern_round_and_a_session_merge_still_asks_no_gate(
) -> anyhow::Result<()> {
    // 治理开着的库
    let Some(f) = Fx::new(true).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        let s = seeds(&f).await?;
        let (a, c) = passing(&f, &s, "Acme").await?;
        let pair = f.pair_of(a, c).await?;
        let (m1, m2) = passing(&f, &s, "Borealis").await?;
        // 这一对上开着一条治理建议，等人答
        let proposal = utopia_store::governance::record(
            &f.pool,
            kb,
            utopia_store::governance::NewDecision {
                run_id: Uuid::now_v7(),
                target_id: pair,
                action: "merge",
                confidence: 0.7,
                reason: None,
                precedents: json!([]),
                status: "proposed",
                merge_id: None,
                question: None,
                trace: json!([]),
                calls: 0,
            },
        )
        .await?;
        let proposal_status = || async {
            sqlx::query_scalar::<_, String>("SELECT status FROM agent_decisions WHERE id = $1")
                .bind(proposal)
                .fetch_one(&f.pool)
                .await
        };
        anyhow::ensure!(govern_jobs(&f).await? == 0);

        // 令牌合队列里的一对、手动合另一对，再碰一次闸门留下的一对：都不排治理任务，
        // 也不替人回答这一对上开着的建议
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let (status, body) = rest_merge(&f, &token, pair).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        ensure_applied(&body)?;
        let (status, body) = rest_manual(&f, &token, m1, m2).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        ensure_applied(&body)?;
        let (h1, h2) = held(&f, &s, "Cygnus").await?;
        let token_held = f.pair_of(h1, h2).await?;
        let (status, body) = rest_merge(&f, &token, token_held).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        ensure_held(&body, "derived", false)?;
        anyhow::ensure!(
            govern_jobs(&f).await? == 0,
            "a token merge must not queue a govern round"
        );
        anyhow::ensure!(
            proposal_status().await? == "proposed",
            "the governor's proposal on the pair must stay open for a person"
        );

        // 会话：闸门会留下的一对，人在队列里合、手动合，都照旧直接落地、回从前的回答
        let (q1, q2) = held(&f, &s, "Draco").await?;
        let by_queue = f.pair_of(q1, q2).await?;
        let (status, body) = rest_merge(&f, &f.editor_session, by_queue).await?;
        anyhow::ensure!(status == StatusCode::OK, "session merge: {status} {body}");
        anyhow::ensure!(body == json!({ "ok": true }), "session merge body: {body}");
        anyhow::ensure!(
            f.review_row(by_queue).await? == ("merged".to_string(), Some(f.editor), None),
            "a session merge is decided by the person and carries no token"
        );
        anyhow::ensure!(merges_between(&f, q1, q2).await?.len() == 1);

        let (p1, p2) = held(&f, &s, "Eridanus").await?;
        let (status, body) = rest_manual(&f, &f.editor_session, p1, p2).await?;
        anyhow::ensure!(
            status == StatusCode::OK,
            "session manual merge: {status} {body}"
        );
        anyhow::ensure!(
            keys(&body) == ["merge_id"],
            "session manual merge body: {body}"
        );
        let merge_id = uuid_at(&body, "merge_id")?;
        anyhow::ensure!(
            merges_between(&f, p1, p2).await? == [(merge_id, p1, p2, Some(f.editor))],
            "the person's manual merge was applied"
        );
        // 令牌碰的那一对还等着：同一种被留下的一对，令牌是提议，人是落地
        anyhow::ensure!(f.review_row(token_held).await?.0 == "pending");
        anyhow::ensure!(merges_between(&f, h1, h2).await?.is_empty());

        // 人的合并不进令牌 agent 的台账：台账里只有令牌的那三行
        let rows = ledger(&f).await?;
        anyhow::ensure!(
            rows.iter()
                .map(|row| row.outcome.as_str())
                .collect::<Vec<_>>()
                == ["applied", "applied", "held"],
            "{rows:?}"
        );
        // 对照：人在队列里的合并照旧排了一轮——上面的零不是因为这个库排不了
        anyhow::ensure!(
            govern_jobs(&f).await? == 1,
            "a person's queue merge still queues a govern round"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}
