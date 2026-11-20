//! 一致性检查与物化推导的令牌调用，以及直接落地的写入的限速（0076）。
//!
//! 裁定表里「跑一致性检查 / 推导」是「直接落地」：编辑者的写令牌跑一遍，跑的就是人跑的
//! 那一段（没有新加的锁），回的就是人拿到的报告，台账里留一行 `applied`（目标是库本身），
//! 审计行记着主人、令牌的 id 与名字、台账行的 id。
//!
//! - **每跑一次一行**（E14）：REST 与 `review_check` 都是；会话跑不写台账，审计行同从前
//! - **推导的开关关着**（E14）：`inference_off`，一行不写——排在限速前面
//! - **限速**（E15、N7）：映射的决定、检查、推导都数进一枚令牌在一个库里一小时 200 笔
//!   落地的写入；第 200 笔之后的那一次是 429 `agent_rate_limited`，什么都不改、一行不写
//!
//! 夹具在 `review_token_write_support.rs`。没有 `UTOPIA_DATABASE_URL` 时跳过。
use axum::http::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;

use super::review_token_write_support::{
    audits, ensure_tool_refused, keys, ledger, post, row, seeded, snapshot, tool_call, Audit, Fx,
};

/// 把这个库的物化推导开关拨到 `on`
async fn set_inference(f: &Fx, on: bool) -> anyhow::Result<()> {
    sqlx::query("UPDATE knowledge_bases SET materialize_inferences = $2 WHERE id = $1")
        .bind(f.kb)
        .bind(on)
        .execute(&f.pool)
        .await?;
    Ok(())
}

/// 这个库某个动作的审计行，最早的在前
async fn audited(f: &Fx, action: &str) -> anyhow::Result<Vec<Audit>> {
    Ok(audits(f)
        .await?
        .into_iter()
        .filter(|a| a.action == action)
        .collect())
}

/// 运行会动的那几张表在这个库里的每一行：违规、本体缺陷、派生事实。两次相等就是没跑
async fn reasoning_rows(f: &Fx) -> anyhow::Result<Value> {
    Ok(sqlx::query_scalar(
        "SELECT jsonb_build_object(
            'violations', (SELECT coalesce(jsonb_agg(to_jsonb(v) ORDER BY v.id), '[]'::jsonb)
                             FROM axiom_violations v WHERE v.kb_id = $1),
            'defects', (SELECT coalesce(jsonb_agg(to_jsonb(d) ORDER BY d.id), '[]'::jsonb)
                          FROM ontology_defects d WHERE d.kb_id = $1),
            'derived', (SELECT coalesce(jsonb_agg(to_jsonb(x) ORDER BY x.id), '[]'::jsonb)
                          FROM derived_facts x WHERE x.kb_id = $1))",
    )
    .bind(f.kb)
    .fetch_one(&f.pool)
    .await?)
}

/// 直接往台账里种 `n` 行这枚令牌在这个库里刚落地的写入（与 `review_token_guard_tests`
/// 同一个种法）：限速数的就是这些行
async fn seed_applied(f: &Fx, token_id: Uuid, n: i32) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO token_agent_actions
             (id, kb_id, operation, target_kind, target_id, owner_id, token_id, token_name,
              outcome, payload, target_key, created_at)
         SELECT gen_random_uuid(), $1, 'review.keep', 'review', gen_random_uuid(), $2, $3,
                'seeded', 'applied', '{}'::jsonb, gen_random_uuid()::text,
                now() - make_interval(mins => 5)
           FROM generate_series(1, $4)",
    )
    .bind(f.kb)
    .bind(f.editor)
    .bind(token_id)
    .bind(n)
    .execute(&f.pool)
    .await?;
    Ok(())
}

/// 这枚令牌在这个库里落地（`applied`）的行数
async fn applied_rows(f: &Fx, token_id: Uuid) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "SELECT count(*) FROM token_agent_actions
          WHERE kb_id = $1 AND token_id = $2 AND outcome = 'applied'",
    )
    .bind(f.kb)
    .bind(token_id)
    .fetch_one(&f.pool)
    .await?)
}

