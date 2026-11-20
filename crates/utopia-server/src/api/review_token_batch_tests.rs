//! 编辑者的写令牌成批裁重复项、整批撤回，以及每一笔令牌动作的归属（0076；SC8、SC10）。
//!
//! 要钉住的行为：
//!
//! - **一批**（`POST /review/batch`，令牌）：1 到 50 个 id、一个动作；每一对走单笔的令牌
//!   路径，所以闸门留下的成员是一条提议，认不出的 id 只是它自己那一条的 `not_found`；
//!   服务端发一个 `batch_id`，这一批的每一行台账、每一条审计都带着它
//! - **整批拒绝**（E18）：0 个或 51 个 id 是 `batch_size`，动作不是 merge / keep 是会话批量
//!   今天的那一句；一行都不写
//! - **逐条结果**（E19）：按请求的次序一个 id 一条，七个键；同一个 id 给两次，第二次拿到
//!   它那时的状态给的回答
//! - **整批撤回**（E20，`POST /review/agent-batches/{batch_id}/revert`）：还没撤回的合并
//!   各撤一次，已经撤回的是 `already_decided`，再来一次什么都不变，分开的不动，认不出的
//!   号是 404；编辑者的写令牌与登录的编辑者都能调
//! - **归属**（E23）：合并、手动合并、撤回、一批的成员、被留下的调用，在 Agent 一档里都是
//!   `source: "token"` 加主人与令牌当时的名字，在 Activity 里都带 `token_id` 与
//!   `token_name`；批里的成员两处都带 `batch_id`；令牌改名、撤销之后名字不变
//! - **MCP**：`review_batch` 与带 `batch_id` 的 `review_revert` 回的就是 REST 的那个 body
//!
//! 夹具在 `review_session_unchanged_tests.rs`。种子直接插表，不跑模型任务。
//! 没有 `UTOPIA_DATABASE_URL` 时跳过。
use axum::http::StatusCode;
use serde_json::{json, Value};
use utopia_store::token_agent_actions::TokenAgentAction;
use uuid::Uuid;

use super::review_session_unchanged_tests::Fx;

const TOKEN_NAME: &str = "cleanup agent";

// ───────────────────────── 调用 ─────────────────────────

/// 一批，经 REST
async fn rest_batch(
    f: &Fx,
    bearer: &str,
    ids: &[Uuid],
    action: &str,
) -> anyhow::Result<(StatusCode, Value)> {
    f.call(
        bearer,
        "POST",
        &format!("/kbs/{}/review/batch", f.kb),
        Some(json!({ "ids": ids, "action": action, "rationale": "same thing" })),
    )
    .await
}

/// 一批合并，必须是 200：回整个 body
async fn batch_ok(f: &Fx, bearer: &str, ids: &[Uuid], action: &str) -> anyhow::Result<Value> {
    let (status, body) = rest_batch(f, bearer, ids, action).await?;
    anyhow::ensure!(status == StatusCode::OK, "batch {action}: {status} {body}");
    Ok(body)
}

