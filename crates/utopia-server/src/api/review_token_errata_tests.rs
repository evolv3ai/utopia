//! 令牌答勘误 agent 留给人的一笔：只留提议，人答了才算（0076）；以及违规与勘误的提议在
//! 等的时候目标被答掉了的那几种过时。
//!
//! 照的是 `review_token_conflict_tests`。要钉住的行为：
//!
//! - **令牌的调用什么都不改**（SC1 / E1）：批、否都回 200、`proposed`、`propose_only`；那
//!   一笔仍然留着、这个库的每一条事实与实体、排着的任务数前后相等，台账里多一条开着的提议
//! - **目标照人的路由校验**（E4）：认不出的、别的库的那一笔是 404，答过的是
//!   `already_decided`；写的检查排在前面；哪一种都不留提议
//! - **人批准之后才落地，而且是这个人的回答**（SC2 / E7 / N2）：那一笔与事实同这个人自己
//!   答的一模一样，审计行的 actor 是这个人、detail 里指着提议、不带 `token_id`；拒绝什么
//!   都不改
//! - **人的回答在批准时报的错原样报出去，提议留着**：不折成 `superseded`
//! - **会话的回答一个字没变**：同一个响应体、同一行审计、台账一行不写
//! - **过时**（E5）：违规在等的时候被裁掉、它要撤的那条事实被撤掉，勘误的那一笔在等的
//!   时候被答掉——批准这样的提议什么都不改，答 `superseded`
//!
//! 留给人的那一笔直接种进 `errata_actions`（连同它的文档与那一轮），不跑勘误的模型任务。
//!
//! 夹具在 `review_token_write_support.rs`。没有 `UTOPIA_DATABASE_URL` 时跳过。
use axum::http::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;

use super::review_token_fact_tests::{
    audits_of, fact_on, ledger_row, predicate_in, proposed, shape, subject_in,
};
use super::review_token_violation_tests as violations;
use super::review_token_write_support::{
    answer, audits, keys, ledger, open_proposals, post, row, snapshot, Audit, Fx,
};

const TOKEN_NAME: &str = "errata agent";

/// 留给人的一笔撤回，直接种进 `errata_actions`：它要撤的是一条新种下的活着的事实。
/// `with_target` 为假时那一笔没有 `proposed`——人批它时 store 会报错。回 (那一笔, 那条事实)
pub(super) async fn held_in(
    f: &Fx,
    kb: Uuid,
    predicate: Uuid,
    with_target: bool,
) -> anyhow::Result<(Uuid, Uuid)> {
    let subject = subject_in(f, kb).await?;
    let fact = fact_on(f, kb, subject, predicate, "held", Some("2024-01-01")).await?;
    let (doc, run, action) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    sqlx::query("INSERT INTO documents (id, kb_id, filename, sha256) VALUES ($1, $2, $3, $4)")
        .bind(doc)
        .bind(kb)
        .bind(format!("errata-{}.txt", doc.simple()))
        .bind(doc.simple().to_string())
        .execute(&f.pool)
        .await?;
    sqlx::query("INSERT INTO errata_runs (id, kb_id, document_id) VALUES ($1, $2, $3)")
        .bind(run)
        .bind(kb)
        .bind(doc)
        .execute(&f.pool)
        .await?;
    let target = with_target.then(|| {
        json!({ "subject": "Lease", "property": "status", "object": "held",
                "subject_id": subject, "predicate_id": predicate, "object_value": "held" })
    });
    sqlx::query(
        "INSERT INTO errata_actions (id, kb_id, run_id, document_id, fact_id, predicate_id,
                                     action, reason, quote, proposed, status, detail)
         VALUES ($1, $2, $3, $4, $5, $6, 'retract', 'wrong', 'the lease is held', $7, 'held',
                 'derived 1')",
    )
    .bind(action)
    .bind(kb)
    .bind(run)
    .bind(doc)
    .bind(fact)
    .bind(predicate)
    .bind(target)
    .execute(&f.pool)
    .await?;
    Ok((action, fact))
}

