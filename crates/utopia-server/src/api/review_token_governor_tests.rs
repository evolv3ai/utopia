//! 令牌对治理 agent 的一笔只能**提议**怎么答，人答了才算（0076，E6）；以及等点头的事实与
//! 治理的一笔这两个 MCP 工具。
//!
//! 人回答治理的那条路由是「以人的身份合并、分开、撤回一对」的第二条路，所以令牌不从它
//! 进——它照旧只收会话，令牌拿到的是 401。令牌走自己的
//! `POST /kbs/{id}/review/agent/{decision_id}/propose`，只留一条提议。要钉住的行为：
//!
//! - **令牌的调用什么都不改**（E6）：治理的那一行、它指着的那一对、合并记录、保险丝（库的
//!   开关）、这个库的每一条事实与实体、排着的任务前后相等，不排治理任务；台账里多一条
//!   开着的提议，令牌的理由只留在提议上
//! - **目标照人的路由校验**：动作对不上这一笔的状态是 `agent_answer`、撤一笔没有合并可撤
//!   的是 `not_a_merge`，与人拿到的同一个回答；有人答过的、建议还开着而那一对已经不等人
//!   的是 `already_decided`；认不出的、别的库的是 404；写的检查排在最前面
//! - **人批准之后才落地，而且是这个人的回答**（E6 / N2）：治理的那一行、那一对、实体同这个
//!   人自己答的一模一样，审计行的 actor 是这个人、detail 里指着提议、不带 `token_id`，
//!   令牌的理由不在里面；人撤回治理的合并到第二次，保险丝照今天那样跳
//! - **两个 MCP 工具**与 REST 是同一个回答，只列给写得了的调用方，同样什么都不改
//!
//! 治理的每一笔直接种进 `agent_decisions`，不跑治理的模型任务。
//!
//! 夹具在 `review_token_write_support.rs`。没有 `UTOPIA_DATABASE_URL` 时跳过。
use axum::http::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;

use super::review_token_fact_tests::{audits_of, ledger_row, proposed};
use super::review_token_pending_tests::{nod_state, waiting};
use super::review_token_write_support::{
    answer, audits, ensure_tool_refused, keys, ledger, open_proposals, post, row, seeded, snapshot,
    tool_call, tools_list, Audit, Fx,
};

const TOKEN_NAME: &str = "governor agent";
/// 令牌写的理由：它只许留在提议上，哪一行审计、哪一行审核记录里都不许出现
const TOKEN_REASON: &str = "TOKEN-RATIONALE the names match";

/// 治理 agent 的一笔，直接种进 `agent_decisions`：指着审核行 `review`
async fn decision(
    f: &Fx,
    kb: Uuid,
    review: Uuid,
    action: &str,
    status: &str,
    merge_id: Option<Uuid>,
) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO agent_decisions (id, kb_id, run_id, target_kind, target_id, action,
                                      confidence, reason, status, merge_id, decided_at)
         VALUES ($1, $2, $3, 'review', $4, $5, 0.8, 'test: same person', $6, $7,
                 CASE WHEN $6 <> 'proposed' THEN now() END)",
    )
    .bind(id)
    .bind(kb)
    .bind(Uuid::now_v7())
    .bind(review)
    .bind(action)
    .bind(status)
    .bind(merge_id)
    .execute(&f.pool)
    .await?;
    Ok(id)
}

/// 一条还开着的建议（建议 `action`），指着一对还在等人的重复项。回 (这一笔, 审核行)。
/// 每一对的名字都不一样：人裁一对会把同名的别的对上的建议作废，测试之间不该互相碰
async fn suggested(f: &Fx, name: &str, action: &str) -> anyhow::Result<(Uuid, Uuid)> {
    let review = f
        .pair(&format!("{name} Left"), &format!("{name} Right"))
        .await?;
    Ok((
        decision(f, f.kb, review, action, "proposed", None).await?,
        review,
    ))
}

/// 治理 agent 自己分开的一对（`applied` 的 keep）。回 (这一笔, 审核行)
async fn kept_apart(f: &Fx, name: &str) -> anyhow::Result<(Uuid, Uuid)> {
    let review = f
        .pair(&format!("{name} Left"), &format!("{name} Right"))
        .await?;
    sqlx::query("UPDATE resolution_reviews SET status = 'kept', decided_at = now() WHERE id = $1")
        .bind(review)
        .execute(&f.pool)
        .await?;
    Ok((
        decision(f, f.kb, review, "keep", "applied", None).await?,
        review,
    ))
}

/// 治理 agent 自己合并的一对（`applied` 的 merge，带着那次真的合并）。
/// 回 (这一笔, 审核行, 那次合并)
async fn merged(f: &Fx, name: &str) -> anyhow::Result<(Uuid, Uuid, Uuid)> {
    let left = f.entity(f.person_type, &format!("{name} Left")).await?;
    let right = f.entity(f.person_type, &format!("{name} Right")).await?;
    let review = f.pair_of(left, right).await?;
    let merge =
        utopia_store::resolution::merge_entities(&f.pool, f.kb, right, left, None, "governance")
            .await?;
    sqlx::query(
        "UPDATE resolution_reviews SET status = 'merged', decided_at = now() WHERE id = $1",
    )
    .bind(review)
    .execute(&f.pool)
    .await?;
    Ok((
        decision(f, f.kb, review, "merge", "applied", Some(merge)).await?,
        review,
        merge,
    ))
}

