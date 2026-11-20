//! 令牌对一处本体缺陷表态：只留提议，人答了才算（0076）。
//!
//! 这是「只能提议」的操作里第一种走通全程的，后面每一档照的是它。要钉住的行为：
//!
//! - **令牌的调用什么都不改**（SC1 / E1）：REST 与 MCP 工具 `review_defect` 都回 200、
//!   `proposed`、`propose_only`；缺陷行、这个库的每一条事实与实体、排着的任务数前后相等，
//!   台账里多一条开着的提议、审计里多一行 `agent.proposal`
//! - **重复调用拿回同一条提议**（E2）：`already_proposed`，不写第二行
//! - **请求体与目标照人的路由校验**（E3 / E4）：认不出的 `resolution` 是 `bad_resolution`，
//!   认不出的、别的库的缺陷是 404，已经裁过的是 `already_decided`；哪一种都不留提议
//! - **提议不算进限速**（E5）：落地过 200 笔的令牌照样提得了
//! - **人批准之后才落地，而且是这个人的决定**（SC2 / E7 / N2）：缺陷行与这个人自己裁的
//!   一模一样，`defect.decided` 审计行的 actor 是这个人、detail 里指着提议、不带 `token_id`；
//!   拒绝什么都不改；目标已经被裁掉的提议关成 `superseded`
//! - **只有登录的编辑者答得了**（E7 / N3）：Viewer 是 403，哪一枚令牌都是 401；答过的再答
//!   是 `already_decided`，一行不多
//! - **`review_defect` 与别的写工具同样列、同样拒**（E19）：只列给编辑者的写令牌；只读令牌
//!   与主人不到 Editor 的令牌调它，拿到带 REST code 的工具错误
//! - **会话的裁决一个字没变**：同一个响应体、同一行审计、台账一行不写
//!
//! 夹具在 `review_token_write_support.rs`。没有 `UTOPIA_DATABASE_URL` 时跳过。
use axum::http::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;

use super::review_token_write_support::{
    answer, audits, ensure_tool_refused, keys, ledger, open_proposals, post, row, seeded, snapshot,
    tool_call, tools_list, Audit, Fx,
};

const TOKEN_NAME: &str = "ontology agent";

/// 一处开着的本体缺陷，直接种进 `ontology_defects`。指着的本体元素是编出来的 id：
/// 缺陷的两列本来就没有外键
async fn defect_in(f: &Fx, kb: Uuid) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO ontology_defects (id, kb_id, kind, subject, detail)
         VALUES ($1, $2, 'symmetric_and_asymmetric', $3, '{}'::jsonb)",
    )
    .bind(id)
    .bind(kb)
    .bind(Uuid::now_v7())
    .execute(&f.pool)
    .await?;
    Ok(id)
}

async fn defect(f: &Fx) -> anyhow::Result<Uuid> {
    defect_in(f, f.kb).await
}

/// 对一处缺陷表态：人的那条路由，令牌与会话都从这里进
async fn decide(
    f: &Fx,
    bearer: &str,
    defect: Uuid,
    resolution: &str,
) -> anyhow::Result<(StatusCode, Value)> {
    post(
        f,
        bearer,
        &format!("review/defects/{defect}"),
        json!({ "resolution": resolution }),
    )
    .await
}

/// 一个留成提议的回答：正好那五个键，`proposed` / `propose_only`。回提议的 id
fn proposed(body: &Value) -> anyhow::Result<Uuid> {
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

/// 令牌经 REST 提一条，必须成：回提议的 id
async fn propose(f: &Fx, token: &str, defect: Uuid, resolution: &str) -> anyhow::Result<Uuid> {
    let (status, body) = decide(f, token, defect, resolution).await?;
    anyhow::ensure!(status == StatusCode::OK, "a proposal: {status} {body}");
    proposed(&body)
}

/// 一次答多条提议（批量的那条路由）
async fn bulk(
    f: &Fx,
    bearer: &str,
    ids: &[Uuid],
    action: &str,
) -> anyhow::Result<(StatusCode, Value)> {
    post(
        f,
        bearer,
        "review/agent-proposals",
        json!({ "ids": ids, "action": action }),
    )
    .await
}

fn audits_of<'a>(rows: &'a [Audit], action: &str) -> Vec<&'a Audit> {
    rows.iter().filter(|a| a.action == action).collect()
}

