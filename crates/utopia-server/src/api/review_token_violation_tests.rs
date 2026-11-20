//! 令牌裁一处公理违规：只留提议，人答了才算（0076）。
//!
//! 照的是 `review_token_conflict_tests`。要钉住的行为：
//!
//! - **令牌的调用什么都不改**（SC1 / E1）：四种出路（撤事实、闭合断言、放宽公理、认可并存）
//!   都回 200、`proposed`、`propose_only`；违规行、这个库的每一条事实与实体、排着的任务数
//!   前后相等，台账里多一条开着的提议
//! - **请求体与目标照人的路由校验**（E3 / E4）：`bad_resolution`、`fact_required`、
//!   `close_at_required`、`bad_precision`、`interval_inverted`，令牌拿到的与会话拿到的是
//!   同一个状态码、同一个 code；认不出的、别的库的违规是 404，裁过的是 `already_decided`；
//!   哪一种都不留提议
//! - **人批准之后才落地，而且是这个人的决定**（SC2 / E7 / N2）：违规行与事实同这个人自己
//!   裁的一模一样，这一次裁决写下的每一行审计的 actor 是这个人、detail 里指着提议、不带
//!   `token_id`；拒绝什么都不改
//! - **人的裁决在批准时报的错原样报出去，提议留着**：不折成 `superseded`
//! - **会话的裁决一个字没变**：同一个响应体、同一行审计、台账一行不写
//!
//! 违规在等的时候被裁掉、它指着的事实被撤掉的那几种过时写在 `review_token_errata_tests`
//! （与勘误的写在一起）；MCP 工具 `review_violation` 的测试写在
//! `review_token_alignment_tests`。种违规的几个函数也给那两个模块用。
//!
//! 撤事实的那条路撤完会重算一遍检查，没算出来的开着的违规随之清掉——所以这里的非对称违规
//! 都是**真的**：种两条互指的事实，跑一遍 store 的检查（不是模型任务）让它自己记下。只有
//! 派生矛盾是直接插进 `axiom_violations` 的，它们在重算之前就用完了。
//!
//! 夹具在 `review_token_write_support.rs`。没有 `UTOPIA_DATABASE_URL` 时跳过。
use axum::http::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;

use super::review_token_fact_tests::{
    audits_of, fact_on, ledger_row, predicate_in, proposed, shape, subject_in, without_proposal,
};
use super::review_token_write_support::{
    answer, audits, keys, ledger, open_proposals, post, row, snapshot, Audit, Fx,
};

const TOKEN_NAME: &str = "violation agent";

/// 一个声明成反对称的关系，种进这个库
pub(super) async fn asymmetric_in(f: &Fx, kb: Uuid) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO relation_types (id, kb_id, key, label, kind, temporal, is_asymmetric)
         VALUES ($1, $2, $3, 'manages', 'relation', 'state', TRUE)",
    )
    .bind(id)
    .bind(kb)
    .bind(format!("manages-{}", id.simple()))
    .execute(&f.pool)
    .await?;
    Ok(id)
}

/// 两个新实体在一个反对称关系上互指：两条活着的事实，直接种进 `facts`。回这两条事实
pub(super) async fn two_way_in(f: &Fx, kb: Uuid, predicate: Uuid) -> anyhow::Result<(Uuid, Uuid)> {
    let (a, b) = (subject_in(f, kb).await?, subject_in(f, kb).await?);
    let mut facts = Vec::new();
    for (subject, object) in [(a, b), (b, a)] {
        let id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO facts (id, kb_id, subject_id, predicate_id, object_id, layer, confidence)
             VALUES ($1, $2, $3, $4, $5, 'typed', 1.0)",
        )
        .bind(id)
        .bind(kb)
        .bind(subject)
        .bind(predicate)
        .bind(object)
        .execute(&f.pool)
        .await?;
        facts.push(id);
    }
    Ok((facts[0], facts[1]))
}