/// 回答治理碰得到的一切（E6）：这个库的每一笔治理记录、每一条审核行、每一次合并记录的
/// 每一列，库自己那一行（保险丝关的就是它的开关），以及排着的治理任务数
async fn governed(f: &Fx) -> anyhow::Result<Value> {
    Ok(sqlx::query_scalar(
        "SELECT jsonb_build_object(
            'decisions', (SELECT coalesce(jsonb_agg(to_jsonb(d) ORDER BY d.id), '[]'::jsonb)
                            FROM agent_decisions d WHERE d.kb_id = $1),
            'reviews', (SELECT coalesce(jsonb_agg(to_jsonb(r) ORDER BY r.id), '[]'::jsonb)
                          FROM resolution_reviews r WHERE r.kb_id = $1),
            'merges', (SELECT coalesce(jsonb_agg(to_jsonb(m) ORDER BY m.id), '[]'::jsonb)
                         FROM entity_merges m WHERE m.kb_id = $1),
            'base', (SELECT to_jsonb(k) FROM knowledge_bases k WHERE k.id = $1),
            'govern_jobs', (SELECT count(*) FROM jobs j
                             WHERE j.kind = 'govern' AND j.payload ->> 'kb_id' = $1::text))",
    )
    .bind(f.kb)
    .fetch_one(&f.pool)
    .await?)
}

/// 保险丝跳没跳：库的治理开关还开不开着
async fn governing(f: &Fx) -> anyhow::Result<bool> {
    Ok(
        sqlx::query_scalar("SELECT governance FROM knowledge_bases WHERE id = $1")
            .bind(f.kb)
            .fetch_one(&f.pool)
            .await?,
    )
}

/// 令牌自己的那条路由
async fn propose(
    f: &Fx,
    bearer: &str,
    decision: Uuid,
    body: Value,
) -> anyhow::Result<(StatusCode, Value)> {
    post(f, bearer, &format!("review/agent/{decision}/propose"), body).await
}

/// 人回答治理的路由
async fn respond(
    f: &Fx,
    bearer: &str,
    decision: Uuid,
    body: Value,
) -> anyhow::Result<(StatusCode, Value)> {
    post(f, bearer, &format!("review/agent/{decision}"), body).await
}

/// 这一笔里人的回答：状态、谁答的、答没答
fn settled(decision: &Value) -> Value {
    json!([
        decision["status"],
        decision["decided_by"],
        decision["decided_at"].is_null()
    ])
}

/// 审核行里人的裁决：状态、谁裁的、理由、经哪枚令牌
fn ruled(review: &Value) -> Value {
    json!([
        review["status"],
        review["decided_by"],
        review["rationale"],
        review["decided_token"]
    ])
}

/// 这一对的两个实体里有几个并进了别人，以及这一对之间没撤回、撤回了的合并各几次
async fn pair_shape(f: &Fx, review: Uuid) -> anyhow::Result<Value> {
    Ok(sqlx::query_scalar(
        "SELECT jsonb_build_object(
            'merged_away', (SELECT count(*) FROM entities e
                             WHERE e.id IN (rr.left_id, rr.right_id)
                               AND e.merged_into IS NOT NULL),
            'live_merges', (SELECT count(*) FROM entity_merges m
                             WHERE m.source_id IN (rr.left_id, rr.right_id)
                               AND m.target_id IN (rr.left_id, rr.right_id)
                               AND m.reverted_at IS NULL),
            'reverted_merges', (SELECT count(*) FROM entity_merges m
                                 WHERE m.source_id IN (rr.left_id, rr.right_id)
                                   AND m.target_id IN (rr.left_id, rr.right_id)
                                   AND m.reverted_at IS NOT NULL))
           FROM resolution_reviews rr WHERE rr.id = $1",
    )
    .bind(review)
    .fetch_one(&f.pool)
    .await?)
}

/// 指着这一笔治理记录的审计行（人的回答写的那一行）
fn answered_audits(rows: &[Audit], decision: Uuid) -> Vec<&Audit> {
    rows.iter()
        .filter(|a| a.detail["agent_decision"] == json!(decision))
        .collect()
}