/// 这处缺陷的 `defect.decided` 审计行
fn decided_audits(rows: &[Audit], defect: Uuid) -> Vec<&Audit> {
    rows.iter()
        .filter(|a| a.action == "defect.decided" && a.target_id == Some(defect))
        .collect()
}

/// 往台账里直接种 `n` 行这枚令牌五分钟前落地的写入：限速数的就是这些行
async fn seed_applied(f: &Fx, token_id: Uuid, n: i32) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO token_agent_actions
             (id, kb_id, operation, target_kind, target_id, owner_id, token_id, token_name,
              outcome, payload, target_key, created_at)
         SELECT gen_random_uuid(), $1, 'review.keep', 'review', gen_random_uuid(), $2, $3,
                'seeded', 'applied', '{}'::jsonb, gen_random_uuid()::text,
                now() - interval '5 minutes'
           FROM generate_series(1, $4) g",
    )
    .bind(f.kb)
    .bind(f.editor)
    .bind(token_id)
    .bind(n)
    .execute(&f.pool)
    .await?;
    Ok(())
}

#[tokio::test]
async fn a_token_defect_decision_waits_as_a_proposal_and_changes_nothing() -> anyhow::Result<()> {
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
        let first = defect(&f).await?;
        let second = defect(&f).await?;
        let foreign = defect_in(&f, f.other_kb).await?;
        // 限速那一段要一对等人的重复项：在拍快照之前种好
        let kept = f.pair("Ada Lovelace", "Ada King").await?;
        let before = snapshot(&f).await?;
        let first_row = row(&f, "ontology_defects", first).await?;
        let rows_before = ledger(&f).await?.len();
        let audits_before = audits(&f).await?.len();

        // ── 没过检查的调用：哪一种都一行不写 ──
        // 请求体解不出来：提取器先答，连只读令牌拿到的也是 422 而不是 403
        for bearer in [&token, &editor_read] {
            let (status, body) =
                post(&f, bearer, &format!("review/defects/{first}"), json!({})).await?;
            anyhow::ensure!(
                status == StatusCode::UNPROCESSABLE_ENTITY,
                "a body without resolution: {status} {body}"
            );
        }
        // 写的检查，次序是 scope、库、角色；每一种带自己的 code
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
            // 认不出的 resolution 也排在写的检查后面：答的仍是那个 403
            for resolution in ["fixed", "dismiss"] {
                let (status, body) = decide(&f, bearer, first, resolution).await?;
                anyhow::ensure!(
                    status == StatusCode::FORBIDDEN && body["code"] == code,
                    "{who}, {resolution}: {status} {body}"
                );
            }
        }
        // 认不出的 resolution：人的那个 code。排在「目标在不在」前面
        for target in [first, Uuid::now_v7()] {
            let (status, body) = decide(&f, &token, target, "dismiss").await?;
            anyhow::ensure!(
                status == StatusCode::UNPROCESSABLE_ENTITY && body["code"] == "bad_resolution",
                "an unknown resolution: {status} {body}"
            );
        }
        // 认不出的缺陷、别的库的缺陷：同一个 404
        for target in [Uuid::now_v7(), foreign] {
            let (status, body) = decide(&f, &token, target, "fixed").await?;
            anyhow::ensure!(
                status == StatusCode::NOT_FOUND,
                "an unknown defect: {status} {body}"
            );
        }
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before,
            "a refusal wrote a row"
        );
        anyhow::ensure!(
            audits(&f).await?.len() == audits_before,
            "a refusal wrote an audit row"
        );
        anyhow::ensure!(snapshot(&f).await? == before, "a refusal changed the graph");

        // ── 提议：200，什么都没动，多了一条开着的提议 ──
        let (status, body) = decide(&f, &token, first, "fixed").await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let proposal = proposed(&body)?;
        anyhow::ensure!(
            snapshot(&f).await? == before,
            "a proposal changed a fact, an entity, evidence or the queued jobs"
        );
        anyhow::ensure!(
            row(&f, "ontology_defects", first).await? == first_row,
            "a proposal changed the defect row"
        );
        let open: Vec<_> = open_proposals(&f)
            .await?
            .into_iter()
            .filter(|p| p.target_id == first)
            .collect();
        anyhow::ensure!(open.len() == 1, "one open proposal: {open:?}");
        let p = &open[0];
        anyhow::ensure!(
            p.id == proposal
                && p.operation == "defect.decide"
                && p.target_kind == "ontology_defect"
                && p.outcome == "proposed"
                && p.gate_reason.as_deref() == Some("propose_only")
                && p.owner_id == f.editor
                && p.token_id == token_id
                && p.token_name == TOKEN_NAME
                && p.merge_id.is_none()
                && p.batch_id.is_none(),
            "{p:?}"
        );
        // payload：请求体原样、目标的那句话、动作；没给理由就没有 `why`，缺陷没有指纹
        anyhow::ensure!(
            keys(&p.payload) == ["action", "request", "target_label"],
            "{}",
            p.payload
        );
        anyhow::ensure!(
            p.payload["request"] == json!({ "resolution": "fixed" })
                && p.payload["action"] == "fixed"
                && p.payload["target_label"]
                    .as_str()
                    .is_some_and(|label| label.starts_with("symmetric_and_asymmetric")),
            "{}",
            p.payload
        );
        anyhow::ensure!(ledger(&f).await?.len() == rows_before + 1);
        // 审计：一行 `agent.proposal`，记在主人名下，认得出是哪枚令牌
        let rows = audits(&f).await?;
        anyhow::ensure!(rows.len() == audits_before + 1, "one audit row");
        let said = audits_of(&rows, "agent.proposal");
        let said = said
            .iter()
            .find(|a| a.target_id == Some(proposal))
            .ok_or_else(|| anyhow::anyhow!("no agent.proposal row for {proposal}"))?;
        anyhow::ensure!(
            said.actor == Some(f.editor)
                && said.target_kind == "agent_action"
                && said.detail["operation"] == "defect.decide"
                && said.detail["outcome"] == "proposed"
                && said.detail["reason"] == "propose_only"
                && said.detail["token_id"] == json!(token_id)
                && said.detail["token_name"] == TOKEN_NAME
                && said.detail["action_id"] == json!(proposal),
            "{said:?}"
        );
        anyhow::ensure!(decided_audits(&rows, first).is_empty());

        // ── 重复：同一条提议加 `already_proposed`，换一个 resolution 也是同一个目标 ──
        for resolution in ["fixed", "accepted"] {
            let (status, again) = decide(&f, &token, first, resolution).await?;
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
                    && again["action_id"] == json!(proposal)
                    && again["outcome"] == "proposed"
                    && again["reason"] == "propose_only",
                "a repeat with {resolution}: {again}"
            );
        }
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before + 1 && audits(&f).await?.len() == rows.len(),
            "a repeat wrote nothing"
        );
        // 存下的还是第一次的请求
        anyhow::ensure!(open_proposals(&f)
            .await?
            .iter()
            .any(|p| p.id == proposal && p.payload["action"] == "fixed"));

        // ── 提议不算进限速：落地过 200 笔的令牌照样提得了，答的不是 `agent_rate_limited` ──
        let (limited_id, limited) = f.token(f.editor, "busy agent", "write", None).await?;
        seed_applied(&f, limited_id, 200).await?;
        let second_proposal = propose(&f, &limited, second, "accepted").await?;
        anyhow::ensure!(second_proposal != proposal);
        // 它落地的写入照样被限速挡着：限速还在，只是不管提议
        let (status, body) = post(
            &f,
            &limited,
            &format!("review/{kept}"),
            json!({ "action": "keep" }),
        )
        .await?;
        anyhow::ensure!(
            status == StatusCode::TOO_MANY_REQUESTS && body["code"] == "agent_rate_limited",
            "{status} {body}"
        );

        // ── 已经裁过的缺陷：`already_decided`，不留提议 ──
        let (status, body) = decide(&f, &f.editor_session, second, "fixed").await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let rows_now = ledger(&f).await?.len();
        let (status, body) = decide(&f, &token, second, "fixed").await?;
        anyhow::ensure!(
            status == StatusCode::CONFLICT && body["code"] == "already_decided",
            "a decided defect: {status} {body}"
        );
        anyhow::ensure!(ledger(&f).await?.len() == rows_now);
        anyhow::ensure!(
            snapshot(&f).await? == before,
            "none of this changed a fact or an entity"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_person_approves_a_defect_proposal_and_it_is_that_persons_decision() -> anyhow::Result<()>
{
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (token_id, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let by_person = defect(&f).await?;
        let by_proposal = defect(&f).await?;
        let stale = defect(&f).await?;
        let proposal = propose(&f, &token, by_proposal, "fixed").await?;
        let stale_proposal = propose(&f, &token, stale, "accepted").await?;
        let before = snapshot(&f).await?;

        // 提议在 Agent History 里，读得出目标与动作
        let (status, history) = f
            .call(
                &f.editor_session,
                "GET",
                &format!("/kbs/{}/review?queue=agent&limit=200", f.kb),
                None,
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {history}");
        let listed = history["items"]
            .as_array()
            .and_then(|rows| rows.iter().find(|r| r["id"] == json!(proposal)))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("the proposal is not in Agent History: {history}"))?;
        anyhow::ensure!(
            listed["source"] == "token"
                && listed["operation"] == "defect.decide"
                && listed["outcome"] == "proposed"
                && listed["reason"] == "propose_only"
                && listed["action"] == "fixed"
                && listed["target_label"].is_string()
                && listed["token_name"] == TOKEN_NAME
                && listed.get("state").is_none(),
            "{listed}"
        );

        // 对照：这个人自己裁一处缺陷
        let (status, body) = decide(&f, &f.editor_session, by_person, "fixed").await?;
        anyhow::ensure!(
            status == StatusCode::OK && body == json!({ "ok": true }),
            "{status} {body}"
        );

        // 批准：正好 `ok` 与 `state`（缺陷的裁决不做合并、不排任务、不写事实）
        let (status, body) = answer(&f, &f.editor_session, proposal, "approve").await?;
        anyhow::ensure!(
            status == StatusCode::OK && body == json!({ "ok": true, "state": "approved" }),
            "{status} {body}"
        );

        // 缺陷行与这个人自己裁的那一处一模一样：状态、出路、谁裁的
        let mine = row(&f, "ontology_defects", by_person).await?;
        let theirs = row(&f, "ontology_defects", by_proposal).await?;
        for column in ["status", "resolution", "decided_by"] {
            anyhow::ensure!(
                mine[column] == theirs[column],
                "{column}: {mine} / {theirs}"
            );
        }
        anyhow::ensure!(
            theirs["status"] == "resolved"
                && theirs["resolution"] == "fixed"
                && theirs["decided_by"] == json!(f.editor)
                && !theirs["decided_at"].is_null(),
            "{theirs}"
        );
        anyhow::ensure!(
            snapshot(&f).await? == before,
            "a defect decision changes no fact and no entity, by either road"
        );

        // 审计：两行 `defect.decided`，actor 都是这个人；批准的那一行指着提议，不带令牌
        let rows = audits(&f).await?;
        let own = decided_audits(&rows, by_person);
        anyhow::ensure!(
            own.len() == 1
                && own[0].actor == Some(f.editor)
                && own[0].detail == json!({ "resolution": "fixed" }),
            "{own:?}"
        );
        let approved = decided_audits(&rows, by_proposal);
        anyhow::ensure!(approved.len() == 1, "{approved:?}");
        anyhow::ensure!(
            approved[0].actor == Some(f.editor)
                && approved[0].target_kind == "ontology_defect"
                && approved[0].detail
                    == json!({ "resolution": "fixed", "agent_proposal": proposal }),
            "the approval is the person's act and names the proposal, with no token: {:?}",
            approved[0]
        );
        anyhow::ensure!(
            audits_of(&rows, "agent.proposal.declined").is_empty()
                && audits_of(&rows, "agent.proposal.superseded").is_empty()
        );

        // 提议关成 approved：谁答的、什么时候；提议本身的 outcome 不改写
        let closed = ledger(&f)
            .await?
            .into_iter()
            .find(|r| r.id == proposal)
            .ok_or_else(|| anyhow::anyhow!("no ledger row {proposal}"))?;
        anyhow::ensure!(
            closed.state.as_deref() == Some("approved")
                && closed.state_by == Some(f.editor)
                && closed.state_at.is_some()
                && closed.outcome == "proposed"
                && closed.token_id == token_id
                && closed.merge_id.is_none(),
            "{closed:?}"
        );
        anyhow::ensure!(
            !open_proposals(&f).await?.iter().any(|p| p.id == proposal),
            "an approved proposal is no longer open"
        );

        // 目标在提议等着的时候被人裁掉了：批准关成 superseded，别的什么都不动
        let (status, body) = decide(&f, &f.editor_session, stale, "fixed").await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let stale_row = row(&f, "ontology_defects", stale).await?;
        let (status, body) = answer(&f, &f.editor_session, stale_proposal, "approve").await?;
        anyhow::ensure!(
            status == StatusCode::OK && body == json!({ "ok": true, "state": "superseded" }),
            "{status} {body}"
        );
        anyhow::ensure!(
            row(&f, "ontology_defects", stale).await? == stale_row,
            "a superseded proposal leaves the person's decision as it was"
        );
        let rows = audits(&f).await?;
        anyhow::ensure!(
            decided_audits(&rows, stale).len() == 1,
            "only the person's own decision is recorded"
        );
        let superseded = audits_of(&rows, "agent.proposal.superseded");
        anyhow::ensure!(superseded.len() == 1, "{superseded:?}");
        anyhow::ensure!(
            superseded[0].actor == Some(f.editor)
                && superseded[0].target_id == Some(stale_proposal)
                && keys(&superseded[0].detail)
                    == ["agent_proposal", "operation", "proposed_by_token", "reason"]
                && superseded[0].detail["operation"] == "defect.decide",
            "{:?}",
            superseded[0]
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_declined_defect_proposal_changes_nothing() -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let target = defect(&f).await?;
        let proposal = propose(&f, &token, target, "accepted").await?;
        let before = snapshot(&f).await?;
        let target_row = row(&f, "ontology_defects", target).await?;

        let (status, body) = answer(&f, &f.editor_session, proposal, "decline").await?;
        anyhow::ensure!(
            status == StatusCode::OK && body == json!({ "ok": true, "state": "declined" }),
            "{status} {body}"
        );
        anyhow::ensure!(
            row(&f, "ontology_defects", target).await? == target_row,
            "a declined proposal leaves the defect open and untouched"
        );
        anyhow::ensure!(target_row["status"] == "open", "{target_row}");
        anyhow::ensure!(snapshot(&f).await? == before, "a decline changed the graph");

        let closed = ledger(&f)
            .await?
            .into_iter()
            .find(|r| r.id == proposal)
            .ok_or_else(|| anyhow::anyhow!("no ledger row {proposal}"))?;
        anyhow::ensure!(
            closed.state.as_deref() == Some("declined")
                && closed.state_by == Some(f.editor)
                && closed.outcome == "proposed",
            "{closed:?}"
        );
        // 审计：一行拒绝，记的是这个人；没有裁决的行。合并之外的提议没有一对的两侧，
        // detail 里不写两个空键
        let rows = audits(&f).await?;
        anyhow::ensure!(decided_audits(&rows, target).is_empty());
        let declined = audits_of(&rows, "agent.proposal.declined");
        anyhow::ensure!(declined.len() == 1, "{declined:?}");
        anyhow::ensure!(
            declined[0].actor == Some(f.editor)
                && declined[0].target_kind == "agent_action"
                && declined[0].target_id == Some(proposal)
                && declined[0].detail
                    == json!({ "agent_proposal": proposal, "operation": "defect.decide",
                               "reason": "propose_only", "proposed_by_token": TOKEN_NAME }),
            "{:?}",
            declined[0]
        );

        // 缺陷还开着：令牌可以再提，是一条新的提议；人照旧自己裁得了
        let again = propose(&f, &token, target, "fixed").await?;
        anyhow::ensure!(again != proposal, "a declined proposal is not reopened");
        let (status, body) = decide(&f, &f.editor_session, target, "accepted").await?;
        anyhow::ensure!(
            status == StatusCode::OK && body == json!({ "ok": true }),
            "{status} {body}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn an_answered_proposal_answers_already_decided_and_only_an_editor_session_answers(
) -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (_, own) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let (_, other_write) = f.token(f.editor, "second agent", "write", None).await?;
        let (_, editor_read) = f.token(f.editor, "reader", "read", None).await?;
        let (_, viewer_write) = f.token(f.viewer, "viewer writer", "write", None).await?;
        let target = defect(&f).await?;
        let in_bulk = defect(&f).await?;
        let proposal = propose(&f, &own, target, "fixed").await?;
        let bulk_proposal = propose(&f, &own, in_bulk, "accepted").await?;
        let before = snapshot(&f).await?;
        let target_row = row(&f, "ontology_defects", target).await?;
        let rows_before = ledger(&f).await?.len();
        let audits_before = audits(&f).await?.len();

        // 哪一枚令牌都答不了——提出这条提议的那一枚也一样：两条路由都是 401
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
                let (status, body) = bulk(&f, token, &[proposal], action).await?;
                anyhow::ensure!(
                    status == StatusCode::UNAUTHORIZED,
                    "{who}, bulk {action}: {status} {body}"
                );
            }
        }
        // 登录的 Viewer：403
        for action in ["approve", "decline"] {
            let (status, body) = answer(&f, &f.viewer_session, proposal, action).await?;
            anyhow::ensure!(status == StatusCode::FORBIDDEN, "a viewer: {status} {body}");
            let (status, body) = bulk(&f, &f.viewer_session, &[proposal], action).await?;
            anyhow::ensure!(status == StatusCode::FORBIDDEN, "a viewer: {status} {body}");
        }
        anyhow::ensure!(
            row(&f, "ontology_defects", target).await? == target_row
                && snapshot(&f).await? == before
                && ledger(&f).await?.len() == rows_before
                && audits(&f).await?.len() == audits_before,
            "a refused answer changed something"
        );
        anyhow::ensure!(
            open_proposals(&f)
                .await?
                .iter()
                .filter(|p| p.id == proposal || p.id == bulk_proposal)
                .count()
                == 2,
            "both proposals stay open"
        );

        // 编辑者批准。再答——批准、拒绝、放进批量——都是 already_decided，一行不多
        let (status, body) = answer(&f, &f.editor_session, proposal, "approve").await?;
        anyhow::ensure!(
            status == StatusCode::OK && body["state"] == "approved",
            "{status} {body}"
        );
        let decided_row = row(&f, "ontology_defects", target).await?;
        let ledger_then = serde_json::to_value(ledger(&f).await?)?;
        let audits_then = audits(&f).await?.len();
        for action in ["approve", "decline"] {
            let (status, body) = answer(&f, &f.editor_session, proposal, action).await?;
            anyhow::ensure!(
                status == StatusCode::CONFLICT && body["code"] == "already_decided",
                "a second {action}: {status} {body}"
            );
        }
        let (status, body) = bulk(&f, &f.editor_session, &[proposal], "decline").await?;
        anyhow::ensure!(
            status == StatusCode::OK
                && body["answered"] == 0
                && body["outcomes"][0]["code"] == "already_decided"
                && body["outcomes"][0]["state"].is_null(),
            "{status} {body}"
        );
        anyhow::ensure!(
            row(&f, "ontology_defects", target).await? == decided_row
                && serde_json::to_value(ledger(&f).await?)? == ledger_then
                && audits(&f).await?.len() == audits_then
                && snapshot(&f).await? == before,
            "answering an answered proposal wrote something"
        );

        // 批量里的缺陷提议与单条同一个回答；一条结果正好那五个键
        let (status, body) =
            bulk(&f, &f.editor_session, &[bulk_proposal, proposal], "approve").await?;
        anyhow::ensure!(
            status == StatusCode::OK && body["answered"] == 1,
            "{status} {body}"
        );
        let outcome = &body["outcomes"][0];
        anyhow::ensure!(
            keys(outcome) == ["code", "error", "id", "merge_id", "state"]
                && outcome["id"] == json!(bulk_proposal)
                && outcome["state"] == "approved"
                && outcome["merge_id"].is_null()
                && outcome["code"].is_null(),
            "{outcome}"
        );
        anyhow::ensure!(body["outcomes"][1]["code"] == "already_decided", "{body}");
        let approved_in_bulk = row(&f, "ontology_defects", in_bulk).await?;
        anyhow::ensure!(
            approved_in_bulk["status"] == "resolved"
                && approved_in_bulk["resolution"] == "accepted"
                && approved_in_bulk["decided_by"] == json!(f.editor),
            "{approved_in_bulk}"
        );
        let rows = audits(&f).await?;
        let recorded = decided_audits(&rows, in_bulk);
        anyhow::ensure!(
            recorded.len() == 1
                && recorded[0].actor == Some(f.editor)
                && recorded[0].detail
                    == json!({ "resolution": "accepted", "agent_proposal": bulk_proposal }),
            "{recorded:?}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn the_mcp_defect_tool_proposes_and_is_listed_and_refused_as_the_write_tools_are(
) -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (token_id, editor_write) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let (_, editor_read) = f.token(f.editor, "reader", "read", None).await?;
        let (_, viewer_read) = f.token(f.viewer, "viewer reader", "read", None).await?;
        let (_, viewer_write) = f.token(f.viewer, "viewer writer", "write", None).await?;
        let target = defect(&f).await?;
        let decided = defect(&f).await?;
        let (status, body) = decide(&f, &f.editor_session, decided, "fixed").await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let before = snapshot(&f).await?;
        let target_row = row(&f, "ontology_defects", target).await?;
        let rows_before = ledger(&f).await?.len();

        // ── 只列给编辑者的写令牌 ──
        let mut described = None;
        for (who, token, listed) in [
            ("write token of an editor", &editor_write, true),
            ("read token of an editor", &editor_read, false),
            ("read token of a viewer", &viewer_read, false),
            ("write token of a viewer", &viewer_write, false),
        ] {
            let tools = tools_list(&f, token).await?;
            let tool = tools.iter().find(|t| t["name"] == "review_defect").cloned();
            anyhow::ensure!(
                tool.is_some() == listed,
                "{who}: review_defect listed = {}",
                tool.is_some()
            );
            anyhow::ensure!(
                tools.iter().any(|t| t["name"] == "review_list"),
                "{who}: the read tools stay listed"
            );
            if listed {
                described = tool;
            }
        }
        let tool = described.ok_or_else(|| anyhow::anyhow!("review_defect is not listed"))?;
        anyhow::ensure!(
            tool["annotations"]
                == json!({ "readOnlyHint": false, "destructiveHint": false,
                           "idempotentHint": false }),
            "{tool}"
        );
        let schema = &tool["inputSchema"];
        anyhow::ensure!(
            schema["required"] == json!(["id", "resolution"])
                && keys(&schema["properties"]) == ["id", "resolution"]
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

        // ── 写不了的调用方调得到这个名字，回来的是带 REST code 的工具错误 ──
        for (token, code) in [
            (&editor_read, "token_read_only"),
            (&viewer_read, "token_read_only"),
            (&viewer_write, "token_owner_below_editor"),
        ] {
            let refused = tool_call(
                &f,
                token,
                "review_defect",
                json!({ "id": target, "resolution": "fixed" }),
            )
            .await?;
            ensure_tool_refused(&refused, code)?;
            let (status, body) = decide(&f, token, target, "fixed").await?;
            anyhow::ensure!(
                status == StatusCode::FORBIDDEN
                    && body["code"] == code
                    && refused["structuredContent"]["message"] == body["error"],
                "{status} {body} {refused}"
            );
        }
        // 操作自己的拒绝：与 REST 同一个 code
        for (args, code) in [
            (
                json!({ "id": target, "resolution": "dismiss" }),
                "bad_resolution",
            ),
            (
                json!({ "id": Uuid::now_v7(), "resolution": "fixed" }),
                "not_found",
            ),
            (
                json!({ "id": decided, "resolution": "fixed" }),
                "already_decided",
            ),
        ] {
            let refused = tool_call(&f, &editor_write, "review_defect", args).await?;
            ensure_tool_refused(&refused, code)?;
        }
        // 参数没给全：没有执行，不带 code
        let refused =
            tool_call(&f, &editor_write, "review_defect", json!({ "id": target })).await?;
        anyhow::ensure!(
            refused["isError"] == true && refused["structuredContent"].get("code").is_none(),
            "{refused}"
        );
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before && snapshot(&f).await? == before,
            "a refused tool call wrote something"
        );

        // ── 提议：`structuredContent` 就是 REST 的那个回答；什么都没动 ──
        let result = tool_call(
            &f,
            &editor_write,
            "review_defect",
            json!({ "id": target, "resolution": "accepted" }),
        )
        .await?;
        anyhow::ensure!(result["isError"] == false, "{result}");
        let content = &result["structuredContent"];
        let proposal = proposed(content)?;
        anyhow::ensure!(
            snapshot(&f).await? == before
                && row(&f, "ontology_defects", target).await? == target_row,
            "the tool changed the graph or the defect row"
        );
        let open: Vec<_> = open_proposals(&f)
            .await?
            .into_iter()
            .filter(|p| p.target_id == target)
            .collect();
        anyhow::ensure!(
            open.len() == 1
                && open[0].id == proposal
                && open[0].operation == "defect.decide"
                && open[0].token_id == token_id
                && open[0].payload["request"] == json!({ "resolution": "accepted" })
                && open[0].payload["action"] == "accepted",
            "{open:?}"
        );
        // 重复：两个入口拿回的是同一个回答，逐字相等
        let (status, over_rest) = decide(&f, &editor_write, target, "accepted").await?;
        anyhow::ensure!(
            status == StatusCode::OK
                && over_rest["code"] == "already_proposed"
                && over_rest["action_id"] == json!(proposal),
            "{status} {over_rest}"
        );
        let again = tool_call(
            &f,
            &editor_write,
            "review_defect",
            json!({ "id": target, "resolution": "accepted" }),
        )
        .await?;
        anyhow::ensure!(
            again["isError"] == false && again["structuredContent"] == over_rest,
            "the tool answers what the token API answers\n{again}\n{over_rest}"
        );
        anyhow::ensure!(ledger(&f).await?.len() == rows_before + 1);

        // 工具答不了提议：没有那样的工具，名字里也没有
        for tool in tools_list(&f, &editor_write).await? {
            let name = tool["name"].as_str().unwrap_or_default().to_string();
            anyhow::ensure!(
                !["proposal", "approve", "decline", "answer"]
                    .iter()
                    .any(|word| name.contains(word)),
                "no tool answers a proposal: {name}"
            );
        }
        anyhow::ensure!(open_proposals(&f).await?.iter().any(|p| p.id == proposal));
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_session_defect_decision_answers_as_before() -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let target = defect(&f).await?;
        let untouched = defect(&f).await?;
        let rows_before = ledger(&f).await?.len();
        let untouched_row = row(&f, "ontology_defects", untouched).await?;

        // 认不出的 resolution：从前的 code、从前的那一句
        let (status, body) = decide(&f, &f.editor_session, target, "dismiss").await?;
        anyhow::ensure!(
            status == StatusCode::UNPROCESSABLE_ENTITY
                && body["code"] == "bad_resolution"
                && body["error"] == "resolution 只能是 fixed 或 accepted",
            "{status} {body}"
        );
        // Viewer：不带 code 的 403。认不出的缺陷：404
        let (status, body) = decide(&f, &f.viewer_session, target, "fixed").await?;
        anyhow::ensure!(
            status == StatusCode::FORBIDDEN && body.get("code").is_none(),
            "{status} {body}"
        );
        let (status, body) = decide(&f, &f.editor_session, Uuid::now_v7(), "fixed").await?;
        anyhow::ensure!(status == StatusCode::NOT_FOUND, "{status} {body}");
        anyhow::ensure!(audits_of(&audits(&f).await?, "defect.decided").is_empty());

        // 裁决：`{"ok": true}`，缺陷行记着这个人
        let (status, body) = decide(&f, &f.editor_session, target, "accepted").await?;
        anyhow::ensure!(
            status == StatusCode::OK && body == json!({ "ok": true }),
            "{status} {body}"
        );
        let decided = row(&f, "ontology_defects", target).await?;
        anyhow::ensure!(
            decided["status"] == "resolved"
                && decided["resolution"] == "accepted"
                && decided["decided_by"] == json!(f.editor)
                && !decided["decided_at"].is_null(),
            "{decided}"
        );
        // 审计：一行，detail 正好是从前的那一个键
        let rows = audits(&f).await?;
        let recorded = audits_of(&rows, "defect.decided");
        anyhow::ensure!(recorded.len() == 1, "{recorded:?}");
        anyhow::ensure!(
            recorded[0].actor == Some(f.editor)
                && recorded[0].target_kind == "ontology_defect"
                && recorded[0].target_id == Some(target)
                && recorded[0].detail == json!({ "resolution": "accepted" }),
            "{:?}",
            recorded[0]
        );
        // 再裁一次：从前的 404；别的缺陷没动；会话的裁决不进令牌 agent 的台账
        let (status, body) = decide(&f, &f.editor_session, target, "fixed").await?;
        anyhow::ensure!(status == StatusCode::NOT_FOUND, "{status} {body}");
        anyhow::ensure!(row(&f, "ontology_defects", target).await? == decided);
        anyhow::ensure!(row(&f, "ontology_defects", untouched).await? == untouched_row);
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before && open_proposals(&f).await?.is_empty(),
            "a session decision writes no ledger row"
        );
        anyhow::ensure!(audits_of(&audits(&f).await?, "defect.decided").len() == 1);
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}