/// `n` 处真的非对称违规：种 `n` 对互指的事实，跑一遍 store 的一致性检查，读回它为每一对
/// 记下的那一行。回违规的 id，次序与种下的次序一致
pub(super) async fn asymmetries(f: &Fx, predicate: Uuid, n: usize) -> anyhow::Result<Vec<Uuid>> {
    let mut pairs = Vec::new();
    for _ in 0..n {
        pairs.push(two_way_in(f, f.kb, predicate).await?);
    }
    utopia_store::reasoning::run(&f.pool, f.kb).await?;
    let mut found = Vec::new();
    for (one, other) in pairs {
        let rows: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM axiom_violations
              WHERE kb_id = $1 AND kind = 'asymmetry' AND status = 'open'
                AND left_fact = ANY($2) AND right_fact = ANY($2)",
        )
        .bind(f.kb)
        .bind(vec![one, other])
        .fetch_all(&f.pool)
        .await?;
        anyhow::ensure!(
            rows.len() == 1,
            "the check records one asymmetry violation for a two-way pair: {rows:?}"
        );
        found.push(rows[0]);
    }
    Ok(found)
}

/// 一处派生矛盾，直接种进 `axiom_violations`：左事实是结论撞上的那条断言（2024-01-01 起、
/// 区间开着），右事实与 `path` 是前提。`predicate` 是一个状态属性
pub(super) async fn derived_in(f: &Fx, kb: Uuid, predicate: Uuid) -> anyhow::Result<Uuid> {
    let against = subject_in(f, kb).await?;
    let against = fact_on(f, kb, against, predicate, "against", Some("2024-01-01")).await?;
    let premise = subject_in(f, kb).await?;
    let premise = fact_on(f, kb, premise, predicate, "premise", None).await?;
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO axiom_violations (id, kb_id, kind, left_fact, right_fact, path)
         VALUES ($1, $2, 'derived_contradiction', $3, $4, ARRAY[$4]::uuid[])",
    )
    .bind(id)
    .bind(kb)
    .bind(against)
    .bind(premise)
    .execute(&f.pool)
    .await?;
    Ok(id)
}

/// 违规行里的一条事实 id
pub(super) fn fact_of(violation: &Value, side: &str) -> anyhow::Result<Uuid> {
    Ok(violation[side]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("no {side} in {violation}"))?
        .parse()?)
}

/// 裁一处违规：人的那条路由，令牌与会话都从这里进
pub(super) async fn decide(
    f: &Fx,
    bearer: &str,
    violation: Uuid,
    body: Value,
) -> anyhow::Result<(StatusCode, Value)> {
    post(f, bearer, &format!("review/violations/{violation}"), body).await
}

/// 令牌经 REST 提一条，必须成：回提议的 id
pub(super) async fn propose(
    f: &Fx,
    token: &str,
    violation: Uuid,
    body: Value,
) -> anyhow::Result<Uuid> {
    let (status, answer) = decide(f, token, violation, body).await?;
    anyhow::ensure!(status == StatusCode::OK, "a proposal: {status} {answer}");
    proposed(&answer)
}

/// 一处违规的两条事实各自主语名下所有行的样子：两处对照的违规各走各的路之后，比的是这个
async fn facts_state(f: &Fx, violation: Uuid) -> anyhow::Result<Value> {
    let stored = row(f, "axiom_violations", violation).await?;
    Ok(json!([
        shape(f, fact_of(&stored, "left_fact")?).await?,
        shape(f, fact_of(&stored, "right_fact")?).await?,
    ]))
}

/// 违规行里人的决定：状态、出路、谁裁的
fn decision_of(violation: &Value) -> Value {
    json!([
        violation["status"],
        violation["resolution"],
        violation["decided_by"]
    ])
}

/// 某个目标上某个动作的审计行
fn audits_on<'a>(rows: &'a [Audit], action: &str, target: Uuid) -> Vec<&'a Audit> {
    rows.iter()
        .filter(|a| a.action == action && a.target_id == Some(target))
        .collect()
}

