//! 登录的编辑者答令牌 agent 的提议，并撤回令牌 agent 的合并（0076）。
//!
//! 要钉住的行为：
//!
//! - **提议看得见**（SC3）：Agent 一档里是一行 `source: "token"`、带理由、没有 `state`；
//!   重复项那一档里，这一对的卡片上多一个 `agent_proposal`
//! - **批准是人的合并**（SC3 / N4 / E8）：合并的 `merged_by` 是这个人，审核行是这个人裁的、
//!   `decided_token` 为空，审计行 `review.merge` 的 actor 是这个人、detail 带 `agent_proposal`；
//!   闸门留下的那一对照样合——人的路径不问闸门
//! - **拒绝什么都不动；一次请求答多条**（SC3 / E10）：1 到 500 条，逐条一个结果，0 或 501 条
//!   整个拒绝
//! - **答过的提议是 `already_decided`**（E8）；**目标已经没法裁的提议关成 `superseded`**（E9）
//! - **没有哪枚令牌答得了提议**（SC4 / N5 / E11）：两条路由对每一枚个人令牌都是 401，
//!   MCP 没有答提议的工具
//! - **令牌撤人或治理 agent 的合并只是提议，保险丝不动；人批准了才按人的撤回算**（SC6 / E15）
//! - **人撤回令牌 agent 的合并，图回到合并之前**（SC7，INTENT C1 的比法）；撤回之后那一行
//!   台账读作 `undone`，令牌再撤是 `already_decided`（E16）
//! - **令牌 agent 的合并在合并列表里带着 `agent`**（E17）；别的行一个键不多
//!
//! 全部经路由进去。夹具在 `review_session_unchanged_tests.rs`。种子直接插表或走 store
//! 函数，不走模型任务。没有 `UTOPIA_DATABASE_URL` 时跳过。自建自拆，只碰自己建的库。
//!
//! 没钉的一件事：人的撤回与令牌的撤回**同时**打到同一次合并上，两边都可能跑进没改过的
//! `resolution::revert_merge`。E16 说的是先后发生的情形，这里只钉先后。
use axum::http::StatusCode;
use serde_json::{json, Value};
use utopia_store::names::NameSource;
use utopia_store::token_agent_actions::TokenAgentAction;
use uuid::Uuid;

use super::review_session_unchanged_tests::Fx;

const TOKEN_NAME: &str = "cleanup agent";

// ───────────────────────── 调用 ─────────────────────────

/// 答一条提议
async fn answer(
    f: &Fx,
    bearer: &str,
    proposal: Uuid,
    action: &str,
) -> anyhow::Result<(StatusCode, Value)> {
    f.call(
        bearer,
        "POST",
        &format!("/kbs/{}/review/agent-proposals/{proposal}", f.kb),
        Some(json!({ "action": action })),
    )
    .await
}

/// 一次答多条
async fn answers(
    f: &Fx,
    bearer: &str,
    ids: &[Uuid],
    action: &str,
) -> anyhow::Result<(StatusCode, Value)> {
    f.call(
        bearer,
        "POST",
        &format!("/kbs/{}/review/agent-proposals", f.kb),
        Some(json!({ "ids": ids, "action": action })),
    )
    .await
}

/// 一档的条目（一页 200 条，够这里的种子）
async fn queue(f: &Fx, bearer: &str, name: &str) -> anyhow::Result<Vec<Value>> {
    let (status, body) = f
        .call(
            bearer,
            "GET",
            &format!("/kbs/{}/review?queue={name}&limit=200", f.kb),
            None,
        )
        .await?;
    anyhow::ensure!(status == StatusCode::OK, "queue {name}: {status} {body}");
    Ok(body["items"].as_array().cloned().unwrap_or_default())
}

fn item(items: &[Value], id: Uuid) -> anyhow::Result<&Value> {
    items
        .iter()
        .find(|row| row["id"] == json!(id))
        .ok_or_else(|| anyhow::anyhow!("no row {id} in {items:?}"))
}

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

async fn rest_queue_merge(
    f: &Fx,
    bearer: &str,
    review: Uuid,
) -> anyhow::Result<(StatusCode, Value)> {
    f.call(
        bearer,
        "POST",
        &format!("/kbs/{}/review/{review}", f.kb),
        Some(json!({ "action": "merge" })),
    )
    .await
}

async fn rest_revert(f: &Fx, bearer: &str, merge_id: Uuid) -> anyhow::Result<(StatusCode, Value)> {
    f.call(
        bearer,
        "POST",
        &format!("/kbs/{}/merges/{merge_id}/revert", f.kb),
        None,
    )
    .await
}

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

/// 一个留成提议的令牌回答：200、`held` 或 `proposed`、理由。回提议的 id
fn proposed(
    (status, body): &(StatusCode, Value),
    outcome: &str,
    reason: &str,
) -> anyhow::Result<Uuid> {
    anyhow::ensure!(*status == StatusCode::OK, "a proposal: {status} {body}");
    anyhow::ensure!(
        body["outcome"] == outcome && body["reason"] == reason,
        "expected {outcome} / {reason}: {body}"
    );
    uuid_at(body, "action_id")
}

/// 一个落地的令牌回答。回 (台账行, 合并)
fn applied((status, body): &(StatusCode, Value)) -> anyhow::Result<(Uuid, Uuid)> {
    anyhow::ensure!(
        *status == StatusCode::OK,
        "an applied call: {status} {body}"
    );
    anyhow::ensure!(body["outcome"] == "applied", "{body}");
    Ok((uuid_at(body, "action_id")?, uuid_at(body, "merge_id")?))
}

/// 批准的回答：正好 `ok`、`state`、`merge_id`。回合并
fn approved((status, body): &(StatusCode, Value)) -> anyhow::Result<Uuid> {
    anyhow::ensure!(*status == StatusCode::OK, "an approval: {status} {body}");
    anyhow::ensure!(keys(body) == ["merge_id", "ok", "state"], "{body}");
    anyhow::ensure!(body["ok"] == true && body["state"] == "approved", "{body}");
    uuid_at(body, "merge_id")
}

/// 没做合并的回答（`declined` / `superseded`）：正好 `ok` 与 `state`
fn closed_as((status, body): &(StatusCode, Value), state: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        *status == StatusCode::OK,
        "expected {state}: {status} {body}"
    );
    anyhow::ensure!(keys(body) == ["ok", "state"], "{body}");
    anyhow::ensure!(body["ok"] == true && body["state"] == state, "{body}");
    Ok(())
}

fn already_decided((status, body): &(StatusCode, Value)) -> anyhow::Result<()> {
    anyhow::ensure!(
        *status == StatusCode::CONFLICT && body["code"] == "already_decided",
        "expected 409 already_decided: {status} {body}"
    );
    Ok(())
}

// ───────────────────────── 读库 ─────────────────────────