#[tokio::test]
async fn a_token_proposed_governor_answer_changes_no_decision_pair_or_fuse() -> anyhow::Result<()> {
    // 治理的开关开着：保险丝有东西可关，人的裁决会排治理任务——提议两样都不许碰
    let Some(f) = Fx::new(true).await? else {
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
        let (open, _) = suggested(&f, "Open Suggestion", "merge").await?;
        let (apart, _) = kept_apart(&f, "Kept Apart").await?;
        let (joined, _, _) = merged(&f, "Joined").await?;
        // 有人答过的一笔
        let (accepted, _) = suggested(&f, "Accepted Suggestion", "keep").await?;
        sqlx::query(
            "UPDATE agent_decisions SET status = 'accepted', decided_at = now(), decided_by = $2
              WHERE id = $1",
        )
        .bind(accepted)
        .bind(f.editor)
        .execute(&f.pool)
        .await?;
        // 建议还开着，它指着的那一对却已经不等人了
        let (orphaned, orphaned_review) = suggested(&f, "Orphaned Suggestion", "merge").await?;
        sqlx::query("UPDATE resolution_reviews SET status = 'kept' WHERE id = $1")
            .bind(orphaned_review)
            .execute(&f.pool)
            .await?;
        // 别的库里的一笔
        let foreign = decision(&f, f.other_kb, Uuid::now_v7(), "merge", "proposed", None).await?;

        let before = snapshot(&f).await?;
        let governed_before = governed(&f).await?;
        let rows_before = ledger(&f).await?.len();
        let audits_before = audits(&f).await?.len();
        anyhow::ensure!(
            governing(&f).await? && governed_before["govern_jobs"] == 0,
            "{governed_before}"
        );

        // ── 没过检查的调用：哪一种都一行不写 ──
        // 写的检查，次序是 scope、库、角色；排在「请求体读不读得出」「目标在不在」前面
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
            for on in [open, accepted, Uuid::now_v7()] {
                for body in [json!({ "action": "keep" }), json!({}), json!([1, 2])] {
                    let (status, answered) = propose(&f, bearer, on, body.clone()).await?;
                    anyhow::ensure!(
                        status == StatusCode::FORBIDDEN && answered["code"] == code,
                        "{who}, {body}: {status} {answered}"
                    );
                }
            }
        }
        // 请求体读不出来：过了写的检查之后才说
        for body in [
            json!({}),
            json!({ "rationale": "no action" }),
            json!({ "action": "keep", "rationale": 7 }),
            json!([1, 2]),
        ] {
            let (status, answered) = propose(&f, &token, open, body.clone()).await?;
            anyhow::ensure!(
                status == StatusCode::UNPROCESSABLE_ENTITY && answered["code"] == "invalid_body",
                "{body}: {status} {answered}"
            );
        }
        // 认不出的、别的库的那一笔
        for on in [Uuid::now_v7(), foreign] {
            let (status, answered) = propose(&f, &token, on, json!({ "action": "keep" })).await?;
            anyhow::ensure!(
                status == StatusCode::NOT_FOUND,
                "an unknown decision: {status} {answered}"
            );
        }
        // 有人答过的、那一对已经不等人的：already_decided，动作对不对都一样
        for (what, on) in [
            ("an answered decision", accepted),
            ("a suggestion whose pair no longer waits", orphaned),
        ] {
            for action in ["merge", "keep", "revert"] {
                let (status, answered) =
                    propose(&f, &token, on, json!({ "action": action })).await?;
                anyhow::ensure!(
                    status == StatusCode::CONFLICT && answered["code"] == "already_decided",
                    "{what}, {action}: {status} {answered}"
                );
            }
        }
        // 动作对不上这一笔的状态：与这个人在他自己的路由上拿到的是同一个回答
        for (what, on, action, code) in [
            ("revert a suggestion", open, "revert", "agent_answer"),
            ("an unknown action", open, "approve", "agent_answer"),
            ("keep a pair already kept", apart, "keep", "agent_answer"),
            ("revert a keep", apart, "revert", "not_a_merge"),
            ("merge a merge", joined, "merge", "agent_answer"),
            ("keep a merge", joined, "keep", "agent_answer"),
        ] {
            let body = json!({ "action": action, "rationale": TOKEN_REASON });
            let by_token = propose(&f, &token, on, body.clone()).await?;
            let by_person = respond(&f, &f.editor_session, on, body).await?;
            anyhow::ensure!(
                by_token.0 == StatusCode::UNPROCESSABLE_ENTITY
                    && by_token.1["code"] == code
                    && by_token == by_person,
                "{what}: {by_token:?} / {by_person:?}"
            );
        }
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before
                && audits(&f).await?.len() == audits_before
                && snapshot(&f).await? == before
                && governed(&f).await? == governed_before,
            "a refusal wrote something"
        );

        // ── 三种对得上的回答各提一条：什么都没动 ──
        let mut proposals = Vec::new();
        for (what, on, action, rationale) in [
            ("keep a suggested merge", open, "keep", Some(TOKEN_REASON)),
            ("revert the governor's merge", joined, "revert", None),
            ("merge a pair the governor kept", apart, "merge", Some("  ")),
        ] {
            let mut body = json!({ "action": action });
            if let Some(rationale) = rationale {
                body["rationale"] = json!(rationale);
            }
            let (status, answered) = propose(&f, &token, on, body).await?;
            anyhow::ensure!(status == StatusCode::OK, "{what}: {status} {answered}");
            proposals.push((what, on, action, proposed(&answered)?));
        }
        anyhow::ensure!(
            governed(&f).await? == governed_before,
            "a proposal changed a governor decision, a pair, a merge, the fuse or queued a \
             govern round"
        );
        anyhow::ensure!(
            snapshot(&f).await? == before,
            "a proposal changed a fact, an entity, evidence or the queued jobs"
        );
        anyhow::ensure!(governing(&f).await?, "the fuse tripped");
        let open_now = open_proposals(&f).await?;
        for (what, on, action, proposal) in &proposals {
            let mine: Vec<_> = open_now.iter().filter(|p| p.target_id == *on).collect();
            anyhow::ensure!(mine.len() == 1, "{what}: one open proposal: {mine:?}");
            let p = mine[0];
            anyhow::ensure!(
                p.id == *proposal
                    && p.operation == "governor.answer"
                    && p.target_kind == "agent_decision"
                    && p.outcome == "proposed"
                    && p.gate_reason.as_deref() == Some("propose_only")
                    && p.owner_id == f.editor
                    && p.token_id == token_id
                    && p.token_name == TOKEN_NAME,
                "{what}: {p:?}"
            );
            // 请求体只存动作；理由只在 `why` 里，空白的理由不留
            anyhow::ensure!(
                p.payload["request"] == json!({ "action": action })
                    && p.payload["action"] == *action
                    && p.payload["target_label"]
                        .as_str()
                        .is_some_and(|said| said.contains(" / ")),
                "{what}: {}",
                p.payload
            );
            if *action == "keep" {
                anyhow::ensure!(
                    keys(&p.payload) == ["action", "request", "target_label", "why"]
                        && p.payload["why"] == TOKEN_REASON,
                    "{what}: {}",
                    p.payload
                );
            } else {
                anyhow::ensure!(
                    keys(&p.payload) == ["action", "request", "target_label"],
                    "{what}: {}",
                    p.payload
                );
            }
        }
        // 重复：同一条提议加 `already_proposed`，换一个对得上的动作也是它
        let (status, again) = propose(&f, &token, open, json!({ "action": "merge" })).await?;
        anyhow::ensure!(
            status == StatusCode::OK
                && again["code"] == "already_proposed"
                && again["action_id"] == json!(proposals[0].3),
            "a repeat: {status} {again}"
        );
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before + 3
                && audits(&f).await?.len() == audits_before + 3,
            "three proposals, three ledger rows, three audit rows"
        );

        // ── 人回答治理的路由：每一枚令牌照旧是 401 ──
        for bearer in [&token, &editor_read, &viewer_write] {
            for (on, action) in [(open, "keep"), (joined, "revert"), (apart, "merge")] {
                let (status, body) = respond(&f, bearer, on, json!({ "action": action })).await?;
                anyhow::ensure!(
                    status == StatusCode::UNAUTHORIZED,
                    "the governor's route, {action}: {status} {body}"
                );
            }
        }
        anyhow::ensure!(
            governed(&f).await? == governed_before && snapshot(&f).await? == before,
            "a refused answer changed something"
        );

        // ── Agent 一档里治理的那一行带着这条提议，说出主人与令牌 ──
        let (status, page) = f
            .call(
                &token,
                "GET",
                &format!("/kbs/{}/review?queue=agent&limit=200", f.kb),
                None,
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {page}");
        let item = page["items"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|item| item["id"] == json!(open))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("the suggestion is not listed: {page}"))?;
        let carried = &item["agent_proposals"];
        anyhow::ensure!(
            carried.as_array().is_some_and(|c| c.len() == 1)
                && carried[0]["id"] == json!(proposals[0].3)
                && carried[0]["operation"] == "governor.answer"
                && carried[0]["action"] == "keep"
                && carried[0]["owner_name"] == "Editor"
                && carried[0]["token_name"] == TOKEN_NAME,
            "{item}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn approving_a_governor_proposal_runs_that_persons_answer() -> anyhow::Result<()> {
    let Some(f) = Fx::new(true).await? else {
        return Ok(());
    };
    let result = async {
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;

        // 四种回答，每一种一对对照：这个人自己答的一笔、令牌提了由这个人批准的一笔。
        // 撤回排在最后：两次撤回会让保险丝跳
        let cases = ["keep", "accept", "override", "revert"];
        for case in cases {
            let (action, mine, theirs) = match case {
                // 建议合并，人说分开：改判
                "keep" => (
                    "keep",
                    suggested(&f, "Mine Overruled", "merge").await?,
                    suggested(&f, "Theirs Overruled", "merge").await?,
                ),
                // 建议合并，人说合并：接受
                "accept" => (
                    "merge",
                    suggested(&f, "Mine Accepted", "merge").await?,
                    suggested(&f, "Theirs Accepted", "merge").await?,
                ),
                // 治理自己分开的一对，人说合并：推翻
                "override" => (
                    "merge",
                    kept_apart(&f, "Mine Kept").await?,
                    kept_apart(&f, "Theirs Kept").await?,
                ),
                // 治理自己合并的一对，人撤回
                _ => {
                    let (mine, mine_review, _) = merged(&f, "Mine Merged").await?;
                    let (theirs, theirs_review, _) = merged(&f, "Theirs Merged").await?;
                    ("revert", (mine, mine_review), (theirs, theirs_review))
                }
            };
            let (mine, mine_review) = mine;
            let (theirs, theirs_review) = theirs;

            // 令牌提一条，带着它的理由：什么都没动
            let governed_before = governed(&f).await?;
            let before = snapshot(&f).await?;
            let (status, answered) = propose(
                &f,
                &token,
                theirs,
                json!({ "action": action, "rationale": TOKEN_REASON }),
            )
            .await?;
            anyhow::ensure!(status == StatusCode::OK, "{case}: {status} {answered}");
            let proposal = proposed(&answered)?;
            anyhow::ensure!(
                governed(&f).await? == governed_before && snapshot(&f).await? == before,
                "{case}: the proposal changed something"
            );

            // 对照：这个人自己答，不写理由。会话的回答同从前，台账一行不写
            let rows_then = ledger(&f).await?.len();
            let (status, own) =
                respond(&f, &f.editor_session, mine, json!({ "action": action })).await?;
            anyhow::ensure!(
                status == StatusCode::OK && own == json!({ "ok": true }),
                "{case}: {status} {own}"
            );
            anyhow::ensure!(
                ledger(&f).await?.len() == rows_then,
                "{case}: a session answer writes no ledger row"
            );
            // 这个人答了他那一笔，令牌提的那一笔原样等着
            anyhow::ensure!(
                row(&f, "agent_decisions", theirs).await?
                    == governed_before["decisions"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .find(|d| d["id"] == json!(theirs))
                        .cloned()
                        .unwrap_or(Value::Null),
                "{case}: the person's own answer touched the other decision"
            );
            if case == "revert" {
                anyhow::ensure!(
                    governing(&f).await?,
                    "one revert of a governor merge does not trip the fuse"
                );
            }

            // 批准：正好 `ok` 与 `state`
            let (status, answered) = answer(&f, &f.editor_session, proposal, "approve").await?;
            anyhow::ensure!(
                status == StatusCode::OK && answered == json!({ "ok": true, "state": "approved" }),
                "{case}: {status} {answered}"
            );
            let (mine_row, theirs_row) = (
                row(&f, "agent_decisions", mine).await?,
                row(&f, "agent_decisions", theirs).await?,
            );
            let expected = match case {
                "keep" | "override" => "overridden",
                "accept" => "accepted",
                _ => "reverted",
            };
            anyhow::ensure!(
                settled(&theirs_row) == settled(&mine_row)
                    && settled(&mine_row) == json!([expected, f.editor, false]),
                "{case}: the decision is settled as the person's own answer settles it\n{theirs_row}\n{mine_row}"
            );
            let (mine_review_row, theirs_review_row) = (
                row(&f, "resolution_reviews", mine_review).await?,
                row(&f, "resolution_reviews", theirs_review).await?,
            );
            anyhow::ensure!(
                ruled(&theirs_review_row) == ruled(&mine_review_row)
                    && theirs_review_row["rationale"].is_null()
                    && theirs_review_row["decided_token"].is_null(),
                "{case}: the pair is ruled as the person's own answer rules it\n{theirs_review_row}\n{mine_review_row}"
            );
            let (mine_pair, theirs_pair) = (
                pair_shape(&f, mine_review).await?,
                pair_shape(&f, theirs_review).await?,
            );
            let expected = match case {
                "keep" => json!({ "merged_away": 0, "live_merges": 0, "reverted_merges": 0 }),
                "accept" | "override" => {
                    json!({ "merged_away": 1, "live_merges": 1, "reverted_merges": 0 })
                }
                _ => json!({ "merged_away": 0, "live_merges": 0, "reverted_merges": 1 }),
            };
            anyhow::ensure!(
                theirs_pair == mine_pair && mine_pair == expected,
                "{case}: the graph changes exactly as the person's own answer changes it\n{theirs_pair}\n{mine_pair}"
            );

            // 审计：同一个动作名、同一种目标，actor 都是这个人；批准的那一行只多指着提议，
            // 不带令牌，也没有令牌的理由
            let rows = audits(&f).await?;
            let own = answered_audits(&rows, mine);
            let approved = answered_audits(&rows, theirs);
            anyhow::ensure!(
                own.len() == 1 && approved.len() == 1,
                "{case}: {own:?} {approved:?}"
            );
            let (own, approved) = (own[0], approved[0]);
            let named = match case {
                "keep" => "review.keep",
                "accept" => "review.merge",
                "override" => "merge.manual",
                _ => "merge.revert",
            };
            anyhow::ensure!(
                own.action == named
                    && approved.action == named
                    && own.actor == Some(f.editor)
                    && approved.actor == Some(f.editor)
                    && own.target_kind == approved.target_kind,
                "{case}: {own:?} / {approved:?}"
            );
            anyhow::ensure!(
                own.detail.get("agent_proposal").is_none(),
                "{case}: a session answer names no proposal: {own:?}"
            );
            let mut expected = keys(&own.detail);
            expected.push("agent_proposal".into());
            expected.sort();
            anyhow::ensure!(
                keys(&approved.detail) == expected
                    && approved.detail["agent_proposal"] == json!(proposal)
                    && approved.detail.get("token_id").is_none()
                    && approved.detail.get("why").is_none_or(Value::is_null)
                    && approved.detail.get("agent_action") == own.detail.get("agent_action"),
                "{case}: the approval is the person's answer and names its proposal: {approved:?}"
            );
            for told in [approved.detail.to_string(), theirs_review_row.to_string()] {
                anyhow::ensure!(
                    !told.contains("TOKEN-RATIONALE"),
                    "{case}: the token's rationale was recorded as the person's: {told}"
                );
            }
            let closed = ledger_row(&f, proposal).await?;
            anyhow::ensure!(
                closed.state.as_deref() == Some("approved")
                    && closed.state_by == Some(f.editor)
                    && closed.outcome == "proposed"
                    && closed.payload["why"] == TOKEN_REASON,
                "{case}: {closed:?}"
            );
            // 再答一次：已经答过了
            let (status, body) = answer(&f, &f.editor_session, proposal, "approve").await?;
            anyhow::ensure!(
                status == StatusCode::CONFLICT && body["code"] == "already_decided",
                "{case}: {status} {body}"
            );
        }
        // 人撤回治理的合并到了两次——第二次是经批准的那一条——保险丝照今天那样跳了
        anyhow::ensure!(
            !governing(&f).await?,
            "the approved revert counts on the fuse as a person's revert does"
        );

        // ── 拒绝：什么都不改 ──
        let (declined, _) = suggested(&f, "Declined Suggestion", "merge").await?;
        let (status, answered) = propose(&f, &token, declined, json!({ "action": "merge" })).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {answered}");
        let proposal = proposed(&answered)?;
        let governed_before = governed(&f).await?;
        let before = snapshot(&f).await?;
        let (status, answered) = answer(&f, &f.editor_session, proposal, "decline").await?;
        anyhow::ensure!(
            status == StatusCode::OK && answered == json!({ "ok": true, "state": "declined" }),
            "{status} {answered}"
        );
        anyhow::ensure!(
            governed(&f).await? == governed_before && snapshot(&f).await? == before,
            "a declined governor proposal changed something"
        );

        // ── 过时：等着的时候这个人自己答了那一笔，批准什么都不改 ──
        let (stale, _) = suggested(&f, "Stale Suggestion", "merge").await?;
        let (status, answered) = propose(&f, &token, stale, json!({ "action": "merge" })).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {answered}");
        let proposal = proposed(&answered)?;
        let (status, body) =
            respond(&f, &f.editor_session, stale, json!({ "action": "keep" })).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let governed_before = governed(&f).await?;
        let before = snapshot(&f).await?;
        let audits_before = audits(&f).await?.len();
        let (status, answered) = answer(&f, &f.editor_session, proposal, "approve").await?;
        anyhow::ensure!(
            status == StatusCode::OK && answered == json!({ "ok": true, "state": "superseded" }),
            "{status} {answered}"
        );
        anyhow::ensure!(
            governed(&f).await? == governed_before && snapshot(&f).await? == before,
            "a superseded governor proposal changed something"
        );
        let rows = audits(&f).await?;
        anyhow::ensure!(
            rows.len() == audits_before + 1
                && audits_of(&rows, "agent.proposal.superseded")
                    .iter()
                    .any(|a| a.target_id == Some(proposal) && a.actor == Some(f.editor)),
            "the supersession is recorded, and nothing else"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn the_mcp_pending_fact_and_governor_tools_propose_and_change_nothing() -> anyhow::Result<()>
{
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (token_id, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let (_, editor_read) = f.token(f.editor, "reader", "read", None).await?;
        let (_, viewer_write) = f.token(f.viewer, "viewer writer", "write", None).await?;
        let (pending, _) = waiting(&f, "Tool Subject").await?;
        let (other_pending, _) = waiting(&f, "Tool Subject").await?;
        let suggestion: Uuid = sqlx::query_scalar(
            "SELECT id FROM agent_decisions WHERE kb_id = $1 AND status = 'proposed' LIMIT 1",
        )
        .bind(f.kb)
        .fetch_one(&f.pool)
        .await?;
        let kept: Uuid = sqlx::query_scalar(
            "SELECT id FROM agent_decisions
              WHERE kb_id = $1 AND status = 'applied' AND action = 'keep' LIMIT 1",
        )
        .bind(f.kb)
        .fetch_one(&f.pool)
        .await?;

        // ── 列出来：只给写得了的调用方，标注是「只提议」的那一组 ──
        let tools = tools_list(&f, &token).await?;
        for name in tools.iter().filter_map(|t| t["name"].as_str()) {
            for word in ["proposal", "approve", "decline", "answer"] {
                anyhow::ensure!(!name.contains(word), "{name} names {word}");
            }
        }
        for (name, properties) in [
            ("review_pending_fact", vec!["action", "id"]),
            ("review_governor", vec!["action", "id", "rationale"]),
        ] {
            let tool = tools
                .iter()
                .find(|t| t["name"] == name)
                .ok_or_else(|| anyhow::anyhow!("{name} is not listed for a write token"))?;
            anyhow::ensure!(
                tool["annotations"]
                    == json!({ "readOnlyHint": false, "destructiveHint": false,
                               "idempotentHint": false }),
                "{tool}"
            );
            let schema = &tool["inputSchema"];
            anyhow::ensure!(
                schema["required"] == json!(["id", "action"])
                    && keys(&schema["properties"]) == properties
                    && schema["properties"]["id"]["format"] == "uuid",
                "{schema}"
            );
            let said = tool["description"].as_str().unwrap_or_default();
            anyhow::ensure!(
                said.starts_with("Propose that a person")
                    && said.ends_with("No tool answers a proposal."),
                "{name}: {said}"
            );
            for phrase in [
                "The call changes nothing",
                "a signed-in editor answers in the Review tab",
                "{ok, outcome: \"proposed\", action_id, reason: \"propose_only\", explanation}",
                "already_proposed",
            ] {
                anyhow::ensure!(
                    said.contains(phrase),
                    "{name} does not say `{phrase}`: {said}"
                );
            }
            for (who, bearer) in [
                ("a read token", &editor_read),
                ("a write token of a viewer", &viewer_write),
            ] {
                anyhow::ensure!(
                    tools_list(&f, bearer)
                        .await?
                        .iter()
                        .all(|t| t["name"] != name),
                    "{name} is listed for {who}"
                );
            }
        }

        let before = snapshot(&f).await?;
        let nods_before = nod_state(&f).await?;
        let suggestion_row = row(&f, "agent_decisions", suggestion).await?;
        let kept_row = row(&f, "agent_decisions", kept).await?;
        let reviews_before: Value = sqlx::query_scalar(
            "SELECT coalesce(jsonb_agg(to_jsonb(r) ORDER BY r.id), '[]'::jsonb)
               FROM resolution_reviews r WHERE r.kb_id = $1",
        )
        .bind(f.kb)
        .fetch_one(&f.pool)
        .await?;
        let base_before: Value =
            sqlx::query_scalar("SELECT to_jsonb(k) FROM knowledge_bases k WHERE k.id = $1")
                .bind(f.kb)
                .fetch_one(&f.pool)
                .await?;
        let rows_before = ledger(&f).await?.len();

        // ── 没列给它的调用方照样被拒，拿到的是 REST 的 code，不是「没有这个工具」 ──
        for (bearer, code) in [
            (&editor_read, "token_read_only"),
            (&viewer_write, "token_owner_below_editor"),
        ] {
            let result = tool_call(
                &f,
                bearer,
                "review_pending_fact",
                json!({ "id": pending, "action": "confirm" }),
            )
            .await?;
            ensure_tool_refused(&result, code)?;
            let result = tool_call(
                &f,
                bearer,
                "review_governor",
                json!({ "id": suggestion, "action": "keep" }),
            )
            .await?;
            ensure_tool_refused(&result, code)?;
        }
        // 操作自己的拒绝：与 REST 同一个 code
        let result = tool_call(
            &f,
            &token,
            "review_pending_fact",
            json!({ "id": pending, "action": "nod" }),
        )
        .await?;
        ensure_tool_refused(&result, "unknown_action")?;
        for (on, action, code) in [
            (suggestion, "revert", "agent_answer"),
            (kept, "keep", "agent_answer"),
            (kept, "revert", "not_a_merge"),
            (Uuid::now_v7(), "keep", "not_found"),
        ] {
            let result = tool_call(
                &f,
                &token,
                "review_governor",
                json!({ "id": on, "action": action }),
            )
            .await?;
            ensure_tool_refused(&result, code)?;
        }
        // 参数没说清：没有执行，不带 code
        for (name, args) in [
            ("review_pending_fact", json!({ "id": pending })),
            (
                "review_pending_fact",
                json!({ "id": "nope", "action": "confirm" }),
            ),
            ("review_governor", json!({ "id": suggestion })),
            (
                "review_governor",
                json!({ "id": suggestion, "action": "keep", "rationale": 7 }),
            ),
        ] {
            let result = tool_call(&f, &token, name, args.clone()).await?;
            anyhow::ensure!(
                result["isError"] == true && result["structuredContent"].get("code").is_none(),
                "{name} {args}: {result}"
            );
        }
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before,
            "a refused tool call left a ledger row"
        );

        // ── 等点头的事实：工具的回答就是 REST 的那个 body，那一行一个字节都不差 ──
        let pending_row = row(&f, "pending_facts", pending).await?;
        let result = tool_call(
            &f,
            &token,
            "review_pending_fact",
            json!({ "id": pending, "action": "confirm" }),
        )
        .await?;
        anyhow::ensure!(result["isError"] != true, "{result}");
        let by_tool = proposed(&result["structuredContent"])?;
        // REST 再提一次（反过来答）：同一条提议加 `already_proposed`
        let (status, again) = post(
            &f,
            &token,
            &format!("review/pending/{pending}/propose"),
            json!({ "action": "reject" }),
        )
        .await?;
        anyhow::ensure!(
            status == StatusCode::OK
                && again["code"] == "already_proposed"
                && again["action_id"] == json!(by_tool),
            "{status} {again}"
        );
        // 反过来：REST 先提，工具再提，工具的回答与 REST 的重复回答是同一个
        let (status, first) = post(
            &f,
            &token,
            &format!("review/pending/{other_pending}/propose"),
            json!({ "action": "reject" }),
        )
        .await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {first}");
        let by_rest = proposed(&first)?;
        let (_, repeat) = post(
            &f,
            &token,
            &format!("review/pending/{other_pending}/propose"),
            json!({ "action": "reject" }),
        )
        .await?;
        let result = tool_call(
            &f,
            &token,
            "review_pending_fact",
            json!({ "id": other_pending, "action": "reject" }),
        )
        .await?;
        anyhow::ensure!(
            result["isError"] != true
                && result["structuredContent"] == repeat
                && repeat["action_id"] == json!(by_rest)
                && repeat["code"] == "already_proposed",
            "{result} / {repeat}"
        );
        anyhow::ensure!(
            row(&f, "pending_facts", pending).await? == pending_row
                && nod_state(&f).await? == nods_before,
            "review_pending_fact changed a pending fact, a rejection or queued a time job"
        );

        // ── 治理的一笔：同样只留提议，令牌的理由留在提议上 ──
        let result = tool_call(
            &f,
            &token,
            "review_governor",
            json!({ "id": suggestion, "action": "keep", "rationale": TOKEN_REASON }),
        )
        .await?;
        anyhow::ensure!(result["isError"] != true, "{result}");
        let on_suggestion = proposed(&result["structuredContent"])?;
        let result = tool_call(
            &f,
            &token,
            "review_governor",
            json!({ "id": kept, "action": "merge" }),
        )
        .await?;
        anyhow::ensure!(result["isError"] != true, "{result}");
        let on_kept = proposed(&result["structuredContent"])?;
        let (status, again) = post(
            &f,
            &token,
            &format!("review/agent/{suggestion}/propose"),
            json!({ "action": "merge" }),
        )
        .await?;
        anyhow::ensure!(
            status == StatusCode::OK
                && again["code"] == "already_proposed"
                && again["action_id"] == json!(on_suggestion),
            "{status} {again}"
        );
        let reviews_after: Value = sqlx::query_scalar(
            "SELECT coalesce(jsonb_agg(to_jsonb(r) ORDER BY r.id), '[]'::jsonb)
               FROM resolution_reviews r WHERE r.kb_id = $1",
        )
        .bind(f.kb)
        .fetch_one(&f.pool)
        .await?;
        let base_after: Value =
            sqlx::query_scalar("SELECT to_jsonb(k) FROM knowledge_bases k WHERE k.id = $1")
                .bind(f.kb)
                .fetch_one(&f.pool)
                .await?;
        anyhow::ensure!(
            row(&f, "agent_decisions", suggestion).await? == suggestion_row
                && row(&f, "agent_decisions", kept).await? == kept_row
                && reviews_after == reviews_before
                && base_after == base_before,
            "review_governor changed a governor decision, a pair or the fuse"
        );
        anyhow::ensure!(
            snapshot(&f).await? == before,
            "a tool call changed a fact, an entity, evidence or the queued jobs"
        );

        // 台账：四条开着的提议，都记在这枚令牌与它的主人名下
        let open = open_proposals(&f).await?;
        for (proposal, operation, target, action) in [
            (by_tool, "pending.nod", pending, "confirm"),
            (by_rest, "pending.nod", other_pending, "reject"),
            (on_suggestion, "governor.answer", suggestion, "keep"),
            (on_kept, "governor.answer", kept, "merge"),
        ] {
            let p = open
                .iter()
                .find(|p| p.id == proposal)
                .ok_or_else(|| anyhow::anyhow!("{operation} proposal {proposal} is not open"))?;
            anyhow::ensure!(
                p.operation == operation
                    && p.target_id == target
                    && p.outcome == "proposed"
                    && p.gate_reason.as_deref() == Some("propose_only")
                    && p.owner_id == f.editor
                    && p.token_id == token_id
                    && p.token_name == TOKEN_NAME
                    && p.payload["request"] == json!({ "action": action }),
                "{p:?}"
            );
        }
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before + 4,
            "four proposals, four ledger rows"
        );
        let on_suggestion_row = ledger_row(&f, on_suggestion).await?;
        anyhow::ensure!(
            on_suggestion_row.payload["why"] == TOKEN_REASON,
            "{on_suggestion_row:?}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}