#[tokio::test]
async fn a_token_violation_decision_waits_as_a_proposal_and_a_person_applies_it(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
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
        let manages = asymmetric_in(&f, f.kb).await?;
        let status_of = predicate_in(&f, f.kb).await?;
        // 十处真的非对称违规：两处给拒绝用，其余每种出路三处（这个人自己裁的、批准的、
        // 拒绝的）；放宽公理只要两处
        let real = asymmetries(&f, manages, 10).await?;
        let (target, resolved) = (real[0], real[1]);
        let (status, body) = decide(
            &f,
            &f.editor_session,
            resolved,
            json!({ "resolution": "accepted" }),
        )
        .await?;
        anyhow::ensure!(
            status == StatusCode::OK && body == json!({ "ok": true }),
            "{status} {body}"
        );
        // 派生矛盾：一处给拒绝用，三处给闭合用，一处给「批准时人的代码报错」用
        let derived = derived_in(&f, f.kb, status_of).await?;
        let closing = [
            derived_in(&f, f.kb, status_of).await?,
            derived_in(&f, f.kb, status_of).await?,
            derived_in(&f, f.kb, status_of).await?,
        ];
        let stuck = derived_in(&f, f.kb, status_of).await?;
        // 别的库里的一处违规
        let foreign_manages = asymmetric_in(&f, f.other_kb).await?;
        let (foreign_left, foreign_right) = two_way_in(&f, f.other_kb, foreign_manages).await?;
        let foreign = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO axiom_violations (id, kb_id, kind, left_fact, right_fact)
             VALUES ($1, $2, 'asymmetry', $3, $4)",
        )
        .bind(foreign)
        .bind(f.other_kb)
        .bind(foreign_left)
        .bind(foreign_right)
        .execute(&f.pool)
        .await?;

        let before = snapshot(&f).await?;
        let target_row = row(&f, "axiom_violations", target).await?;
        let derived_row = row(&f, "axiom_violations", derived).await?;
        let rows_before = ledger(&f).await?.len();
        let audits_before = audits(&f).await?.len();
        let close = json!({ "resolution": "fact_closed", "close_at": "2024-08-13T00:00:00Z",
                            "close_at_precision": "month" });

        // ── 没过检查的调用：哪一种都一行不写 ──
        // 请求体解不出来：提取器先答，连只读令牌拿到的也是 422 而不是 403
        for bearer in [&token, &editor_read] {
            let (status, body) = decide(&f, bearer, target, json!({})).await?;
            anyhow::ensure!(
                status == StatusCode::UNPROCESSABLE_ENTITY,
                "a body without resolution: {status} {body}"
            );
        }
        // 写的检查，次序是 scope、库、角色；认不出的出路也排在它后面
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
            for resolution in ["accepted", "dismissed"] {
                let (status, body) =
                    decide(&f, bearer, target, json!({ "resolution": resolution })).await?;
                anyhow::ensure!(
                    status == StatusCode::FORBIDDEN && body["code"] == code,
                    "{who}, {resolution}: {status} {body}"
                );
            }
        }
        // 人的拒绝：令牌拿到的与会话拿到的是同一个状态码、同一个 code（会话的那句话是
        // 从前的中文，令牌的是英文）。会话的这几次同样什么都不改
        for (what, on, body, code) in [
            (
                "an unknown resolution",
                target,
                json!({ "resolution": "dismissed" }),
                "bad_resolution",
            ),
            (
                "a retraction that names no fact of a two-fact violation",
                target,
                json!({ "resolution": "fact_retracted" }),
                "fact_required",
            ),
            (
                "a retraction that names a fact the violation does not list",
                target,
                json!({ "resolution": "fact_retracted", "fact_id": Uuid::now_v7() }),
                "fact_required",
            ),
            (
                "a close on a violation that is not a derived contradiction",
                target,
                close.clone(),
                "bad_resolution",
            ),
            (
                "a close without a date",
                derived,
                json!({ "resolution": "fact_closed" }),
                "close_at_required",
            ),
            (
                "a close with an unknown precision",
                derived,
                json!({ "resolution": "fact_closed", "close_at": "2024-08-13T00:00:00Z",
                        "close_at_precision": "week" }),
                "bad_precision",
            ),
            (
                "a close before the assertion's start",
                derived,
                json!({ "resolution": "fact_closed", "close_at": "2023-06-01T00:00:00Z" }),
                "interval_inverted",
            ),
        ] {
            let by_token = decide(&f, &token, on, body.clone()).await?;
            let by_person = decide(&f, &f.editor_session, on, body).await?;
            anyhow::ensure!(
                by_token.0 == StatusCode::UNPROCESSABLE_ENTITY && by_token.1["code"] == code,
                "{what}: {by_token:?}"
            );
            anyhow::ensure!(
                by_person.0 == by_token.0 && by_person.1["code"] == by_token.1["code"],
                "{what}: the token gets the person's refusal\n{by_token:?}\n{by_person:?}"
            );
        }
        // 认不出的出路排在「目标在不在」前面；认不出的、别的库的违规是同一个 404
        let (status, body) = decide(
            &f,
            &token,
            Uuid::now_v7(),
            json!({ "resolution": "dismissed" }),
        )
        .await?;
        anyhow::ensure!(
            status == StatusCode::UNPROCESSABLE_ENTITY && body["code"] == "bad_resolution",
            "{status} {body}"
        );
        for on in [Uuid::now_v7(), foreign] {
            let (status, body) = decide(&f, &token, on, json!({ "resolution": "accepted" })).await?;
            anyhow::ensure!(
                status == StatusCode::NOT_FOUND,
                "an unknown violation: {status} {body}"
            );
        }
        // 已经裁过的违规：already_decided
        let (status, body) =
            decide(&f, &token, resolved, json!({ "resolution": "accepted" })).await?;
        anyhow::ensure!(
            status == StatusCode::CONFLICT && body["code"] == "already_decided",
            "a decided violation: {status} {body}"
        );
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before,
            "a refusal wrote a row"
        );
        anyhow::ensure!(
            audits(&f).await?.len() == audits_before,
            "a refusal wrote an audit row"
        );
        anyhow::ensure!(snapshot(&f).await? == before, "a refusal changed the graph");
        anyhow::ensure!(
            row(&f, "axiom_violations", target).await? == target_row
                && row(&f, "axiom_violations", derived).await? == derived_row,
            "a refusal changed a violation row"
        );

        // ── 人的裁决在批准时报错：原样报出去，提议留着，不是 superseded ──
        // 提的时候闭合点在断言的起点之后；等着的时候断言的起点被改到了闭合点之后
        let on_stuck = propose(&f, &token, stuck, close.clone()).await?;
        let stuck_row = row(&f, "axiom_violations", stuck).await?;
        sqlx::query("UPDATE facts SET valid_from = '2025-01-01' WHERE id = $1")
            .bind(fact_of(&stuck_row, "left_fact")?)
            .execute(&f.pool)
            .await?;
        let before = snapshot(&f).await?;
        let audits_before = audits(&f).await?.len();
        let (status, body) = answer(&f, &f.editor_session, on_stuck, "approve").await?;
        anyhow::ensure!(
            status == StatusCode::UNPROCESSABLE_ENTITY && body["code"] == "interval_inverted",
            "the person's error is returned as it is: {status} {body}"
        );
        let waiting = ledger_row(&f, on_stuck).await?;
        anyhow::ensure!(
            waiting.state.is_none() && waiting.state_by.is_none(),
            "the proposal stays open: {waiting:?}"
        );
        anyhow::ensure!(
            snapshot(&f).await? == before
                && row(&f, "axiom_violations", stuck).await? == stuck_row
                && audits(&f).await?.len() == audits_before,
            "a failed approval changed something"
        );
        anyhow::ensure!(audits_of(&audits(&f).await?, "agent.proposal.superseded").is_empty());

        // ── 四种出路：各提一条、什么都没动；这个人批准，落地的与他自己裁的一模一样 ──
        // 撤事实排在最后：它撤完会重算检查，直接插进来的派生矛盾那时已经用完了
        let cases = [
            ("accepted", [real[2], real[3], real[4]]),
            ("axiom_relaxed", [real[5], real[6], real[5]]),
            ("fact_closed", closing),
            ("fact_retracted", [real[7], real[8], real[9]]),
        ];
        let mut approvals = Vec::new();
        for (resolution, [by_person, by_proposal, declined]) in cases {
            // 放宽公理不另做一遍拒绝：前面两种出路已经拒过了
            let declines = declined != by_person;
            // 请求体：撤事实要说撤哪一条，说的是这处违规自己的左事实
            let body_for = |violation: &Value| -> anyhow::Result<Value> {
                Ok(match resolution {
                    "fact_closed" => close.clone(),
                    "fact_retracted" => {
                        json!({ "resolution": resolution,
                                "fact_id": fact_of(violation, "left_fact")? })
                    }
                    _ => json!({ "resolution": resolution }),
                })
            };
            let open_row = row(&f, "axiom_violations", by_proposal).await?;
            let body = body_for(&open_row)?;
            let before = snapshot(&f).await?;
            let untouched = facts_state(&f, by_proposal).await?;

            let (status, answered) = decide(&f, &token, by_proposal, body.clone()).await?;
            anyhow::ensure!(
                status == StatusCode::OK,
                "{resolution}: {status} {answered}"
            );
            let proposal = proposed(&answered)?;
            let declined_proposal = if declines {
                let body = body_for(&row(&f, "axiom_violations", declined).await?)?;
                Some(propose(&f, &token, declined, body).await?)
            } else {
                None
            };
            anyhow::ensure!(
                snapshot(&f).await? == before,
                "a {resolution} proposal changed a fact, an entity, evidence or the queued jobs"
            );
            anyhow::ensure!(
                row(&f, "axiom_violations", by_proposal).await? == open_row,
                "a {resolution} proposal changed the violation row"
            );
            let open: Vec<_> = open_proposals(&f)
                .await?
                .into_iter()
                .filter(|p| p.target_id == by_proposal)
                .collect();
            anyhow::ensure!(open.len() == 1, "one open proposal: {open:?}");
            let p = &open[0];
            anyhow::ensure!(
                p.id == proposal
                    && p.operation == "violation.decide"
                    && p.target_kind == "axiom_violation"
                    && p.outcome == "proposed"
                    && p.gate_reason.as_deref() == Some("propose_only")
                    && p.owner_id == f.editor
                    && p.token_id == token_id
                    && p.token_name == TOKEN_NAME,
                "{p:?}"
            );
            // payload：请求体原样、目标的那句话、动作
            anyhow::ensure!(
                keys(&p.payload) == ["action", "request", "target_label"],
                "{}",
                p.payload
            );
            let kind = open_row["kind"].as_str().unwrap_or_default();
            anyhow::ensure!(
                p.payload["request"] == body
                    && p.payload["action"] == resolution
                    && p.payload["target_label"]
                        .as_str()
                        .is_some_and(|said| said.starts_with(&format!("{kind}: Lease "))),
                "{}",
                p.payload
            );
            // 重复：同一条提议加 `already_proposed`，换一种出路也是同一个目标
            let (status, again) = decide(
                &f,
                &token,
                by_proposal,
                json!({ "resolution": "accepted" }),
            )
            .await?;
            anyhow::ensure!(
                status == StatusCode::OK
                    && again["code"] == "already_proposed"
                    && again["action_id"] == json!(proposal),
                "a repeat: {status} {again}"
            );

            // 对照：这个人自己裁同一种违规。会话的回答、审计行同从前，台账一行不写
            let person_row = row(&f, "axiom_violations", by_person).await?;
            let person_body = body_for(&person_row)?;
            let rows_then = ledger(&f).await?.len();
            let (status, answered) = decide(&f, &f.editor_session, by_person, person_body).await?;
            anyhow::ensure!(
                status == StatusCode::OK && answered == json!({ "ok": true }),
                "{resolution}: {status} {answered}"
            );
            anyhow::ensure!(
                ledger(&f).await?.len() == rows_then,
                "a session decision writes no ledger row"
            );
            let mine = facts_state(&f, by_person).await?;
            let mine_row = row(&f, "axiom_violations", by_person).await?;
            anyhow::ensure!(
                decision_of(&mine_row) == json!(["resolved", resolution, f.editor]),
                "{mine_row}"
            );

            // 批准：正好 `ok` 与 `state`
            let (status, answered) = answer(&f, &f.editor_session, proposal, "approve").await?;
            anyhow::ensure!(
                status == StatusCode::OK && answered == json!({ "ok": true, "state": "approved" }),
                "{resolution}: {status} {answered}"
            );
            let theirs_row = row(&f, "axiom_violations", by_proposal).await?;
            anyhow::ensure!(
                decision_of(&theirs_row) == decision_of(&mine_row)
                    && !theirs_row["decided_at"].is_null(),
                "{theirs_row} / {mine_row}"
            );
            let theirs = facts_state(&f, by_proposal).await?;
            anyhow::ensure!(
                theirs == mine,
                "an approved {resolution} changes the facts exactly as the person's own\n{theirs}\n{mine}"
            );
            match resolution {
                "fact_closed" | "fact_retracted" => {
                    anyhow::ensure!(mine != untouched, "the person's {resolution} changed a fact")
                }
                _ => anyhow::ensure!(mine == untouched, "{resolution} changes no fact"),
            }
            let closed = ledger_row(&f, proposal).await?;
            anyhow::ensure!(
                closed.state.as_deref() == Some("approved")
                    && closed.state_by == Some(f.editor)
                    && closed.outcome == "proposed"
                    && closed.merge_id.is_none(),
                "{closed:?}"
            );

            // 审计：actor 都是这个人，同一个动作名，不带令牌；会话的那一行不指着哪条提议，
            // detail 正好是从前的那一个键
            let rows = audits(&f).await?;
            let own = audits_on(&rows, "violation.decided", by_person);
            let approved = audits_on(&rows, "violation.decided", by_proposal);
            anyhow::ensure!(own.len() == 1 && approved.len() == 1, "{own:?} {approved:?}");
            anyhow::ensure!(
                own[0].actor == Some(f.editor)
                    && own[0].target_kind == "axiom_violation"
                    && own[0].detail == json!({ "resolution": resolution }),
                "a session {resolution} records what it always did: {:?}",
                own[0]
            );
            anyhow::ensure!(
                approved[0].actor == Some(f.editor)
                    && approved[0].target_kind == "axiom_violation"
                    && approved[0].detail
                        == json!({ "resolution": resolution, "agent_proposal": proposal }),
                "the approval is the person's act and names its proposal: {:?}",
                approved[0]
            );
            approvals.push((resolution, proposal, approved[0].detail.clone()));
            // 撤、闭合事实时那条事实自己的审计行：同样是这个人的，同样指着提议
            let moved = match resolution {
                "fact_retracted" => Some("fact.reject"),
                "fact_closed" => Some("fact.close"),
                _ => None,
            };
            if let Some(action) = moved {
                let own = audits_on(&rows, action, fact_of(&person_row, "left_fact")?);
                let approved = audits_on(&rows, action, fact_of(&open_row, "left_fact")?);
                anyhow::ensure!(own.len() == 1 && approved.len() == 1, "{own:?} {approved:?}");
                anyhow::ensure!(
                    own[0].actor == Some(f.editor)
                        && own[0].detail.get("agent_proposal").is_none()
                        && own[0].detail.get("token_id").is_none(),
                    "{:?}",
                    own[0]
                );
                anyhow::ensure!(
                    approved[0].actor == Some(f.editor)
                        && approved[0].detail["agent_proposal"] == json!(proposal)
                        && approved[0].detail.get("token_id").is_none()
                        && without_proposal(&approved[0].detail) == own[0].detail,
                    "the fact's own row is the person's too and names the proposal: {:?}",
                    approved[0]
                );
            } else {
                anyhow::ensure!(
                    audits_of(&rows, "fact.reject").is_empty()
                        && audits_of(&rows, "fact.close").is_empty(),
                    "{resolution} moves no fact"
                );
            }

            // 拒绝：什么都不改
            let Some(declined_proposal) = declined_proposal else {
                continue;
            };
            let before = snapshot(&f).await?;
            let declined_row = row(&f, "axiom_violations", declined).await?;
            let (status, answered) =
                answer(&f, &f.editor_session, declined_proposal, "decline").await?;
            anyhow::ensure!(
                status == StatusCode::OK && answered == json!({ "ok": true, "state": "declined" }),
                "{resolution}: {status} {answered}"
            );
            anyhow::ensure!(
                snapshot(&f).await? == before
                    && row(&f, "axiom_violations", declined).await? == declined_row
                    && declined_row["status"] == "open",
                "a declined {resolution} proposal changed something"
            );
            let rows = audits(&f).await?;
            anyhow::ensure!(audits_on(&rows, "violation.decided", declined).is_empty());
            let said: Vec<_> = audits_of(&rows, "agent.proposal.declined")
                .into_iter()
                .filter(|a| a.target_id == Some(declined_proposal))
                .collect();
            anyhow::ensure!(
                said.len() == 1
                    && said[0].actor == Some(f.editor)
                    && said[0].detail
                        == json!({ "agent_proposal": declined_proposal,
                                   "operation": "violation.decide", "reason": "propose_only",
                                   "proposed_by_token": TOKEN_NAME }),
                "{said:?}"
            );
        }
        anyhow::ensure!(audits_of(&audits(&f).await?, "agent.proposal.superseded").is_empty());
        anyhow::ensure!(approvals.len() == 4, "{approvals:?}");
        // 每一条批准的审计行都指着它自己的提议（N2），而且没有哪一行带着令牌
        for (resolution, proposal, detail) in approvals {
            anyhow::ensure!(
                detail["agent_proposal"] == json!(proposal) && detail.get("token_id").is_none(),
                "an approved {resolution} names its proposal in the audit detail: {detail}"
            );
        }
        // 批准时出过错的那一条还开着：人照样答得了它
        let (status, body) = answer(&f, &f.editor_session, on_stuck, "decline").await?;
        anyhow::ensure!(
            status == StatusCode::OK && body == json!({ "ok": true, "state": "declined" }),
            "{status} {body}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}