async fn ledger(f: &Fx) -> anyhow::Result<Vec<TokenAgentAction>> {
    let mut rows = utopia_store::token_agent_actions::list(&f.pool, f.kb, 2000, 0).await?;
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

/// 这条提议还开着：没有 `state`，没有答的人
async fn ensure_open(f: &Fx, proposal: Uuid) -> anyhow::Result<()> {
    let row = ledger_row(f, proposal).await?;
    anyhow::ensure!(
        row.state.is_none() && row.state_at.is_none() && row.state_by.is_none(),
        "the proposal stays open: {row:?}"
    );
    Ok(())
}

/// 这条提议关上了：状态、答的人
async fn ensure_closed(f: &Fx, proposal: Uuid, state: &str) -> anyhow::Result<TokenAgentAction> {
    let row = ledger_row(f, proposal).await?;
    anyhow::ensure!(
        row.state.as_deref() == Some(state)
            && row.state_at.is_some()
            && row.state_by == Some(f.editor),
        "expected {state} by the editor: {row:?}"
    );
    anyhow::ensure!(
        row.outcome == "held" || row.outcome == "proposed",
        "the outcome of a proposal is not rewritten: {row:?}"
    );
    Ok(row)
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

/// 一次合并：源、目标、谁合的、是否已经撤回
async fn merge_row(f: &Fx, merge_id: Uuid) -> anyhow::Result<(Uuid, Uuid, Option<Uuid>, bool)> {
    Ok(sqlx::query_as(
        "SELECT source_id, target_id, merged_by, reverted_at IS NOT NULL
           FROM entity_merges WHERE id = $1",
    )
    .bind(merge_id)
    .fetch_one(&f.pool)
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

/// 「什么都没动」比的东西：这个库的实体、事实、证据、类型化事实的来源、审核行、
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

/// 两个公司，各有一条事实，其中一条带证据；闸门不拦。回 (左, 右)
async fn passing(f: &Fx, s: &Seeds, name: &str) -> anyhow::Result<(Uuid, Uuid)> {
    let a = f.entity(f.company_type, name).await?;
    let c = f.entity(f.company_type, &format!("{name} Corp")).await?;
    let based = fact(f, a, s.based_in, s.city).await?;
    fact(f, c, s.part_of, s.city).await?;
    sqlx::query(
        "INSERT INTO fact_evidence (fact_id, chunk_id, quote, document_id)
         SELECT $1, $2, 'based in Port Ellen', document_id FROM chunks WHERE id = $2",
    )
    .bind(based)
    .bind(chunk(f).await?)
    .execute(&f.pool)
    .await?;
    Ok((a, c))
}

/// 闸门会留下的一对：人把这两个实体分开过（裁的是反过来的那一对），之后它们又进了队列。
/// 回 (等着的审核行, 左, 右)
async fn held_pair(f: &Fx, s: &Seeds, name: &str) -> anyhow::Result<(Uuid, Uuid, Uuid)> {
    let (a, c) = passing(f, s, name).await?;
    let earlier = f.pair_of(c, a).await?;
    let (status, body) = f
        .call(
            &f.editor_session,
            "POST",
            &format!("/kbs/{}/review/{earlier}", f.kb),
            Some(json!({ "action": "keep", "rationale": "two companies" })),
        )
        .await?;
    anyhow::ensure!(status == StatusCode::OK, "a person's keep: {status} {body}");
    Ok((f.pair_of(a, c).await?, a, c))
}

/// 一条队列合并的提议：闸门留下的一对，令牌在队列里合它。回 (提议, 审核行, 左, 右)
async fn queue_proposal(
    f: &Fx,
    s: &Seeds,
    token: &str,
    name: &str,
) -> anyhow::Result<(Uuid, Uuid, Uuid, Uuid)> {
    let (review, a, c) = held_pair(f, s, name).await?;
    let proposal = proposed(
        &rest_queue_merge(f, token, review).await?,
        "held",
        "kept_by_person",
    )?;
    anyhow::ensure!(f.review_row(review).await?.0 == "pending");
    Ok((proposal, review, a, c))
}

/// 令牌 agent 的一次合并：闸门不拦的一对，经令牌的手动合并落地。回 (台账行, 合并, 源, 目标)
async fn token_merge(
    f: &Fx,
    s: &Seeds,
    token: &str,
    name: &str,
) -> anyhow::Result<(Uuid, Uuid, Uuid, Uuid)> {
    let (source, target) = passing(f, s, name).await?;
    let (action_id, merge_id) = applied(&rest_manual(f, token, source, target).await?)?;
    anyhow::ensure!(merged_into(f, source).await? == Some(target));
    Ok((action_id, merge_id, source, target))
}

// ───────────────────────── SC3：看见提议，批准，合并是人的 ─────────────────────────

#[tokio::test]
async fn an_editor_sees_a_proposal_in_agent_history_and_on_the_pairs_item_approves_it_and_the_merge_is_the_persons(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let s = seeds(&f).await?;
        let (token_id, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let (_, reader) = f.token(f.viewer, "reader", "read", None).await?;

        // 队列合并的提议，和另一对上手动合并的提议（令牌给的方向是右并进左）
        let (proposal, review, a, c) = queue_proposal(&f, &s, &token, "Acme").await?;
        let (review2, a2, c2) = held_pair(&f, &s, "Borealis").await?;
        let manual = proposed(
            &rest_manual(&f, &token, c2, a2).await?,
            "held",
            "kept_by_person",
        )?;
        // 第三对：没有提议
        let (pa, pc) = passing(&f, &s, "Cygnus").await?;
        let plain = f.pair_of(pa, pc).await?;

        // Agent 一档：令牌的一行，带理由，没有 state
        let agent = queue(&f, &f.editor_session, "agent").await?;
        let row = item(&agent, proposal)?;
        anyhow::ensure!(
            row["source"] == "token"
                && row["operation"] == "review.merge"
                && row["outcome"] == "held"
                && row["reason"] == "kept_by_person"
                && row["owner_name"] == "Editor"
                && row["token_name"] == TOKEN_NAME,
            "{row}"
        );
        anyhow::ensure!(
            row.get("state").is_none(),
            "an open proposal has no state: {row}"
        );
        let manual_row = item(&agent, manual)?;
        anyhow::ensure!(
            manual_row["operation"] == "merge.manual" && manual_row.get("state").is_none(),
            "{manual_row}"
        );

        // 重复项一档：这一对的卡片上挂着提议；会话与令牌读到的是同一页
        let duplicates = queue(&f, &f.editor_session, "duplicates").await?;
        anyhow::ensure!(
            duplicates == queue(&f, &reader, "duplicates").await?,
            "a token reads the same items"
        );
        let card = &item(&duplicates, review)?["agent_proposal"];
        anyhow::ensure!(
            keys(card)
                == [
                    "created_at",
                    "id",
                    "operation",
                    "owner_name",
                    "reason",
                    "token_name"
                ],
            "{card}"
        );
        anyhow::ensure!(
            card["id"] == json!(proposal)
                && card["operation"] == "review.merge"
                && card["reason"] == "kept_by_person"
                && card["owner_name"] == "Editor"
                && card["token_name"] == TOKEN_NAME
                && card["created_at"] == row["created_at"],
            "{card}"
        );
        // 手动合并的提议按两个实体挂，方向不论
        let card2 = &item(&duplicates, review2)?["agent_proposal"];
        anyhow::ensure!(
            card2["id"] == json!(manual) && card2["operation"] == "merge.manual",
            "{card2}"
        );
        // 没有提议的卡片一个键不多
        let untouched = item(&duplicates, plain)?;
        anyhow::ensure!(
            keys(untouched)
                == [
                    "created_at",
                    "id",
                    "left",
                    "proposal",
                    "reason",
                    "right",
                    "score",
                    "stage"
                ],
            "{untouched}"
        );

        // 批准队列合并的提议：闸门留下的这一对照样合——人的路径不问闸门
        let merge_id = approved(&answer(&f, &f.editor_session, proposal, "approve").await?)?;
        let (source, target, merged_by, reverted) = merge_row(&f, merge_id).await?;
        anyhow::ensure!(
            merged_by == Some(f.editor) && !reverted,
            "the merge is the person's"
        );
        anyhow::ensure!(
            (source == a && target == c) || (source == c && target == a),
            "the merge is of the proposed pair"
        );
        anyhow::ensure!(merged_into(&f, source).await? == Some(target));
        anyhow::ensure!(
            f.review_row(review).await? == ("merged".to_string(), Some(f.editor), None),
            "the review row is decided by the person, with no token"
        );
        let audits = f.audits("review.merge", review).await?;
        anyhow::ensure!(audits.len() == 1, "{audits:?}");
        let (actor, detail) = &audits[0];
        anyhow::ensure!(
            *actor == Some(f.editor),
            "the person is the actor: {audits:?}"
        );
        anyhow::ensure!(
            detail["agent_proposal"] == json!(proposal),
            "the audit detail names the proposal: {detail}"
        );
        anyhow::ensure!(
            detail.get("token_id").is_none() && detail.get("token_name").is_none(),
            "an approved proposal is not recorded as the agent's act: {detail}"
        );
        // 台账：提议关成 approved，指着批准时做出的合并；不多一行
        let closed = ensure_closed(&f, proposal, "approved").await?;
        anyhow::ensure!(
            closed.merge_id == Some(merge_id) && closed.token_id == token_id,
            "{closed:?}"
        );
        anyhow::ensure!(ledger(&f).await?.len() == 2, "no new ledger row");

        // 读回来：Agent 一档里这一行是 approved；卡片离开了重复项；合并列表里是人的合并
        let agent = queue(&f, &f.editor_session, "agent").await?;
        anyhow::ensure!(item(&agent, proposal)?["state"] == "approved");
        let duplicates = queue(&f, &f.editor_session, "duplicates").await?;
        anyhow::ensure!(
            item(&duplicates, review).is_err(),
            "the pair left the queue"
        );
        let merges = queue(&f, &f.editor_session, "merges").await?;
        let listed = item(&merges, merge_id)?;
        anyhow::ensure!(
            listed["merged_by_name"] == "Editor" && listed.get("agent").is_none(),
            "a merge a person made by approving a proposal reads as the person's: {listed}"
        );

        // 批准手动合并的提议：方向是提议存下的那一个，合并也是人的
        let manual_merge = approved(&answer(&f, &f.editor_session, manual, "approve").await?)?;
        anyhow::ensure!(
            merge_row(&f, manual_merge).await? == (c2, a2, Some(f.editor), false),
            "the stored direction, merged by the person"
        );
        let audits: Vec<_> = audit_rows(&f, "merge.manual")
            .await?
            .into_iter()
            .filter(|row| row.2 == Some(manual_merge))
            .collect();
        anyhow::ensure!(audits.len() == 1, "{audits:?}");
        anyhow::ensure!(
            audits[0].0 == Some(f.editor)
                && audits[0].3["agent_proposal"] == json!(manual)
                && audits[0].3.get("token_id").is_none(),
            "{audits:?}"
        );
        anyhow::ensure!(
            ensure_closed(&f, manual, "approved").await?.merge_id == Some(manual_merge)
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── SC3：拒绝什么都不动；一次答多条 ─────────────────────────

#[tokio::test]
async fn a_declined_proposal_changes_nothing_and_several_proposals_are_answered_in_one_request(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let s = seeds(&f).await?;
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let (declined, declined_review, _, _) = queue_proposal(&f, &s, &token, "Acme").await?;
        let mut several = Vec::new();
        for name in ["Borealis", "Cygnus", "Draco"] {
            several.push(queue_proposal(&f, &s, &token, name).await?);
        }

        let before = graph(&f).await?;
        for part in ["entities", "facts", "fact_evidence", "resolution_reviews"] {
            anyhow::ensure!(
                before[part].as_array().is_some_and(|rows| !rows.is_empty()),
                "{part} must have rows for the comparison to mean anything"
            );
        }
        closed_as(
            &answer(&f, &f.editor_session, declined, "decline").await?,
            "declined",
        )?;
        ensure_same(&before, &graph(&f).await?, "a declined proposal")?;
        ensure_closed(&f, declined, "declined").await?;
        anyhow::ensure!(f.review_row(declined_review).await?.0 == "pending");
        let audits = audit_rows(&f, "agent.proposal.declined").await?;
        anyhow::ensure!(audits.len() == 1, "{audits:?}");
        let (actor, target_kind, target_id, detail) = &audits[0];
        anyhow::ensure!(
            *actor == Some(f.editor)
                && target_kind == "agent_action"
                && *target_id == Some(declined)
                && detail.get("token_id").is_none(),
            "the person declined it: {audits:?}"
        );
        anyhow::ensure!(audit_rows(&f, "review.merge").await?.is_empty());
        // 卡片还在队列里，上面不再挂提议；Agent 一档里这一行是 declined
        let duplicates = queue(&f, &f.editor_session, "duplicates").await?;
        anyhow::ensure!(
            item(&duplicates, declined_review)?
                .get("agent_proposal")
                .is_none(),
            "a declined proposal is no longer open on the card"
        );
        let agent = queue(&f, &f.editor_session, "agent").await?;
        anyhow::ensure!(item(&agent, declined)?["state"] == "declined");

        // 一次请求批准三条
        let ids: Vec<Uuid> = several.iter().map(|p| p.0).collect();
        let (status, body) = answers(&f, &f.editor_session, &ids, "approve").await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        anyhow::ensure!(keys(&body) == ["answered", "outcomes"], "{body}");
        anyhow::ensure!(body["answered"] == 3, "{body}");
        let outcomes = body["outcomes"].as_array().cloned().unwrap_or_default();
        anyhow::ensure!(outcomes.len() == 3, "{body}");
        for (outcome, (proposal, review, _, _)) in outcomes.iter().zip(&several) {
            anyhow::ensure!(
                keys(outcome) == ["code", "error", "id", "merge_id", "state"],
                "{outcome}"
            );
            anyhow::ensure!(
                outcome["id"] == json!(proposal)
                    && outcome["state"] == "approved"
                    && outcome["code"].is_null()
                    && outcome["error"].is_null(),
                "{outcome}"
            );
            let merge_id = uuid_at(outcome, "merge_id")?;
            anyhow::ensure!(merge_row(&f, merge_id).await?.2 == Some(f.editor));
            anyhow::ensure!(
                f.review_row(*review).await? == ("merged".to_string(), Some(f.editor), None)
            );
            anyhow::ensure!(
                ensure_closed(&f, *proposal, "approved").await?.merge_id == Some(merge_id)
            );
            let audits = f.audits("review.merge", *review).await?;
            anyhow::ensure!(
                audits.len() == 1 && audits[0].1["agent_proposal"] == json!(proposal),
                "{audits:?}"
            );
        }
        anyhow::ensure!(merge_count(&f).await? == 3);
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── E8：要登录的编辑者；答过的是 already_decided ─────────────────────────

#[tokio::test]
async fn answering_needs_a_signed_in_editor_and_an_answered_proposal_answers_already_decided(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let s = seeds(&f).await?;
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let (first, review, _, _) = queue_proposal(&f, &s, &token, "Acme").await?;
        let (second, second_review, _, _) = queue_proposal(&f, &s, &token, "Borealis").await?;
        // 一行落地的台账行（令牌分开的一对）：它不是提议
        let kept = f.pair("Ada Lovelace", "Ada King").await?;
        let (status, body) = f
            .call(
                &token,
                "POST",
                &format!("/kbs/{}/review/{kept}", f.kb),
                Some(json!({ "action": "keep" })),
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let not_a_proposal = uuid_at(&body, "action_id")?;
        let before = graph(&f).await?;

        // Viewer：403。没登录：401。两条路由都是
        for action in ["approve", "decline"] {
            let (status, body) = answer(&f, &f.viewer_session, first, action).await?;
            anyhow::ensure!(status == StatusCode::FORBIDDEN, "a viewer: {status} {body}");
            let (status, body) = answers(&f, &f.viewer_session, &[first], action).await?;
            anyhow::ensure!(status == StatusCode::FORBIDDEN, "a viewer: {status} {body}");
            let (status, body) = answer(&f, "not-a-session", first, action).await?;
            anyhow::ensure!(status == StatusCode::UNAUTHORIZED, "{status} {body}");
            let (status, body) = answers(&f, "not-a-session", &[first], action).await?;
            anyhow::ensure!(status == StatusCode::UNAUTHORIZED, "{status} {body}");
        }
        // 认不出的 id、不是提议的台账行：404。认不出的动作：422
        for id in [Uuid::now_v7(), not_a_proposal] {
            let (status, body) = answer(&f, &f.editor_session, id, "approve").await?;
            anyhow::ensure!(status == StatusCode::NOT_FOUND, "{status} {body}");
        }
        let (status, body) = answer(&f, &f.editor_session, first, "merge").await?;
        anyhow::ensure!(
            status == StatusCode::UNPROCESSABLE_ENTITY && body["code"] == "unknown_action",
            "{status} {body}"
        );
        // 这个库的编辑者在别的库名下答：那个库里没有这条提议
        let (status, body) = f
            .call(
                &f.editor_session,
                "POST",
                &format!("/kbs/{}/review/agent-proposals/{first}", f.other_kb),
                Some(json!({ "action": "approve" })),
            )
            .await?;
        anyhow::ensure!(status == StatusCode::NOT_FOUND, "{status} {body}");
        ensure_same(&before, &graph(&f).await?, "refused answers")?;
        ensure_open(&f, first).await?;
        ensure_open(&f, second).await?;
        anyhow::ensure!(ledger_row(&f, not_a_proposal).await?.state.is_none());

        // 编辑者批准：合了。再答（批准、拒绝、放进批量）：already_decided，什么都不动
        let merge_id = approved(&answer(&f, &f.editor_session, first, "approve").await?)?;
        let after = graph(&f).await?;
        let row = serde_json::to_value(ledger_row(&f, first).await?)?;
        already_decided(&answer(&f, &f.editor_session, first, "approve").await?)?;
        already_decided(&answer(&f, &f.editor_session, first, "decline").await?)?;
        let (status, body) = answers(&f, &f.editor_session, &[first], "decline").await?;
        anyhow::ensure!(
            status == StatusCode::OK
                && body["answered"] == 0
                && body["outcomes"][0]["code"] == "already_decided"
                && body["outcomes"][0]["state"].is_null(),
            "{status} {body}"
        );
        ensure_same(&after, &graph(&f).await?, "answering an answered proposal")?;
        anyhow::ensure!(row == serde_json::to_value(ledger_row(&f, first).await?)?);
        anyhow::ensure!(merge_count(&f).await? == 1 && !merge_row(&f, merge_id).await?.3);
        anyhow::ensure!(f.audits("review.merge", review).await?.len() == 1);

        // 拒绝过的再批准：already_decided，没有合并
        closed_as(
            &answer(&f, &f.editor_session, second, "decline").await?,
            "declined",
        )?;
        already_decided(&answer(&f, &f.editor_session, second, "approve").await?)?;
        ensure_closed(&f, second, "declined").await?;
        anyhow::ensure!(f.review_row(second_review).await?.0 == "pending");
        anyhow::ensure!(merge_count(&f).await? == 1);
        anyhow::ensure!(audit_rows(&f, "agent.proposal.declined").await?.len() == 1);
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── E9：目标没法裁了，关成 superseded ─────────────────────────

#[tokio::test]
async fn approving_a_proposal_whose_target_can_no_longer_be_decided_closes_it_superseded_and_merges_nothing(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let s = seeds(&f).await?;
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;

        // 一、这一对在提议之后被人裁了（分开）
        let (decided, decided_review, _, _) = queue_proposal(&f, &s, &token, "Acme").await?;
        let (status, body) = f
            .call(
                &f.editor_session,
                "POST",
                &format!("/kbs/{}/review/{decided_review}", f.kb),
                Some(json!({ "action": "keep" })),
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");

        // 二、队列提议的一侧被并进了别的实体
        let (absorbed, _, a, _) = queue_proposal(&f, &s, &token, "Borealis").await?;
        let elsewhere = f.entity(f.company_type, "Borealis Holdings").await?;
        let (status, body) = rest_manual(&f, &f.editor_session, a, elsewhere).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");

        // 三、手动合并的提议，源被并进了别的实体
        let (_, a3, c3) = held_pair(&f, &s, "Cygnus").await?;
        let manual = proposed(
            &rest_manual(&f, &token, c3, a3).await?,
            "held",
            "kept_by_person",
        )?;
        let elsewhere = f.entity(f.company_type, "Cygnus Holdings").await?;
        let (status, body) = rest_manual(&f, &f.editor_session, c3, elsewhere).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");

        // 四、撤回的提议，那次合并已经被人撤回了
        let (pa, pc) = passing(&f, &s, "Draco").await?;
        let (status, body) = rest_manual(&f, &f.editor_session, pa, pc).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let by_person = uuid_at(&body, "merge_id")?;
        let revert = proposed(
            &rest_revert(&f, &token, by_person).await?,
            "proposed",
            "merge_by_person",
        )?;
        let (status, body) = rest_revert(&f, &f.editor_session, by_person).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");

        let before = graph(&f).await?;
        let merges = merge_count(&f).await?;
        let reverts = audit_rows(&f, "merge.revert").await?.len();
        // 前三条单条答，第四条放进批量：都说 superseded，什么都没做
        for proposal in [decided, absorbed, manual] {
            closed_as(
                &answer(&f, &f.editor_session, proposal, "approve").await?,
                "superseded",
            )?;
            ensure_closed(&f, proposal, "superseded").await?;
        }
        let (status, body) = answers(&f, &f.editor_session, &[revert], "approve").await?;
        anyhow::ensure!(
            status == StatusCode::OK
                && body["answered"] == 1
                && body["outcomes"][0]["state"] == "superseded"
                && body["outcomes"][0]["merge_id"].is_null()
                && body["outcomes"][0]["code"].is_null(),
            "{status} {body}"
        );
        ensure_closed(&f, revert, "superseded").await?;

        ensure_same(&before, &graph(&f).await?, "superseded proposals")?;
        anyhow::ensure!(merge_count(&f).await? == merges, "no merge was applied");
        anyhow::ensure!(audit_rows(&f, "merge.revert").await?.len() == reverts);
        anyhow::ensure!(audit_rows(&f, "review.merge").await?.is_empty());
        let audits = audit_rows(&f, "agent.proposal.superseded").await?;
        anyhow::ensure!(audits.len() == 4, "{audits:?}");
        for (actor, target_kind, target_id, _) in &audits {
            anyhow::ensure!(
                *actor == Some(f.editor)
                    && target_kind == "agent_action"
                    && [decided, absorbed, manual, revert].contains(&target_id.unwrap_or_default()),
                "{audits:?}"
            );
        }
        // 关上了就是答过了
        already_decided(&answer(&f, &f.editor_session, decided, "approve").await?)?;
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── E10：1 到 500 条；0 或 501 条整个拒绝 ─────────────────────────

#[tokio::test]
async fn a_bulk_answer_takes_1_to_500_ids_with_one_outcome_each_and_refuses_0_or_501_as_a_whole(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let s = seeds(&f).await?;
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let mut open = Vec::new();
        for name in ["Acme", "Borealis", "Cygnus", "Draco"] {
            open.push(queue_proposal(&f, &s, &token, name).await?.0);
        }
        let before = graph(&f).await?;

        // 0 条、501 条（开头是真的提议）、认不出的动作：整个拒绝，一条都没答
        let (status, body) = answers(&f, &f.editor_session, &[], "approve").await?;
        anyhow::ensure!(
            status == StatusCode::UNPROCESSABLE_ENTITY && body["code"] == "batch_size",
            "0 ids: {status} {body}"
        );
        let mut too_many = open.clone();
        too_many.resize_with(501, Uuid::now_v7);
        for action in ["approve", "decline"] {
            let (status, body) = answers(&f, &f.editor_session, &too_many, action).await?;
            anyhow::ensure!(
                status == StatusCode::UNPROCESSABLE_ENTITY && body["code"] == "batch_size",
                "501 ids: {status} {body}"
            );
        }
        let (status, body) = answers(&f, &f.editor_session, &open, "merge").await?;
        anyhow::ensure!(
            status == StatusCode::UNPROCESSABLE_ENTITY && body["code"] == "unknown_action",
            "{status} {body}"
        );
        ensure_same(&before, &graph(&f).await?, "refused bulk answers")?;
        for proposal in &open {
            ensure_open(&f, *proposal).await?;
        }
        anyhow::ensure!(merge_count(&f).await? == 0);

        // 1 条
        let (status, body) = answers(&f, &f.editor_session, &open[..1], "decline").await?;
        anyhow::ensure!(
            status == StatusCode::OK
                && body["answered"] == 1
                && body["outcomes"].as_array().is_some_and(|o| o.len() == 1)
                && body["outcomes"][0]["id"] == json!(open[0])
                && body["outcomes"][0]["state"] == "declined",
            "1 id: {status} {body}"
        );

        // 500 条：两条真的夹在认不出的 id 中间，其中一条给了两次；一条一个结果，次序是请求的
        let mut ids: Vec<Uuid> = std::iter::repeat_with(Uuid::now_v7).take(500).collect();
        ids[7] = open[1];
        ids[250] = open[2];
        ids[499] = open[1];
        let (status, body) = answers(&f, &f.editor_session, &ids, "approve").await?;
        anyhow::ensure!(status == StatusCode::OK, "500 ids: {status}");
        let outcomes = body["outcomes"].as_array().cloned().unwrap_or_default();
        anyhow::ensure!(
            outcomes.len() == 500,
            "one outcome per id: {}",
            outcomes.len()
        );
        anyhow::ensure!(body["answered"] == 2, "{}", body["answered"]);
        for (i, (outcome, id)) in outcomes.iter().zip(&ids).enumerate() {
            anyhow::ensure!(
                keys(outcome) == ["code", "error", "id", "merge_id", "state"],
                "{outcome}"
            );
            anyhow::ensure!(
                outcome["id"] == json!(id),
                "request order at {i}: {outcome}"
            );
            match i {
                7 | 250 => anyhow::ensure!(
                    outcome["state"] == "approved"
                        && outcome["merge_id"].is_string()
                        && outcome["code"].is_null()
                        && outcome["error"].is_null(),
                    "{outcome}"
                ),
                // 第二次给的同一条：它那时已经答过了
                499 => anyhow::ensure!(
                    outcome["code"] == "already_decided"
                        && outcome["state"].is_null()
                        && outcome["merge_id"].is_null()
                        && outcome["error"].is_string(),
                    "{outcome}"
                ),
                _ => anyhow::ensure!(
                    outcome["code"] == "not_found"
                        && outcome["state"].is_null()
                        && outcome["merge_id"].is_null()
                        && outcome["error"].is_string(),
                    "{outcome}"
                ),
            }
        }
        ensure_closed(&f, open[1], "approved").await?;
        ensure_closed(&f, open[2], "approved").await?;
        ensure_open(&f, open[3]).await?;
        anyhow::ensure!(merge_count(&f).await? == 2);
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── SC4 / E11：令牌答不了提议 ─────────────────────────

#[tokio::test]
async fn every_token_gets_401_on_the_proposal_answer_and_mcp_offers_no_tool_that_answers_one(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let s = seeds(&f).await?;
        let (_, own) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let (_, other_write) = f.token(f.editor, "second agent", "write", None).await?;
        let (_, editor_read) = f.token(f.editor, "reader", "read", None).await?;
        let (_, viewer_write) = f.token(f.viewer, "viewer writer", "write", None).await?;
        let (proposal, review, _, _) = queue_proposal(&f, &s, &own, "Acme").await?;
        let before = graph(&f).await?;
        let rows = ledger(&f).await?.len();

        for (who, token) in [
            ("the proposal's own token", &own),
            ("another write token of the editor", &other_write),
            ("a read token of the editor", &editor_read),
            ("a write token of a viewer", &viewer_write),
        ] {
            for action in ["approve", "decline"] {
                let (status, body) = answer(&f, token, proposal, action).await?;
                anyhow::ensure!(
                    status == StatusCode::UNAUTHORIZED,
                    "{who}, single {action}: {status} {body}"
                );
                let (status, body) = answers(&f, token, &[proposal], action).await?;
                anyhow::ensure!(
                    status == StatusCode::UNAUTHORIZED,
                    "{who}, bulk {action}: {status} {body}"
                );
            }
        }
        ensure_open(&f, proposal).await?;

        // MCP：写令牌看到的工具里没有一个答提议的；编出来的名字是「没有这个工具」
        let (status, body) = rpc(&f, &own, "tools/list", json!({})).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let tools = body["result"]["tools"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        anyhow::ensure!(
            tools.iter().any(|t| t["name"] == "review_merge"),
            "the write tools are listed: {body}"
        );
        for tool in &tools {
            let name = tool["name"].as_str().unwrap_or_default();
            anyhow::ensure!(
                !["proposal", "approve", "decline", "answer"]
                    .iter()
                    .any(|word| name.contains(word)),
                "no tool answers a proposal: {name}"
            );
        }
        for name in [
            "review_approve",
            "review_decline",
            "review_answer",
            "review_proposal_answer",
            "agent_proposal_answer",
        ] {
            for action in ["approve", "decline"] {
                let (_, body) = rpc(
                    &f,
                    &own,
                    "tools/call",
                    json!({ "name": name,
                            "arguments": { "id": proposal, "action": action } }),
                )
                .await?;
                anyhow::ensure!(
                    body["error"]["code"] == -32601 && body.get("result").is_none(),
                    "{name} is an unknown tool: {body}"
                );
            }
        }

        ensure_open(&f, proposal).await?;
        ensure_same(&before, &graph(&f).await?, "token answers")?;
        anyhow::ensure!(ledger(&f).await?.len() == rows);
        anyhow::ensure!(f.review_row(review).await?.0 == "pending");
        anyhow::ensure!(audit_rows(&f, "agent.proposal.declined").await?.is_empty());
        anyhow::ensure!(audit_rows(&f, "review.merge").await?.is_empty());
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── SC6 / E15：令牌撤人或治理 agent 的合并，保险丝不动 ─────────────────────────

/// 治理 agent 自己裁的一次合并：没有 `merged_by`，带一行 `applied` 的决定指着它——
/// 保险丝数的就是这种行被人撤回的次数。它的落地步骤就是这样调的，不走模型任务
async fn governor_merge(f: &Fx, s: &Seeds, name: &str) -> anyhow::Result<(Uuid, Uuid)> {
    let (a, c) = passing(f, s, name).await?;
    let review = f.pair_of(a, c).await?;
    let merge_id =
        utopia_store::resolution::merge_entities(&f.pool, f.kb, a, c, None, "governor").await?;
    utopia_store::resolution::close_review_auto(&f.pool, review, "merged", "governor").await?;
    let decision = utopia_store::governance::record(
        &f.pool,
        f.kb,
        utopia_store::governance::NewDecision {
            run_id: Uuid::now_v7(),
            target_id: review,
            action: "merge",
            confidence: 0.97,
            reason: None,
            precedents: json!([]),
            status: "applied",
            merge_id: Some(merge_id),
            question: None,
            trace: json!([]),
            calls: 0,
        },
    )
    .await?;
    Ok((merge_id, decision))
}

/// 保险丝读的东西：窗口里被人撤回的治理决定有几条、开关开没开、每一行治理决定的状态
async fn fuse_reading(f: &Fx) -> anyhow::Result<(i64, bool, Vec<(Uuid, String)>)> {
    let since = chrono::Utc::now() - chrono::Duration::days(1);
    let reverts = utopia_store::governance::reverts_since(&f.pool, f.kb, since).await?;
    let on = utopia_store::kbs::get(&f.pool, f.kb).await?.governance;
    let decisions = sqlx::query_as(
        "SELECT id, status FROM agent_decisions WHERE kb_id = $1 ORDER BY created_at, id",
    )
    .bind(f.kb)
    .fetch_all(&f.pool)
    .await?;
    Ok((reverts, on, decisions))
}

#[tokio::test]
async fn a_token_revert_of_a_persons_or_the_governors_merge_is_a_proposal_that_moves_no_fuse_until_a_person_approves_it(
) -> anyhow::Result<()> {
    // 治理开着：保险丝只在开着的库上数
    let Some(f) = Fx::new(true).await? else {
        return Ok(());
    };
    let result = async {
        let s = seeds(&f).await?;
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;

        // 人做的合并，两种：在队列里点的；批准令牌的提议做出的
        let (pa, pc) = passing(&f, &s, "Acme").await?;
        let review = f.pair_of(pa, pc).await?;
        let (status, body) = rest_queue_merge(&f, &f.editor_session, review).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let by_person = utopia_store::token_agent_actions::merge_between(&f.pool, f.kb, pa, pc)
            .await?
            .ok_or_else(|| anyhow::anyhow!("the person's merge"))?;
        let (merge_proposal, _, _, _) = queue_proposal(&f, &s, &token, "Borealis").await?;
        let by_approval =
            approved(&answer(&f, &f.editor_session, merge_proposal, "approve").await?)?;
        // 治理 agent 做的合并，两次
        let (by_governor, decision) = governor_merge(&f, &s, "Cygnus").await?;
        let (by_governor_2, decision_2) = governor_merge(&f, &s, "Draco").await?;

        let before = graph(&f).await?;
        let fuse = fuse_reading(&f).await?;
        anyhow::ensure!(
            fuse.0 == 0 && fuse.1 && fuse.2.iter().all(|d| d.1 == "applied"),
            "{fuse:?}"
        );

        // 令牌撤这四次合并：都只是提议；再来一次是 already_proposed。什么都没撤，保险丝没动
        let mut proposals = Vec::new();
        for (merge_id, reason) in [
            (by_person, "merge_by_person"),
            (by_approval, "merge_by_person"),
            (by_governor, "merge_by_governor"),
            (by_governor_2, "merge_by_governor"),
        ] {
            let proposal = proposed(
                &rest_revert(&f, &token, merge_id).await?,
                "proposed",
                reason,
            )?;
            let repeat = rest_revert(&f, &token, merge_id).await?;
            anyhow::ensure!(
                proposed(&repeat, "proposed", reason)? == proposal
                    && repeat.1["code"] == "already_proposed",
                "a repeat: {repeat:?}"
            );
            anyhow::ensure!(!merge_row(&f, merge_id).await?.3, "nothing was reverted");
            ensure_open(&f, proposal).await?;
            proposals.push(proposal);
        }
        ensure_same(&before, &graph(&f).await?, "propose-only reverts")?;
        anyhow::ensure!(
            fuse_reading(&f).await? == fuse,
            "the governance fuse reads the same before and after"
        );
        anyhow::ensure!(audit_rows(&f, "merge.revert").await?.is_empty());
        // 合并列表里，这几行挂着撤回的提议
        let merges = queue(&f, &f.editor_session, "merges").await?;
        let listed = &item(&merges, by_governor)?["agent_proposal"];
        anyhow::ensure!(
            listed["id"] == json!(proposals[2])
                && listed["operation"] == "merge.revert"
                && listed["reason"] == "merge_by_governor",
            "{listed}"
        );

        // 人批准撤自己的合并：撤回是这个人的；撤的不是治理 agent 的合并，保险丝照旧不数
        for (proposal, merge_id) in [(proposals[0], by_person), (proposals[1], by_approval)] {
            let reverted = approved(&answer(&f, &f.editor_session, proposal, "approve").await?)?;
            anyhow::ensure!(reverted == merge_id, "the answer names the reverted merge");
            anyhow::ensure!(merge_row(&f, merge_id).await?.3, "the merge is reverted");
            ensure_closed(&f, proposal, "approved").await?;
            let audits: Vec<_> = audit_rows(&f, "merge.revert")
                .await?
                .into_iter()
                .filter(|row| row.2 == Some(merge_id))
                .collect();
            anyhow::ensure!(audits.len() == 1, "{audits:?}");
            anyhow::ensure!(
                audits[0].0 == Some(f.editor)
                    && audits[0].3["agent_proposal"] == json!(proposal)
                    && audits[0].3.get("token_id").is_none(),
                "the revert runs as the person's: {audits:?}"
            );
        }
        anyhow::ensure!(
            fuse_reading(&f).await? == fuse,
            "a person's own merge is no fuse count"
        );

        // 人批准撤治理 agent 的合并：保险丝照今天那样数——第一次数一，第二次到二、开关关掉
        approved(&answer(&f, &f.editor_session, proposals[2], "approve").await?)?;
        let (reverts, on, decisions) = fuse_reading(&f).await?;
        anyhow::ensure!(
            reverts == 1 && on,
            "one revert of the governor's merge counts once"
        );
        anyhow::ensure!(
            decisions.contains(&(decision, "reverted".to_string()))
                && decisions.contains(&(decision_2, "applied".to_string())),
            "{decisions:?}"
        );
        let decided_by: Option<Uuid> =
            sqlx::query_scalar("SELECT decided_by FROM agent_decisions WHERE id = $1")
                .bind(decision)
                .fetch_one(&f.pool)
                .await?;
        anyhow::ensure!(
            decided_by == Some(f.editor),
            "counted as the person's revert"
        );
        approved(&answer(&f, &f.editor_session, proposals[3], "approve").await?)?;
        let (reverts, on, _) = fuse_reading(&f).await?;
        anyhow::ensure!(
            reverts == 2 && !on,
            "the second revert trips the fuse as it does today: {reverts} {on}"
        );
        anyhow::ensure!(
            merge_row(&f, by_governor).await?.3 && merge_row(&f, by_governor_2).await?.3
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── SC7：人撤回令牌 agent 的合并，图回到原样 ─────────────────────────

/// SC7 比的东西（INTENT C1）：`facts` 的每一行、`entities` 的每一行（不含 `updated_at`）、
/// `fact_evidence` 与 `typed_fact_sources` 的每一行
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

/// C1 的比较：`facts`、`entities`（不含 `updated_at`）、`fact_evidence` 逐行相同；
/// `typed_fact_sources` 合并之前的每一行都还在，多出来的**正好**是 `copied` 这一条——
/// 合并把被去重的那条类型化事实的陈述来源抄到了存活事实上，现有的撤回不删它
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

/// 一对有血有肉的重复项，两边都有类型：各有名字事实与证据；各有一条「based in」的类型化
/// 事实，带自己的证据与陈述来源（合并时后记的那条作废，证据与来源抄到存活的那条上）；
/// 被并方是一条事实的主语、另一条的宾语；两边之间有一条互指的事实；画像不同
struct Rich {
    survivor: Uuid,
    absorbed: Uuid,
    review: Uuid,
    /// 合并会抄到存活事实上的那一条陈述来源；现有的撤回不删它（INTENT C1）
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
async fn a_signed_in_editor_reverts_an_agent_merge_and_the_graph_returns_to_its_state(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let s = seeds(&f).await?;
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;

        // 令牌的队列合并与手动合并各一次，都由登录的编辑者从合并历史撤回
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
            let untyped: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM entities WHERE kb_id = $1 AND type_id IS NULL",
            )
            .bind(f.kb)
            .fetch_one(&f.pool)
            .await?;
            anyhow::ensure!(untyped == 0, "the comparison is made on typed entities");

            let merged = if manual {
                rest_manual(&f, &token, rich.absorbed, rich.survivor).await?
            } else {
                rest_queue_merge(&f, &token, rich.review).await?
            };
            let (action_id, merge_id) = applied(&merged)?;
            let (source, target, _, reverted) = merge_row(&f, merge_id).await?;
            anyhow::ensure!(!reverted && merged_into(&f, source).await? == Some(target));
            // 合并真的动了每一部分：否则下面的「回到原样」什么都证明不了
            let during = state(&f).await?;
            for (part, (was, is)) in STATE_PARTS.iter().zip(before.iter().zip(&during)) {
                anyhow::ensure!(
                    was != is,
                    "the merge did not change {part}: the seed is too thin"
                );
            }
            // 合并历史里这一行是令牌 agent 的
            let merges = queue(&f, &f.editor_session, "merges").await?;
            anyhow::ensure!(
                item(&merges, merge_id)?["agent"]["action_id"] == json!(action_id),
                "{merges:?}"
            );

            // 合并历史里的 Revert 按钮调的就是这条路由，不带请求体
            let (status, body) = rest_revert(&f, &f.editor_session, merge_id).await?;
            anyhow::ensure!(
                status == StatusCode::OK && body == json!({ "ok": true }),
                "a person's revert answers as it does today: {status} {body}"
            );
            anyhow::ensure!(merge_row(&f, merge_id).await?.3, "the merge is reverted");
            anyhow::ensure!(merged_into(&f, source).await?.is_none());
            ensure_restored(
                &before,
                &state(&f).await?,
                rich.copied_link,
                "after a token merge and a person's revert",
            )?;
            // 撤回是人的：审计行是会话处理函数今天写的那一行，没有令牌；台账没有 merge.revert 行
            let audits: Vec<_> = audit_rows(&f, "merge.revert")
                .await?
                .into_iter()
                .filter(|row| row.2 == Some(merge_id))
                .collect();
            anyhow::ensure!(audits.len() == 1, "{audits:?}");
            anyhow::ensure!(
                audits[0].0 == Some(f.editor) && keys(&audits[0].3) == ["source", "target"],
                "{audits:?}"
            );
            anyhow::ensure!(
                ledger(&f)
                    .await?
                    .iter()
                    .all(|row| row.operation != "merge.revert"),
                "a person's revert writes no ledger row"
            );
        }
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── E16：人撤回之后台账读作 undone ─────────────────────────

#[tokio::test]
async fn after_a_person_reverts_an_agent_merge_its_ledger_row_reads_undone_and_a_token_revert_answers_already_decided(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let s = seeds(&f).await?;
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let (action_id, merge_id, _, _) = token_merge(&f, &s, &token, "Acme").await?;
        let (kept_action, kept_merge, _, _) = token_merge(&f, &s, &token, "Borealis").await?;

        let agent = queue(&f, &f.editor_session, "agent").await?;
        anyhow::ensure!(
            item(&agent, action_id)?.get("state").is_none(),
            "a live merge has no later state"
        );

        let (status, body) = rest_revert(&f, &f.editor_session, merge_id).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let reverted_at: Option<chrono::DateTime<chrono::Utc>> =
            sqlx::query_scalar("SELECT reverted_at FROM entity_merges WHERE id = $1")
                .bind(merge_id)
                .fetch_one(&f.pool)
                .await?;
        anyhow::ensure!(reverted_at.is_some());

        // 台账读作 undone，时刻是撤回的时刻；表里那一行一个字没写
        let row = ledger_row(&f, action_id).await?;
        anyhow::ensure!(
            row.state.as_deref() == Some("undone") && row.state_at == reverted_at,
            "{row:?}"
        );
        anyhow::ensure!(row.outcome == "applied" && row.merge_id == Some(merge_id));
        let stored: (
            Option<String>,
            Option<chrono::DateTime<chrono::Utc>>,
            Option<Uuid>,
        ) = sqlx::query_as(
            "SELECT state, state_at, state_by FROM token_agent_actions WHERE id = $1",
        )
        .bind(action_id)
        .fetch_one(&f.pool)
        .await?;
        anyhow::ensure!(
            stored == (None, None, None),
            "undone is read, never stored: {stored:?}"
        );
        // Agent 一档里也是 undone；另一次没撤的合并没有 state
        let agent = queue(&f, &f.editor_session, "agent").await?;
        let listed = item(&agent, action_id)?;
        anyhow::ensure!(
            listed["state"] == "undone" && listed["state_at"].is_string(),
            "{listed}"
        );
        anyhow::ensure!(item(&agent, kept_action)?.get("state").is_none());
        anyhow::ensure!(ledger_row(&f, kept_action).await?.state.is_none());

        // 令牌再撤同一次合并：already_decided，REST 与 MCP 都是；什么都没写
        let rows = ledger(&f).await?.len();
        already_decided(&rest_revert(&f, &token, merge_id).await?)?;
        let (_, body) = rpc(
            &f,
            &token,
            "tools/call",
            json!({ "name": "review_revert", "arguments": { "merge_id": merge_id } }),
        )
        .await?;
        anyhow::ensure!(
            body["result"]["isError"] == true
                && body["result"]["structuredContent"]["code"] == "already_decided",
            "{body}"
        );
        anyhow::ensure!(ledger(&f).await?.len() == rows, "no new ledger row");
        anyhow::ensure!(audit_rows(&f, "agent.proposal").await?.is_empty());
        anyhow::ensure!(!merge_row(&f, kept_merge).await?.3);
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── E17：合并列表里的「主人 via 令牌名」 ─────────────────────────

#[tokio::test]
async fn a_token_agents_merge_is_listed_as_owner_via_token_name_and_every_other_merge_row_is_unchanged(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let s = seeds(&f).await?;
        let (token_id, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let (_, reader) = f.token(f.viewer, "reader", "read", None).await?;
        const TODAY: [&str; 7] = [
            "created_at",
            "id",
            "merged_by_name",
            "reason",
            "reverted_at",
            "source_name",
            "target_name",
        ];

        // 四次合并：令牌 agent 的（手动与队列各一）、人的、治理 agent 的、人批准提议做出的
        let (action_id, by_token, _, _) = token_merge(&f, &s, &token, "Acme").await?;
        let (qa, qc) = passing(&f, &s, "Borealis").await?;
        let queued = f.pair_of(qa, qc).await?;
        let (queue_action, by_token_queue) = applied(&rest_queue_merge(&f, &token, queued).await?)?;
        let (pa, pc) = passing(&f, &s, "Cygnus").await?;
        let (status, body) = rest_manual(&f, &f.editor_session, pa, pc).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let by_person = uuid_at(&body, "merge_id")?;
        let (ga, gc) = passing(&f, &s, "Draco").await?;
        let by_governor =
            utopia_store::resolution::merge_entities(&f.pool, f.kb, ga, gc, None, "governor")
                .await?;
        let (proposal, _, _, _) = queue_proposal(&f, &s, &token, "Eridanus").await?;
        let by_approval = approved(&answer(&f, &f.editor_session, proposal, "approve").await?)?;
        // 令牌之后改了名：列表里仍是合并那一刻的名字
        sqlx::query("UPDATE personal_tokens SET name = 'renamed later' WHERE id = $1")
            .bind(token_id)
            .execute(&f.pool)
            .await?;

        let merges = queue(&f, &f.editor_session, "merges").await?;
        anyhow::ensure!(merges.len() == 5, "{merges:?}");
        // 会话、令牌、MCP 读到的是同一页
        anyhow::ensure!(
            merges == queue(&f, &reader, "merges").await?,
            "a token reads the same merges list"
        );
        let (_, listed) = rpc(
            &f,
            &reader,
            "tools/call",
            json!({ "name": "review_list",
                    "arguments": { "section": "merges", "limit": 200 } }),
        )
        .await?;
        anyhow::ensure!(
            listed["result"]["structuredContent"]["items"] == json!(merges),
            "MCP reads the same merges list: {listed}"
        );

        for (merge_id, action) in [(by_token, action_id), (by_token_queue, queue_action)] {
            let row = item(&merges, merge_id)?;
            let mut expected = TODAY.to_vec();
            expected.push("agent");
            expected.sort_unstable();
            anyhow::ensure!(keys(row) == expected, "{row}");
            anyhow::ensure!(
                row["agent"]
                    == json!({ "action_id": action, "owner_name": "Editor",
                               "token_name": TOKEN_NAME }),
                "owner via token name, from the ledger link: {row}"
            );
            // 合并日志自己的那一列没变：仍是主人的名字
            anyhow::ensure!(row["merged_by_name"] == "Editor", "{row}");
        }
        // 别的行：正好是今天的那七个键
        for (merge_id, merged_by) in [
            (by_person, json!("Editor")),
            (by_governor, Value::Null),
            (by_approval, json!("Editor")),
        ] {
            let row = item(&merges, merge_id)?;
            anyhow::ensure!(
                keys(row) == TODAY,
                "every other merge row is unchanged: {row}"
            );
            anyhow::ensure!(row["merged_by_name"] == merged_by, "{row}");
        }

        // 令牌想撤人的那次合并：那一行挂上提议；人拒绝之后那个键又没了。撤回之后的令牌合并仍带 agent
        let revert = proposed(
            &rest_revert(&f, &token, by_person).await?,
            "proposed",
            "merge_by_person",
        )?;
        let merges = queue(&f, &f.editor_session, "merges").await?;
        let row = item(&merges, by_person)?;
        let mut expected = TODAY.to_vec();
        expected.push("agent_proposal");
        expected.sort_unstable();
        anyhow::ensure!(keys(row) == expected, "{row}");
        let card = &row["agent_proposal"];
        anyhow::ensure!(
            keys(card)
                == [
                    "created_at",
                    "id",
                    "operation",
                    "owner_name",
                    "reason",
                    "token_name"
                ]
                && card["id"] == json!(revert)
                && card["operation"] == "merge.revert"
                && card["reason"] == "merge_by_person"
                && card["owner_name"] == "Editor",
            "{card}"
        );
        closed_as(
            &answer(&f, &f.editor_session, revert, "decline").await?,
            "declined",
        )?;
        let (status, body) = rest_revert(&f, &f.editor_session, by_token).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let merges = queue(&f, &f.editor_session, "merges").await?;
        anyhow::ensure!(keys(item(&merges, by_person)?) == TODAY);
        let row = item(&merges, by_token)?;
        anyhow::ensure!(
            row["agent"]["action_id"] == json!(action_id) && row["reverted_at"].is_string(),
            "{row}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}