/// 整批撤回，经 REST。`kb` 单给：要拿别的库去试
async fn rest_unit_revert(
    f: &Fx,
    bearer: &str,
    kb: Uuid,
    batch_id: Uuid,
) -> anyhow::Result<(StatusCode, Value)> {
    f.call(
        bearer,
        "POST",
        &format!("/kbs/{kb}/review/agent-batches/{batch_id}/revert"),
        None,
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

/// 一次工具调用，回工具结果。工具自己的拒绝是 `isError: true` 的正常结果
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

fn outcomes(body: &Value) -> Vec<Value> {
    body["outcomes"].as_array().cloned().unwrap_or_default()
}

/// 一批的回答：三个键，一个 id 一条、次序与请求相同、每一条正好七个键。回 `batch_id`
fn ensure_batch_shape(body: &Value, ids: &[Uuid]) -> anyhow::Result<Uuid> {
    anyhow::ensure!(
        keys(body) == ["batch_id", "decided", "outcomes"],
        "a batch answer: {body}"
    );
    let rows = outcomes(body);
    anyhow::ensure!(rows.len() == ids.len(), "one outcome per id: {body}");
    for (row, id) in rows.iter().zip(ids) {
        anyhow::ensure!(row["id"] == json!(id), "request order: {body}");
        anyhow::ensure!(
            keys(row)
                == [
                    "action_id",
                    "code",
                    "error",
                    "id",
                    "merge_id",
                    "outcome",
                    "reason"
                ],
            "the seven keys: {row}"
        );
    }
    let applied = rows.iter().filter(|r| r["outcome"] == "applied").count();
    anyhow::ensure!(body["decided"] == json!(applied), "decided: {body}");
    uuid_at(body, "batch_id")
}

/// 落地的一条：`applied`，带台账行与合并，没有理由、code、错误。回 (台账行, 合并)
fn ensure_applied_item(row: &Value) -> anyhow::Result<(Uuid, Uuid)> {
    anyhow::ensure!(row["outcome"] == "applied", "{row}");
    anyhow::ensure!(
        row["reason"].is_null() && row["code"].is_null() && row["error"].is_null(),
        "{row}"
    );
    Ok((uuid_at(row, "action_id")?, uuid_at(row, "merge_id")?))
}

/// 闸门留下的一条：`held`、理由，带提议的台账行，没有合并。回提议的 id
fn ensure_held_item(row: &Value, reason: &str, repeat: bool) -> anyhow::Result<Uuid> {
    anyhow::ensure!(row["outcome"] == "held" && row["reason"] == reason, "{row}");
    anyhow::ensure!(row["merge_id"].is_null() && row["error"].is_null(), "{row}");
    if repeat {
        anyhow::ensure!(row["code"] == "already_proposed", "{row}");
    } else {
        anyhow::ensure!(row["code"].is_null(), "{row}");
    }
    uuid_at(row, "action_id")
}

/// 没成的一条：`outcome` 是 null，带稳定的 code 与一句话，别的都是 null
fn ensure_refused_item(row: &Value, code: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        row["outcome"].is_null() && row["code"] == code,
        "expected {code}: {row}"
    );
    anyhow::ensure!(
        row["error"].as_str().is_some_and(|s| !s.is_empty()),
        "{row}"
    );
    anyhow::ensure!(
        row["action_id"].is_null() && row["merge_id"].is_null() && row["reason"].is_null(),
        "{row}"
    );
    Ok(())
}

/// 整批撤回的回答：三个键，一次合并一条、每一条正好四个键
fn ensure_revert_shape(body: &Value, batch_id: Uuid, merges: &[Uuid]) -> anyhow::Result<()> {
    anyhow::ensure!(
        keys(body) == ["batch_id", "outcomes", "reverted"],
        "a unit revert answer: {body}"
    );
    anyhow::ensure!(body["batch_id"] == json!(batch_id), "{body}");
    let rows = outcomes(body);
    anyhow::ensure!(rows.len() == merges.len(), "one outcome per merge: {body}");
    for (row, merge_id) in rows.iter().zip(merges) {
        anyhow::ensure!(row["merge_id"] == json!(merge_id), "{body}");
        anyhow::ensure!(
            keys(row) == ["code", "error", "merge_id", "outcome"],
            "the four keys: {row}"
        );
    }
    let applied = rows.iter().filter(|r| r["outcome"] == "applied").count();
    anyhow::ensure!(body["reverted"] == json!(applied), "reverted: {body}");
    Ok(())
}

fn ensure_reverted_item(row: &Value) -> anyhow::Result<()> {
    anyhow::ensure!(
        row["outcome"] == "applied" && row["code"].is_null() && row["error"].is_null(),
        "{row}"
    );
    Ok(())
}

fn ensure_already_reverted_item(row: &Value) -> anyhow::Result<()> {
    anyhow::ensure!(
        row["outcome"].is_null()
            && row["code"] == "already_decided"
            && row["error"].as_str().is_some_and(|s| !s.is_empty()),
        "{row}"
    );
    Ok(())
}

// ───────────────────────── 读库 ─────────────────────────

/// 这个库的台账，最早的在前
async fn ledger(f: &Fx) -> anyhow::Result<Vec<TokenAgentAction>> {
    let mut rows = utopia_store::token_agent_actions::list(&f.pool, f.kb, 500, 0).await?;
    rows.reverse();
    Ok(rows)
}

/// 这个库的审计行：动作、actor、target_id、detail，最早的在前。`mcp.tool_called` 不算：
/// 那是 MCP 入口给每一次工具调用记的一行（被拒绝的也记），说的是「调过」，不是对图做了什么
async fn audits(f: &Fx) -> anyhow::Result<Vec<(String, Option<Uuid>, Option<Uuid>, Value)>> {
    Ok(sqlx::query_as(
        "SELECT action, actor_id, target_id, detail FROM audit_events
          WHERE kb_id = $1 AND action <> 'mcp.tool_called' ORDER BY created_at, id",
    )
    .bind(f.kb)
    .fetch_all(&f.pool)
    .await?)
}

/// 「什么都没变」比的东西：这个库的实体、事实、审核行、合并日志，每一行的每一列
async fn graph(f: &Fx) -> anyhow::Result<Value> {
    let mut out = serde_json::Map::new();
    for table in ["entities", "facts", "resolution_reviews", "entity_merges"] {
        // 表名是上面写死的四个，不是外面来的
        let rows: Value = sqlx::query_scalar(&format!(
            "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY t.id), '[]'::jsonb)
               FROM {table} t WHERE t.kb_id = $1"
        ))
        .bind(f.kb)
        .fetch_one(&f.pool)
        .await?;
        out.insert(table.to_string(), rows);
    }
    Ok(Value::Object(out))
}

async fn merged_into(f: &Fx, entity: Uuid) -> anyhow::Result<Option<Uuid>> {
    Ok(
        sqlx::query_scalar("SELECT merged_into FROM entities WHERE id = $1")
            .bind(entity)
            .fetch_one(&f.pool)
            .await?,
    )
}

/// 一次合并此刻的样子：源、目标、撤回了没有
async fn merge_row(f: &Fx, merge_id: Uuid) -> anyhow::Result<(Uuid, Uuid, bool)> {
    Ok(sqlx::query_as(
        "SELECT source_id, target_id, reverted_at IS NOT NULL FROM entity_merges
          WHERE id = $1 AND kb_id = $2",
    )
    .bind(merge_id)
    .bind(f.kb)
    .fetch_one(&f.pool)
    .await?)
}

/// 这次合并现在是不是在着：源并在目标里、没撤回
async fn merge_is_live(f: &Fx, merge_id: Uuid) -> anyhow::Result<bool> {
    let (source, target, reverted) = merge_row(f, merge_id).await?;
    Ok(!reverted && merged_into(f, source).await? == Some(target))
}

/// 这次合并现在是不是撤回了：日志上记着，源也不再并在谁里面
async fn merge_is_undone(f: &Fx, merge_id: Uuid) -> anyhow::Result<bool> {
    let (source, _, reverted) = merge_row(f, merge_id).await?;
    Ok(reverted && merged_into(f, source).await?.is_none())
}

// ───────────────────────── 种子 ─────────────────────────

async fn relation(f: &Fx, key: &str, label: &str) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO relation_types (id, kb_id, key, label) VALUES ($1, $2, $3, $4)")
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

/// 两样关系与一个共用的宾语，够种出闸门放行的一对和留下的一对
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

/// 两个公司，各有一条事实，闸门不拦：回 (审核行, 左, 右)
async fn passing(f: &Fx, s: &Seeds, name: &str) -> anyhow::Result<(Uuid, Uuid, Uuid)> {
    let a = f.entity(f.company_type, name).await?;
    let c = f.entity(f.company_type, &format!("{name} Corp")).await?;
    fact(f, a, s.based_in, s.city).await?;
    fact(f, c, s.part_of, s.city).await?;
    Ok((f.pair_of(a, c).await?, a, c))
}

/// 闸门会留下的一对：左边身上有一条还成立的派生事实（`derived`）。回 (审核行, 左, 右)
async fn held(f: &Fx, s: &Seeds, name: &str) -> anyhow::Result<(Uuid, Uuid, Uuid)> {
    let (review, a, c) = passing(f, s, name).await?;
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
    .bind(a)
    .bind(s.part_of)
    .bind(s.city)
    .bind(rule)
    .execute(&f.pool)
    .await?;
    Ok((review, a, c))
}

// ───────────────────────── SC8：一批、它的号、整批撤回 ─────────────────────────

#[tokio::test]
async fn a_mixed_token_batch_lands_gated_members_as_proposals_under_one_batch_id_and_its_unit_revert_undoes_every_merge(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let s = seeds(&f).await?;
        let (first, first_left, first_right) = passing(&f, &s, "Acme").await?;
        let (gated, gated_left, gated_right) = held(&f, &s, "Globex").await?;
        let (second, ..) = passing(&f, &s, "Initech").await?;
        let unknown = Uuid::now_v7();
        let (token_id, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;

        // 51 个 id：整批被拒，一行都没写
        let mut too_many = vec![first, gated, second];
        too_many.extend((0..48).map(|_| Uuid::now_v7()));
        anyhow::ensure!(too_many.len() == 51);
        let (status, body) = rest_batch(&f, &token, &too_many, "merge").await?;
        anyhow::ensure!(
            status == StatusCode::UNPROCESSABLE_ENTITY && body["code"] == "batch_size",
            "51 ids: {status} {body}"
        );
        anyhow::ensure!(ledger(&f).await?.is_empty(), "a refused batch wrote a row");
        anyhow::ensure!(
            f.review_row(first).await?.0 == "pending",
            "a refused batch decided a pair"
        );

        // 放行的、被闸门留下的、认不出的、放行的：一个 id 一条结果，一个 batch_id
        let ids = [first, gated, unknown, second];
        let body = batch_ok(&f, &token, &ids, "merge").await?;
        let batch_id = ensure_batch_shape(&body, &ids)?;
        anyhow::ensure!(body["decided"] == 2, "{body}");
        let rows = outcomes(&body);
        let (first_action, first_merge) = ensure_applied_item(&rows[0])?;
        let proposal = ensure_held_item(&rows[1], "derived", false)?;
        ensure_refused_item(&rows[2], "not_found")?;
        let (second_action, second_merge) = ensure_applied_item(&rows[3])?;

        // 落地的两对合并了，记在主人名下、带着令牌
        anyhow::ensure!(merge_is_live(&f, first_merge).await?);
        anyhow::ensure!(merge_is_live(&f, second_merge).await?);
        anyhow::ensure!(
            f.review_row(first).await? == ("merged".to_string(), Some(f.editor), Some(token_id))
        );
        // 被留下的那一对什么都没变：还在等，两个实体都在
        anyhow::ensure!(f.review_row(gated).await?.0 == "pending");
        anyhow::ensure!(merged_into(&f, gated_left).await?.is_none());
        anyhow::ensure!(merged_into(&f, gated_right).await?.is_none());

        // 这一批的每一行台账都带着同一个号：两行落地的合并，一条提议
        let rows = ledger(&f).await?;
        anyhow::ensure!(rows.len() == 3, "{rows:?}");
        for row in &rows {
            anyhow::ensure!(row.batch_id == Some(batch_id), "{row:?}");
            anyhow::ensure!(
                row.operation == "review.merge" && row.token_name == TOKEN_NAME,
                "{row:?}"
            );
        }
        let by_id = |id: Uuid| rows.iter().find(|row| row.id == id);
        let applied = by_id(first_action).ok_or_else(|| anyhow::anyhow!("no row"))?;
        anyhow::ensure!(
            applied.outcome == "applied" && applied.merge_id == Some(first_merge),
            "{applied:?}"
        );
        anyhow::ensure!(by_id(second_action).is_some_and(|row| row.outcome == "applied"));
        let open = by_id(proposal).ok_or_else(|| anyhow::anyhow!("no proposal row"))?;
        anyhow::ensure!(
            open.outcome == "held"
                && open.gate_reason.as_deref() == Some("derived")
                && open.state.is_none()
                && open.target_id == gated,
            "{open:?}"
        );

        // 这一批的每一条审计行也带着它：两条 review.merge，一条 agent.proposal
        let events = audits(&f).await?;
        anyhow::ensure!(events.len() == 3, "{events:?}");
        for (action, actor, _, detail) in &events {
            anyhow::ensure!(*actor == Some(f.editor), "{action}: {detail}");
            anyhow::ensure!(
                detail["batch_id"] == json!(batch_id)
                    && detail["token_id"] == json!(token_id)
                    && detail["token_name"] == TOKEN_NAME,
                "{action}: {detail}"
            );
        }
        let actions: Vec<&str> = events.iter().map(|e| e.0.as_str()).collect();
        anyhow::ensure!(
            actions == ["review.merge", "agent.proposal", "review.merge"],
            "{actions:?}"
        );

        // 整批撤回：这一批做出的每一次合并都撤了，提议原样开着
        let (status, undone) = rest_unit_revert(&f, &token, f.kb, batch_id).await?;
        anyhow::ensure!(status == StatusCode::OK, "unit revert: {status} {undone}");
        ensure_revert_shape(&undone, batch_id, &[first_merge, second_merge])?;
        anyhow::ensure!(undone["reverted"] == 2, "{undone}");
        for row in outcomes(&undone) {
            ensure_reverted_item(&row)?;
        }
        anyhow::ensure!(merge_is_undone(&f, first_merge).await?);
        anyhow::ensure!(merge_is_undone(&f, second_merge).await?);
        anyhow::ensure!(merged_into(&f, first_left).await?.is_none());
        anyhow::ensure!(merged_into(&f, first_right).await?.is_none());
        let rows = ledger(&f).await?;
        let reverts: Vec<_> = rows
            .iter()
            .filter(|row| row.operation == "merge.revert")
            .collect();
        anyhow::ensure!(reverts.len() == 2, "{rows:?}");
        for row in &reverts {
            anyhow::ensure!(
                row.outcome == "applied" && row.batch_id == Some(batch_id),
                "{row:?}"
            );
        }
        let open = rows
            .iter()
            .find(|row| row.id == proposal)
            .ok_or_else(|| anyhow::anyhow!("no proposal row"))?;
        anyhow::ensure!(open.state.is_none(), "{open:?}");
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── E18：整批拒绝 ─────────────────────────

#[tokio::test]
async fn a_token_batch_of_0_or_51_ids_or_of_another_action_is_refused_as_a_whole_and_writes_nothing(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let s = seeds(&f).await?;
        let (first, ..) = passing(&f, &s, "Acme").await?;
        let (second, ..) = passing(&f, &s, "Initech").await?;
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let before = graph(&f).await?;
        let mut fifty_one = vec![first, second];
        fifty_one.extend((0..49).map(|_| Uuid::now_v7()));
        anyhow::ensure!(fifty_one.len() == 51);

        for action in ["merge", "keep"] {
            for (what, ids) in [("no ids", Vec::new()), ("51 ids", fifty_one.clone())] {
                let (status, body) = rest_batch(&f, &token, &ids, action).await?;
                anyhow::ensure!(
                    status == StatusCode::UNPROCESSABLE_ENTITY && body["code"] == "batch_size",
                    "{what} {action}: {status} {body}"
                );
                anyhow::ensure!(body.get("batch_id").is_none(), "{what}: {body}");
                // MCP 上是同一个 code，同一句话
                let refused = tool(
                    &f,
                    &token,
                    "review_batch",
                    json!({ "ids": ids, "action": action }),
                )
                .await?;
                ensure_refused(&refused, "batch_size")?;
                anyhow::ensure!(
                    refused["structuredContent"]["message"] == body["error"],
                    "{what}: {refused} {body}"
                );
            }
        }

        // 别的动作：整批被拒，答的是会话批量对同一个请求今天的那一句
        for action in ["reject", "revert", ""] {
            let ids = [first, second];
            let (status, body) = rest_batch(&f, &token, &ids, action).await?;
            anyhow::ensure!(
                status == StatusCode::UNPROCESSABLE_ENTITY && body.get("batch_id").is_none(),
                "action {action:?}: {status} {body}"
            );
            let (session_status, session_body) =
                rest_batch(&f, &f.editor_session, &ids, action).await?;
            anyhow::ensure!(
                (status, &body) == (session_status, &session_body),
                "action {action:?}: token {status} {body}, session {session_status} {session_body}"
            );
            let refused = tool(
                &f,
                &token,
                "review_batch",
                json!({ "ids": ids, "action": action }),
            )
            .await?;
            anyhow::ensure!(refused["isError"] == true, "action {action:?}: {refused}");
        }

        // 什么都没变，台账与审计一行都没写
        anyhow::ensure!(
            graph(&f).await? == before,
            "a refused batch changed the graph"
        );
        anyhow::ensure!(
            ledger(&f).await?.is_empty(),
            "a refused batch wrote a ledger row"
        );
        anyhow::ensure!(
            audits(&f).await?.is_empty(),
            "a refused batch wrote an audit row"
        );

        // 边界的另一边：50 个 id 收下（这里全是认不出的，各是各的 `not_found`），1 个也收下
        let fifty: Vec<Uuid> = (0..50).map(|_| Uuid::now_v7()).collect();
        let body = batch_ok(&f, &token, &fifty, "merge").await?;
        ensure_batch_shape(&body, &fifty)?;
        anyhow::ensure!(body["decided"] == 0, "{body}");
        for row in outcomes(&body) {
            ensure_refused_item(&row, "not_found")?;
        }
        anyhow::ensure!(graph(&f).await? == before);
        anyhow::ensure!(ledger(&f).await?.is_empty());
        let body = batch_ok(&f, &token, &[first], "keep").await?;
        ensure_batch_shape(&body, &[first])?;
        anyhow::ensure!(body["decided"] == 1, "{body}");
        anyhow::ensure!(f.review_row(first).await?.0 == "kept");
        anyhow::ensure!(f.review_row(second).await?.0 == "pending");
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── E19：逐条结果 ─────────────────────────

#[tokio::test]
async fn a_token_batch_answers_one_outcome_per_id_in_request_order_with_its_code(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let s = seeds(&f).await?;
        let (passed, ..) = passing(&f, &s, "Acme").await?;
        let (gated, ..) = held(&f, &s, "Globex").await?;
        let (decided, ..) = passing(&f, &s, "Initech").await?;
        let unknown = Uuid::now_v7();
        // 一对人已经裁过的
        let (status, body) = f
            .call(
                &f.editor_session,
                "POST",
                &format!("/kbs/{}/review/{decided}", f.kb),
                Some(json!({ "action": "keep" })),
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "seeding a keep: {status} {body}");
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;

        // 次序是故意乱的；放行的那一对与被留下的那一对各给了两次
        let ids = [gated, unknown, passed, decided, passed, gated];
        let body = batch_ok(&f, &token, &ids, "merge").await?;
        let batch_id = ensure_batch_shape(&body, &ids)?;
        anyhow::ensure!(body["decided"] == 1, "{body}");
        let rows = outcomes(&body);
        let proposal = ensure_held_item(&rows[0], "derived", false)?;
        ensure_refused_item(&rows[1], "not_found")?;
        let (action, merge) = ensure_applied_item(&rows[2])?;
        ensure_refused_item(&rows[3], "already_decided")?;
        // 第二次：它那时的状态给的回答。合并过的一对已经定了；被留下的是同一条提议
        ensure_refused_item(&rows[4], "already_decided")?;
        anyhow::ensure!(ensure_held_item(&rows[5], "derived", true)? == proposal);
        anyhow::ensure!(merge_is_live(&f, merge).await?);

        // 写下的只有两行：一行落地的合并、一条提议，都带着这一批的号
        let written = ledger(&f).await?;
        anyhow::ensure!(written.len() == 2, "{written:?}");
        anyhow::ensure!(written.iter().all(|row| row.batch_id == Some(batch_id)));
        anyhow::ensure!(written.iter().any(|row| row.id == action));
        anyhow::ensure!(written.iter().any(|row| row.id == proposal));

        // 分开也是一批：落地的、第二次给的、人裁过的、认不出的
        let (apart, ..) = passing(&f, &s, "Hooli").await?;
        let ids = [apart, apart, decided, unknown];
        let body = batch_ok(&f, &token, &ids, "keep").await?;
        let keep_batch = ensure_batch_shape(&body, &ids)?;
        anyhow::ensure!(keep_batch != batch_id, "each batch has its own id");
        anyhow::ensure!(body["decided"] == 1, "{body}");
        let rows = outcomes(&body);
        anyhow::ensure!(
            rows[0]["outcome"] == "applied"
                && rows[0]["merge_id"].is_null()
                && rows[0]["code"].is_null()
                && rows[0]["error"].is_null(),
            "{body}"
        );
        let kept_action = uuid_at(&rows[0], "action_id")?;
        ensure_refused_item(&rows[1], "already_decided")?;
        ensure_refused_item(&rows[2], "already_decided")?;
        ensure_refused_item(&rows[3], "not_found")?;
        anyhow::ensure!(f.review_row(apart).await?.0 == "kept");
        // 分开的那一行台账与它的审计行同样带着号
        let written = ledger(&f).await?;
        let kept = written
            .iter()
            .find(|row| row.id == kept_action)
            .ok_or_else(|| anyhow::anyhow!("no keep row"))?;
        anyhow::ensure!(
            kept.operation == "review.keep" && kept.batch_id == Some(keep_batch),
            "{kept:?}"
        );
        let keeps = f.audits("review.keep", apart).await?;
        anyhow::ensure!(keeps.len() == 1, "{keeps:?}");
        anyhow::ensure!(
            keeps[0].1["batch_id"] == json!(keep_batch) && keeps[0].1["token_name"] == TOKEN_NAME,
            "{keeps:?}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── E20：整批撤回 ─────────────────────────

#[tokio::test]
async fn a_unit_revert_reverts_each_unreverted_merge_of_the_batch_once_and_leaves_keeps(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let s = seeds(&f).await?;
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let (_, read_only) = f.token(f.editor, "reader", "read", None).await?;
        let (_, viewer_write) = f.token(f.viewer, "viewer writer", "write", None).await?;

        // 一批三次合并
        let mut ids = Vec::new();
        for name in ["Acme", "Globex", "Initech"] {
            ids.push(passing(&f, &s, name).await?.0);
        }
        let body = batch_ok(&f, &token, &ids, "merge").await?;
        let batch_id = ensure_batch_shape(&body, &ids)?;
        let mut merges = Vec::new();
        for row in outcomes(&body) {
            merges.push(ensure_applied_item(&row)?.1);
        }
        // 另一批：两对分开
        let mut apart = Vec::new();
        for name in ["Hooli", "Umbrella"] {
            apart.push(passing(&f, &s, name).await?.0);
        }
        let body = batch_ok(&f, &token, &apart, "keep").await?;
        let keep_batch = ensure_batch_shape(&body, &apart)?;
        anyhow::ensure!(body["decided"] == 2, "{body}");

        // 谁调不了：只读令牌、主人是 Viewer 的写令牌、登录的 Viewer。什么都没撤
        for (refused, code) in [
            (&read_only, "token_read_only"),
            (&viewer_write, "token_owner_below_editor"),
        ] {
            let (status, body) = rest_unit_revert(&f, refused, f.kb, batch_id).await?;
            anyhow::ensure!(
                status == StatusCode::FORBIDDEN && body["code"] == code,
                "{code}: {status} {body}"
            );
        }
        let (status, body) = rest_unit_revert(&f, &f.viewer_session, f.kb, batch_id).await?;
        anyhow::ensure!(status == StatusCode::FORBIDDEN, "a viewer: {status} {body}");
        // 认不出的号、别的库的号：404
        for (what, kb, id) in [
            ("an unknown batch id", f.kb, Uuid::now_v7()),
            ("another base", f.other_kb, batch_id),
        ] {
            for bearer in [&token, &f.editor_session] {
                let (status, body) = rest_unit_revert(&f, bearer, kb, id).await?;
                anyhow::ensure!(status == StatusCode::NOT_FOUND, "{what}: {status} {body}");
            }
        }
        for merge in &merges {
            anyhow::ensure!(merge_is_live(&f, *merge).await?, "a refusal reverted");
        }

        // 其中一次合并先被单独撤掉了
        let (status, body) = f
            .call(
                &token,
                "POST",
                &format!("/kbs/{}/merges/{}/revert", f.kb, merges[1]),
                None,
            )
            .await?;
        anyhow::ensure!(
            status == StatusCode::OK && body["outcome"] == "applied",
            "{status} {body}"
        );

        // 整批撤回：没撤过的两次各撤一次，撤过的那一次是它自己的 `already_decided`
        let (status, body) = rest_unit_revert(&f, &token, f.kb, batch_id).await?;
        anyhow::ensure!(status == StatusCode::OK, "unit revert: {status} {body}");
        ensure_revert_shape(&body, batch_id, &merges)?;
        anyhow::ensure!(body["reverted"] == 2, "{body}");
        let rows = outcomes(&body);
        ensure_reverted_item(&rows[0])?;
        ensure_already_reverted_item(&rows[1])?;
        ensure_reverted_item(&rows[2])?;
        for merge in &merges {
            anyhow::ensure!(merge_is_undone(&f, *merge).await?);
        }

        // 再来一次：什么都不变，每一条都是 `already_decided`；人来调也一样
        let settled = graph(&f).await?;
        let rows_before = ledger(&f).await?.len();
        let events_before = audits(&f).await?.len();
        for bearer in [&token, &f.editor_session] {
            let (status, body) = rest_unit_revert(&f, bearer, f.kb, batch_id).await?;
            anyhow::ensure!(status == StatusCode::OK, "a second unit revert: {status}");
            ensure_revert_shape(&body, batch_id, &merges)?;
            anyhow::ensure!(body["reverted"] == 0, "{body}");
            for row in outcomes(&body) {
                ensure_already_reverted_item(&row)?;
            }
        }
        anyhow::ensure!(
            graph(&f).await? == settled,
            "a second unit revert changed the graph"
        );
        anyhow::ensure!(ledger(&f).await?.len() == rows_before);
        anyhow::ensure!(audits(&f).await?.len() == events_before);

        // 分开的一批：号认得，没有合并可撤，分开的原样
        let (status, body) = rest_unit_revert(&f, &token, f.kb, keep_batch).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        ensure_revert_shape(&body, keep_batch, &[])?;
        anyhow::ensure!(body["reverted"] == 0, "{body}");
        for review in &apart {
            anyhow::ensure!(f.review_row(*review).await?.0 == "kept");
        }
        anyhow::ensure!(graph(&f).await? == settled, "keeps were touched");

        // 登录的编辑者撤一批：走的是人的撤回，记在这个人名下，台账里没有令牌的撤回行
        let mut ids = Vec::new();
        for name in ["Stark", "Wayne"] {
            ids.push(passing(&f, &s, name).await?.0);
        }
        let body = batch_ok(&f, &token, &ids, "merge").await?;
        let by_person = ensure_batch_shape(&body, &ids)?;
        let mut person_merges = Vec::new();
        for row in outcomes(&body) {
            person_merges.push(ensure_applied_item(&row)?.1);
        }
        let rows_before = ledger(&f).await?.len();
        let (status, body) = rest_unit_revert(&f, &f.editor_session, f.kb, by_person).await?;
        anyhow::ensure!(status == StatusCode::OK, "a person: {status} {body}");
        ensure_revert_shape(&body, by_person, &person_merges)?;
        anyhow::ensure!(body["reverted"] == 2, "{body}");
        for merge in &person_merges {
            anyhow::ensure!(merge_is_undone(&f, *merge).await?);
        }
        let written = ledger(&f).await?;
        anyhow::ensure!(
            written.len() == rows_before,
            "a person's revert wrote a row"
        );
        // 那两次合并的台账行读作 undone：人的撤回一行不用多写就看得出来
        for merge in &person_merges {
            let row = written
                .iter()
                .find(|row| row.merge_id == Some(*merge) && row.operation == "review.merge")
                .ok_or_else(|| anyhow::anyhow!("no merge row"))?;
            anyhow::ensure!(row.state.as_deref() == Some("undone"), "{row:?}");
        }
        let events = audits(&f).await?;
        for merge in &person_merges {
            let mine: Vec<_> = events
                .iter()
                .filter(|e| e.0 == "merge.revert" && e.2 == Some(*merge))
                .collect();
            anyhow::ensure!(mine.len() == 1, "{mine:?}");
            let (_, actor, _, detail) = mine[0];
            anyhow::ensure!(*actor == Some(f.editor), "{detail}");
            anyhow::ensure!(
                detail.get("token_id").is_none()
                    && detail.get("token_name").is_none()
                    && detail.get("batch_id").is_none()
                    && detail["agent_batch"] == json!(by_person),
                "a person's revert is the person's: {detail}"
            );
        }
        // 令牌再来撤这一批：都撤过了
        let (status, body) = rest_unit_revert(&f, &token, f.kb, by_person).await?;
        anyhow::ensure!(status == StatusCode::OK && body["reverted"] == 0, "{body}");
        for row in outcomes(&body) {
            ensure_already_reverted_item(&row)?;
        }
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── SC10 / E23：归属 ─────────────────────────

/// Agent 一档里令牌的行，按 id 找
fn token_row(rows: &[Value], id: Uuid) -> anyhow::Result<&Value> {
    rows.iter()
        .find(|row| row["id"] == json!(id))
        .ok_or_else(|| anyhow::anyhow!("no Agent History row {id}"))
}

#[tokio::test]
async fn every_token_merge_revert_batch_member_and_proposal_reads_as_owner_via_token_name_in_agent_history_and_activity(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let s = seeds(&f).await?;
        let (token_id, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let post = |path: String, body: Value| {
            let (f, token) = (&f, &token);
            async move {
                let (status, answer) = f.call(token, "POST", &path, Some(body)).await?;
                anyhow::ensure!(status == StatusCode::OK, "{path}: {status} {answer}");
                anyhow::Ok(answer)
            }
        };
        let kb = f.kb;

        // 一次队列合并、一次手动合并、撤回那次手动合并、一次被留下的调用
        let (queued, ..) = passing(&f, &s, "Acme").await?;
        let merged = post(
            format!("/kbs/{kb}/review/{queued}"),
            json!({ "action": "merge" }),
        )
        .await?;
        anyhow::ensure!(merged["outcome"] == "applied", "{merged}");
        let (source, target) = (
            f.entity(f.company_type, "Wonka").await?,
            f.entity(f.company_type, "Wonka Industries").await?,
        );
        let manual = post(
            format!("/kbs/{kb}/entities/merge"),
            json!({ "source": source, "target": target }),
        )
        .await?;
        let manual_merge = uuid_at(&manual, "merge_id")?;
        let reverted = post(format!("/kbs/{kb}/merges/{manual_merge}/revert"), json!({})).await?;
        let (lone, ..) = held(&f, &s, "Globex").await?;
        let lone_held = post(
            format!("/kbs/{kb}/review/{lone}"),
            json!({ "action": "merge" }),
        )
        .await?;
        anyhow::ensure!(lone_held["outcome"] == "held", "{lone_held}");

        // 一批合并（一对放行、一对被留下）与一批分开
        let (batch_passed, ..) = passing(&f, &s, "Initech").await?;
        let (batch_gated, ..) = held(&f, &s, "Hooli").await?;
        let ids = [batch_passed, batch_gated];
        let body = batch_ok(&f, &token, &ids, "merge").await?;
        let merge_batch = ensure_batch_shape(&body, &ids)?;
        let batch_rows = outcomes(&body);
        let (apart, ..) = passing(&f, &s, "Umbrella").await?;
        let body = batch_ok(&f, &token, &[apart], "keep").await?;
        let keep_batch = ensure_batch_shape(&body, &[apart])?;
        let keep_rows = outcomes(&body);

        // 哪一行属于哪一批；不在批里的没有号
        let mut expected: Vec<(Uuid, &str, &str, Option<Uuid>)> = vec![
            (
                uuid_at(&merged, "action_id")?,
                "review.merge",
                "applied",
                None,
            ),
            (
                uuid_at(&manual, "action_id")?,
                "merge.manual",
                "applied",
                None,
            ),
            (
                uuid_at(&reverted, "action_id")?,
                "merge.revert",
                "applied",
                None,
            ),
            (
                uuid_at(&lone_held, "action_id")?,
                "review.merge",
                "held",
                None,
            ),
            (
                uuid_at(&batch_rows[0], "action_id")?,
                "review.merge",
                "applied",
                Some(merge_batch),
            ),
            (
                uuid_at(&batch_rows[1], "action_id")?,
                "review.merge",
                "held",
                Some(merge_batch),
            ),
            (
                uuid_at(&keep_rows[0], "action_id")?,
                "review.keep",
                "applied",
                Some(keep_batch),
            ),
        ];

        // 整批撤回：它的撤回行也带着那一批的号
        let (status, undone) = rest_unit_revert(&f, &token, kb, merge_batch).await?;
        anyhow::ensure!(
            status == StatusCode::OK && undone["reverted"] == 1,
            "{undone}"
        );
        let unit_revert_row = ledger(&f)
            .await?
            .into_iter()
            .find(|row| row.operation == "merge.revert" && row.batch_id == Some(merge_batch))
            .ok_or_else(|| anyhow::anyhow!("the unit revert wrote no row"))?;
        expected.push((
            unit_revert_row.id,
            "merge.revert",
            "applied",
            Some(merge_batch),
        ));
        anyhow::ensure!(ledger(&f).await?.len() == expected.len());

        // 令牌之后改了名、又被撤销：存下的名字不动（E23）
        sqlx::query("UPDATE personal_tokens SET name = 'renamed later' WHERE id = $1")
            .bind(token_id)
            .execute(&f.pool)
            .await?;
        utopia_store::tokens::revoke(&f.pool, f.editor, token_id).await?;

        // Agent 一档：每一笔都是令牌的行，主人的名字加令牌当时的名字
        let (status, page) = f
            .call(
                &f.editor_session,
                "GET",
                &format!("/kbs/{kb}/review?queue=agent&limit=200"),
                None,
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "agent queue: {status} {page}");
        let rows = page["items"].as_array().cloned().unwrap_or_default();
        anyhow::ensure!(
            rows.len() == expected.len(),
            "one row per action: {}",
            rows.len()
        );
        for (id, operation, outcome, batch) in &expected {
            let row = token_row(&rows, *id)?;
            anyhow::ensure!(row["source"] == "token", "not a token row: {row}");
            anyhow::ensure!(
                row["owner_id"] == json!(f.editor) && row["owner_name"] == "Editor",
                "{row}"
            );
            anyhow::ensure!(
                row["token_id"] == json!(token_id) && row["token_name"] == TOKEN_NAME,
                "{row}"
            );
            anyhow::ensure!(
                row["operation"] == *operation && row["outcome"] == *outcome,
                "{row}"
            );
            match batch {
                Some(batch) => anyhow::ensure!(row["batch_id"] == json!(batch), "{row}"),
                None => anyhow::ensure!(row.get("batch_id").is_none(), "{row}"),
            }
            // 不是治理 agent 的行：没有它的那几个键
            for key in ["status", "confidence", "run_id", "trace", "precedents"] {
                anyhow::ensure!(row.get(key).is_none(), "{key} on a token row: {row}");
            }
        }
        // 撤回过的两次合并读作 undone，没撤的那一次没有后来的状态
        for (id, state) in [
            (uuid_at(&manual, "action_id")?, Some("undone")),
            (uuid_at(&batch_rows[0], "action_id")?, Some("undone")),
            (uuid_at(&merged, "action_id")?, None),
        ] {
            let row = token_row(&rows, id)?;
            anyhow::ensure!(row["state"].as_str() == state, "{row}");
        }

        // Activity 只有库 Admin 读得到：编辑者被拒，令牌读不了
        let path = format!("/kbs/{kb}/audit?limit=200");
        let (status, body) = f.call(&f.editor_session, "GET", &path, None).await?;
        anyhow::ensure!(
            status == StatusCode::FORBIDDEN && body.get("events").is_none(),
            "an editor read Activity: {status} {body}"
        );
        sqlx::query("UPDATE kb_members SET role = 'admin' WHERE kb_id = $1 AND user_id = $2")
            .bind(kb)
            .bind(f.editor)
            .execute(&f.pool)
            .await?;
        let (status, body) = f.call(&f.editor_session, "GET", &path, None).await?;
        anyhow::ensure!(status == StatusCode::OK, "Activity: {status} {body}");
        let events = body["events"].as_array().cloned().unwrap_or_default();
        anyhow::ensure!(
            events.len() == expected.len(),
            "one event per action: {body}"
        );
        // 每一条事件：主人是 actor，带令牌的 id 与当时的名字；找得到它的台账行
        let stored = audits(&f).await?;
        for (id, operation, outcome, batch) in &expected {
            let action_of = |detail: &Value| detail["action_id"] == json!(id);
            let hit = match *operation {
                // 分开的审计 detail 没有 `action_id`：按目标找
                "review.keep" => events
                    .iter()
                    .find(|e| e["action"] == "review.keep" && e["target_id"] == json!(apart)),
                _ => events.iter().find(|e| action_of(&e["detail"])),
            };
            let event = hit.ok_or_else(|| anyhow::anyhow!("no event for {operation} {id}"))?;
            let action = if *outcome == "applied" {
                *operation
            } else {
                "agent.proposal"
            };
            anyhow::ensure!(event["action"] == action, "{event}");
            anyhow::ensure!(
                event["actor_id"] == json!(f.editor) && event["actor_name"] == "Editor",
                "{event}"
            );
            anyhow::ensure!(
                event["token_id"] == token_id.to_string() && event["token_name"] == TOKEN_NAME,
                "{event}"
            );
            match batch {
                Some(batch) => {
                    anyhow::ensure!(event["batch_id"] == batch.to_string(), "{event}")
                }
                None => anyhow::ensure!(event.get("batch_id").is_none(), "{event}"),
            }
        }
        anyhow::ensure!(stored.len() == expected.len(), "{stored:?}");

        // 人答掉那条单独的提议：提议的行读作 declined；Activity 里那一条是这个人的，
        // 不带令牌——令牌的动作不会读成人的决定，人的决定也不会读成令牌的
        let lone_proposal = uuid_at(&lone_held, "action_id")?;
        let (status, body) = f
            .call(
                &f.editor_session,
                "POST",
                &format!("/kbs/{kb}/review/agent-proposals/{lone_proposal}"),
                Some(json!({ "action": "decline" })),
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "decline: {status} {body}");
        let (_, page) = f
            .call(
                &f.editor_session,
                "GET",
                &format!("/kbs/{kb}/review?queue=agent&limit=200"),
                None,
            )
            .await?;
        let rows = page["items"].as_array().cloned().unwrap_or_default();
        let row = token_row(&rows, lone_proposal)?;
        anyhow::ensure!(
            row["state"] == "declined" && row["token_name"] == TOKEN_NAME,
            "{row}"
        );
        let (_, body) = f
            .call(
                &f.editor_session,
                "GET",
                &format!("/kbs/{kb}/audit?action=agent.proposal.declined"),
                None,
            )
            .await?;
        let declined = body["events"].as_array().cloned().unwrap_or_default();
        anyhow::ensure!(declined.len() == 1, "{body}");
        for key in ["token_id", "token_name", "batch_id"] {
            anyhow::ensure!(declined[0].get(key).is_none(), "{key}: {}", declined[0]);
        }
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── MCP：与 REST 同一个回答 ─────────────────────────

#[tokio::test]
async fn review_batch_and_the_unit_revert_run_over_mcp_with_the_rest_answers() -> anyhow::Result<()>
{
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let s = seeds(&f).await?;
        let (token_id, editor_write) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let (_, editor_read) = f.token(f.editor, "editor reader", "read", None).await?;
        let (_, viewer_read) = f.token(f.viewer, "viewer reader", "read", None).await?;
        let (_, viewer_write) = f.token(f.viewer, "viewer writer", "write", None).await?;

        // `review_batch` 只列给编辑者的写令牌
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
            let batch = tools.iter().find(|t| t["name"] == "review_batch").cloned();
            anyhow::ensure!(
                batch.is_some() == listed,
                "{who}: review_batch listed = {}",
                batch.is_some()
            );
            let revert = tools.iter().any(|t| t["name"] == "review_revert");
            anyhow::ensure!(revert == listed, "{who}: review_revert listed = {revert}");
            anyhow::ensure!(
                tools.iter().any(|t| t["name"] == "review_list"),
                "{who}: {tools:?}"
            );
            if listed {
                described = batch;
            }
        }
        let batch = described.ok_or_else(|| anyhow::anyhow!("review_batch is not listed"))?;
        anyhow::ensure!(batch["annotations"]["readOnlyHint"] == false, "{batch}");
        let schema = &batch["inputSchema"];
        anyhow::ensure!(
            schema["required"] == json!(["ids", "action"])
                && keys(&schema["properties"]) == ["action", "ids", "rationale"]
                && schema["properties"]["ids"]["type"] == "array",
            "{schema}"
        );
        let description = batch["description"].as_str().unwrap_or_default();
        for said in ["batch_id", "batch_size", "review_revert", "50"] {
            anyhow::ensure!(description.contains(said), "{said}: {description}");
        }

        // 两组一样的对：一组走 MCP，一组走 REST，回答的形状与每一条的结果相同
        let (mcp_passed, ..) = passing(&f, &s, "Acme").await?;
        let (mcp_gated, ..) = held(&f, &s, "Globex").await?;
        let (rest_passed, ..) = passing(&f, &s, "Initech").await?;
        let (rest_gated, ..) = held(&f, &s, "Hooli").await?;
        let unknown = Uuid::now_v7();
        let mcp_ids = [mcp_passed, mcp_gated, unknown];
        let over_mcp = structured(
            &tool(
                &f,
                &editor_write,
                "review_batch",
                json!({ "ids": mcp_ids, "action": "merge", "rationale": "same thing" }),
            )
            .await?,
        )?;
        let mcp_batch = ensure_batch_shape(&over_mcp, &mcp_ids)?;
        let rest_ids = [rest_passed, rest_gated, unknown];
        let over_rest = batch_ok(&f, &editor_write, &rest_ids, "merge").await?;
        ensure_batch_shape(&over_rest, &rest_ids)?;
        anyhow::ensure!(
            over_mcp["decided"] == 1 && over_rest["decided"] == 1,
            "{over_mcp}"
        );
        for (mine, theirs) in outcomes(&over_mcp).iter().zip(outcomes(&over_rest).iter()) {
            for key in ["outcome", "reason", "code", "error"] {
                anyhow::ensure!(mine[key] == theirs[key], "{key}: {mine} / {theirs}");
            }
        }
        let rows = outcomes(&over_mcp);
        let (_, merge) = ensure_applied_item(&rows[0])?;
        ensure_held_item(&rows[1], "derived", false)?;
        ensure_refused_item(&rows[2], "not_found")?;
        // 走 MCP 的那一批同样记在台账上：带号、带令牌
        let written: Vec<_> = ledger(&f)
            .await?
            .into_iter()
            .filter(|row| row.batch_id == Some(mcp_batch))
            .collect();
        anyhow::ensure!(written.len() == 2, "{written:?}");
        anyhow::ensure!(written.iter().all(|row| row.token_id == token_id));

        // 写不了的调用方调得到这个名字，回来的是带 REST code 的工具错误；什么都没撤
        for (token, code) in [
            (&editor_read, "token_read_only"),
            (&viewer_read, "token_read_only"),
            (&viewer_write, "token_owner_below_editor"),
        ] {
            let (pending, ..) = passing(&f, &s, "Wonka").await?;
            let refused = tool(
                &f,
                token,
                "review_batch",
                json!({ "ids": [pending], "action": "merge" }),
            )
            .await?;
            ensure_refused(&refused, code)?;
            anyhow::ensure!(f.review_row(pending).await?.0 == "pending");
            let refused =
                tool(&f, token, "review_revert", json!({ "batch_id": mcp_batch })).await?;
            ensure_refused(&refused, code)?;
            let (status, body) = rest_unit_revert(&f, token, f.kb, mcp_batch).await?;
            anyhow::ensure!(
                status == StatusCode::FORBIDDEN
                    && body["code"] == code
                    && refused["structuredContent"]["message"] == body["error"],
                "{status} {body} {refused}"
            );
        }
        anyhow::ensure!(merge_is_live(&f, merge).await?);

        // `review_revert` 要的是正好一个：两个都给、都不给，都不执行
        for args in [
            json!({ "merge_id": merge, "batch_id": mcp_batch }),
            json!({}),
            json!({ "batch_id": "not a uuid" }),
        ] {
            let refused = tool(&f, &editor_write, "review_revert", args.clone()).await?;
            anyhow::ensure!(refused["isError"] == true, "{args}: {refused}");
        }
        anyhow::ensure!(merge_is_live(&f, merge).await?, "a bad call reverted");
        // 认不出的号：`not_found`，REST 上是 404
        let refused = tool(
            &f,
            &editor_write,
            "review_revert",
            json!({ "batch_id": unknown }),
        )
        .await?;
        ensure_refused(&refused, "not_found")?;

        // 带 `batch_id` 的 `review_revert`：整批撤回，回的是 REST 那个 body
        let undone = structured(
            &tool(
                &f,
                &editor_write,
                "review_revert",
                json!({ "batch_id": mcp_batch, "rationale": "wrong batch" }),
            )
            .await?,
        )?;
        ensure_revert_shape(&undone, mcp_batch, &[merge])?;
        anyhow::ensure!(undone["reverted"] == 1, "{undone}");
        ensure_reverted_item(&outcomes(&undone)[0])?;
        anyhow::ensure!(merge_is_undone(&f, merge).await?);
        // 再来一次，两个入口回的 body 逐字相等
        let again = structured(
            &tool(
                &f,
                &editor_write,
                "review_revert",
                json!({ "batch_id": mcp_batch }),
            )
            .await?,
        )?;
        let (status, rest_again) = rest_unit_revert(&f, &editor_write, f.kb, mcp_batch).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {rest_again}");
        anyhow::ensure!(again == rest_again, "{again}\n{rest_again}");
        ensure_already_reverted_item(&outcomes(&again)[0])?;

        // 一次合并的 `review_revert` 照旧
        let rest_merge = ensure_applied_item(&outcomes(&over_rest)[0])?.1;
        let single = structured(
            &tool(
                &f,
                &editor_write,
                "review_revert",
                json!({ "merge_id": rest_merge }),
            )
            .await?,
        )?;
        anyhow::ensure!(
            single["outcome"] == "applied" && single["merge_id"] == json!(rest_merge),
            "{single}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}
