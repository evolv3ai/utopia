//! 令牌确认、拒绝、闭合一条事实：只留提议，人答了才算（0076）。
//!
//! 照的是 `review_token_defect_tests`。要钉住的行为：
//!
//! - **令牌的调用什么都不改**（SC1 / E1）：三条路由都回 200、`proposed`、`propose_only`；
//!   这个库的每一条事实、实体、证据与排着的任务数前后相等，每种操作留一条开着的提议
//! - **重复的、同时到的调用拿回同一条提议**（E2）：`already_proposed`，不写第二行；
//!   同一条事实上确认与拒绝是两种操作，可以同时开着
//! - **请求体与目标照人的路由校验**（E3 / E4）：`bad_precision`、`interval_inverted`；
//!   认不出的、别的库的事实是 404，作废了的事实对拒绝与闭合是 `already_decided`
//! - **人批准之后才落地，而且是这个人的决定**（SC2 / E7 / N2）：事实与这个人自己确认、
//!   拒绝、闭合的一模一样，审计行的 actor 是这个人、detail 里指着提议、不带 `token_id`；
//!   拒绝什么都不改
//! - **目标在等的时候被裁掉了，批准关成 `superseded`**（E8）；确认没有「定了」这一态，
//!   事实还活着就照做
//! - **`review_fact` 与 `review_conflict` 与别的写工具同样列、同样拒**（E19）
//! - **会话的三条路由一个字没变**
//!
//! 夹具在 `review_token_write_support.rs`；种事实与矛盾的几个函数也给
//! `review_token_conflict_tests` 用。没有 `UTOPIA_DATABASE_URL` 时跳过。
use axum::http::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;

use super::review_token_write_support::{
    answer, audits, ensure_tool_refused, keys, ledger, open_proposals, post, row, seeded, snapshot,
    tool_call, tools_list, Audit, Fx,
};

const TOKEN_NAME: &str = "fact agent";

/// 一个声明成状态的单值属性，种进这个库：事实与矛盾都挂在它上面
pub(super) async fn predicate_in(f: &Fx, kb: Uuid) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO relation_types (id, kb_id, key, label, kind, datatype, temporal, functional)
         VALUES ($1, $2, $3, 'status', 'attribute', 'text', 'state', TRUE)",
    )
    .bind(id)
    .bind(kb)
    .bind(format!("status-{}", id.simple()))
    .execute(&f.pool)
    .await?;
    Ok(id)
}

/// 一个新的主语。名字都一样：两条对照的事实的审计快照才逐字相等
pub(super) async fn subject_in(f: &Fx, kb: Uuid) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO entities (id, kb_id, canonical_name) VALUES ($1, $2, 'Lease')")
        .bind(id)
        .bind(kb)
        .execute(&f.pool)
        .await?;
    Ok(id)
}

/// 一条活着的、区间开着的类型化事实，直接种进 `facts`。`from` 是按日的起点，可以没有
pub(super) async fn fact_on(
    f: &Fx,
    kb: Uuid,
    subject: Uuid,
    predicate: Uuid,
    value: &str,
    from: Option<&str>,
) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO facts (id, kb_id, subject_id, predicate_id, object_value, layer, confidence,
                            valid_from, valid_from_precision)
         VALUES ($1, $2, $3, $4, to_jsonb($5::text), 'typed', 0.5,
                 $6::timestamptz, CASE WHEN $6 IS NULL THEN NULL ELSE 'day' END)",
    )
    .bind(id)
    .bind(kb)
    .bind(subject)
    .bind(predicate)
    .bind(value)
    .bind(from)
    .execute(&f.pool)
    .await?;
    Ok(id)
}

/// 一条事实，自己一个主语，2024-01-01 起
async fn fact(f: &Fx, predicate: Uuid) -> anyhow::Result<Uuid> {
    let subject = subject_in(f, f.kb).await?;
    fact_on(f, f.kb, subject, predicate, "open", Some("2024-01-01")).await
}

/// 一条事实的主语名下所有行的样子，不带 id 与时刻：两条对照的事实各走各的路之后，
/// 比的是这个
pub(super) async fn shape(f: &Fx, fact: Uuid) -> anyhow::Result<Value> {
    Ok(sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(jsonb_build_object(
                    'value', t.object_value, 'invalidated', t.invalidated_at IS NOT NULL,
                    'confidence', t.confidence, 'valid_from', t.valid_from,
                    'valid_from_precision', t.valid_from_precision, 'valid_to', t.valid_to,
                    'valid_to_precision', t.valid_to_precision, 'end_derived', t.end_derived,
                    'corrected_ends', t.corrected_ends, 'corrects', t.supersedes IS NOT NULL)
                  ORDER BY t.object_value::text, t.invalidated_at IS NOT NULL, t.valid_from,
                           t.valid_to), '[]'::jsonb)
           FROM facts t
          WHERE t.kb_id = $1
            AND t.subject_id = (SELECT subject_id FROM facts WHERE id = $2)",
    )
    .bind(f.kb)
    .bind(fact)
    .fetch_one(&f.pool)
    .await?)
}

/// 一个留成提议的回答：正好那五个键，`proposed` / `propose_only`。回提议的 id
pub(super) fn proposed(body: &Value) -> anyhow::Result<Uuid> {
    anyhow::ensure!(
        keys(body) == ["action_id", "explanation", "ok", "outcome", "reason"],
        "a first proposal has exactly these keys: {body}"
    );
    anyhow::ensure!(
        body["ok"] == true && body["outcome"] == "proposed" && body["reason"] == "propose_only",
        "{body}"
    );
    anyhow::ensure!(
        body["explanation"]
            .as_str()
            .is_some_and(|said| said.contains("Nothing was changed")),
        "{body}"
    );
    Ok(body["action_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("no action_id in {body}"))?
        .parse()?)
}

/// 闭合的请求体：2024 年 6 月结束
fn closing() -> Value {
    json!({ "valid_to": "2024-06-01T00:00:00Z", "valid_to_precision": "month" })
}

/// 那条路由要的请求体：确认与拒绝没有，闭合是 [`closing`]
fn body_of(action: &str) -> Value {
    if action == "close" {
        closing()
    } else {
        json!({})
    }
}

/// 人的三条路由之一，令牌与会话都从这里进
async fn decide(
    f: &Fx,
    bearer: &str,
    fact: Uuid,
    action: &str,
    body: Value,
) -> anyhow::Result<(StatusCode, Value)> {
    post(f, bearer, &format!("facts/{fact}/{action}"), body).await
}

/// 令牌经 REST 提一条，必须成：回提议的 id
async fn propose(f: &Fx, token: &str, fact: Uuid, action: &str) -> anyhow::Result<Uuid> {
    let (status, body) = decide(f, token, fact, action, body_of(action)).await?;
    anyhow::ensure!(
        status == StatusCode::OK,
        "a {action} proposal: {status} {body}"
    );
    proposed(&body)
}

pub(super) fn audits_of<'a>(rows: &'a [Audit], action: &str) -> Vec<&'a Audit> {
    rows.iter().filter(|a| a.action == action).collect()
}

