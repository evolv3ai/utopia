//! 令牌分开的一对，人在哪里看得到是谁做的（0076 决定 7、8；SC7 / SC8）。
//!
//! 要钉住的行为：
//!
//! - **Agent 一档**（`GET /review?queue=agent`）：台账的行带 `source: "token"`、主人的名字、
//!   令牌当时的名字、动作、结果与这一对的两个名字；治理 agent 的行与 `governance::list`
//!   序列化出来的逐键相等；`counts.agent_rows` 数两种行，`counts.agent` 不变
//! - **令牌的名字是快照**（E16）：令牌改名、撤销之后，这一行还是动作那一刻的名字
//! - **两种行按动作时间并在一起**（E17）：最新的在前，治理 agent 的行彼此的先后与
//!   `governance::list` 一样，翻页时也一样
//! - **Activity**（`GET /audit`，库 Admin）：令牌的事件带 `token_id`、`token_name`，请求
//!   报了 User-Agent 的还带 `declared_client`；人自己做的事件三个键都没有；不是 Admin 的
//!   编辑者照旧被拒
//! - **没报 User-Agent**（E18）：事件上没有 `declared_client` 这个键
//!
//! 令牌的分开走 T004 的 REST 路由；治理 agent 的行用 `governance::record` 种。
//! 夹具在 `review_session_unchanged_tests.rs`。没有 `UTOPIA_DATABASE_URL` 时跳过。
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

use super::super::review_session_unchanged_tests::Fx;

/// 夹具的 `call` 不带 User-Agent。这里要两种都发：`user_agent` 给了就带上这个头，
/// 没给就不带——请求层（`client_ctx::capture`）存的就是这个头
async fn send(
    f: &Fx,
    bearer: &str,
    method: &str,
    path: &str,
    user_agent: Option<&str>,
    body: Option<Value>,
) -> anyhow::Result<(StatusCode, Value)> {
    let mut request = Request::builder()
        .method(method)
        .uri(format!("/api/v1{path}"))
        .header("Authorization", format!("Bearer {bearer}"));
    if let Some(ua) = user_agent {
        request = request.header("User-Agent", ua);
    }
    let request = match body {
        Some(b) => request
            .header("Content-Type", "application/json")
            .body(Body::from(b.to_string()))?,
        None => request.body(Body::empty())?,
    };
    let response = f.app.clone().oneshot(request).await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await?;
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    Ok((status, value))
}

