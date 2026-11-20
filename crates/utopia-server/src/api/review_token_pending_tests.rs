//! 令牌对一条等点头的事实只能**提议**怎么答：它点不了头，人答了才算（0076，SC5）。
//!
//! 这是宪章里最硬的一条：令牌不许经任何一条路径确认或否决一条等点头的事实。所以令牌
//! 不从人的点头路由进——那条路由、按句取的那条读路由照旧只收会话，令牌拿到的是 401——
//! 它走自己的 `POST /kbs/{id}/review/pending/{pending_id}/propose`，只留一条提议。
//! 要钉住的行为：
//!
//! - **令牌的调用之后那一行一个字节都不差**（E17 / N4）：`pending_facts` 整张表、拒绝记录、
//!   这个库的每一条事实与实体、排着的任务前后相等；台账里多一条开着的提议
//! - **认不出的、别的库的、已经点过头的 id 是 404**（E17），哪一种都不留提议
//! - **人批准之后才是点头，而且是这个人的点头**（E18 / N2）：落下的事实、拒绝记录与他自己
//!   点头的一模一样，审计行（`fact.nod_confirmed` / `fact.nod_rejected`）的 actor 是这个人、
//!   detail 里指着提议、不带 `token_id`；拒绝提议时那一行原样等着
//! - **这两条 `…/propose` 路由只收令牌**：会话来的是 403 `token_required`，什么都不做
//! - **条目上看得出是令牌提的**（N5）：带提议的条目多一个 `agent_proposals`，说出主人与
//!   令牌的名字；它原来的每一个字段（谁记下的那一句也在内）都还在
//!
//! 等点头的事实直接种进 `pending_facts`（照种子里的那一行），不跑记忆的模型任务。
//!
//! 夹具在 `review_token_write_support.rs`。没有 `UTOPIA_DATABASE_URL` 时跳过。
use axum::http::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;

use super::review_token_fact_tests::{audits_of, ledger_row, proposed, without_proposal};
use super::review_token_write_support::{
    answer, audits, keys, ledger, open_proposals, post, row, seeded, snapshot, tool_call, Audit, Fx,
};

const TOKEN_NAME: &str = "nod agent";

/// 一条新的等点头的事实：照种子里那条开放陈述（`advises`）抄一行，主语是一个新实体，
/// 记下它的人是 `f.viewer`。主语的名字由调用方给——两条对照的事实同名，审计快照才逐字
/// 相等。回 (等点头的那一行, 主语)
pub(super) async fn waiting(f: &Fx, subject: &str) -> anyhow::Result<(Uuid, Uuid)> {
    let subject = f.entity(f.person_type, subject).await?;
    let id = Uuid::now_v7();
    let inserted = sqlx::query(
        "INSERT INTO pending_facts (id, kb_id, subject_id, object_id, proposed_predicate,
                                    confidence, chunk_id, phrase, proposed_by)
         SELECT $1, p.kb_id, $2, p.object_id, p.proposed_predicate, p.confidence, p.chunk_id,
                p.phrase, $3
           FROM pending_facts p
          WHERE p.kb_id = $4 AND p.phrase = 'advises' AND p.proposed_by IS NULL
          ORDER BY p.created_at, p.id LIMIT 1",
    )
    .bind(id)
    .bind(subject)
    .bind(f.viewer)
    .bind(f.kb)
    .execute(&f.pool)
    .await?
    .rows_affected();
    anyhow::ensure!(
        inserted == 1,
        "the seeded `advises` pending fact is missing"
    );
    Ok((id, subject))
}

/// 点头这件事碰得到的一切（E17）：这个库的 `pending_facts` 每一行的每一列、拒绝记录的
/// 每一行、这个库的文档上排着的时间解析任务数。两次相等，就是没有谁点过头、摇过头
pub(super) async fn nod_state(f: &Fx) -> anyhow::Result<Value> {
    Ok(sqlx::query_scalar(
        "SELECT jsonb_build_object(
            'pending', (SELECT coalesce(jsonb_agg(to_jsonb(p) ORDER BY p.id), '[]'::jsonb)
                          FROM pending_facts p WHERE p.kb_id = $1),
            'rejected', (SELECT coalesce(jsonb_agg(to_jsonb(r) ORDER BY to_jsonb(r)::text),
                                         '[]'::jsonb)
                           FROM rejected_facts r WHERE r.kb_id = $1),
            'time_jobs', (SELECT count(*) FROM jobs j
                           WHERE j.kind = 'resolve_time'
                             AND j.payload ->> 'document_id' IN
                                 (SELECT d.id::text FROM documents d WHERE d.kb_id = $1)))",
    )
    .bind(f.kb)
    .fetch_one(&f.pool)
    .await?)
}

