//! 语义层映射的令牌读与写（0076）。
//!
//! 裁定表里映射是「直接落地」：编辑者的写令牌定一条映射，状态当场就改，台账里留一行
//! `applied`，审计行 `mapping.decided` 记着主人、令牌的 id 与名字、台账行的 id。
//!
//! - **落地与归属**（SC3、E20）：REST 的回答就是人拿到的那一个；Agent 一档与审计里读得出
//!   「主人经哪一枚令牌」
//! - **映射没有「定了」这一态**（E13）：状态已经是请求要的那个是 `already_decided`，一行
//!   不写；换一个状态照样落地；认不出的 id 是 404；confirmed / rejected 之外是 `bad_status`。
//!   REST 与 `review_mapping` 四种情形都一样
//! - **读**：够得着这个库的令牌（主人至少是 Viewer）从列表路由、从 `review_mapping_list`
//!   读到的就是会话读到的那一页；Review 的共享列表里 `mappings` 仍是 `unknown_queue`
//!
//! 这个文件从前钉着「令牌在映射路由上仍是 401」；把路由开给令牌的这一刀换掉了它。
//! 夹具在 `review_token_write_support.rs`。没有 `UTOPIA_DATABASE_URL` 时跳过。
use axum::http::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;

use super::review_token_write_support::{
    audits, ensure_tool_refused, keys, ledger, post, row, seeded, snapshot, tool_call, Fx,
};

/// 带 Bearer 的一次 GET，`tail` 接在 `/kbs/{f.kb}/` 后面
async fn get(f: &Fx, bearer: &str, tail: &str) -> anyhow::Result<(StatusCode, Value)> {
    f.call(bearer, "GET", &format!("/kbs/{}/{tail}", f.kb), None)
        .await
}

/// 种子里那一条等人表态的映射
async fn seeded_mapping(f: &Fx) -> anyhow::Result<Uuid> {
    Ok(
        sqlx::query_scalar("SELECT id FROM concept_mappings WHERE kb_id = $1")
            .bind(f.kb)
            .fetch_one(&f.pool)
            .await?,
    )
}

/// 另一个库里的一条映射：对 `f.kb` 的调用方来说它不存在
async fn foreign_mapping(f: &Fx) -> anyhow::Result<Uuid> {
    let concept = Uuid::now_v7();
    sqlx::query("INSERT INTO entities (id, kb_id, canonical_name) VALUES ($1, $2, 'Elsewhere')")
        .bind(concept)
        .bind(f.other_kb)
        .execute(&f.pool)
        .await?;
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO concept_mappings (id, kb_id, concept_id, source, table_name)
         VALUES ($1, $2, $3, 'warehouse', 'elsewhere')",
    )
    .bind(id)
    .bind(f.other_kb)
    .bind(concept)
    .execute(&f.pool)
    .await?;
    Ok(id)
}

/// 映射此刻的状态与表态的人
async fn status_of(f: &Fx, id: Uuid) -> anyhow::Result<(String, Option<Uuid>)> {
    Ok(
        sqlx::query_as("SELECT status, decided_by FROM concept_mappings WHERE id = $1")
            .bind(id)
            .fetch_one(&f.pool)
            .await?,
    )
}

/// 台账里映射决定的行数
async fn mapping_rows(f: &Fx) -> anyhow::Result<usize> {
    Ok(ledger(f)
        .await?
        .iter()
        .filter(|r| r.operation == "mapping.decide")
        .count())
}

/// `mapping.decided` 的审计行
async fn decided(f: &Fx) -> anyhow::Result<Vec<super::review_token_write_support::Audit>> {
    Ok(audits(f)
        .await?
        .into_iter()
        .filter(|a| a.action == "mapping.decided")
        .collect())
}