/// 这条事实上某个动作的审计行
fn fact_audits<'a>(rows: &'a [Audit], action: &str, fact: Uuid) -> Vec<&'a Audit> {
    rows.iter()
        .filter(|a| a.action == action && a.target_id == Some(fact))
        .collect()
}

/// 台账里的这一行
pub(super) async fn ledger_row(
    f: &Fx,
    id: Uuid,
) -> anyhow::Result<utopia_store::token_agent_actions::TokenAgentAction> {
    ledger(f)
        .await?
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| anyhow::anyhow!("no ledger row {id}"))
}

/// 审计 detail 去掉 `agent_proposal` 之后的样子
pub(super) fn without_proposal(detail: &Value) -> Value {
    let mut detail = detail.clone();
    if let Some(fields) = detail.as_object_mut() {
        fields.remove("agent_proposal");
    }
    detail
}

const OPERATIONS: [(&str, &str); 3] = [
    ("confirm", "fact.confirm"),
    ("reject", "fact.reject"),
    ("close", "fact.close"),
];

#[tokio::test]
async fn a_token_fact_confirm_reject_and_close_each_wait_as_a_proposal_and_change_nothing(
) -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (token_id, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let (_, editor_read) = f.token(f.editor, "reader", "read", None).await?;
        let (_, viewer_write) = f.token(f.viewer, "viewer writer", "write", None).await?;
        let elsewhere = [f.other_kb];
        let (_, out_of_scope) = f
            .token(f.editor, "elsewhere", "write", Some(&elsewhere))
            .await?;
        let predicate = predicate_in(&f, f.kb).await?;
        let target = fact(&f, predicate).await?;
        // 作废了的、已经有终点的、别的库的：各种「提不了」
        let retracted = fact(&f, predicate).await?;
        let (status, body) = decide(&f, &f.editor_session, retracted, "reject", json!({})).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let ended = fact(&f, predicate).await?;
        sqlx::query(
            "UPDATE facts SET valid_to = '2024-03-01', valid_to_precision = 'day' WHERE id = $1",
        )
        .bind(ended)
        .execute(&f.pool)
        .await?;
        let foreign_predicate = predicate_in(&f, f.other_kb).await?;
        let foreign_subject = subject_in(&f, f.other_kb).await?;
        let foreign = fact_on(
            &f,
            f.other_kb,
            foreign_subject,
            foreign_predicate,
            "open",
            Some("2024-01-01"),
        )
        .await?;
        let before = snapshot(&f).await?;
        let target_row = row(&f, "facts", target).await?;
        let rows_before = ledger(&f).await?.len();
        let audits_before = audits(&f).await?.len();

        // ── 没过检查的调用：哪一种都一行不写 ──
        // 闭合的请求体解不出来：提取器先答，连只读令牌拿到的也是 422 而不是 403
        for bearer in [&token, &editor_read] {
            let (status, body) = decide(&f, bearer, target, "close", json!({})).await?;
            anyhow::ensure!(
                status == StatusCode::UNPROCESSABLE_ENTITY,
                "a close without valid_to: {status} {body}"
            );
        }
        // 写的检查，次序是 scope、库、角色；每一种带自己的 code，三条路由都一样
        for (who, bearer, code) in [
            ("a read token", &editor_read, "token_read_only"),
            (
                "a token of another base",
                &out_of_scope,
                "token_kb_out_of_scope",
            ),
            (
                "a write token of a viewer",
                &viewer_write,
                "token_owner_below_editor",
            ),
        ] {
            for (action, _) in OPERATIONS {
                let (status, body) = decide(&f, bearer, target, action, body_of(action)).await?;
                anyhow::ensure!(
                    status == StatusCode::FORBIDDEN && body["code"] == code,
                    "{who}, {action}: {status} {body}"
                );
            }
            // 认不出的精度也排在写的检查后面：答的仍是那个 403
            let (status, body) = decide(
                &f,
                bearer,
                target,
                "close",
                json!({ "valid_to": "2024-06-01T00:00:00Z", "valid_to_precision": "week" }),
            )
            .await?;
            anyhow::ensure!(
                status == StatusCode::FORBIDDEN && body["code"] == code,
                "{who}, a bad precision: {status} {body}"
            );
        }
        // 认不出的精度：人的那个 code，排在「目标在不在」前面
        for fact_id in [target, Uuid::now_v7()] {
            let (status, body) = decide(
                &f,
                &token,
                fact_id,
                "close",
                json!({ "valid_to": "2024-06-01T00:00:00Z", "valid_to_precision": "week" }),
            )
            .await?;
            anyhow::ensure!(
                status == StatusCode::UNPROCESSABLE_ENTITY && body["code"] == "bad_precision",
                "a bad precision: {status} {body}"
            );
        }
        // 闭合点早于起点：人的那个 code；要读目标才知道，所以排在「目标在不在」后面
        let (status, body) = decide(
            &f,
            &token,
            target,
            "close",
            json!({ "valid_to": "2023-06-01T00:00:00Z" }),
        )
        .await?;
        anyhow::ensure!(
            status == StatusCode::UNPROCESSABLE_ENTITY && body["code"] == "interval_inverted",
            "a close before the start: {status} {body}"
        );
        // 认不出的事实、别的库的事实：三条路由都是同一个 404
        for fact_id in [Uuid::now_v7(), foreign] {
            for (action, _) in OPERATIONS {
                let (status, body) = decide(&f, &token, fact_id, action, body_of(action)).await?;
                anyhow::ensure!(
                    status == StatusCode::NOT_FOUND,
                    "an unknown fact, {action}: {status} {body}"
                );
            }
        }
        // 作废了的事实：确认没有「定了」这一态，是 404；拒绝与闭合是 already_decided
        let (status, body) = decide(&f, &token, retracted, "confirm", json!({})).await?;
        anyhow::ensure!(status == StatusCode::NOT_FOUND, "{status} {body}");
        for action in ["reject", "close"] {
            let (status, body) = decide(&f, &token, retracted, action, body_of(action)).await?;
            anyhow::ensure!(
                status == StatusCode::CONFLICT && body["code"] == "already_decided",
                "a retracted fact, {action}: {status} {body}"
            );
        }
        // 活着但已经有终点的事实：人的闭合路由答 404，这里也是
        let (status, body) = decide(&f, &token, ended, "close", closing()).await?;
        anyhow::ensure!(status == StatusCode::NOT_FOUND, "{status} {body}");
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before,
            "a refusal wrote a row"
        );
        anyhow::ensure!(
            audits(&f).await?.len() == audits_before,
            "a refusal wrote an audit row"
        );
        anyhow::ensure!(snapshot(&f).await? == before, "a refusal changed the graph");

        // ── 三种操作各提一条：200，什么都没动，各多一条开着的提议 ──
        let mut landed = Vec::new();
        for (action, operation) in OPERATIONS {
            let (status, body) = decide(&f, &token, target, action, body_of(action)).await?;
            anyhow::ensure!(status == StatusCode::OK, "{action}: {status} {body}");
            let proposal = proposed(&body)?;
            anyhow::ensure!(
                snapshot(&f).await? == before,
                "a {action} proposal changed a fact, an entity, evidence or the queued jobs"
            );
            anyhow::ensure!(
                row(&f, "facts", target).await? == target_row,
                "a {action} proposal changed the fact row"
            );
            let open: Vec<_> = open_proposals(&f)
                .await?
                .into_iter()
                .filter(|p| p.target_id == target && p.operation == operation)
                .collect();
            anyhow::ensure!(open.len() == 1, "one open {operation} proposal: {open:?}");
            let p = &open[0];
            anyhow::ensure!(
                p.id == proposal
                    && p.target_kind == "fact"
                    && p.outcome == "proposed"
                    && p.gate_reason.as_deref() == Some("propose_only")
                    && p.owner_id == f.editor
                    && p.token_id == token_id
                    && p.token_name == TOKEN_NAME
                    && p.merge_id.is_none()
                    && p.batch_id.is_none(),
                "{p:?}"
            );
            // payload：请求体原样、目标的那句话、动作；没有理由，事实没有指纹
            anyhow::ensure!(
                keys(&p.payload) == ["action", "request", "target_label"],
                "{}",
                p.payload
            );
            anyhow::ensure!(
                p.payload["request"] == body_of(action)
                    && p.payload["action"] == action
                    && p.payload["target_label"] == "Lease status open",
                "{}",
                p.payload
            );
            landed.push((operation, proposal));
        }
        anyhow::ensure!(ledger(&f).await?.len() == rows_before + 3);
        // 审计：每条提议一行 `agent.proposal`，记在主人名下，认得出是哪枚令牌
        let rows = audits(&f).await?;
        anyhow::ensure!(rows.len() == audits_before + 3, "three audit rows");
        for (operation, proposal) in &landed {
            let said = audits_of(&rows, "agent.proposal");
            let said = said
                .iter()
                .find(|a| a.target_id == Some(*proposal))
                .ok_or_else(|| anyhow::anyhow!("no agent.proposal row for {proposal}"))?;
            anyhow::ensure!(
                said.actor == Some(f.editor)
                    && said.target_kind == "agent_action"
                    && said.detail["operation"] == *operation
                    && said.detail["outcome"] == "proposed"
                    && said.detail["reason"] == "propose_only"
                    && said.detail["token_id"] == json!(token_id)
                    && said.detail["token_name"] == TOKEN_NAME,
                "{said:?}"
            );
        }
        for action in ["fact.confirm", "fact.reject", "fact.close"] {
            anyhow::ensure!(
                fact_audits(&rows, action, target).is_empty(),
                "a proposal is not a {action}"
            );
        }
        // 没给精度的闭合：存下的请求体里就没有那个键
        let bare = fact(&f, predicate).await?;
        let (status, body) = decide(
            &f,
            &token,
            bare,
            "close",
            json!({ "valid_to": "2024-06-01T00:00:00Z" }),
        )
        .await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let stored = ledger_row(&f, proposed(&body)?).await?;
        anyhow::ensure!(
            stored.payload["request"] == json!({ "valid_to": "2024-06-01T00:00:00Z" }),
            "{}",
            stored.payload
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_repeated_or_simultaneous_fact_proposal_returns_the_open_one() -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let (_, second) = f.token(f.editor, "second agent", "write", None).await?;
        let predicate = predicate_in(&f, f.kb).await?;
        let rows_before = ledger(&f).await?.len();

        for (action, operation) in OPERATIONS {
            // 重复：同一条提议加 `already_proposed`；换一枚令牌、换一个闭合日期也是同一个目标
            let repeated = fact(&f, predicate).await?;
            let raced = fact(&f, predicate).await?;
            let before = snapshot(&f).await?;
            let first = propose(&f, &token, repeated, action).await?;
            let audits_then = audits(&f).await?.len();
            let other_body = if action == "close" {
                json!({ "valid_to": "2025-01-01T00:00:00Z" })
            } else {
                json!({})
            };
            for (bearer, body) in [(&token, body_of(action)), (&second, other_body)] {
                let (status, again) = decide(&f, bearer, repeated, action, body).await?;
                anyhow::ensure!(status == StatusCode::OK, "{status} {again}");
                anyhow::ensure!(
                    keys(&again)
                        == [
                            "action_id",
                            "code",
                            "explanation",
                            "ok",
                            "outcome",
                            "reason"
                        ]
                        && again["code"] == "already_proposed"
                        && again["action_id"] == json!(first)
                        && again["outcome"] == "proposed"
                        && again["reason"] == "propose_only",
                    "a repeated {action}: {again}"
                );
            }
            anyhow::ensure!(
                audits(&f).await?.len() == audits_then,
                "a repeated {action} wrote an audit row"
            );
            // 存下的还是第一次的请求
            anyhow::ensure!(
                ledger_row(&f, first).await?.payload["request"] == body_of(action),
                "the first request is the one kept"
            );

            // 同时到的两次：两边拿到同一个 id，台账里只有一行
            let (a, b) = tokio::join!(
                decide(&f, &token, raced, action, body_of(action)),
                decide(&f, &second, raced, action, body_of(action)),
            );
            let (a, b) = (a?, b?);
            anyhow::ensure!(
                a.0 == StatusCode::OK && b.0 == StatusCode::OK,
                "two simultaneous {action} calls: {a:?} {b:?}"
            );
            anyhow::ensure!(
                a.1["action_id"] == b.1["action_id"] && a.1["action_id"].is_string(),
                "one id for both: {a:?} {b:?}"
            );
            let met = [&a.1, &b.1]
                .iter()
                .filter(|body| body["code"] == "already_proposed")
                .count();
            anyhow::ensure!(
                met == 1,
                "exactly one call met the open proposal: {a:?} {b:?}"
            );
            let open: Vec<_> = open_proposals(&f)
                .await?
                .into_iter()
                .filter(|p| p.target_id == raced)
                .collect();
            anyhow::ensure!(
                open.len() == 1 && open[0].operation == operation,
                "one row for two simultaneous calls: {open:?}"
            );
            anyhow::ensure!(
                snapshot(&f).await? == before,
                "repeats and races changed the graph"
            );
        }
        // 三种操作、各两条事实、各一行
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before + 6,
            "one ledger row per operation and target"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn confirm_and_reject_of_one_fact_may_both_be_open() -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let predicate = predicate_in(&f, f.kb).await?;
        let target = fact(&f, predicate).await?;
        let before = snapshot(&f).await?;

        let confirm = propose(&f, &token, target, "confirm").await?;
        let reject = propose(&f, &token, target, "reject").await?;
        let close = propose(&f, &token, target, "close").await?;
        anyhow::ensure!(
            confirm != reject && reject != close && confirm != close,
            "each operation has its own proposal"
        );
        let open: Vec<_> = open_proposals(&f)
            .await?
            .into_iter()
            .filter(|p| p.target_id == target)
            .collect();
        let mut operations: Vec<&str> = open.iter().map(|p| p.operation.as_str()).collect();
        operations.sort_unstable();
        anyhow::ensure!(
            operations == ["fact.close", "fact.confirm", "fact.reject"],
            "all three stay open on one fact: {operations:?}"
        );
        // 一种操作的重复只碰它自己的那一条
        let (status, again) = decide(&f, &token, target, "reject", json!({})).await?;
        anyhow::ensure!(
            status == StatusCode::OK
                && again["code"] == "already_proposed"
                && again["action_id"] == json!(reject),
            "{status} {again}"
        );
        anyhow::ensure!(
            snapshot(&f).await? == before,
            "three proposals changed the graph"
        );

        // 拒绝掉确认的那一条，拒绝的那一条还开着；批准它，事实作废，剩下的闭合提议过时
        let (status, body) = answer(&f, &f.editor_session, confirm, "decline").await?;
        anyhow::ensure!(
            status == StatusCode::OK && body == json!({ "ok": true, "state": "declined" }),
            "{status} {body}"
        );
        anyhow::ensure!(snapshot(&f).await? == before, "a decline changed the graph");
        anyhow::ensure!(
            open_proposals(&f)
                .await?
                .iter()
                .filter(|p| p.target_id == target)
                .count()
                == 2,
            "declining one leaves the other two open"
        );
        let (status, body) = answer(&f, &f.editor_session, reject, "approve").await?;
        anyhow::ensure!(
            status == StatusCode::OK && body == json!({ "ok": true, "state": "approved" }),
            "{status} {body}"
        );
        let (status, body) = answer(&f, &f.editor_session, close, "approve").await?;
        anyhow::ensure!(
            status == StatusCode::OK && body == json!({ "ok": true, "state": "superseded" }),
            "{status} {body}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn approving_a_fact_proposal_changes_the_graph_as_that_persons_decision_would(
) -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (token_id, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let predicate = predicate_in(&f, f.kb).await?;

        for (action, operation) in OPERATIONS {
            let by_person = fact(&f, predicate).await?;
            let by_proposal = fact(&f, predicate).await?;
            let declined = fact(&f, predicate).await?;
            let untouched = shape(&f, declined).await?;
            let proposal = propose(&f, &token, by_proposal, action).await?;
            let declined_proposal = propose(&f, &token, declined, action).await?;
            anyhow::ensure!(
                shape(&f, by_proposal).await? == untouched,
                "a {action} proposal left the fact as it was"
            );

            // 对照：这个人自己做同一个决定
            let (status, body) =
                decide(&f, &f.editor_session, by_person, action, body_of(action)).await?;
            anyhow::ensure!(
                status == StatusCode::OK && body == json!({ "ok": true }),
                "{action}: {status} {body}"
            );
            let mine = shape(&f, by_person).await?;
            anyhow::ensure!(mine != untouched, "the person's {action} changed the fact");

            // 批准：正好 `ok` 与 `state`
            let (status, body) = answer(&f, &f.editor_session, proposal, "approve").await?;
            anyhow::ensure!(
                status == StatusCode::OK && body == json!({ "ok": true, "state": "approved" }),
                "{action}: {status} {body}"
            );
            anyhow::ensure!(
                shape(&f, by_proposal).await? == mine,
                "an approved {action} changes the fact exactly as the person's own {action}\n{}\n{mine}",
                shape(&f, by_proposal).await?
            );

            // 审计：actor 都是这个人；批准的那一行与这个人自己的那一行只差一个
            // `agent_proposal`，不带令牌
            let rows = audits(&f).await?;
            let own = fact_audits(&rows, operation, by_person);
            let approved = fact_audits(&rows, operation, by_proposal);
            anyhow::ensure!(own.len() == 1 && approved.len() == 1, "{own:?} {approved:?}");
            anyhow::ensure!(
                own[0].actor == Some(f.editor) && own[0].detail.get("agent_proposal").is_none(),
                "{:?}",
                own[0]
            );
            anyhow::ensure!(
                approved[0].actor == Some(f.editor)
                    && approved[0].target_kind == "fact"
                    && approved[0].detail["agent_proposal"] == json!(proposal)
                    && approved[0].detail.get("token_id").is_none()
                    && without_proposal(&approved[0].detail) == own[0].detail,
                "the approval is the person's act and names the proposal, with no token: {:?}",
                approved[0]
            );

            // 提议关成 approved：谁答的；提议本身的 outcome 不改写
            let closed = ledger_row(&f, proposal).await?;
            anyhow::ensure!(
                closed.state.as_deref() == Some("approved")
                    && closed.state_by == Some(f.editor)
                    && closed.state_at.is_some()
                    && closed.outcome == "proposed"
                    && closed.token_id == token_id
                    && closed.merge_id.is_none(),
                "{closed:?}"
            );

            // 拒绝：什么都不改，只记一行拒绝
            let before = snapshot(&f).await?;
            let (status, body) =
                answer(&f, &f.editor_session, declined_proposal, "decline").await?;
            anyhow::ensure!(
                status == StatusCode::OK && body == json!({ "ok": true, "state": "declined" }),
                "{action}: {status} {body}"
            );
            anyhow::ensure!(
                snapshot(&f).await? == before && shape(&f, declined).await? == untouched,
                "a declined {action} proposal changed the graph"
            );
            let rows = audits(&f).await?;
            anyhow::ensure!(fact_audits(&rows, operation, declined).is_empty());
            let said: Vec<_> = audits_of(&rows, "agent.proposal.declined")
                .into_iter()
                .filter(|a| a.target_id == Some(declined_proposal))
                .collect();
            anyhow::ensure!(said.len() == 1, "{said:?}");
            anyhow::ensure!(
                said[0].actor == Some(f.editor)
                    && said[0].detail
                        == json!({ "agent_proposal": declined_proposal, "operation": operation,
                                   "reason": "propose_only", "proposed_by_token": TOKEN_NAME }),
                "{:?}",
                said[0]
            );
        }
        // 没有哪一条提议被记成过时
        anyhow::ensure!(audits_of(&audits(&f).await?, "agent.proposal.superseded").is_empty());
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn approving_a_proposal_whose_fact_was_decided_meanwhile_supersedes_it() -> anyhow::Result<()>
{
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let predicate = predicate_in(&f, f.kb).await?;
        // 等着的时候被人拒绝了的事实：拒绝、闭合、确认的提议都过时
        let retracted = fact(&f, predicate).await?;
        let reject = propose(&f, &token, retracted, "reject").await?;
        let close = propose(&f, &token, retracted, "close").await?;
        let confirm = propose(&f, &token, retracted, "confirm").await?;
        // 等着的时候被人闭合了的事实：原来那一行作废了，闭合的提议过时
        let ended = fact(&f, predicate).await?;
        let close_ended = propose(&f, &token, ended, "close").await?;

        let (status, body) = decide(&f, &f.editor_session, retracted, "reject", json!({})).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let (status, body) = decide(&f, &f.editor_session, ended, "close", closing()).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let before = snapshot(&f).await?;
        let audits_before = audits(&f).await?;

        for (what, proposal, operation) in [
            ("a reject of a retracted fact", reject, "fact.reject"),
            ("a close of a retracted fact", close, "fact.close"),
            ("a confirm of a retracted fact", confirm, "fact.confirm"),
            ("a close of a closed fact", close_ended, "fact.close"),
        ] {
            let (status, body) = answer(&f, &f.editor_session, proposal, "approve").await?;
            anyhow::ensure!(
                status == StatusCode::OK && body == json!({ "ok": true, "state": "superseded" }),
                "{what}: {status} {body}"
            );
            anyhow::ensure!(
                snapshot(&f).await? == before,
                "{what}: a superseded proposal changed the graph"
            );
            let closed = ledger_row(&f, proposal).await?;
            anyhow::ensure!(
                closed.state.as_deref() == Some("superseded")
                    && closed.state_by == Some(f.editor)
                    && closed.operation == operation,
                "{what}: {closed:?}"
            );
            // 答过的再答：already_decided
            let (status, body) = answer(&f, &f.editor_session, proposal, "approve").await?;
            anyhow::ensure!(
                status == StatusCode::CONFLICT && body["code"] == "already_decided",
                "{what}, answered twice: {status} {body}"
            );
        }
        // 审计：只多了四行「过时」，记的是这个人；没有哪一行说事实又被裁了一次
        let rows = audits(&f).await?;
        anyhow::ensure!(
            rows.len() == audits_before.len() + 4,
            "four audit rows, one per superseded proposal"
        );
        let superseded = audits_of(&rows, "agent.proposal.superseded");
        anyhow::ensure!(superseded.len() == 4, "{superseded:?}");
        for said in superseded {
            anyhow::ensure!(
                said.actor == Some(f.editor)
                    && said.target_kind == "agent_action"
                    && keys(&said.detail)
                        == ["agent_proposal", "operation", "proposed_by_token", "reason"],
                "{said:?}"
            );
        }
        for action in ["fact.confirm", "fact.reject", "fact.close"] {
            anyhow::ensure!(
                audits_of(&rows, action).len() == audits_of(&audits_before, action).len(),
                "a superseded proposal recorded a {action}"
            );
        }
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_confirm_proposal_applies_while_the_fact_is_live() -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let predicate = predicate_in(&f, f.kb).await?;
        let target = fact(&f, predicate).await?;
        let proposal = propose(&f, &token, target, "confirm").await?;

        // 等着的时候这个人自己确认了这条事实。确认没有「定了」这一态：事实还活着
        let (status, body) = decide(&f, &f.editor_session, target, "confirm", json!({})).await?;
        anyhow::ensure!(
            status == StatusCode::OK && body == json!({ "ok": true }),
            "{status} {body}"
        );
        // 令牌再提一次也还提得了：拿回的是那条开着的提议
        let (status, again) = decide(&f, &token, target, "confirm", json!({})).await?;
        anyhow::ensure!(
            status == StatusCode::OK
                && again["code"] == "already_proposed"
                && again["action_id"] == json!(proposal),
            "{status} {again}"
        );

        // 批准照做，不是过时
        let (status, body) = answer(&f, &f.editor_session, proposal, "approve").await?;
        anyhow::ensure!(
            status == StatusCode::OK && body == json!({ "ok": true, "state": "approved" }),
            "{status} {body}"
        );
        let confirmed = row(&f, "facts", target).await?;
        anyhow::ensure!(
            confirmed["confidence"] == 1.0 && confirmed["invalidated_at"].is_null(),
            "{confirmed}"
        );
        let rows = audits(&f).await?;
        let recorded = fact_audits(&rows, "fact.confirm", target);
        anyhow::ensure!(recorded.len() == 2, "{recorded:?}");
        anyhow::ensure!(
            recorded.iter().all(|a| a.actor == Some(f.editor))
                && recorded[0].detail.get("agent_proposal").is_none()
                && recorded[1].detail["agent_proposal"] == json!(proposal),
            "{recorded:?}"
        );
        anyhow::ensure!(audits_of(&rows, "agent.proposal.superseded").is_empty());
        anyhow::ensure!(
            ledger_row(&f, proposal).await?.state.as_deref() == Some("approved"),
            "the proposal is approved"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn the_mcp_fact_and_conflict_tools_propose_and_change_nothing() -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (token_id, editor_write) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let (_, editor_read) = f.token(f.editor, "reader", "read", None).await?;
        let (_, viewer_read) = f.token(f.viewer, "viewer reader", "read", None).await?;
        let (_, viewer_write) = f.token(f.viewer, "viewer writer", "write", None).await?;
        let predicate = predicate_in(&f, f.kb).await?;
        let target = fact(&f, predicate).await?;
        let retracted = fact(&f, predicate).await?;
        let (status, body) = decide(&f, &f.editor_session, retracted, "reject", json!({})).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let (_, _, conflict) =
            super::review_token_conflict_tests::conflict(&f, predicate, Some("2024-03-01")).await?;
        let (_, _, unstarted) =
            super::review_token_conflict_tests::conflict(&f, predicate, None).await?;
        let (_, _, resolved) =
            super::review_token_conflict_tests::conflict(&f, predicate, Some("2024-03-01")).await?;
        let (status, body) = post(
            &f,
            &f.editor_session,
            &format!("conflicts/{resolved}"),
            json!({ "action": "keep" }),
        )
        .await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let before = snapshot(&f).await?;
        let target_row = row(&f, "facts", target).await?;
        let conflict_row = row(&f, "fact_conflicts", conflict).await?;
        let rows_before = ledger(&f).await?.len();

        // ── 两个工具只列给编辑者的写令牌 ──
        let mut described = Vec::new();
        for (who, token, listed) in [
            ("write token of an editor", &editor_write, true),
            ("read token of an editor", &editor_read, false),
            ("read token of a viewer", &viewer_read, false),
            ("write token of a viewer", &viewer_write, false),
        ] {
            let tools = tools_list(&f, token).await?;
            for name in ["review_fact", "review_conflict"] {
                let tool = tools.iter().find(|t| t["name"] == name).cloned();
                anyhow::ensure!(
                    tool.is_some() == listed,
                    "{who}: {name} listed = {}",
                    tool.is_some()
                );
                if let Some(tool) = tool {
                    described.push(tool);
                }
            }
            anyhow::ensure!(
                tools.iter().any(|t| t["name"] == "review_list"),
                "{who}: the read tools stay listed"
            );
            // 没有哪个工具能答提议，名字里也没有
            for tool in &tools {
                let name = tool["name"].as_str().unwrap_or_default();
                anyhow::ensure!(
                    !["proposal", "approve", "decline", "answer"]
                        .iter()
                        .any(|word| name.contains(word)),
                    "no tool answers a proposal: {name}"
                );
            }
        }
        anyhow::ensure!(described.len() == 2, "{described:?}");
        for tool in &described {
            anyhow::ensure!(
                tool["annotations"]
                    == json!({ "readOnlyHint": false, "destructiveHint": false,
                               "idempotentHint": false }),
                "{tool}"
            );
            let schema = &tool["inputSchema"];
            anyhow::ensure!(
                schema["required"] == json!(["id", "action"])
                    && schema["properties"]["id"]["format"] == "uuid",
                "{schema}"
            );
            // 说明：是提议、什么都不改、谁来答、回什么、重复时怎样；没有答提议的工具
            let description = tool["description"].as_str().unwrap_or_default();
            anyhow::ensure!(
                description.starts_with("Propose that a person")
                    && description.ends_with("No tool answers a proposal."),
                "{description}"
            );
            for said in [
                "changes nothing",
                "a signed-in editor answers in the Review tab",
                "outcome: \"proposed\"",
                "action_id",
                "propose_only",
                "already_proposed",
            ] {
                anyhow::ensure!(description.contains(said), "{said}: {description}");
            }
        }
        // 参数就是路由的 id 与请求体的字段
        anyhow::ensure!(
            keys(&described[0]["inputSchema"]["properties"])
                == ["action", "id", "valid_to", "valid_to_precision"],
            "{}",
            described[0]
        );
        anyhow::ensure!(
            keys(&described[1]["inputSchema"]["properties"])
                == [
                    "action",
                    "close_at",
                    "close_at_precision",
                    "id",
                    "new_from",
                    "new_from_precision"
                ],
            "{}",
            described[1]
        );

        // ── 写不了的调用方调得到这两个名字，回来的是带 REST code 的工具错误 ──
        for (token, code) in [
            (&editor_read, "token_read_only"),
            (&viewer_read, "token_read_only"),
            (&viewer_write, "token_owner_below_editor"),
        ] {
            let refused = tool_call(
                &f,
                token,
                "review_fact",
                json!({ "id": target, "action": "confirm" }),
            )
            .await?;
            ensure_tool_refused(&refused, code)?;
            let (status, body) = decide(&f, token, target, "confirm", json!({})).await?;
            anyhow::ensure!(
                status == StatusCode::FORBIDDEN
                    && body["code"] == code
                    && refused["structuredContent"]["message"] == body["error"],
                "{status} {body} {refused}"
            );
            let refused = tool_call(
                &f,
                token,
                "review_conflict",
                json!({ "id": conflict, "action": "keep" }),
            )
            .await?;
            ensure_tool_refused(&refused, code)?;
            let (status, body) = post(
                &f,
                token,
                &format!("conflicts/{conflict}"),
                json!({ "action": "keep" }),
            )
            .await?;
            anyhow::ensure!(
                status == StatusCode::FORBIDDEN
                    && body["code"] == code
                    && refused["structuredContent"]["message"] == body["error"],
                "{status} {body} {refused}"
            );
        }
        // 操作自己的拒绝：与 REST 同一个 code
        for (name, args, code) in [
            (
                "review_fact",
                json!({ "id": Uuid::now_v7(), "action": "reject" }),
                "not_found",
            ),
            (
                "review_fact",
                json!({ "id": retracted, "action": "confirm" }),
                "not_found",
            ),
            (
                "review_fact",
                json!({ "id": retracted, "action": "reject" }),
                "already_decided",
            ),
            (
                "review_fact",
                json!({ "id": target, "action": "close",
                        "valid_to": "2024-06-01T00:00:00Z", "valid_to_precision": "week" }),
                "bad_precision",
            ),
            (
                "review_fact",
                json!({ "id": target, "action": "close", "valid_to": "2023-06-01T00:00:00Z" }),
                "interval_inverted",
            ),
            (
                "review_fact",
                json!({ "id": target, "action": "delete" }),
                "unknown_action",
            ),
            (
                "review_conflict",
                json!({ "id": Uuid::now_v7(), "action": "keep" }),
                "not_found",
            ),
            (
                "review_conflict",
                json!({ "id": resolved, "action": "keep" }),
                "already_decided",
            ),
            (
                "review_conflict",
                json!({ "id": unstarted, "action": "close" }),
                "close_at_required",
            ),
            (
                "review_conflict",
                json!({ "id": conflict, "action": "close",
                        "close_at": "2023-06-01T00:00:00Z" }),
                "interval_inverted",
            ),
            (
                "review_conflict",
                json!({ "id": conflict, "action": "retime_new" }),
                "new_from_required",
            ),
            (
                "review_conflict",
                json!({ "id": conflict, "action": "keep", "close_at_precision": "week" }),
                "bad_precision",
            ),
        ] {
            let refused = tool_call(&f, &editor_write, name, args.clone()).await?;
            ensure_tool_refused(&refused, code)
                .map_err(|e| anyhow::anyhow!("{name} {args}: {e}"))?;
        }
        // 认不出的矛盾动作：store 给人的那一句，没有 code
        let refused = tool_call(
            &f,
            &editor_write,
            "review_conflict",
            json!({ "id": conflict, "action": "merge" }),
        )
        .await?;
        let (status, over_rest) = post(
            &f,
            &editor_write,
            &format!("conflicts/{conflict}"),
            json!({ "action": "merge" }),
        )
        .await?;
        anyhow::ensure!(
            refused["isError"] == true
                && refused["structuredContent"].get("code").is_none()
                && status == StatusCode::UNPROCESSABLE_ENTITY
                && refused["structuredContent"]["message"] == over_rest["error"],
            "{refused} {status} {over_rest}"
        );
        // 参数没给全、给得不对：没有执行，不带 code
        for (name, args) in [
            ("review_fact", json!({ "id": target })),
            ("review_fact", json!({ "id": target, "action": "close" })),
            (
                "review_fact",
                json!({ "id": target, "action": "close", "valid_to": "next June" }),
            ),
            ("review_conflict", json!({ "id": conflict })),
            (
                "review_conflict",
                json!({ "id": conflict, "action": "close", "close_at": "soon" }),
            ),
        ] {
            let refused = tool_call(&f, &editor_write, name, args.clone()).await?;
            anyhow::ensure!(
                refused["isError"] == true && refused["structuredContent"].get("code").is_none(),
                "{name} {args}: {refused}"
            );
        }
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before && snapshot(&f).await? == before,
            "a refused tool call wrote something"
        );

        // ── 提议：`structuredContent` 就是 REST 的那个回答；什么都没动 ──
        let mut landed = 0;
        for (action, operation) in OPERATIONS {
            let mut args = json!({ "id": target, "action": action });
            if action == "close" {
                args["valid_to"] = json!("2024-06-01T00:00:00Z");
                args["valid_to_precision"] = json!("month");
            }
            let result = tool_call(&f, &editor_write, "review_fact", args.clone()).await?;
            anyhow::ensure!(result["isError"] == false, "{result}");
            let proposal = proposed(&result["structuredContent"])?;
            landed += 1;
            anyhow::ensure!(
                snapshot(&f).await? == before && row(&f, "facts", target).await? == target_row,
                "review_fact {action} changed the graph or the fact row"
            );
            let stored = ledger_row(&f, proposal).await?;
            anyhow::ensure!(
                stored.operation == operation
                    && stored.target_id == target
                    && stored.token_id == token_id
                    && stored.state.is_none()
                    && stored.payload["request"] == body_of(action)
                    && stored.payload["action"] == action,
                "{stored:?}"
            );
            // 重复：两个入口拿回的是同一个回答，逐字相等
            let (status, over_rest) =
                decide(&f, &editor_write, target, action, body_of(action)).await?;
            anyhow::ensure!(
                status == StatusCode::OK
                    && over_rest["code"] == "already_proposed"
                    && over_rest["action_id"] == json!(proposal),
                "{status} {over_rest}"
            );
            let again = tool_call(&f, &editor_write, "review_fact", args).await?;
            anyhow::ensure!(
                again["isError"] == false && again["structuredContent"] == over_rest,
                "the tool answers what the token API answers\n{again}\n{over_rest}"
            );
        }
        let args = json!({ "id": conflict, "action": "close",
                           "close_at": "2024-08-13T00:00:00Z", "close_at_precision": "month" });
        let result = tool_call(&f, &editor_write, "review_conflict", args.clone()).await?;
        anyhow::ensure!(result["isError"] == false, "{result}");
        let proposal = proposed(&result["structuredContent"])?;
        landed += 1;
        anyhow::ensure!(
            snapshot(&f).await? == before
                && row(&f, "fact_conflicts", conflict).await? == conflict_row,
            "review_conflict changed the graph or the conflict row"
        );
        let stored = ledger_row(&f, proposal).await?;
        anyhow::ensure!(
            stored.operation == "conflict.resolve"
                && stored.target_kind == "conflict"
                && stored.target_id == conflict
                && stored.token_id == token_id
                && stored.state.is_none()
                && stored.payload["request"]
                    == json!({ "action": "close", "close_at": "2024-08-13T00:00:00Z",
                               "close_at_precision": "month" })
                && stored.payload["action"] == "close",
            "{stored:?}"
        );
        let (status, over_rest) = post(
            &f,
            &editor_write,
            &format!("conflicts/{conflict}"),
            json!({ "action": "keep" }),
        )
        .await?;
        anyhow::ensure!(
            status == StatusCode::OK
                && over_rest["code"] == "already_proposed"
                && over_rest["action_id"] == json!(proposal),
            "{status} {over_rest}"
        );
        let again = tool_call(&f, &editor_write, "review_conflict", args).await?;
        anyhow::ensure!(
            again["isError"] == false && again["structuredContent"] == over_rest,
            "the tool answers what the token API answers\n{again}\n{over_rest}"
        );
        anyhow::ensure!(ledger(&f).await?.len() == rows_before + landed);
        anyhow::ensure!(
            open_proposals(&f)
                .await?
                .iter()
                .filter(|p| p.target_id == target || p.target_id == conflict)
                .count()
                == landed,
            "every proposal a tool landed is still open"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn approving_a_close_that_changes_nothing_supersedes_the_proposal() -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let predicate = predicate_in(&f, f.kb).await?;
        let target = fact(&f, predicate).await?;
        let proposal = propose(&f, &token, target, "close").await?;

        // 别人的一个事务先锁住这条事实。批准读到它还活着、区间还开着（读不等锁），
        // 走到 store 的 `FOR UPDATE` 才停下；这时那个事务把事实作废并提交，store 醒来
        // 看到的是一条已经作废的行，回「没动」
        let mut tx = f.pool.begin().await?;
        sqlx::query("SELECT id FROM facts WHERE id = $1 FOR UPDATE")
            .bind(target)
            .execute(&mut *tx)
            .await?;
        let holder: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *tx)
            .await?;
        let approving = answer(&f, &f.editor_session, proposal, "approve");
        let retracting = async {
            let mut polls = 0;
            loop {
                // 从另一条连接上问：事务里的 `pg_stat_activity` 是第一次读时的快照
                let waiting: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM pg_stat_activity WHERE $1 = ANY(pg_blocking_pids(pid))",
                )
                .bind(holder)
                .fetch_one(&f.pool)
                .await?;
                if waiting > 0 {
                    break;
                }
                polls += 1;
                anyhow::ensure!(polls < 400, "the approval never reached the row lock");
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
            sqlx::query("UPDATE facts SET invalidated_at = now() WHERE id = $1")
                .bind(target)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            anyhow::Ok(())
        };
        let (answered, retracted) = tokio::join!(approving, retracting);
        let (status, body) = answered?;
        retracted.map_err(|e| anyhow::anyhow!("{e}: the approval answered {status} {body}"))?;
        anyhow::ensure!(
            status == StatusCode::OK && body == json!({ "ok": true, "state": "superseded" }),
            "a close that changed nothing is not approved: {status} {body}"
        );
        // 没有修正行：这个主语名下只有那一条作废了的事实
        let rows = shape(&f, target).await?;
        anyhow::ensure!(
            rows.as_array().map(Vec::len) == Some(1)
                && rows[0]["invalidated"] == true
                && rows[0]["valid_to"].is_null(),
            "{rows}"
        );
        // 审计：没有 `fact.close`，只有一行「过时」；提议关成 superseded
        let rows = audits(&f).await?;
        anyhow::ensure!(
            fact_audits(&rows, "fact.close", target).is_empty(),
            "a close that changed nothing is not recorded as a close"
        );
        let said: Vec<_> = audits_of(&rows, "agent.proposal.superseded")
            .into_iter()
            .filter(|a| a.target_id == Some(proposal))
            .collect();
        anyhow::ensure!(
            said.len() == 1 && said[0].actor == Some(f.editor),
            "{said:?}"
        );
        anyhow::ensure!(
            ledger_row(&f, proposal).await?.state.as_deref() == Some("superseded"),
            "the proposal is superseded"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_session_fact_confirm_reject_and_close_answer_as_before() -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let predicate = predicate_in(&f, f.kb).await?;
        let rows_before = ledger(&f).await?.len();
        for (action, operation) in OPERATIONS {
            let target = fact(&f, predicate).await?;
            // Viewer：不带 code 的 403。认不出的事实：404。哪一种都不记
            let (status, body) =
                decide(&f, &f.viewer_session, target, action, body_of(action)).await?;
            anyhow::ensure!(
                status == StatusCode::FORBIDDEN && body.get("code").is_none(),
                "{action}: {status} {body}"
            );
            let (status, body) = decide(
                &f,
                &f.editor_session,
                Uuid::now_v7(),
                action,
                body_of(action),
            )
            .await?;
            anyhow::ensure!(status == StatusCode::NOT_FOUND, "{action}: {status} {body}");
            anyhow::ensure!(fact_audits(&audits(&f).await?, operation, target).is_empty());

            // 裁决：`{"ok": true}`
            let (status, body) =
                decide(&f, &f.editor_session, target, action, body_of(action)).await?;
            anyhow::ensure!(
                status == StatusCode::OK && body == json!({ "ok": true }),
                "{action}: {status} {body}"
            );
            // 审计：一行，detail 正好是从前的那几个键
            let rows = audits(&f).await?;
            let recorded = fact_audits(&rows, operation, target);
            anyhow::ensure!(recorded.len() == 1, "{recorded:?}");
            let mut expected = json!({ "subject": "Lease", "predicate": "status",
                                       "object": "open", "confidence": 0.5 });
            if action == "close" {
                expected["valid_to"] = json!("2024-06-01T00:00:00+00:00");
                expected["valid_to_precision"] = json!("month");
            }
            anyhow::ensure!(
                recorded[0].actor == Some(f.editor)
                    && recorded[0].target_kind == "fact"
                    && recorded[0].detail == expected,
                "{action}: {:?}",
                recorded[0]
            );
            let after = row(&f, "facts", target).await?;
            match action {
                "confirm" => anyhow::ensure!(
                    after["confidence"] == 1.0 && after["invalidated_at"].is_null(),
                    "{after}"
                ),
                _ => anyhow::ensure!(!after["invalidated_at"].is_null(), "{after}"),
            }
        }
        // 闭合认不出的精度：从前的 code、从前的那一句
        let target = fact(&f, predicate).await?;
        let (status, body) = decide(
            &f,
            &f.editor_session,
            target,
            "close",
            json!({ "valid_to": "2024-06-01T00:00:00Z", "valid_to_precision": "week" }),
        )
        .await?;
        anyhow::ensure!(
            status == StatusCode::UNPROCESSABLE_ENTITY
                && body["code"] == "bad_precision"
                && body["error"]
                    == "A closing date's precision is year, month, day, hour, minute or second.",
            "{status} {body}"
        );
        // 会话的裁决不进令牌 agent 的台账
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before && open_proposals(&f).await?.is_empty(),
            "a session decision writes no ledger row"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}
