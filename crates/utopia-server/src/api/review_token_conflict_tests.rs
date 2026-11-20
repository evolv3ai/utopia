//! 令牌裁一处时态矛盾：只留提议，人答了才算（0076）。
//!
//! 照的是 `review_token_defect_tests`。要钉住的行为：
//!
//! - **令牌的调用什么都不改**（SC1 / E1）：四种动作（闭合旧事实、重定时新事实、都留着、
//!   拒绝新事实）都回 200、`proposed`、`propose_only`；矛盾行、这个库的每一条事实与实体、
//!   排着的任务数前后相等，台账里多一条开着的提议，带着矛盾当时指着的两条事实
//! - **请求体与目标照人的路由校验**（E3 / E4）：认不出的动作、没给闭合日期、终点早于起点、
//!   认不出的精度，令牌拿到的与会话拿到的是同一个状态码、同一个响应体；哪一种都不留提议
//! - **人批准之后才落地，而且是这个人的决定**（SC2 / E7 / N2）：两条事实与矛盾行同这个人
//!   自己裁的一模一样，审计行的 actor 是这个人、detail 里指着提议、不带 `token_id`
//! - **矛盾被裁掉了、或它指着的事实换了，批准关成 `superseded`**（E8），什么都不改
//! - **会话的裁决一个字没变**：同一个响应体、同一行审计、台账一行不写
//!
//! MCP 工具 `review_conflict` 的测试与 `review_fact` 的写在一起（`review_token_fact_tests`）。
//! 夹具在 `review_token_write_support.rs`。没有 `UTOPIA_DATABASE_URL` 时跳过。
use axum::http::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;

use super::review_token_fact_tests::{
    audits_of, fact_on, ledger_row, predicate_in, proposed, shape, subject_in, without_proposal,
};
use super::review_token_write_support::{
    answer, audits, keys, ledger, open_proposals, post, row, seeded, snapshot, Audit, Fx,
};

const TOKEN_NAME: &str = "conflict agent";

/// 一处开着的时态矛盾，直接种进 `fact_conflicts`：同一个主语、同一个单值属性上的两条活着的
/// 事实。旧的 2024-01-01 起；新的起点是 `new_from`（按日），可以没有。回（旧、新、矛盾）
pub(super) async fn conflict_in(
    f: &Fx,
    kb: Uuid,
    predicate: Uuid,
    new_from: Option<&str>,
) -> anyhow::Result<(Uuid, Uuid, Uuid)> {
    let subject = subject_in(f, kb).await?;
    let old = fact_on(f, kb, subject, predicate, "old", Some("2024-01-01")).await?;
    let new = fact_on(f, kb, subject, predicate, "new", new_from).await?;
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO fact_conflicts (id, kb_id, old_fact_id, new_fact_id, reason)
         VALUES ($1, $2, $3, $4, 'simultaneous')",
    )
    .bind(id)
    .bind(kb)
    .bind(old)
    .bind(new)
    .execute(&f.pool)
    .await?;
    Ok((old, new, id))
}

pub(super) async fn conflict(
    f: &Fx,
    predicate: Uuid,
    new_from: Option<&str>,
) -> anyhow::Result<(Uuid, Uuid, Uuid)> {
    conflict_in(f, f.kb, predicate, new_from).await
}

/// 裁一处矛盾：人的那条路由，令牌与会话都从这里进
async fn decide(
    f: &Fx,
    bearer: &str,
    conflict: Uuid,
    body: Value,
) -> anyhow::Result<(StatusCode, Value)> {
    post(f, bearer, &format!("conflicts/{conflict}"), body).await
}

/// 令牌经 REST 提一条，必须成：回提议的 id
async fn propose(f: &Fx, token: &str, conflict: Uuid, body: Value) -> anyhow::Result<Uuid> {
    let (status, answer) = decide(f, token, conflict, body).await?;
    anyhow::ensure!(status == StatusCode::OK, "a proposal: {status} {answer}");
    proposed(&answer)
}

/// 这处矛盾的裁决审计行（`conflict.close_old`、`conflict.keep_both`……）
fn decided_audits(rows: &[Audit], conflict: Uuid) -> Vec<&Audit> {
    rows.iter()
        .filter(|a| {
            a.target_kind == "conflict"
                && a.target_id == Some(conflict)
                && a.action.starts_with("conflict.")
        })
        .collect()
}