#[tokio::test]
async fn a_token_mapping_decision_applies_at_once_with_one_applied_row() -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (token_id, token) = f.token(f.editor, "mapping agent", "write", None).await?;
        let id = seeded_mapping(&f).await?;
        let tail = format!("review/mappings/{id}");
        anyhow::ensure!(status_of(&f, id).await? == ("proposed".to_string(), None));
        let ledger_before = ledger(&f).await?.len();

        // 没有写权的调用方：与 M001 到 M003 的写入同一组 code，什么都不改
        let (_, reader) = f.token(f.editor, "reader", "read", None).await?;
        let (_, below) = f.token(f.viewer, "viewer writer", "write", None).await?;
        let (_, elsewhere) = f
            .token(f.editor, "elsewhere", "write", Some(&[f.other_kb]))
            .await?;
        for (bearer, code) in [
            (&reader, "token_read_only"),
            (&below, "token_owner_below_editor"),
            (&elsewhere, "token_kb_out_of_scope"),
        ] {
            let (status, body) = post(&f, bearer, &tail, json!({ "status": "confirmed" })).await?;
            anyhow::ensure!(
                status == StatusCode::FORBIDDEN && body["code"] == code,
                "{code}: {status} {body}"
            );
        }
        anyhow::ensure!(status_of(&f, id).await?.0 == "proposed");
        anyhow::ensure!(ledger(&f).await?.len() == ledger_before);

        // 写令牌：当场落地，回答就是人拿到的那一个
        let (status, body) = post(&f, &token, &tail, json!({ "status": "confirmed" })).await?;
        anyhow::ensure!(
            status == StatusCode::OK && body == json!({ "ok": true }),
            "{status} {body}"
        );
        anyhow::ensure!(
            status_of(&f, id).await? == ("confirmed".to_string(), Some(f.editor)),
            "the mapping is confirmed under the token's owner"
        );

        // 台账：正好多一行 applied，目标是这条映射
        let rows = ledger(&f).await?;
        anyhow::ensure!(rows.len() == ledger_before + 1, "one ledger row");
        let applied = rows.last().expect("the row");
        anyhow::ensure!(
            applied.operation == "mapping.decide"
                && applied.target_kind == "concept_mapping"
                && applied.target_id == id
                && applied.outcome == "applied"
                && applied.gate_reason.is_none()
                && applied.owner_id == f.editor
                && applied.token_id == token_id
                && applied.token_name == "mapping agent",
            "{applied:?}"
        );
        anyhow::ensure!(
            applied.payload["request"] == json!({ "status": "confirmed" })
                && applied.payload["action"] == "confirmed"
                && applied.payload["target_label"]
                    .as_str()
                    .is_some_and(|label| !label.trim().is_empty()),
            "{}",
            applied.payload
        );

        // 审计：actor 是主人，detail 里是令牌的 id、名字与台账行的 id
        let audited = decided(&f).await?;
        anyhow::ensure!(audited.len() == 1, "{audited:?}");
        anyhow::ensure!(
            audited[0].actor == Some(f.editor)
                && audited[0].target_kind == "concept_mapping"
                && audited[0].target_id == Some(id)
                && audited[0].detail
                    == json!({
                        "status": "confirmed",
                        "token_id": token_id,
                        "token_name": "mapping agent",
                        "action_id": applied.id,
                    }),
            "{audited:?}"
        );

        // Agent 一档：会话与令牌读到同一行，读得出「主人经哪一枚令牌」做了什么
        for bearer in [&f.editor_session, &token] {
            let (status, history) = get(&f, bearer, "review?queue=agent&limit=200").await?;
            anyhow::ensure!(status == StatusCode::OK, "{status} {history}");
            let shown = history["items"]
                .as_array()
                .and_then(|items| items.iter().find(|i| i["id"] == json!(applied.id)))
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("not in Agent History: {history}"))?;
            anyhow::ensure!(
                shown["source"] == "token"
                    && shown["operation"] == "mapping.decide"
                    && shown["outcome"] == "applied"
                    && shown["action"] == "confirmed"
                    && shown["owner_name"] == "Editor"
                    && shown["token_name"] == "mapping agent"
                    && shown["target_label"] == applied.payload["target_label"],
                "{shown}"
            );
            // 落地的行不是提议：没有理由，也没有等人答的状态
            anyhow::ensure!(
                shown.get("reason").is_none() && shown.get("state").is_none(),
                "{shown}"
            );
        }

        // 认不出的 id、别的库的 id：404，与会话对一条不存在的映射的回答同一个状态
        let foreign = foreign_mapping(&f).await?;
        for missing in [Uuid::now_v7(), foreign] {
            let (status, body) = post(
                &f,
                &token,
                &format!("review/mappings/{missing}"),
                json!({ "status": "confirmed" }),
            )
            .await?;
            anyhow::ensure!(status == StatusCode::NOT_FOUND, "{status} {body}");
        }
        anyhow::ensure!(
            status_of(&f, foreign).await?.0 == "proposed",
            "another base's mapping is untouched"
        );

        // confirmed / rejected 之外：与会话同一个状态、同一个 body
        let bad = json!({ "status": "proposed" });
        let by_session = post(&f, &f.editor_session, &tail, bad.clone()).await?;
        let by_token = post(&f, &token, &tail, bad).await?;
        anyhow::ensure!(
            by_token == by_session && by_token.1["code"] == "bad_status",
            "{by_token:?} {by_session:?}"
        );
        anyhow::ensure!(mapping_rows(&f).await? == 1, "refusals write no row");

        // 会话的决定同从前：同一个 body，不写台账，审计 detail 里只有状态
        let (status, body) = post(
            &f,
            &f.editor_session,
            &tail,
            json!({ "status": "rejected" }),
        )
        .await?;
        anyhow::ensure!(
            status == StatusCode::OK && body == json!({ "ok": true }),
            "{status} {body}"
        );
        anyhow::ensure!(
            mapping_rows(&f).await? == 1,
            "a session writes no ledger row"
        );
        let audited = decided(&f).await?;
        anyhow::ensure!(
            audited.len() == 2
                && audited[1].actor == Some(f.editor)
                && audited[1].detail == json!({ "status": "rejected" }),
            "{audited:?}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_mapping_decision_equal_to_the_status_answers_already_decided_and_writes_no_row(
) -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (_, token) = f.token(f.editor, "mapping agent", "write", None).await?;
        let id = seeded_mapping(&f).await?;
        let tail = format!("review/mappings/{id}");

        // 不一样的状态：落地，一行
        let (status, body) = post(&f, &token, &tail, json!({ "status": "confirmed" })).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        anyhow::ensure!(mapping_rows(&f).await? == 1);

        // 同一个状态：`already_decided`，映射那一行、图、台账、审计都不动——REST 与工具一样
        let before = (
            row(&f, "concept_mappings", id).await?,
            snapshot(&f).await?,
            ledger(&f).await?.len(),
            audits(&f).await?.len(),
        );
        let (status, body) = post(&f, &token, &tail, json!({ "status": "confirmed" })).await?;
        anyhow::ensure!(
            status == StatusCode::CONFLICT && body["code"] == "already_decided",
            "{status} {body}"
        );
        let by_tool = tool_call(
            &f,
            &token,
            "review_mapping",
            json!({ "id": id, "status": "confirmed" }),
        )
        .await?;
        ensure_tool_refused(&by_tool, "already_decided")?;
        anyhow::ensure!(
            row(&f, "concept_mappings", id).await? == before.0
                && snapshot(&f).await? == before.1
                && ledger(&f).await?.len() == before.2,
            "an equal decision changes nothing and writes no ledger row"
        );
        // 工具调用自己留一行审计（M002 起就有）；映射的审计行一行不多
        anyhow::ensure!(decided(&f).await?.len() == 1, "no mapping.decided row");

        // 换一个状态：映射没有「定了」这一态，经工具照样落地，回答是 REST 的那一个
        let by_tool = tool_call(
            &f,
            &token,
            "review_mapping",
            json!({ "id": id, "status": "rejected" }),
        )
        .await?;
        anyhow::ensure!(
            by_tool["isError"] == false && by_tool["structuredContent"] == json!({ "ok": true }),
            "{by_tool}"
        );
        anyhow::ensure!(
            sqlx::query_scalar::<_, String>("SELECT status FROM concept_mappings WHERE id = $1")
                .bind(id)
                .fetch_one(&f.pool)
                .await?
                == "rejected"
        );
        anyhow::ensure!(
            mapping_rows(&f).await? == 2,
            "a different status writes one row"
        );
        let rows = ledger(&f).await?;
        let last = rows.last().expect("the row");
        anyhow::ensure!(
            last.outcome == "applied" && last.payload["action"] == "rejected",
            "{last:?}"
        );
        anyhow::ensure!(decided(&f).await?.len() == 2);

        // 再回到 confirmed（REST）：又是一个不一样的状态，再落地一次
        let (status, body) = post(&f, &token, &tail, json!({ "status": "confirmed" })).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        anyhow::ensure!(mapping_rows(&f).await? == 3);

        // 工具上的另外两种拒绝：认不出的 id 是 not_found，认不出的状态是 bad_status
        let by_tool = tool_call(
            &f,
            &token,
            "review_mapping",
            json!({ "id": Uuid::now_v7(), "status": "confirmed" }),
        )
        .await?;
        ensure_tool_refused(&by_tool, "not_found")?;
        let by_tool = tool_call(
            &f,
            &token,
            "review_mapping",
            json!({ "id": id, "status": "proposed" }),
        )
        .await?;
        ensure_tool_refused(&by_tool, "bad_status")?;
        anyhow::ensure!(mapping_rows(&f).await? == 3, "refusals write no row");
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_token_reads_the_mappings_a_person_sees() -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        // 读得最少的那一种令牌：Viewer 的只读令牌
        let (_, token) = f.token(f.viewer, "reader", "read", None).await?;
        // 再添两条不同状态的映射，筛选才筛得出东西
        for (name, status) in [("Seed Margin", "confirmed"), ("Seed Churn", "rejected")] {
            let concept = f.entity(f.company_type, name).await?;
            sqlx::query(
                "INSERT INTO concept_mappings (id, kb_id, concept_id, source, table_name, status)
                 VALUES ($1, $2, $3, 'seed_warehouse', 'seed_facts', $4)",
            )
            .bind(Uuid::now_v7())
            .bind(f.kb)
            .bind(concept)
            .bind(status)
            .execute(&f.pool)
            .await?;
        }

        // 列表路由：令牌读到的与会话读到的逐字相等，带筛选、翻页时也是
        for query in [
            "mappings",
            "mappings?status=proposed",
            "mappings?status=rejected&q=churn",
            "mappings?q=seed&limit=1&offset=1",
            "mappings?limit=0&offset=-3",
            "mappings?q=no-such-concept",
        ] {
            let by_session = get(&f, &f.viewer_session, query).await?;
            let by_token = get(&f, &token, query).await?;
            anyhow::ensure!(by_session.0 == StatusCode::OK, "{query}: {by_session:?}");
            anyhow::ensure!(
                by_token == by_session,
                "{query}\n{by_token:?}\n{by_session:?}"
            );
        }
        let (_, all) = get(&f, &token, "mappings").await?;
        anyhow::ensure!(
            keys(&all) == ["counts", "items", "last_run", "total"]
                && all["total"] == 3
                && all["counts"] == json!({ "proposed": 1, "confirmed": 1, "rejected": 1 }),
            "{all}"
        );
        let (_, one) = get(&f, &token, "mappings?status=rejected&q=churn").await?;
        anyhow::ensure!(
            one["total"] == 1 && one["items"][0]["concept_name"] == "Seed Churn",
            "{one}"
        );

        // 认不出的筛选：与会话同一个状态、同一个 body
        let by_session = get(&f, &f.viewer_session, "mappings?status=open").await?;
        let by_token = get(&f, &token, "mappings?status=open").await?;
        anyhow::ensure!(
            by_token == by_session && by_token.1["code"] == "bad_status",
            "{by_token:?}"
        );

        // `review_mapping_list`：`structuredContent` 就是那条路由的 body
        let by_tool = tool_call(&f, &token, "review_mapping_list", json!({})).await?;
        anyhow::ensure!(
            by_tool["isError"] == false && by_tool["structuredContent"] == all,
            "{by_tool}"
        );
        let by_tool = tool_call(
            &f,
            &token,
            "review_mapping_list",
            json!({ "status": "rejected", "q": "churn", "limit": 5, "offset": 0 }),
        )
        .await?;
        anyhow::ensure!(by_tool["structuredContent"] == one, "{by_tool}");
        let by_tool = tool_call(
            &f,
            &token,
            "review_mapping_list",
            json!({ "status": "open" }),
        )
        .await?;
        ensure_tool_refused(&by_tool, "bad_status")?;

        // 够不着这个库的令牌：404，不带条目
        let (_, excluded) = f
            .token(f.viewer, "elsewhere", "read", Some(&[f.other_kb]))
            .await?;
        let (status, body) = get(&f, &excluded, "mappings").await?;
        anyhow::ensure!(
            status == StatusCode::NOT_FOUND && body.get("items").is_none(),
            "{status} {body}"
        );

        // Review 的共享列表里 `mappings` 仍不是开给令牌的档：REST 与 `review_list` 都是
        // `unknown_queue`
        let (status, body) = get(&f, &token, "review?queue=mappings").await?;
        anyhow::ensure!(
            status != StatusCode::OK && body["code"] == "unknown_queue",
            "{status} {body}"
        );
        let by_tool =
            tool_call(&f, &token, "review_list", json!({ "section": "mappings" })).await?;
        ensure_tool_refused(&by_tool, "unknown_queue")?;

        // 读不是写：只读令牌定不了映射，写一条口径的路由也仍只给会话
        let id = seeded_mapping(&f).await?;
        let (status, body) = post(
            &f,
            &token,
            &format!("review/mappings/{id}"),
            json!({ "status": "confirmed" }),
        )
        .await?;
        anyhow::ensure!(
            status == StatusCode::FORBIDDEN && body["code"] == "token_read_only",
            "{status} {body}"
        );
        let (status, body) = post(
            &f,
            &token,
            "mappings",
            json!({ "concept": "X", "source": "seed_warehouse", "sql": "SELECT 1" }),
        )
        .await?;
        anyhow::ensure!(status == StatusCode::UNAUTHORIZED, "{status} {body}");
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}