/// 经令牌把一对分开，回台账行的 id
async fn token_keep(
    f: &Fx,
    token: &str,
    review: Uuid,
    user_agent: Option<&str>,
) -> anyhow::Result<Uuid> {
    let (status, body) = send(
        f,
        token,
        "POST",
        &format!("/kbs/{}/review/{review}", f.kb),
        user_agent,
        Some(json!({ "action": "keep", "rationale": "different people" })),
    )
    .await?;
    anyhow::ensure!(status == StatusCode::OK, "token keep: {status} {body}");
    anyhow::ensure!(body["outcome"] == "applied", "{body}");
    Ok(body["action_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("no action_id in {body}"))?
        .parse()?)
}

/// 种一条治理 agent 的行
async fn governor_row(f: &Fx, review: Uuid, action: &str, status: &str) -> anyhow::Result<Uuid> {
    Ok(utopia_store::governance::record(
        &f.pool,
        f.kb,
        utopia_store::governance::NewDecision {
            run_id: Uuid::now_v7(),
            target_id: review,
            action,
            confidence: 0.9,
            reason: Some("same name, different employers"),
            precedents: json!([]),
            status,
            merge_id: None,
            question: None,
            trace: json!([]),
            calls: 1,
        },
    )
    .await?)
}

/// 会话编辑者看到的 Agent 一档
async fn agent_queue(f: &Fx, tail: &str) -> anyhow::Result<Value> {
    let path = format!("/kbs/{}/review?queue=agent{tail}", f.kb);
    let (status, body) = f.call(&f.editor_session, "GET", &path, None).await?;
    anyhow::ensure!(status == StatusCode::OK, "{path}: {status} {body}");
    Ok(body)
}

fn items(body: &Value) -> Vec<Value> {
    body["items"].as_array().cloned().unwrap_or_default()
}

fn id_of(row: &Value) -> String {
    row["id"].as_str().unwrap_or_default().to_string()
}

fn is_token(row: &Value) -> bool {
    row.get("source").is_some()
}

/// `governance::list` 序列化出来的样子：治理 agent 的行要与它逐键相等
async fn governor_list(f: &Fx) -> anyhow::Result<Vec<Value>> {
    let rows = utopia_store::governance::list(&f.pool, f.kb, 200, 0).await?;
    Ok(rows
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<_, _>>()?)
}

/// 把编辑者提成这个库的 Admin：Activity 只有库 Admin 读得到，夹具里没有现成的
async fn promote_editor(f: &Fx) -> anyhow::Result<()> {
    sqlx::query("UPDATE kb_members SET role = 'admin' WHERE kb_id = $1 AND user_id = $2")
        .bind(f.kb)
        .bind(f.editor)
        .execute(&f.pool)
        .await?;
    Ok(())
}

/// 这个库的 Activity 里某一对上的 `review.keep` 事件
async fn keep_event(f: &Fx, review: Uuid) -> anyhow::Result<Value> {
    let path = format!("/kbs/{}/audit?action=review.keep", f.kb);
    let (status, body) = f.call(&f.editor_session, "GET", &path, None).await?;
    anyhow::ensure!(status == StatusCode::OK, "{path}: {status} {body}");
    let events = body["events"].as_array().cloned().unwrap_or_default();
    let mut hits = events
        .into_iter()
        .filter(|e| e["target_id"] == review.to_string());
    let event = hits
        .next()
        .ok_or_else(|| anyhow::anyhow!("no review.keep event for {review} in {body}"))?;
    anyhow::ensure!(hits.next().is_none(), "two keep events for {review}");
    Ok(event)
}

#[tokio::test]
async fn agent_history_shows_a_token_keep_beside_unchanged_governor_rows() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let proposed = f.pair("Zhang Wei", "Zhang Wei").await?;
        let applied = f.pair("Li Na", "Li Na").await?;
        let kept = f.pair("Ada Lovelace", "Ada King").await?;
        governor_row(&f, proposed, "merge", "proposed").await?;
        governor_row(&f, applied, "keep", "applied").await?;

        let before = agent_queue(&f, "").await?;
        anyhow::ensure!(before["counts"]["agent_rows"] == 2, "{before}");
        anyhow::ensure!(before["counts"]["agent"] == 1, "{before}");
        let governors = governor_list(&f).await?;
        anyhow::ensure!(governors.len() == 2, "{governors:?}");
        // 没有台账行的时候，这一档就是今天的样子
        anyhow::ensure!(items(&before) == governors, "{before}");

        let (token_id, token) = f.token(f.editor, "curator", "write", None).await?;
        let action_id = token_keep(&f, &token, kept, None).await?;

        let after = agent_queue(&f, "").await?;
        let rows = items(&after);
        let token_rows: Vec<&Value> = rows.iter().filter(|r| is_token(r)).collect();
        anyhow::ensure!(token_rows.len() == 1, "one ledger row: {after}");
        let row = token_rows[0];
        anyhow::ensure!(row["source"] == "token", "{row}");
        anyhow::ensure!(row["id"] == action_id.to_string(), "{row}");
        anyhow::ensure!(row["owner_id"] == f.editor.to_string(), "{row}");
        anyhow::ensure!(row["owner_name"] == "Editor", "{row}");
        anyhow::ensure!(row["token_id"] == token_id.to_string(), "{row}");
        anyhow::ensure!(row["token_name"] == "curator", "{row}");
        anyhow::ensure!(row["operation"] == "review.keep", "{row}");
        anyhow::ensure!(row["outcome"] == "applied", "{row}");
        anyhow::ensure!(row["target_kind"] == "review", "{row}");
        anyhow::ensure!(row["target_id"] == kept.to_string(), "{row}");
        anyhow::ensure!(row["left"] == "Ada Lovelace", "{row}");
        anyhow::ensure!(row["right"] == "Ada King", "{row}");
        anyhow::ensure!(row["rationale"] == "different people", "{row}");
        anyhow::ensure!(row["created_at"].is_string(), "{row}");
        let mut keys: Vec<&str> = row
            .as_object()
            .map(|o| o.keys().map(String::as_str).collect())
            .unwrap_or_default();
        keys.sort_unstable();
        anyhow::ensure!(
            keys == [
                "created_at",
                "id",
                "left",
                "operation",
                "outcome",
                "owner_id",
                "owner_name",
                "rationale",
                "right",
                "source",
                "target_id",
                "target_kind",
                "token_id",
                "token_name",
            ],
            "ledger row keys: {keys:?}"
        );

        // 治理 agent 的行：逐键相等，一个键不多（没有 source），先后也不变
        let governor_rows: Vec<Value> = rows.iter().filter(|r| !is_token(r)).cloned().collect();
        let governors = governor_list(&f).await?;
        anyhow::ensure!(governors.len() == 2, "{governors:?}");
        anyhow::ensure!(
            governor_rows == governors,
            "governor rows changed\n{governor_rows:?}\n{governors:?}"
        );
        // 令牌的分开不答治理 agent 的建议，也不把它的行改掉
        anyhow::ensure!(
            governor_rows == items(&before),
            "governor rows differ from before the keep"
        );

        // 翻页总数数两种行；徽标（等人答的建议）不变
        anyhow::ensure!(after["counts"]["agent_rows"] == 3, "{after}");
        anyhow::ensure!(
            after["counts"]["agent"] == before["counts"]["agent"],
            "{after}"
        );
        anyhow::ensure!(rows.len() == 3, "{after}");
        // 令牌自己这一刀还读不了这一档
        let (status, body) = f
            .call(
                &token,
                "GET",
                &format!("/kbs/{}/review?queue=agent", f.kb),
                None,
            )
            .await?;
        anyhow::ensure!(
            status == StatusCode::UNPROCESSABLE_ENTITY && body.get("items").is_none(),
            "a token must not read queue=agent: {status} {body}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn agent_history_keeps_the_token_name_it_had_after_a_rename_and_a_revoke(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kept = f.pair("Ada Lovelace", "Ada King").await?;
        let (token_id, token) = f.token(f.editor, "curator", "write", None).await?;
        let action_id = token_keep(&f, &token, kept, None).await?;

        let the_row = |body: &Value| -> anyhow::Result<Value> {
            items(body)
                .into_iter()
                .find(|r| r["id"] == action_id.to_string())
                .ok_or_else(|| anyhow::anyhow!("ledger row missing in {body}"))
        };
        let row = the_row(&agent_queue(&f, "").await?)?;
        anyhow::ensure!(row["token_name"] == "curator", "{row}");

        // 改名：令牌自己的行换了名字，台账里的快照不跟着换
        let renamed = sqlx::query("UPDATE personal_tokens SET name = 'renamed' WHERE id = $1")
            .bind(token_id)
            .execute(&f.pool)
            .await?
            .rows_affected();
        anyhow::ensure!(renamed == 1, "the token row was not renamed");
        let row = the_row(&agent_queue(&f, "").await?)?;
        anyhow::ensure!(row["token_name"] == "curator", "after the rename: {row}");

        // 撤销：这枚令牌再也用不了，它做过的那一笔还读得出当时叫什么
        utopia_store::tokens::revoke(&f.pool, f.editor, token_id).await?;
        let (status, _) = f
            .call(
                &token,
                "GET",
                &format!("/kbs/{}/review?queue=duplicates", f.kb),
                None,
            )
            .await?;
        anyhow::ensure!(
            status == StatusCode::UNAUTHORIZED,
            "the token is not revoked: {status}"
        );
        let row = the_row(&agent_queue(&f, "").await?)?;
        anyhow::ensure!(row["source"] == "token", "{row}");
        anyhow::ensure!(row["token_name"] == "curator", "after the revoke: {row}");
        anyhow::ensure!(row["token_id"] == token_id.to_string(), "{row}");
        anyhow::ensure!(row["owner_name"] == "Editor", "{row}");
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn agent_history_merges_ledger_rows_by_action_time_and_keeps_governor_order(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let (_, token) = f.token(f.editor, "curator", "write", None).await?;
        // 交替着种：治理、令牌、治理、令牌、治理、令牌、治理、治理。然后把动作时间写死成
        // 一分钟一格，测试不靠墙上的钟
        let base: DateTime<Utc> = "2026-11-01T00:00:00Z".parse()?;
        let mut expected_oldest_first: Vec<(Uuid, bool)> = Vec::new();
        for (i, from_token) in [false, true, false, true, false, true, false, false]
            .into_iter()
            .enumerate()
        {
            let name = format!("Person {i}");
            let review = f.pair(&name, &name).await?;
            let at = base + Duration::minutes(i as i64);
            let id = if from_token {
                let id = token_keep(&f, &token, review, None).await?;
                sqlx::query("UPDATE token_agent_actions SET created_at = $2 WHERE id = $1")
                    .bind(id)
                    .bind(at)
                    .execute(&f.pool)
                    .await?;
                id
            } else {
                let id = governor_row(&f, review, "keep", "applied").await?;
                sqlx::query("UPDATE agent_decisions SET created_at = $2 WHERE id = $1")
                    .bind(id)
                    .bind(at)
                    .execute(&f.pool)
                    .await?;
                id
            };
            expected_oldest_first.push((id, from_token));
        }
        let expected: Vec<(String, bool)> = expected_oldest_first
            .iter()
            .rev()
            .map(|(id, from_token)| (id.to_string(), *from_token))
            .collect();

        let whole = agent_queue(&f, "&limit=200").await?;
        anyhow::ensure!(whole["counts"]["agent_rows"] == 8, "{whole}");
        let rows = items(&whole);
        let got: Vec<(String, bool)> = rows.iter().map(|r| (id_of(r), is_token(r))).collect();
        anyhow::ensure!(got == expected, "merged order\n{got:?}\n{expected:?}");
        // 最新的在前
        let times: Vec<DateTime<Utc>> = rows
            .iter()
            .map(|r| r["created_at"].as_str().unwrap_or_default().parse())
            .collect::<Result<_, _>>()?;
        anyhow::ensure!(
            times.windows(2).all(|w| w[0] > w[1]),
            "not newest first: {times:?}"
        );

        // 治理 agent 的行：彼此的先后与 `governance::list` 一样，内容也一样
        let governors = governor_list(&f).await?;
        anyhow::ensure!(governors.len() == 5, "{governors:?}");
        let governor_rows: Vec<Value> = rows.iter().filter(|r| !is_token(r)).cloned().collect();
        anyhow::ensure!(governor_rows == governors, "governor order changed");

        // 翻页：limit / offset 作用在并起来的序列上，每一页都是整个序列里的那一段。
        // 页边界落在两种行之间、也落在同一种行中间
        for (limit, offset) in [
            (2, 0),
            (2, 2),
            (2, 4),
            (2, 6),
            (3, 1),
            (3, 4),
            (4, 3),
            (1, 7),
            (5, 8),
        ] {
            let page = agent_queue(&f, &format!("&limit={limit}&offset={offset}")).await?;
            let page_rows = items(&page);
            let slice: Vec<Value> = rows.iter().skip(offset).take(limit).cloned().collect();
            anyhow::ensure!(
                page_rows == slice,
                "limit={limit} offset={offset}\n{page_rows:?}\n{slice:?}"
            );
            // 这一页里治理 agent 的行，是 `governance::list` 里连续的一段，先后不变
            let page_governors: Vec<Value> =
                page_rows.iter().filter(|r| !is_token(r)).cloned().collect();
            let start = rows
                .iter()
                .take(offset)
                .filter(|r| !is_token(r))
                .count();
            let run: Vec<Value> = governors
                .iter()
                .skip(start)
                .take(page_governors.len())
                .cloned()
                .collect();
            anyhow::ensure!(
                page_governors == run,
                "limit={limit} offset={offset}: governor rows out of order"
            );
            anyhow::ensure!(page["counts"]["agent_rows"] == 8, "{page}");
        }
        // 两页接起来就是前四行，不重不漏
        let mut walked = items(&agent_queue(&f, "&limit=2&offset=0").await?);
        walked.extend(items(&agent_queue(&f, "&limit=2&offset=2").await?));
        anyhow::ensure!(walked == rows[..4], "pages do not join");
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn activity_returns_token_id_token_name_and_declared_client_for_a_token_keep(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let by_person = f.pair("Zhang Wei", "Zhang Wei").await?;
        let by_token = f.pair("Ada Lovelace", "Ada King").await?;
        // 人自己分开一对，浏览器当然报了 User-Agent
        let (status, body) = send(
            &f,
            &f.editor_session,
            "POST",
            &format!("/kbs/{}/review/{by_person}", f.kb),
            Some("Mozilla/5.0 (a browser)"),
            Some(json!({ "action": "keep" })),
        )
        .await?;
        anyhow::ensure!(status == StatusCode::OK, "session keep: {status} {body}");
        let (token_id, token) = f.token(f.editor, "curator", "write", None).await?;
        token_keep(&f, &token, by_token, Some("curl/8.5.0")).await?;

        // 编辑者不是 Admin：照旧被拒，令牌也读不了这条路由
        let path = format!("/kbs/{}/audit", f.kb);
        let (status, body) = f.call(&f.editor_session, "GET", &path, None).await?;
        anyhow::ensure!(
            status == StatusCode::FORBIDDEN && body.get("events").is_none(),
            "an editor read Activity: {status} {body}"
        );
        let (status, body) = f.call(&token, "GET", &path, None).await?;
        anyhow::ensure!(
            status == StatusCode::UNAUTHORIZED && body.get("events").is_none(),
            "a token read Activity: {status} {body}"
        );

        promote_editor(&f).await?;
        let event = keep_event(&f, by_token).await?;
        anyhow::ensure!(event["action"] == "review.keep", "{event}");
        anyhow::ensure!(event["actor_id"] == f.editor.to_string(), "{event}");
        anyhow::ensure!(event["actor_name"] == "Editor", "{event}");
        anyhow::ensure!(event["token_id"] == token_id.to_string(), "{event}");
        anyhow::ensure!(event["token_name"] == "curator", "{event}");
        anyhow::ensure!(event["declared_client"] == "curl/8.5.0", "{event}");

        // 人自己做的那一笔：三个键都没有，哪怕请求报了 User-Agent
        let event = keep_event(&f, by_person).await?;
        anyhow::ensure!(event["actor_name"] == "Editor", "{event}");
        for key in ["token_id", "token_name", "declared_client"] {
            anyhow::ensure!(event.get(key).is_none(), "{key} on a person's event: {event}");
        }

        // 共用同一个结构的决策台账：形状原样，令牌那一笔上也没有这三个键
        let (status, history) = f
            .call(
                &f.editor_session,
                "GET",
                &format!("/kbs/{}/review/history", f.kb),
                None,
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "history: {status} {history}");
        let events = history["events"].as_array().cloned().unwrap_or_default();
        anyhow::ensure!(events.len() == 2, "{history}");
        for event in &events {
            for key in ["token_id", "token_name", "declared_client"] {
                anyhow::ensure!(event.get(key).is_none(), "{key} in Decisions: {event}");
            }
        }
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn activity_omits_the_declared_client_when_no_user_agent_was_sent() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let by_token = f.pair("Ada Lovelace", "Ada King").await?;
        let (token_id, token) = f.token(f.editor, "curator", "write", None).await?;
        // 这个请求没有 User-Agent 头
        token_keep(&f, &token, by_token, None).await?;

        promote_editor(&f).await?;
        let event = keep_event(&f, by_token).await?;
        anyhow::ensure!(event["actor_name"] == "Editor", "{event}");
        anyhow::ensure!(event["token_id"] == token_id.to_string(), "{event}");
        anyhow::ensure!(event["token_name"] == "curator", "{event}");
        anyhow::ensure!(
            event.get("declared_client").is_none(),
            "declared_client without a User-Agent: {event}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}