/// 答一笔勘误：人的那条路由，令牌与会话都从这里进
pub(super) async fn decide(
    f: &Fx,
    bearer: &str,
    action: Uuid,
    body: Value,
) -> anyhow::Result<(StatusCode, Value)> {
    post(f, bearer, &format!("review/errata/{action}"), body).await
}

/// 令牌经 REST 提一条，必须成：回提议的 id
async fn propose(f: &Fx, token: &str, action: Uuid, approve: bool) -> anyhow::Result<Uuid> {
    let (status, answer) = decide(f, token, action, json!({ "approve": approve })).await?;
    anyhow::ensure!(status == StatusCode::OK, "a proposal: {status} {answer}");
    proposed(&answer)
}

/// 那一笔里人的回答：状态、谁答的、落下的新事实
fn decision_of(action: &Value) -> Value {
    json!([
        action["status"],
        action["decided_by"],
        action["new_fact_id"]
    ])
}

/// 那一笔的 `errata.decided` 审计行
fn decided_audits(rows: &[Audit], action: Uuid) -> Vec<&Audit> {
    rows.iter()
        .filter(|a| a.action == "errata.decided" && a.target_id == Some(action))
        .collect()
}

#[tokio::test]
async fn a_token_errata_answer_waits_as_a_proposal_and_a_person_applies_it() -> anyhow::Result<()> {
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
        let predicate = predicate_in(&f, f.kb).await?;
        let (target, _) = held_in(&f, f.kb, predicate, true).await?;
        let (answered, _) = held_in(&f, f.kb, predicate, true).await?;
        let (status, body) = decide(
            &f,
            &f.editor_session,
            answered,
            json!({ "approve": false }),
        )
        .await?;
        anyhow::ensure!(
            status == StatusCode::OK && body == json!({ "ok": true }),
            "{status} {body}"
        );
        let foreign_predicate = predicate_in(&f, f.other_kb).await?;
        let (foreign, _) = held_in(&f, f.other_kb, foreign_predicate, true).await?;
        let (stuck, _) = held_in(&f, f.kb, predicate, false).await?;

        let before = snapshot(&f).await?;
        let target_row = row(&f, "errata_actions", target).await?;
        let rows_before = ledger(&f).await?.len();
        let audits_before = audits(&f).await?.len();

        // ── 没过检查的调用：哪一种都一行不写 ──
        // 请求体解不出来：提取器先答
        for body in [json!({}), json!({ "approve": "yes" })] {
            let (status, answered) = decide(&f, &token, target, body.clone()).await?;
            anyhow::ensure!(
                status == StatusCode::UNPROCESSABLE_ENTITY,
                "{body}: {status} {answered}"
            );
        }
        // 写的检查，次序是 scope、库、角色；排在「目标在不在」前面
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
            for on in [target, Uuid::now_v7()] {
                let (status, body) = decide(&f, bearer, on, json!({ "approve": true })).await?;
                anyhow::ensure!(
                    status == StatusCode::FORBIDDEN && body["code"] == code,
                    "{who}: {status} {body}"
                );
            }
        }
        // 认不出的、别的库的那一笔：与会话拿到的是同一个 404
        for on in [Uuid::now_v7(), foreign] {
            let by_token = decide(&f, &token, on, json!({ "approve": true })).await?;
            let by_person = decide(&f, &f.editor_session, on, json!({ "approve": true })).await?;
            anyhow::ensure!(
                by_token.0 == StatusCode::NOT_FOUND && by_person.0 == StatusCode::NOT_FOUND,
                "an unknown action: {by_token:?} {by_person:?}"
            );
        }
        // 答过的那一笔：already_decided
        for approve in [true, false] {
            let (status, body) = decide(&f, &token, answered, json!({ "approve": approve })).await?;
            anyhow::ensure!(
                status == StatusCode::CONFLICT && body["code"] == "already_decided",
                "an answered action: {status} {body}"
            );
        }
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before
                && audits(&f).await?.len() == audits_before
                && snapshot(&f).await? == before
                && row(&f, "errata_actions", target).await? == target_row,
            "a refusal wrote something"
        );

        // ── 人的回答在批准时报错：原样报出去，提议留着，不是 superseded ──
        // 这一笔没有要落的目标：人自己批它，store 报的是同一个错
        let on_stuck = propose(&f, &token, stuck, true).await?;
        let stuck_row = row(&f, "errata_actions", stuck).await?;
        let before = snapshot(&f).await?;
        let audits_before = audits(&f).await?.len();
        let by_person = decide(&f, &f.editor_session, stuck, json!({ "approve": true })).await?;
        let (status, body) = answer(&f, &f.editor_session, on_stuck, "approve").await?;
        anyhow::ensure!(
            status == StatusCode::UNPROCESSABLE_ENTITY && (status, body.clone()) == by_person,
            "the person's error is returned as it is: {status} {body} / {by_person:?}"
        );
        let waiting = ledger_row(&f, on_stuck).await?;
        anyhow::ensure!(
            waiting.state.is_none() && waiting.state_by.is_none(),
            "the proposal stays open: {waiting:?}"
        );
        anyhow::ensure!(
            snapshot(&f).await? == before
                && row(&f, "errata_actions", stuck).await? == stuck_row
                && audits(&f).await?.len() == audits_before,
            "a failed approval changed something"
        );

        // ── 批与否：各提一条、什么都没动；这个人批准，落地的与他自己答的一模一样 ──
        for approve in [true, false] {
            let action = if approve { "approve" } else { "reject" };
            let body = json!({ "approve": approve });
            let (by_person, person_fact) = held_in(&f, f.kb, predicate, true).await?;
            let (by_proposal, proposal_fact) = held_in(&f, f.kb, predicate, true).await?;
            let (declined, _) = held_in(&f, f.kb, predicate, true).await?;
            let before = snapshot(&f).await?;
            let held_row = row(&f, "errata_actions", by_proposal).await?;
            let untouched = shape(&f, proposal_fact).await?;

            let (status, answered) = decide(&f, &token, by_proposal, body.clone()).await?;
            anyhow::ensure!(status == StatusCode::OK, "{action}: {status} {answered}");
            let proposal = proposed(&answered)?;
            let declined_proposal = propose(&f, &token, declined, approve).await?;
            anyhow::ensure!(
                snapshot(&f).await? == before,
                "an errata proposal changed a fact, an entity, evidence or the queued jobs"
            );
            anyhow::ensure!(
                row(&f, "errata_actions", by_proposal).await? == held_row
                    && held_row["status"] == "held",
                "an errata proposal changed the held action"
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
                    && p.operation == "errata.decide"
                    && p.target_kind == "errata_action"
                    && p.outcome == "proposed"
                    && p.gate_reason.as_deref() == Some("propose_only")
                    && p.owner_id == f.editor
                    && p.token_id == token_id
                    && p.token_name == TOKEN_NAME,
                "{p:?}"
            );
            anyhow::ensure!(
                keys(&p.payload) == ["action", "request", "target_label"],
                "{}",
                p.payload
            );
            anyhow::ensure!(
                p.payload["request"] == body
                    && p.payload["action"] == action
                    && p.payload["target_label"]
                        .as_str()
                        .is_some_and(|said| !said.is_empty()),
                "{}",
                p.payload
            );
            // 重复：同一条提议加 `already_proposed`，反过来答也是同一个目标
            let (status, again) =
                decide(&f, &token, by_proposal, json!({ "approve": !approve })).await?;
            anyhow::ensure!(
                status == StatusCode::OK
                    && again["code"] == "already_proposed"
                    && again["action_id"] == json!(proposal),
                "a repeat: {status} {again}"
            );

            // 对照：这个人自己答同样的一笔。会话的回答、审计行同从前，台账一行不写
            let rows_then = ledger(&f).await?.len();
            let (status, answered) = decide(&f, &f.editor_session, by_person, body.clone()).await?;
            anyhow::ensure!(
                status == StatusCode::OK && answered == json!({ "ok": true }),
                "{action}: {status} {answered}"
            );
            anyhow::ensure!(
                ledger(&f).await?.len() == rows_then,
                "a session answer writes no ledger row"
            );
            let mine = shape(&f, person_fact).await?;
            let mine_row = row(&f, "errata_actions", by_person).await?;
            anyhow::ensure!(
                decision_of(&mine_row)
                    == json!([if approve { "applied" } else { "rejected" }, f.editor, null]),
                "{mine_row}"
            );

            // 批准：正好 `ok` 与 `state`
            let (status, answered) = answer(&f, &f.editor_session, proposal, "approve").await?;
            anyhow::ensure!(
                status == StatusCode::OK && answered == json!({ "ok": true, "state": "approved" }),
                "{action}: {status} {answered}"
            );
            let theirs_row = row(&f, "errata_actions", by_proposal).await?;
            anyhow::ensure!(
                decision_of(&theirs_row) == decision_of(&mine_row)
                    && !theirs_row["decided_at"].is_null(),
                "{theirs_row} / {mine_row}"
            );
            let theirs = shape(&f, proposal_fact).await?;
            anyhow::ensure!(
                theirs == mine,
                "an approved errata {action} changes the fact exactly as the person's own\n{theirs}\n{mine}"
            );
            // 批了就撤掉那条事实，否了事实不动
            anyhow::ensure!(
                (mine != untouched) == approve,
                "{action}: {mine} / {untouched}"
            );
            let closed = ledger_row(&f, proposal).await?;
            anyhow::ensure!(
                closed.state.as_deref() == Some("approved")
                    && closed.state_by == Some(f.editor)
                    && closed.outcome == "proposed"
                    && closed.merge_id.is_none(),
                "{closed:?}"
            );
            // 审计：actor 都是这个人，同一个动作名；会话的那一行正好是从前的那一个键，
            // 批准的那一行多指着提议，都不带令牌
            let rows = audits(&f).await?;
            let own = decided_audits(&rows, by_person);
            let approved = decided_audits(&rows, by_proposal);
            anyhow::ensure!(own.len() == 1 && approved.len() == 1, "{own:?} {approved:?}");
            anyhow::ensure!(
                own[0].actor == Some(f.editor)
                    && own[0].target_kind == "errata_action"
                    && own[0].detail == json!({ "approve": approve }),
                "a session answer records what it always did: {:?}",
                own[0]
            );
            anyhow::ensure!(
                approved[0].actor == Some(f.editor)
                    && approved[0].target_kind == "errata_action"
                    && approved[0].detail
                        == json!({ "approve": approve, "agent_proposal": proposal }),
                "the approval is the person's act and names its proposal: {:?}",
                approved[0]
            );

            // 拒绝：什么都不改，那一笔仍然留着
            let before = snapshot(&f).await?;
            let declined_row = row(&f, "errata_actions", declined).await?;
            let (status, answered) =
                answer(&f, &f.editor_session, declined_proposal, "decline").await?;
            anyhow::ensure!(
                status == StatusCode::OK && answered == json!({ "ok": true, "state": "declined" }),
                "{action}: {status} {answered}"
            );
            anyhow::ensure!(
                snapshot(&f).await? == before
                    && row(&f, "errata_actions", declined).await? == declined_row
                    && declined_row["status"] == "held",
                "a declined errata proposal changed something"
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
                                   "operation": "errata.decide", "reason": "propose_only",
                                   "proposed_by_token": TOKEN_NAME }),
                "{said:?}"
            );
        }
        anyhow::ensure!(audits_of(&audits(&f).await?, "agent.proposal.superseded").is_empty());
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_violation_or_errata_proposal_on_a_decided_target_is_superseded() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let manages = violations::asymmetric_in(&f, f.kb).await?;
        let predicate = predicate_in(&f, f.kb).await?;
        let real = violations::asymmetries(&f, manages, 2).await?;
        let accepted = json!({ "resolution": "accepted" });

        // 违规：等着的时候被这个人裁掉了
        let decided = real[0];
        let on_decided = violations::propose(&f, &token, decided, accepted.clone()).await?;
        let (status, body) = violations::decide(
            &f,
            &f.editor_session,
            decided,
            json!({ "resolution": "axiom_relaxed" }),
        )
        .await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");

        // 违规：提的是撤某一条事实，等着的时候那条事实被这个人拒绝了
        let withdrawn = real[1];
        let withdrawn_fact =
            violations::fact_of(&row(&f, "axiom_violations", withdrawn).await?, "left_fact")?;
        let on_withdrawn = violations::propose(
            &f,
            &token,
            withdrawn,
            json!({ "resolution": "fact_retracted", "fact_id": withdrawn_fact }),
        )
        .await?;
        let (status, body) = post(
            &f,
            &f.editor_session,
            &format!("facts/{withdrawn_fact}/reject"),
            json!({}),
        )
        .await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");

        // 勘误：等着的时候那一笔被这个人答掉了——批掉的一笔、否掉的一笔
        let (applied, _) = held_in(&f, f.kb, predicate, true).await?;
        let on_applied = propose(&f, &token, applied, false).await?;
        let (rejected, rejected_fact) = held_in(&f, f.kb, predicate, true).await?;
        let on_rejected = propose(&f, &token, rejected, true).await?;
        for (on, approve) in [(applied, true), (rejected, false)] {
            let (status, body) =
                decide(&f, &f.editor_session, on, json!({ "approve": approve })).await?;
            anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        }
        let rejected_shape = shape(&f, rejected_fact).await?;

        let before = snapshot(&f).await?;
        let audits_before = audits(&f).await?.len();
        for (what, proposal, table, on, operation) in [
            (
                "a violation a person decided",
                on_decided,
                "axiom_violations",
                decided,
                "violation.decide",
            ),
            (
                "a violation whose named fact was retracted",
                on_withdrawn,
                "axiom_violations",
                withdrawn,
                "violation.decide",
            ),
            (
                "an errata action a person approved",
                on_applied,
                "errata_actions",
                applied,
                "errata.decide",
            ),
            (
                "an errata action a person rejected",
                on_rejected,
                "errata_actions",
                rejected,
                "errata.decide",
            ),
        ] {
            let target_row = row(&f, table, on).await?;
            let (status, body) = answer(&f, &f.editor_session, proposal, "approve").await?;
            anyhow::ensure!(
                status == StatusCode::OK && body == json!({ "ok": true, "state": "superseded" }),
                "{what}: {status} {body}"
            );
            anyhow::ensure!(
                snapshot(&f).await? == before && row(&f, table, on).await? == target_row,
                "{what}: a superseded proposal changed something"
            );
            let closed = ledger_row(&f, proposal).await?;
            anyhow::ensure!(
                closed.state.as_deref() == Some("superseded")
                    && closed.state_by == Some(f.editor)
                    && closed.operation == operation,
                "{what}: {closed:?}"
            );
            // 再答一次：已经答过了
            let (status, body) = answer(&f, &f.editor_session, proposal, "approve").await?;
            anyhow::ensure!(
                status == StatusCode::CONFLICT && body["code"] == "already_decided",
                "{what}: {status} {body}"
            );
        }
        // 否掉的那一笔要撤的事实还活着：过时的批准没有去撤它
        anyhow::ensure!(shape(&f, rejected_fact).await? == rejected_shape);
        // 审计：只多了四行「过时」，记的是这个人；没有哪一行是人的裁决
        let rows = audits(&f).await?;
        anyhow::ensure!(rows.len() == audits_before + 4, "four audit rows");
        let superseded = audits_of(&rows, "agent.proposal.superseded");
        anyhow::ensure!(superseded.len() == 4, "{superseded:?}");
        for said in superseded {
            anyhow::ensure!(
                said.actor == Some(f.editor)
                    && keys(&said.detail)
                        == ["agent_proposal", "operation", "proposed_by_token", "reason"],
                "{said:?}"
            );
        }
        // 已经裁掉的、答掉的：提不了
        let (status, body) = violations::decide(&f, &token, decided, accepted).await?;
        anyhow::ensure!(
            status == StatusCode::CONFLICT && body["code"] == "already_decided",
            "{status} {body}"
        );
        for on in [applied, rejected] {
            let (status, body) = decide(&f, &token, on, json!({ "approve": true })).await?;
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