/// 四种动作：请求体、审计行的动作名、矛盾行记下的出路
fn actions() -> [(Value, &'static str, &'static str); 4] {
    [
        (
            json!({ "action": "close", "close_at": "2024-08-13T00:00:00Z",
                    "close_at_precision": "month" }),
            "conflict.close_old",
            "closed",
        ),
        (
            json!({ "action": "retime_new", "new_from": "2024-06-19T14:00:00Z",
                    "new_from_precision": "month" }),
            "conflict.retime_new",
            "retimed_new",
        ),
        (
            json!({ "action": "keep" }),
            "conflict.keep_both",
            "kept_both",
        ),
        (
            json!({ "action": "reject_new" }),
            "conflict.reject_new",
            "rejected_new",
        ),
    ]
}

#[tokio::test]
async fn a_token_conflict_resolution_waits_as_a_proposal_and_a_person_applies_it(
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
        let (_, _, target) = conflict(&f, predicate, Some("2024-03-01")).await?;
        let (_, _, unstarted) = conflict(&f, predicate, None).await?;
        let (_, _, resolved) = conflict(&f, predicate, Some("2024-03-01")).await?;
        let (status, body) =
            decide(&f, &f.editor_session, resolved, json!({ "action": "keep" })).await?;
        anyhow::ensure!(
            status == StatusCode::OK && body == json!({ "ok": true }),
            "{status} {body}"
        );
        let foreign_predicate = predicate_in(&f, f.other_kb).await?;
        let (_, _, foreign) =
            conflict_in(&f, f.other_kb, foreign_predicate, Some("2024-03-01")).await?;
        let before = snapshot(&f).await?;
        let target_row = row(&f, "fact_conflicts", target).await?;
        let rows_before = ledger(&f).await?.len();
        let audits_before = audits(&f).await?.len();

        // ── 没过检查的调用：哪一种都一行不写 ──
        // 请求体解不出来：提取器先答，连只读令牌拿到的也是 422 而不是 403
        for bearer in [&token, &editor_read] {
            let (status, body) = decide(&f, bearer, target, json!({})).await?;
            anyhow::ensure!(
                status == StatusCode::UNPROCESSABLE_ENTITY,
                "a body without action: {status} {body}"
            );
        }
        // 写的检查，次序是 scope、库、角色；认不出的动作也排在它后面
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
            for action in ["keep", "merge"] {
                let (status, body) = decide(&f, bearer, target, json!({ "action": action })).await?;
                anyhow::ensure!(
                    status == StatusCode::FORBIDDEN && body["code"] == code,
                    "{who}, {action}: {status} {body}"
                );
            }
        }
        // 人的拒绝：令牌拿到的与会话拿到的是同一个状态码、同一个响应体。会话的这几次
        // 同样什么都不改（store 的事务整个回滚）
        for (what, on, body, code) in [
            (
                "an unknown action",
                target,
                json!({ "action": "merge" }),
                None,
            ),
            (
                "a close without a date on a new fact that has no start",
                unstarted,
                json!({ "action": "close" }),
                Some("close_at_required"),
            ),
            (
                "a close before the old fact's start",
                target,
                json!({ "action": "close", "close_at": "2023-06-01T00:00:00Z" }),
                Some("interval_inverted"),
            ),
            (
                "an unknown precision",
                target,
                json!({ "action": "keep", "close_at_precision": "week" }),
                Some("bad_precision"),
            ),
            (
                "a retime without a new start",
                target,
                json!({ "action": "retime_new" }),
                Some("new_from_required"),
            ),
        ] {
            let by_token = decide(&f, &token, on, body.clone()).await?;
            let by_person = decide(&f, &f.editor_session, on, body).await?;
            anyhow::ensure!(
                by_token.0 == StatusCode::UNPROCESSABLE_ENTITY
                    && by_token.1.get("code").and_then(Value::as_str) == code,
                "{what}: {by_token:?}"
            );
            anyhow::ensure!(
                by_token == by_person,
                "{what}: the token gets the person's refusal\n{by_token:?}\n{by_person:?}"
            );
        }
        // 认不出的动作排在「目标在不在」前面；认不出的、别的库的矛盾是同一个 404
        let (status, body) = decide(&f, &token, Uuid::now_v7(), json!({ "action": "merge" })).await?;
        anyhow::ensure!(
            status == StatusCode::UNPROCESSABLE_ENTITY,
            "{status} {body}"
        );
        for on in [Uuid::now_v7(), foreign] {
            let (status, body) = decide(&f, &token, on, json!({ "action": "keep" })).await?;
            anyhow::ensure!(
                status == StatusCode::NOT_FOUND,
                "an unknown conflict: {status} {body}"
            );
        }
        // 已经裁过的矛盾：already_decided
        let (status, body) = decide(&f, &token, resolved, json!({ "action": "keep" })).await?;
        anyhow::ensure!(
            status == StatusCode::CONFLICT && body["code"] == "already_decided",
            "a resolved conflict: {status} {body}"
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
        anyhow::ensure!(row(&f, "fact_conflicts", target).await? == target_row);

        // ── 四种动作：各提一条、什么都没动；这个人批准，落地的与他自己裁的一模一样 ──
        let mut approvals = Vec::new();
        for (body, event, resolution) in actions() {
            let action = body["action"].as_str().unwrap_or_default().to_string();
            let (person_old, _, by_person) = conflict(&f, predicate, Some("2024-03-01")).await?;
            let (old, new, by_proposal) = conflict(&f, predicate, Some("2024-03-01")).await?;
            let (declined_old, _, declined) = conflict(&f, predicate, Some("2024-03-01")).await?;
            let before = snapshot(&f).await?;
            let open_row = row(&f, "fact_conflicts", by_proposal).await?;
            let untouched = shape(&f, old).await?;

            let (status, answered) = decide(&f, &token, by_proposal, body.clone()).await?;
            anyhow::ensure!(status == StatusCode::OK, "{action}: {status} {answered}");
            let proposal = proposed(&answered)?;
            let declined_proposal = propose(&f, &token, declined, body.clone()).await?;
            anyhow::ensure!(
                snapshot(&f).await? == before,
                "a {action} proposal changed a fact, an entity, evidence or the queued jobs"
            );
            anyhow::ensure!(
                row(&f, "fact_conflicts", by_proposal).await? == open_row,
                "a {action} proposal changed the conflict row"
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
                    && p.operation == "conflict.resolve"
                    && p.target_kind == "conflict"
                    && p.outcome == "proposed"
                    && p.gate_reason.as_deref() == Some("propose_only")
                    && p.owner_id == f.editor
                    && p.token_id == token_id
                    && p.token_name == TOKEN_NAME,
                "{p:?}"
            );
            // payload：请求体原样、目标的那句话、动作、矛盾当时指着的两条事实
            anyhow::ensure!(
                keys(&p.payload) == ["action", "fingerprint", "request", "target_label"],
                "{}",
                p.payload
            );
            anyhow::ensure!(
                p.payload["request"] == body
                    && p.payload["action"] == action.as_str()
                    && p.payload["fingerprint"]
                        == json!({ "old_fact_id": old, "new_fact_id": new })
                    && p.payload["target_label"] == "Lease status old / Lease status new",
                "{}",
                p.payload
            );
            // 重复：同一条提议加 `already_proposed`，换一个动作也是同一个目标
            let (status, again) =
                decide(&f, &token, by_proposal, json!({ "action": "keep" })).await?;
            anyhow::ensure!(
                status == StatusCode::OK
                    && again["code"] == "already_proposed"
                    && again["action_id"] == json!(proposal),
                "a repeat: {status} {again}"
            );

            // 对照：这个人自己裁同一种矛盾。会话的回答、审计行同从前，台账一行不写
            let rows_then = ledger(&f).await?.len();
            let (status, answered) = decide(&f, &f.editor_session, by_person, body.clone()).await?;
            anyhow::ensure!(
                status == StatusCode::OK && answered == json!({ "ok": true }),
                "{action}: {status} {answered}"
            );
            anyhow::ensure!(
                ledger(&f).await?.len() == rows_then,
                "a session decision writes no ledger row"
            );
            let mine = shape(&f, person_old).await?;
            let mine_row = row(&f, "fact_conflicts", by_person).await?;
            anyhow::ensure!(
                mine_row["status"] == "resolved" && mine_row["resolution"] == resolution,
                "{mine_row}"
            );

            // 批准：正好 `ok` 与 `state`
            let (status, answered) = answer(&f, &f.editor_session, proposal, "approve").await?;
            anyhow::ensure!(
                status == StatusCode::OK
                    && answered == json!({ "ok": true, "state": "approved" }),
                "{action}: {status} {answered}"
            );
            let theirs_row = row(&f, "fact_conflicts", by_proposal).await?;
            anyhow::ensure!(
                theirs_row["status"] == mine_row["status"]
                    && theirs_row["resolution"] == mine_row["resolution"]
                    && !theirs_row["resolved_at"].is_null(),
                "{theirs_row} / {mine_row}"
            );
            anyhow::ensure!(
                shape(&f, old).await? == mine,
                "an approved {action} changes both facts exactly as the person's own {action}\n{}\n{mine}",
                shape(&f, old).await?
            );
            if action != "keep" {
                anyhow::ensure!(mine != untouched, "the person's {action} changed a fact");
            }
            let closed = ledger_row(&f, proposal).await?;
            anyhow::ensure!(
                closed.state.as_deref() == Some("approved")
                    && closed.state_by == Some(f.editor)
                    && closed.outcome == "proposed"
                    && closed.merge_id.is_none(),
                "{closed:?}"
            );

            // 审计：actor 都是这个人，同一个动作名，不带令牌；会话的那一行不指着哪条提议
            let rows = audits(&f).await?;
            let own = decided_audits(&rows, by_person);
            let approved = decided_audits(&rows, by_proposal);
            anyhow::ensure!(own.len() == 1 && approved.len() == 1, "{own:?} {approved:?}");
            anyhow::ensure!(
                own[0].action == event
                    && own[0].actor == Some(f.editor)
                    && own[0].detail.get("agent_proposal").is_none()
                    && own[0].detail.get("token_id").is_none(),
                "{:?}",
                own[0]
            );
            // 会话的那一行 detail 正好是从前的那几个键：store 多出来的入口没给它添东西
            let mut expected = vec![
                "new_fact_id",
                "new_object",
                "new_subject",
                "old_fact_id",
                "old_object",
                "old_subject",
                "predicate",
            ];
            match action.as_str() {
                "close" => expected.extend(["close_at", "close_at_precision", "corrected_fact_id"]),
                "retime_new" => {
                    expected.extend(["new_from", "new_from_precision", "corrected_fact_id"])
                }
                _ => {}
            }
            expected.sort_unstable();
            anyhow::ensure!(
                keys(&own[0].detail) == expected,
                "a session {action} records the keys it always did: {:?}",
                own[0]
            );
            anyhow::ensure!(
                approved[0].action == event
                    && approved[0].actor == Some(f.editor)
                    && approved[0].detail.get("token_id").is_none()
                    && keys(&without_proposal(&approved[0].detail)) == keys(&own[0].detail),
                "the approval is the person's act, with no token: {:?}",
                approved[0]
            );
            approvals.push((action.clone(), proposal, approved[0].detail.clone()));

            // 拒绝：什么都不改
            let before = snapshot(&f).await?;
            let declined_row = row(&f, "fact_conflicts", declined).await?;
            let (status, answered) =
                answer(&f, &f.editor_session, declined_proposal, "decline").await?;
            anyhow::ensure!(
                status == StatusCode::OK
                    && answered == json!({ "ok": true, "state": "declined" }),
                "{action}: {status} {answered}"
            );
            anyhow::ensure!(
                snapshot(&f).await? == before
                    && row(&f, "fact_conflicts", declined).await? == declined_row
                    && declined_row["status"] == "open"
                    && shape(&f, declined_old).await? == untouched,
                "a declined {action} proposal changed something"
            );
            let rows = audits(&f).await?;
            anyhow::ensure!(decided_audits(&rows, declined).is_empty());
            let said: Vec<_> = audits_of(&rows, "agent.proposal.declined")
                .into_iter()
                .filter(|a| a.target_id == Some(declined_proposal))
                .collect();
            anyhow::ensure!(
                said.len() == 1
                    && said[0].actor == Some(f.editor)
                    && said[0].detail
                        == json!({ "agent_proposal": declined_proposal,
                                   "operation": "conflict.resolve", "reason": "propose_only",
                                   "proposed_by_token": TOKEN_NAME }),
                "{said:?}"
            );
        }
        anyhow::ensure!(audits_of(&audits(&f).await?, "agent.proposal.superseded").is_empty());

        // 批准的那一行审计指着它的提议（N2）
        for (action, proposal, detail) in approvals {
            anyhow::ensure!(
                detail["agent_proposal"] == json!(proposal),
                "an approved {action} names its proposal in the audit detail: {detail}"
            );
        }
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_conflict_proposal_whose_facts_changed_is_superseded() -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let predicate = predicate_in(&f, f.kb).await?;
        let keep = json!({ "action": "keep" });

        // 等着的时候被这个人裁掉了
        let (_, _, resolved) = conflict(&f, predicate, Some("2024-03-01")).await?;
        let on_resolved = propose(&f, &token, resolved, keep.clone()).await?;
        let (status, body) = decide(
            &f,
            &f.editor_session,
            resolved,
            json!({ "action": "reject_new" }),
        )
        .await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");

        // 等着的时候新事实被这个人拒绝了：矛盾跟着撤了
        let (_, withdrawn_new, withdrawn) = conflict(&f, predicate, Some("2024-03-01")).await?;
        let on_withdrawn = propose(&f, &token, withdrawn, keep.clone()).await?;
        let (status, body) = post(
            &f,
            &f.editor_session,
            &format!("facts/{withdrawn_new}/reject"),
            json!({}),
        )
        .await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");

        // 等着的时候旧事实被这个人闭合了：矛盾还开着，但指着的已经是修正行，不是提议时的
        // 那两条事实
        let (old, new, drifted) = conflict(&f, predicate, Some("2024-03-01")).await?;
        let on_drifted = propose(&f, &token, drifted, json!({ "action": "reject_new" })).await?;
        let (status, body) = post(
            &f,
            &f.editor_session,
            &format!("facts/{old}/close"),
            json!({ "valid_to": "2025-01-01T00:00:00Z" }),
        )
        .await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let drifted_row = row(&f, "fact_conflicts", drifted).await?;
        anyhow::ensure!(
            drifted_row["status"] == "open"
                && drifted_row["old_fact_id"] != json!(old)
                && drifted_row["new_fact_id"] == json!(new),
            "the conflict stays open and now names the corrected row: {drifted_row}"
        );

        let before = snapshot(&f).await?;
        let audits_before = audits(&f).await?.len();
        for (what, proposal, on) in [
            ("a conflict a person resolved", on_resolved, resolved),
            (
                "a conflict whose new fact was retracted",
                on_withdrawn,
                withdrawn,
            ),
            ("a conflict that names other facts now", on_drifted, drifted),
        ] {
            let conflict_row = row(&f, "fact_conflicts", on).await?;
            let (status, body) = answer(&f, &f.editor_session, proposal, "approve").await?;
            anyhow::ensure!(
                status == StatusCode::OK && body == json!({ "ok": true, "state": "superseded" }),
                "{what}: {status} {body}"
            );
            anyhow::ensure!(
                snapshot(&f).await? == before
                    && row(&f, "fact_conflicts", on).await? == conflict_row,
                "{what}: a superseded proposal changed something"
            );
            let closed = ledger_row(&f, proposal).await?;
            anyhow::ensure!(
                closed.state.as_deref() == Some("superseded") && closed.state_by == Some(f.editor),
                "{what}: {closed:?}"
            );
        }
        // 审计：只多了三行「过时」，记的是这个人
        let rows = audits(&f).await?;
        anyhow::ensure!(rows.len() == audits_before + 3, "three audit rows");
        let superseded = audits_of(&rows, "agent.proposal.superseded");
        anyhow::ensure!(superseded.len() == 3, "{superseded:?}");
        for said in superseded {
            anyhow::ensure!(
                said.actor == Some(f.editor)
                    && said.detail["operation"] == "conflict.resolve"
                    && keys(&said.detail)
                        == ["agent_proposal", "operation", "proposed_by_token", "reason"],
                "{said:?}"
            );
        }

        // 还开着的那一处：令牌看过它现在的样子可以再提，是一条新的提议，带着新的指纹
        let again = propose(&f, &token, drifted, keep).await?;
        anyhow::ensure!(again != on_drifted, "a superseded proposal is not reopened");
        let stored = ledger_row(&f, again).await?;
        anyhow::ensure!(
            stored.payload["fingerprint"]
                == json!({ "old_fact_id": drifted_row["old_fact_id"], "new_fact_id": new }),
            "{}",
            stored.payload
        );
        // 已经裁掉的、撤了的：提不了
        for on in [resolved, withdrawn] {
            let (status, body) = decide(&f, &token, on, json!({ "action": "keep" })).await?;
            anyhow::ensure!(
                status == StatusCode::CONFLICT && body["code"] == "already_decided",
                "{status} {body}"
            );
        }
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}