/// 令牌自己的那条路由
pub(super) async fn propose(
    f: &Fx,
    bearer: &str,
    pending: Uuid,
    body: Value,
) -> anyhow::Result<(StatusCode, Value)> {
    post(
        f,
        bearer,
        &format!("review/pending/{pending}/propose"),
        body,
    )
    .await
}

/// 人的点头路由
async fn nod(
    f: &Fx,
    bearer: &str,
    pending: Uuid,
    action: &str,
) -> anyhow::Result<(StatusCode, Value)> {
    post(
        f,
        bearer,
        &format!("review/pending/{pending}"),
        json!({ "action": action }),
    )
    .await
}

/// 「等你点头」一档里的这一条，经列表路由读
async fn listed(f: &Fx, bearer: &str, pending: Uuid) -> anyhow::Result<Value> {
    let (status, body) = f
        .call(
            bearer,
            "GET",
            &format!("/kbs/{}/review?queue=pending&limit=200", f.kb),
            None,
        )
        .await?;
    anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
    item_in(&body, pending)
}

fn item_in(page: &Value, pending: Uuid) -> anyhow::Result<Value> {
    page["items"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|item| item["id"] == json!(pending))
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("{pending} is not listed as pending"))
}

/// 这个主语名下的事实：谓词、宾语、短语、层、置信度、活没活着，各带几条证据。
/// 不含 id 与时刻——两个主语各落一条，比的是落成了什么样
async fn said_of(f: &Fx, subject: Uuid) -> anyhow::Result<Value> {
    Ok(sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(jsonb_build_object(
                    'predicate', t.predicate_id, 'object', t.object_id, 'value', t.object_value,
                    'phrase', t.phrase, 'layer', t.layer, 'confidence', t.confidence,
                    'live', t.invalidated_at IS NULL, 'valid_from', t.valid_from,
                    'valid_to', t.valid_to,
                    'evidence', (SELECT count(*) FROM fact_evidence v WHERE v.fact_id = t.id))
                  ORDER BY t.phrase, t.object_id), '[]'::jsonb)
           FROM facts t WHERE t.kb_id = $1 AND t.subject_id = $2",
    )
    .bind(f.kb)
    .bind(subject)
    .fetch_one(&f.pool)
    .await?)
}

/// 这个主语名下的拒绝记录：谁拒的、哪句话
async fn refused_of(f: &Fx, subject: Uuid) -> anyhow::Result<Value> {
    Ok(sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(jsonb_build_object(
                    'predicate', r.predicate_id, 'object', r.object_id,
                    'by', r.rejected_by, 'phrase', r.phrase)), '[]'::jsonb)
           FROM rejected_facts r WHERE r.kb_id = $1 AND r.subject_id = $2",
    )
    .bind(f.kb)
    .bind(subject)
    .fetch_one(&f.pool)
    .await?)
}

/// 把这个库的文档上排着的时间解析任务清掉：下一次点头排不排得出一条才看得出来
async fn clear_time_jobs(f: &Fx) -> anyhow::Result<()> {
    sqlx::query(
        "DELETE FROM jobs j WHERE j.kind = 'resolve_time'
            AND j.payload ->> 'document_id' IN
                (SELECT d.id::text FROM documents d WHERE d.kb_id = $1)",
    )
    .bind(f.kb)
    .execute(&f.pool)
    .await?;
    Ok(())
}