/// 会话把一种运行跑到稳定（第一遍会新记违规、新落派生，之后每一遍的报告都一样），
/// 回稳定下来的那一份报告
async fn settled(f: &Fx, tail: &str) -> anyhow::Result<Value> {
    let mut last = Value::Null;
    for _ in 0..4 {
        let (status, body) = post(f, &f.editor_session, tail, json!({})).await?;
        anyhow::ensure!(status == StatusCode::OK, "{tail}: {status} {body}");
        if body == last {
            return Ok(body);
        }
        last = body;
    }
    anyhow::bail!("{tail} never settled: {last}")
}

#[tokio::test]
async fn a_token_consistency_check_and_inference_run_each_write_one_applied_row(
) -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        set_inference(&f, true).await?;
        let (token_id, token) = f.token(f.editor, "run agent", "write", None).await?;
        let kb_name: String = sqlx::query_scalar("SELECT name FROM knowledge_bases WHERE id = $1")
            .bind(f.kb)
            .fetch_one(&f.pool)
            .await?;

        for (tail, run, operation, action, report_keys) in [
            (
                "consistency/check",
                "consistency",
                "consistency.check",
                "consistency.checked",
                vec![
                    "classes",
                    "cleared",
                    "cycles_capped",
                    "defects_found",
                    "defects_new",
                    "edges",
                    "found",
                    "inserted",
                    "predicates_with_axioms",
                ],
            ),
            (
                "inference/run",
                "inference",
                "inference.run",
                "inference.materialized",
                vec![
                    "capped",
                    "derived",
                    "edges",
                    "inserted",
                    "invalidated",
                    "rules",
                ],
            ),
        ] {
            // 会话先跑到稳定：那一份报告就是「人拿到的报告」，会话不写台账
            let report = settled(&f, tail).await?;
            anyhow::ensure!(keys(&report) == report_keys, "{tail}: {report}");
            let session_audits = audited(&f, action).await?;
            let session_detail = session_audits.last().expect("a session row").detail.clone();
            anyhow::ensure!(
                session_audits.iter().all(|a| a.actor == Some(f.editor)
                    && a.detail.get("token_id").is_none()
                    && a.detail.get("token_name").is_none()
                    && a.detail.get("action_id").is_none()),
                "{tail}: a session row names no token: {session_audits:?}"
            );
            let of_run = |rows: &[utopia_store::token_agent_actions::TokenAgentAction]| {
                rows.iter()
                    .filter(|r| r.operation == operation)
                    .cloned()
                    .collect::<Vec<_>>()
            };
            anyhow::ensure!(
                of_run(&ledger(&f).await?).is_empty(),
                "{tail}: a session run writes no ledger row"
            );

            // 令牌跑一遍：人拿到的报告，一行 applied，目标是库本身
            let before = reasoning_rows(&f).await?;
            let (status, body) = post(&f, &token, tail, json!({})).await?;
            anyhow::ensure!(status == StatusCode::OK, "{tail}: {status} {body}");
            anyhow::ensure!(body == report, "{tail}\n{body}\n{report}");
            anyhow::ensure!(
                reasoning_rows(&f).await? == before,
                "{tail}: a settled base is left as a person's run leaves it"
            );
            let rows = of_run(&ledger(&f).await?);
            anyhow::ensure!(rows.len() == 1, "{tail}: one ledger row: {rows:?}");
            let applied = &rows[0];
            anyhow::ensure!(
                applied.outcome == "applied"
                    && applied.target_kind == "knowledge_base"
                    && applied.target_id == f.kb
                    && applied.gate_reason.is_none()
                    && applied.owner_id == f.editor
                    && applied.token_id == token_id
                    && applied.token_name == "run agent",
                "{applied:?}"
            );
            anyhow::ensure!(
                applied.payload == json!({ "request": {}, "target_label": kb_name }),
                "{tail}: {}",
                applied.payload
            );
            let key: String =
                sqlx::query_scalar("SELECT target_key FROM token_agent_actions WHERE id = $1")
                    .bind(applied.id)
                    .fetch_one(&f.pool)
                    .await?;
            anyhow::ensure!(key == f.kb.to_string(), "{tail}: target_key {key}");

            // 审计：人的那一份 detail，加上令牌的 id、名字与台账行的 id；actor 是主人
            let all = audited(&f, action).await?;
            anyhow::ensure!(
                all.len() == session_audits.len() + 1,
                "{tail}: one audit row"
            );
            let by_token = all.last().expect("the token's row");
            let mut expected = session_detail.clone();
            expected["token_id"] = json!(token_id);
            expected["token_name"] = json!("run agent");
            expected["action_id"] = json!(applied.id);
            anyhow::ensure!(
                by_token.actor == Some(f.editor)
                    && by_token.target_kind == "knowledge_base"
                    && by_token.target_id == Some(f.kb)
                    && by_token.detail == expected,
                "{tail}: {by_token:?}\n{expected}"
            );

            // 经工具再跑一遍：`structuredContent` 就是 REST 的那一份，再多一行
            let by_tool = tool_call(&f, &token, "review_check", json!({ "run": run })).await?;
            anyhow::ensure!(
                by_tool["isError"] == false && by_tool["structuredContent"] == report,
                "{tail}: {by_tool}"
            );
            let rows = of_run(&ledger(&f).await?);
            anyhow::ensure!(
                rows.len() == 2 && rows.iter().all(|r| r.outcome == "applied"),
                "{tail}: each run writes one row: {rows:?}"
            );
            anyhow::ensure!(audited(&f, action).await?.len() == session_audits.len() + 2);

            // Agent 一档：这一行说得出目标（库的名字）与「主人经哪一枚令牌」
            let (status, history) = f
                .call(
                    &f.editor_session,
                    "GET",
                    &format!("/kbs/{}/review?queue=agent&limit=200", f.kb),
                    None,
                )
                .await?;
            anyhow::ensure!(status == StatusCode::OK, "{status} {history}");
            let shown = history["items"]
                .as_array()
                .and_then(|items| items.iter().find(|i| i["id"] == json!(applied.id)))
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("{tail}: not in Agent History: {history}"))?;
            anyhow::ensure!(
                shown["source"] == "token"
                    && shown["operation"] == operation
                    && shown["outcome"] == "applied"
                    && shown["owner_name"] == "Editor"
                    && shown["token_name"] == "run agent"
                    && shown["target_label"] == json!(kb_name)
                    && !kb_name.trim().is_empty()
                    && shown.get("action").is_none(),
                "{tail}: {shown}"
            );
        }

        // 认不出的 run：操作自己的拒绝，一行不写
        let rows_before = ledger(&f).await?.len();
        let by_tool = tool_call(&f, &token, "review_check", json!({ "run": "everything" })).await?;
        ensure_tool_refused(&by_tool, "unknown_run")?;
        anyhow::ensure!(ledger(&f).await?.len() == rows_before);
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn inference_on_a_base_with_the_setting_off_answers_inference_off_and_writes_no_row(
) -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        set_inference(&f, false).await?;
        let (token_id, token) = f.token(f.editor, "run agent", "write", None).await?;
        let rows_before = ledger(&f).await?.len();
        let before = (snapshot(&f).await?, reasoning_rows(&f).await?);

        // 与会话同一个状态、同一个 body
        let by_session = post(&f, &f.editor_session, "inference/run", json!({})).await?;
        let by_token = post(&f, &token, "inference/run", json!({})).await?;
        anyhow::ensure!(
            by_token == by_session && by_token.1["code"] == "inference_off",
            "{by_token:?} {by_session:?}"
        );
        let by_tool = tool_call(&f, &token, "review_check", json!({ "run": "inference" })).await?;
        ensure_tool_refused(&by_tool, "inference_off")?;

        // 开关排在限速前面：这枚令牌已经用完了这一小时的额度，答的仍是 `inference_off`
        seed_applied(&f, token_id, 200).await?;
        let (status, body) = post(&f, &token, "inference/run", json!({})).await?;
        anyhow::ensure!(
            status == by_session.0 && body["code"] == "inference_off",
            "{status} {body}"
        );
        let by_tool = tool_call(&f, &token, "review_check", json!({ "run": "inference" })).await?;
        ensure_tool_refused(&by_tool, "inference_off")?;

        // 什么都没跑、一行没写：台账只多了种下的那 200 行，没有推导的审计行
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before + 200
                && applied_rows(&f, token_id).await? == 200,
            "no ledger row for a refused run"
        );
        anyhow::ensure!(
            !ledger(&f)
                .await?
                .iter()
                .any(|r| r.operation == "inference.run"),
            "no inference.run row"
        );
        anyhow::ensure!(audited(&f, "inference.materialized").await?.is_empty());
        anyhow::ensure!(
            (snapshot(&f).await?, reasoning_rows(&f).await?) == before,
            "nothing ran"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn the_call_after_the_200th_finalized_write_answers_429_and_changes_nothing(
) -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        set_inference(&f, true).await?;
        let (token_id, token) = f.token(f.editor, "busy agent", "write", None).await?;
        let mapping: Uuid = sqlx::query_scalar("SELECT id FROM concept_mappings WHERE kb_id = $1")
            .bind(f.kb)
            .fetch_one(&f.pool)
            .await?;
        let mapping_tail = format!("review/mappings/{mapping}");
        // 两种运行先由会话跑到稳定：之后「什么都没改」比的是一个不再变的库
        settled(&f, "consistency/check").await?;
        settled(&f, "inference/run").await?;

        // 这一小时里已经落地 197 笔；下面三笔——映射、检查、推导——各落地一次，各数一笔
        seed_applied(&f, token_id, 197).await?;
        let (status, body) =
            post(&f, &token, &mapping_tail, json!({ "status": "confirmed" })).await?;
        anyhow::ensure!(status == StatusCode::OK, "the 198th: {status} {body}");
        anyhow::ensure!(applied_rows(&f, token_id).await? == 198);
        let (status, body) = post(&f, &token, "consistency/check", json!({})).await?;
        anyhow::ensure!(status == StatusCode::OK, "the 199th: {status} {body}");
        anyhow::ensure!(applied_rows(&f, token_id).await? == 199);
        let (status, body) = post(&f, &token, "inference/run", json!({})).await?;
        anyhow::ensure!(status == StatusCode::OK, "the 200th: {status} {body}");
        anyhow::ensure!(
            applied_rows(&f, token_id).await? == 200,
            "each of the three wrote one applied row"
        );

        // 第 200 笔之后：三样各自被拒，429 `agent_rate_limited`，REST 与工具都是
        let before = (
            snapshot(&f).await?,
            reasoning_rows(&f).await?,
            row(&f, "concept_mappings", mapping).await?,
            ledger(&f).await?.len(),
        );
        let audits_before = (
            audited(&f, "mapping.decided").await?.len(),
            audited(&f, "consistency.checked").await?.len(),
            audited(&f, "inference.materialized").await?.len(),
        );
        for (what, tail, body) in [
            (
                "a mapping decision",
                mapping_tail.as_str(),
                json!({ "status": "rejected" }),
            ),
            ("a consistency check", "consistency/check", json!({})),
            ("an inference run", "inference/run", json!({})),
        ] {
            let (status, answer) = post(&f, &token, tail, body).await?;
            anyhow::ensure!(
                status == StatusCode::TOO_MANY_REQUESTS && answer["code"] == "agent_rate_limited",
                "{what}: {status} {answer}"
            );
        }
        for (name, args) in [
            (
                "review_mapping",
                json!({ "id": mapping, "status": "rejected" }),
            ),
            ("review_check", json!({ "run": "consistency" })),
            ("review_check", json!({ "run": "inference" })),
        ] {
            let by_tool = tool_call(&f, &token, name, args).await?;
            ensure_tool_refused(&by_tool, "agent_rate_limited")?;
        }
        anyhow::ensure!(
            (
                snapshot(&f).await?,
                reasoning_rows(&f).await?,
                row(&f, "concept_mappings", mapping).await?,
                ledger(&f).await?.len(),
            ) == before,
            "a rate-limited call changes nothing and writes no ledger row"
        );
        anyhow::ensure!(applied_rows(&f, token_id).await? == 200, "no applied row");
        anyhow::ensure!(
            (
                audited(&f, "mapping.decided").await?.len(),
                audited(&f, "consistency.checked").await?.len(),
                audited(&f, "inference.materialized").await?.len(),
            ) == audits_before,
            "a rate-limited call writes no decision audit row"
        );

        // 限速是这枚令牌的：同一个人的另一枚令牌照常落地
        let (_, other) = f.token(f.editor, "fresh agent", "write", None).await?;
        let (status, body) = post(&f, &other, "consistency/check", json!({})).await?;
        anyhow::ensure!(status == StatusCode::OK, "another token: {status} {body}");
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}