#[tokio::test]
async fn a_token_proposal_about_a_pending_fact_leaves_the_pending_row_byte_equal(
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
        let (to_confirm, _) = waiting(&f, "Nod Subject").await?;
        let (to_reject, _) = waiting(&f, "Nod Subject").await?;
        let (untouched, _) = waiting(&f, "Nod Subject").await?;

        let before = snapshot(&f).await?;
        let nods_before = nod_state(&f).await?;
        let confirm_row = row(&f, "pending_facts", to_confirm).await?;
        let reject_row = row(&f, "pending_facts", to_reject).await?;
        let rows_before = ledger(&f).await?.len();
        let audits_before = audits(&f).await?.len();
        let item_before = listed(&f, &f.editor_session, to_confirm).await?;
        let plain_before = listed(&f, &f.editor_session, untouched).await?;
        anyhow::ensure!(
            item_before.get("agent_proposals").is_none()
                && item_before["proposed_by_name"] == "Viewer",
            "{item_before}"
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
            for on in [to_confirm, Uuid::now_v7()] {
                for body in [
                    json!({ "action": "confirm" }),
                    json!({ "action": "reject" }),
                    json!({}),
                    json!("confirm"),
                ] {
                    let (status, answered) = propose(&f, bearer, on, body.clone()).await?;
                    anyhow::ensure!(
                        status == StatusCode::FORBIDDEN && answered["code"] == code,
                        "{who}, {body}: {status} {answered}"
                    );
                }
            }
        }
        // 请求体读不出来：过了写的检查之后才说
        for body in [json!({}), json!({ "action": 7 }), json!("confirm")] {
            let (status, answered) = propose(&f, &token, to_confirm, body.clone()).await?;
            anyhow::ensure!(
                status == StatusCode::UNPROCESSABLE_ENTITY && answered["code"] == "invalid_body",
                "{body}: {status} {answered}"
            );
        }
        // 认不出的动作：与人的点头路由同一个 code、同一句话
        let by_token = propose(&f, &token, to_confirm, json!({ "action": "nod" })).await?;
        let by_person = nod(&f, &f.editor_session, to_confirm, "nod").await?;
        anyhow::ensure!(
            by_token.0 == StatusCode::UNPROCESSABLE_ENTITY
                && by_token.1["code"] == "unknown_action"
                && by_token == by_person,
            "{by_token:?} / {by_person:?}"
        );
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before
                && audits(&f).await?.len() == audits_before
                && snapshot(&f).await? == before
                && nod_state(&f).await? == nods_before,
            "a refusal wrote something"
        );

        // ── 确认与否决各提一条：回的是提议，那两行一个字节都不差 ──
        let (status, answered) =
            propose(&f, &token, to_confirm, json!({ "action": "confirm" })).await?;
        anyhow::ensure!(status == StatusCode::OK, "confirm: {status} {answered}");
        let confirm_proposal = proposed(&answered)?;
        let (status, answered) =
            propose(&f, &token, to_reject, json!({ "action": "reject" })).await?;
        anyhow::ensure!(status == StatusCode::OK, "reject: {status} {answered}");
        let reject_proposal = proposed(&answered)?;
        anyhow::ensure!(
            row(&f, "pending_facts", to_confirm).await? == confirm_row
                && row(&f, "pending_facts", to_reject).await? == reject_row,
            "a proposal changed the pending row"
        );
        anyhow::ensure!(
            nod_state(&f).await? == nods_before,
            "a proposal changed a pending fact, a rejection or queued a time job"
        );
        anyhow::ensure!(
            snapshot(&f).await? == before,
            "a proposal changed a fact, an entity, evidence or the queued jobs"
        );
        let open = open_proposals(&f).await?;
        for (pending, proposal, action) in [
            (to_confirm, confirm_proposal, "confirm"),
            (to_reject, reject_proposal, "reject"),
        ] {
            let mine: Vec<_> = open.iter().filter(|p| p.target_id == pending).collect();
            anyhow::ensure!(mine.len() == 1, "one open proposal: {mine:?}");
            let p = mine[0];
            anyhow::ensure!(
                p.id == proposal
                    && p.operation == "pending.nod"
                    && p.target_kind == "pending_fact"
                    && p.outcome == "proposed"
                    && p.gate_reason.as_deref() == Some("propose_only")
                    && p.owner_id == f.editor
                    && p.token_id == token_id
                    && p.token_name == TOKEN_NAME,
                "{p:?}"
            );
            anyhow::ensure!(
                keys(&p.payload) == ["action", "request", "target_label"]
                    && p.payload["request"] == json!({ "action": action })
                    && p.payload["action"] == action,
                "{}",
                p.payload
            );
            // 提议的那句话说得出是谁记下的（N5）
            anyhow::ensure!(
                p.payload["target_label"]
                    .as_str()
                    .is_some_and(|said| said.contains("Nod Subject")
                        && said.contains("(remembered by Viewer)")),
                "{}",
                p.payload
            );
        }
        // 重复：一条等点头的事实上只有一条开着的提议，反过来答也是它
        for action in ["confirm", "reject"] {
            let (status, again) =
                propose(&f, &token, to_confirm, json!({ "action": action })).await?;
            anyhow::ensure!(
                status == StatusCode::OK
                    && again["code"] == "already_proposed"
                    && again["action_id"] == json!(confirm_proposal),
                "a repeat: {status} {again}"
            );
        }
        let rows = audits(&f).await?;
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before + 2 && rows.len() == audits_before + 2,
            "two proposals, two ledger rows, two audit rows"
        );
        anyhow::ensure!(
            audits_of(&rows, "fact.nod_confirmed").is_empty()
                && audits_of(&rows, "fact.nod_rejected").is_empty(),
            "no nod was recorded"
        );

        // ── 人的点头路由与按句取的读路由：每一枚令牌照旧是 401 ──
        for bearer in [&token, &editor_read, &viewer_write] {
            for action in ["confirm", "reject"] {
                let (status, body) = nod(&f, bearer, to_confirm, action).await?;
                anyhow::ensure!(
                    status == StatusCode::UNAUTHORIZED,
                    "nod {action}: {status} {body}"
                );
            }
            let (status, _) = f
                .call(
                    bearer,
                    "GET",
                    &format!("/kbs/{}/review/pending?chunk_id={}", f.kb, Uuid::now_v7()),
                    None,
                )
                .await?;
            anyhow::ensure!(
                status == StatusCode::UNAUTHORIZED,
                "pending_for_chunk: {status}"
            );
        }
        anyhow::ensure!(
            nod_state(&f).await? == nods_before && snapshot(&f).await? == before,
            "a refused nod changed something"
        );

        // ── 条目上看得出是令牌提的，原来的每一个字段都还在（N5） ──
        let by_tool = tool_call(&f, &token, "review_list", json!({ "section": "pending" })).await?;
        let readers = [
            (
                "a session",
                listed(&f, &f.editor_session, to_confirm).await?,
            ),
            ("a token", listed(&f, &token, to_confirm).await?),
            (
                "the MCP list",
                item_in(&by_tool["structuredContent"], to_confirm)?,
            ),
        ];
        for (who, item) in &readers {
            let mut expected = keys(&item_before);
            expected.push("agent_proposals".into());
            expected.sort();
            anyhow::ensure!(keys(item) == expected, "{who}: {item}");
            for key in keys(&item_before) {
                anyhow::ensure!(
                    item[&key] == item_before[&key],
                    "{who}: {key} changed: {item}"
                );
            }
            anyhow::ensure!(item["proposed_by_name"] == "Viewer", "{who}: {item}");
            let carried = item["agent_proposals"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            anyhow::ensure!(carried.len() == 1, "{who}: {item}");
            let p = &carried[0];
            anyhow::ensure!(
                keys(p)
                    == [
                        "action",
                        "created_at",
                        "id",
                        "operation",
                        "owner_name",
                        "reason",
                        "token_name"
                    ]
                    && p["id"] == json!(confirm_proposal)
                    && p["operation"] == "pending.nod"
                    && p["action"] == "confirm"
                    && p["reason"] == "propose_only"
                    && p["owner_name"] == "Editor"
                    && p["token_name"] == TOKEN_NAME,
                "{who}: {p}"
            );
        }
        // 没有提议的那一条与从前逐字相等
        anyhow::ensure!(
            listed(&f, &f.editor_session, untouched).await? == plain_before,
            "an item without a proposal changed"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn an_unknown_or_already_nodded_pending_id_is_not_found() -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        // 这个人点过头的一条、摇过头的一条：行已经不在了
        let (nodded, _) = waiting(&f, "Nodded Subject").await?;
        let (shaken, _) = waiting(&f, "Shaken Subject").await?;
        for (pending, action) in [(nodded, "confirm"), (shaken, "reject")] {
            let (status, body) = nod(&f, &f.editor_session, pending, action).await?;
            anyhow::ensure!(status == StatusCode::OK, "a person's nod: {status} {body}");
        }
        // 别的库里的一条
        let foreign = Uuid::now_v7();
        let foreign_subject = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO entities (id, kb_id, canonical_name) VALUES ($1, $2, 'Elsewhere')",
        )
        .bind(foreign_subject)
        .bind(f.other_kb)
        .execute(&f.pool)
        .await?;
        sqlx::query(
            "INSERT INTO pending_facts (id, kb_id, subject_id, object_value, confidence, chunk_id,
                                        phrase)
             SELECT $1, $2, $3, '\"somewhere\"'::jsonb, 0.5, p.chunk_id, 'lives in'
               FROM pending_facts p WHERE p.kb_id = $4 LIMIT 1",
        )
        .bind(foreign)
        .bind(f.other_kb)
        .bind(foreign_subject)
        .bind(f.kb)
        .execute(&f.pool)
        .await?;
        let foreign_row: Value =
            sqlx::query_scalar("SELECT to_jsonb(p) FROM pending_facts p WHERE p.id = $1")
                .bind(foreign)
                .fetch_one(&f.pool)
                .await?;

        let before = snapshot(&f).await?;
        let nods_before = nod_state(&f).await?;
        let rows_before = ledger(&f).await?.len();
        // MCP 入口给每一次工具调用记一行 `mcp.tool_called`（被拒绝的也记，从前就这样）：
        // 那一行说的是「调过」，不算这里要数的审计行
        let counted = |rows: Vec<Audit>| {
            rows.into_iter()
                .filter(|a| a.action != "mcp.tool_called")
                .count()
        };
        let audits_before = counted(audits(&f).await?);
        for (what, on) in [
            ("an unknown id", Uuid::now_v7()),
            ("an id of another base", foreign),
            ("a confirmed pending fact", nodded),
            ("a rejected pending fact", shaken),
        ] {
            for action in ["confirm", "reject"] {
                let (status, body) = propose(&f, &token, on, json!({ "action": action })).await?;
                anyhow::ensure!(
                    status == StatusCode::NOT_FOUND,
                    "{what}, {action}: {status} {body}"
                );
                let result = tool_call(
                    &f,
                    &token,
                    "review_pending_fact",
                    json!({ "id": on, "action": action }),
                )
                .await?;
                super::review_token_write_support::ensure_tool_refused(&result, "not_found")?;
            }
        }
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before
                && counted(audits(&f).await?) == audits_before
                && open_proposals(&f)
                    .await?
                    .iter()
                    .all(|p| p.operation != "pending.nod"),
            "a not-found call left a proposal"
        );
        anyhow::ensure!(
            snapshot(&f).await? == before && nod_state(&f).await? == nods_before,
            "a not-found call changed something"
        );
        let foreign_after: Value =
            sqlx::query_scalar("SELECT to_jsonb(p) FROM pending_facts p WHERE p.id = $1")
                .bind(foreign)
                .fetch_one(&f.pool)
                .await?;
        anyhow::ensure!(foreign_after == foreign_row, "the other base's row changed");
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn approving_a_pending_fact_proposal_records_the_nod_as_that_persons() -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;

        // ── 确认：批准落下的事实与这个人自己点头落下的一模一样 ──
        let (mine, my_subject) = waiting(&f, "Confirmed Subject").await?;
        let (theirs, their_subject) = waiting(&f, "Confirmed Subject").await?;
        let (status, answered) = propose(&f, &token, theirs, json!({ "action": "confirm" })).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {answered}");
        let proposal = proposed(&answered)?;
        anyhow::ensure!(
            said_of(&f, their_subject).await? == json!([]),
            "the proposal wrote a fact"
        );
        // 答提议的只能是登录的编辑者：提它的那一枚令牌是 401，Viewer 是 403
        let (status, _) = answer(&f, &token, proposal, "approve").await?;
        anyhow::ensure!(status == StatusCode::UNAUTHORIZED, "a token answered: {status}");
        let (status, _) = answer(&f, &f.viewer_session, proposal, "approve").await?;
        anyhow::ensure!(status == StatusCode::FORBIDDEN, "a viewer answered: {status}");
        anyhow::ensure!(
            said_of(&f, their_subject).await? == json!([])
                && !row(&f, "pending_facts", theirs).await?.is_null(),
            "a refused answer nodded"
        );

        clear_time_jobs(&f).await?;
        let (status, answered) = answer(&f, &f.editor_session, proposal, "approve").await?;
        anyhow::ensure!(
            status == StatusCode::OK
                && keys(&answered) == ["fact_id", "ok", "state"]
                && answered["ok"] == true
                && answered["state"] == "approved",
            "{status} {answered}"
        );
        let their_fact: Uuid = serde_json::from_value(answered["fact_id"].clone())?;
        let fact_subject: Uuid =
            sqlx::query_scalar("SELECT subject_id FROM facts WHERE id = $1 AND kb_id = $2")
                .bind(their_fact)
                .bind(f.kb)
                .fetch_one(&f.pool)
                .await?;
        anyhow::ensure!(fact_subject == their_subject, "fact_id names the fact written");
        anyhow::ensure!(
            row(&f, "pending_facts", theirs).await?.is_null(),
            "the approved nod takes the row off the queue"
        );
        anyhow::ensure!(
            nod_state(&f).await?["time_jobs"] == 1,
            "the approved nod queues the time resolution a person's nod queues"
        );

        // 对照：这个人自己点头
        clear_time_jobs(&f).await?;
        let (status, own) = nod(&f, &f.editor_session, mine, "confirm").await?;
        anyhow::ensure!(
            status == StatusCode::OK && keys(&own) == ["conflicts", "created", "fact_id", "ok"],
            "{status} {own}"
        );
        let my_fact: Uuid = serde_json::from_value(own["fact_id"].clone())?;
        anyhow::ensure!(nod_state(&f).await?["time_jobs"] == 1);
        let (said_mine, said_theirs) = (
            said_of(&f, my_subject).await?,
            said_of(&f, their_subject).await?,
        );
        anyhow::ensure!(
            said_mine == said_theirs && said_mine.as_array().is_some_and(|facts| facts.len() == 1),
            "an approved confirm writes the fact exactly as the person's own nod\n{said_theirs}\n{said_mine}"
        );
        // 审计：同一个动作名，actor 都是这个人；批准的那一行只多指着提议，不带令牌
        let rows = audits(&f).await?;
        let confirmed = audits_of(&rows, "fact.nod_confirmed");
        let own_row: Vec<_> = confirmed
            .iter()
            .filter(|a| a.target_id == Some(my_fact))
            .collect();
        let approved_row: Vec<_> = confirmed
            .iter()
            .filter(|a| a.target_id == Some(their_fact))
            .collect();
        anyhow::ensure!(
            own_row.len() == 1 && approved_row.len() == 1,
            "{confirmed:?}"
        );
        let (own_row, approved_row) = (own_row[0], approved_row[0]);
        anyhow::ensure!(
            own_row.actor == Some(f.editor)
                && own_row.target_kind == "fact"
                && own_row.detail.get("agent_proposal").is_none(),
            "{own_row:?}"
        );
        anyhow::ensure!(
            approved_row.actor == Some(f.editor)
                && approved_row.target_kind == "fact"
                && approved_row.detail["agent_proposal"] == json!(proposal)
                && approved_row.detail.get("token_id").is_none()
                && approved_row.detail.get("token_name").is_none()
                && without_proposal(&approved_row.detail) == own_row.detail,
            "the approval is the person's nod and names its proposal: {approved_row:?} / {own_row:?}"
        );
        let closed = ledger_row(&f, proposal).await?;
        anyhow::ensure!(
            closed.state.as_deref() == Some("approved")
                && closed.state_by == Some(f.editor)
                && closed.outcome == "proposed"
                && closed.merge_id.is_none(),
            "{closed:?}"
        );
        // 再答一次：已经答过了
        let (status, body) = answer(&f, &f.editor_session, proposal, "approve").await?;
        anyhow::ensure!(
            status == StatusCode::CONFLICT && body["code"] == "already_decided",
            "{status} {body}"
        );

        // ── 否决：批准记下的拒绝与这个人自己摇头记下的一模一样，不落事实 ──
        let (mine, my_subject) = waiting(&f, "Rejected Subject").await?;
        let (theirs, their_subject) = waiting(&f, "Rejected Subject").await?;
        let (status, answered) = propose(&f, &token, theirs, json!({ "action": "reject" })).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {answered}");
        let proposal = proposed(&answered)?;
        anyhow::ensure!(refused_of(&f, their_subject).await? == json!([]));
        let facts_before = snapshot(&f).await?;
        let (status, answered) = answer(&f, &f.editor_session, proposal, "approve").await?;
        anyhow::ensure!(
            status == StatusCode::OK && answered == json!({ "ok": true, "state": "approved" }),
            "{status} {answered}"
        );
        let (status, own) = nod(&f, &f.editor_session, mine, "reject").await?;
        anyhow::ensure!(
            status == StatusCode::OK && own == json!({ "ok": true }),
            "{status} {own}"
        );
        let (refused_mine, refused_theirs) = (
            refused_of(&f, my_subject).await?,
            refused_of(&f, their_subject).await?,
        );
        anyhow::ensure!(
            refused_mine == refused_theirs
                && refused_mine.as_array().is_some_and(|rows| rows.len() == 1)
                && refused_mine[0]["by"] == json!(f.editor),
            "an approved reject records the rejection as the person's own\n{refused_theirs}\n{refused_mine}"
        );
        anyhow::ensure!(
            row(&f, "pending_facts", theirs).await?.is_null()
                && snapshot(&f).await? == facts_before,
            "a rejection takes the row off the queue and writes no fact"
        );
        let rows = audits(&f).await?;
        let rejected = audits_of(&rows, "fact.nod_rejected");
        let own_row: Vec<_> = rejected
            .iter()
            .filter(|a| a.target_id == Some(mine))
            .collect();
        let approved_row: Vec<_> = rejected
            .iter()
            .filter(|a| a.target_id == Some(theirs))
            .collect();
        anyhow::ensure!(
            own_row.len() == 1 && approved_row.len() == 1,
            "{rejected:?}"
        );
        anyhow::ensure!(
            approved_row[0].actor == Some(f.editor)
                && approved_row[0].target_kind == "pending_fact"
                && approved_row[0].detail["agent_proposal"] == json!(proposal)
                && approved_row[0].detail.get("token_id").is_none()
                && without_proposal(&approved_row[0].detail) == own_row[0].detail
                && own_row[0].actor == Some(f.editor),
            "{approved_row:?} / {own_row:?}"
        );

        // ── 过时：等着的时候这个人自己点了头，批准什么都不改 ──
        let (stale, _) = waiting(&f, "Stale Subject").await?;
        let (status, answered) = propose(&f, &token, stale, json!({ "action": "confirm" })).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {answered}");
        let proposal = proposed(&answered)?;
        let (status, body) = nod(&f, &f.editor_session, stale, "reject").await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let before = snapshot(&f).await?;
        let nods_before = nod_state(&f).await?;
        let (status, answered) = answer(&f, &f.editor_session, proposal, "approve").await?;
        anyhow::ensure!(
            status == StatusCode::OK && answered == json!({ "ok": true, "state": "superseded" }),
            "{status} {answered}"
        );
        anyhow::ensure!(
            snapshot(&f).await? == before && nod_state(&f).await? == nods_before,
            "a superseded proposal changed something"
        );
        let rows = audits(&f).await?;
        anyhow::ensure!(
            audits_of(&rows, "agent.proposal.superseded")
                .iter()
                .any(|a| a.target_id == Some(proposal) && a.actor == Some(f.editor)),
            "the supersession is recorded"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn declining_a_pending_fact_proposal_leaves_the_row_waiting() -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        for action in ["confirm", "reject"] {
            let (pending, subject) = waiting(&f, "Declined Subject").await?;
            let (status, answered) =
                propose(&f, &token, pending, json!({ "action": action })).await?;
            anyhow::ensure!(status == StatusCode::OK, "{action}: {status} {answered}");
            let proposal = proposed(&answered)?;
            let before = snapshot(&f).await?;
            let nods_before = nod_state(&f).await?;
            let waiting_row = row(&f, "pending_facts", pending).await?;
            let audits_before = audits(&f).await?.len();

            let (status, answered) = answer(&f, &f.editor_session, proposal, "decline").await?;
            anyhow::ensure!(
                status == StatusCode::OK && answered == json!({ "ok": true, "state": "declined" }),
                "{action}: {status} {answered}"
            );
            anyhow::ensure!(
                row(&f, "pending_facts", pending).await? == waiting_row && !waiting_row.is_null(),
                "{action}: a declined proposal changed the pending row"
            );
            anyhow::ensure!(
                nod_state(&f).await? == nods_before && snapshot(&f).await? == before,
                "{action}: a declined proposal changed a fact, a rejection or the queue"
            );
            anyhow::ensure!(
                said_of(&f, subject).await? == json!([])
                    && refused_of(&f, subject).await? == json!([]),
                "{action}: a declined proposal nodded"
            );
            // 审计：只多了一行「拒绝了提议」，记的是这个人；没有哪一行是点头
            let rows = audits(&f).await?;
            anyhow::ensure!(rows.len() == audits_before + 1, "{action}: one audit row");
            let said: Vec<_> = audits_of(&rows, "agent.proposal.declined")
                .into_iter()
                .filter(|a| a.target_id == Some(proposal))
                .collect();
            anyhow::ensure!(
                said.len() == 1
                    && said[0].actor == Some(f.editor)
                    && said[0].detail
                        == json!({ "agent_proposal": proposal, "operation": "pending.nod",
                                   "reason": "propose_only", "proposed_by_token": TOKEN_NAME }),
                "{action}: {said:?}"
            );
            let closed = ledger_row(&f, proposal).await?;
            anyhow::ensure!(
                closed.state.as_deref() == Some("declined") && closed.state_by == Some(f.editor),
                "{closed:?}"
            );
            // 那一条还在队列里等着，条目上不再带提议；这个人照旧点得了头
            let item = listed(&f, &f.editor_session, pending).await?;
            anyhow::ensure!(item.get("agent_proposals").is_none(), "{item}");
            let (status, body) = nod(&f, &f.editor_session, pending, "reject").await?;
            anyhow::ensure!(status == StatusCode::OK, "{action}: {status} {body}");
        }
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_caller_without_a_token_is_refused_on_the_two_propose_routes() -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (pending, _) = waiting(&f, "Session Subject").await?;
        let decision: Uuid = sqlx::query_scalar(
            "SELECT id FROM agent_decisions WHERE kb_id = $1 AND status = 'proposed' LIMIT 1",
        )
        .bind(f.kb)
        .fetch_one(&f.pool)
        .await?;
        let decision_row = row(&f, "agent_decisions", decision).await?;
        let before = snapshot(&f).await?;
        let nods_before = nod_state(&f).await?;
        let rows_before = ledger(&f).await?.len();
        let audits_before = audits(&f).await?.len();

        // 登录的人——编辑者也一样——在这两条路由上拿到的是 `token_required`：请求体对不对、
        // 目标在不在都不看
        for (who, session) in [
            ("an editor's session", &f.editor_session),
            ("a viewer's session", &f.viewer_session),
        ] {
            for tail in [
                format!("review/pending/{pending}/propose"),
                format!("review/pending/{}/propose", Uuid::now_v7()),
                format!("review/agent/{decision}/propose"),
                format!("review/agent/{}/propose", Uuid::now_v7()),
            ] {
                for body in [
                    json!({ "action": "confirm" }),
                    json!({ "action": "reject" }),
                    json!({ "action": "keep", "rationale": "as a person" }),
                    json!({}),
                ] {
                    let (status, answered) = post(&f, session, &tail, body.clone()).await?;
                    anyhow::ensure!(
                        status == StatusCode::FORBIDDEN
                            && answered["code"] == "token_required"
                            && keys(&answered) == ["code", "error"],
                        "{who}, {tail}, {body}: {status} {answered}"
                    );
                }
            }
        }
        // 认不出的凭据照旧是 401
        for tail in [
            format!("review/pending/{pending}/propose"),
            format!("review/agent/{decision}/propose"),
        ] {
            let (status, _) =
                post(&f, "not-a-credential", &tail, json!({ "action": "keep" })).await?;
            anyhow::ensure!(status == StatusCode::UNAUTHORIZED, "{tail}: {status}");
        }
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before && audits(&f).await?.len() == audits_before,
            "a refused session left a ledger or audit row"
        );
        anyhow::ensure!(
            snapshot(&f).await? == before
                && nod_state(&f).await? == nods_before
                && row(&f, "agent_decisions", decision).await? == decision_row,
            "a refused session changed something"
        );
        // 这个人照旧走他自己的路由
        let (status, body) = nod(&f, &f.editor_session, pending, "reject").await?;
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
