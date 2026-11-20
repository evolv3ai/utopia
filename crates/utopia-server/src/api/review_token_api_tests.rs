//! 个人令牌走 Review 的三条路由（0076）：读重复项、读总览、把一对重复项分开。
//!
//! 要钉住的行为：
//!
//! - **读到的和编辑者在 Review 页看到的一样**（SC2）：Viewer 的只读令牌拿到的 `items`、
//!   `counts` 与总览，和会话编辑者同一个请求拿到的相等；空档一样是空列表和零（E3）；
//!   顺序一样，翻页也一样（E4）
//! - **够不着的库是 404**（N2）：`kb_ids` 排除了这个库的令牌，两条读路由都答 404，
//!   与一个不存在的库同一个回答，响应里没有条目、计数或总览
//! - **别的档没开**：令牌问 `pending`、`agent`、`merges` 得到 `unknown_queue`，会话照旧
//! - **被拒绝的合并、手动合并、撤销、批量什么都不改**：合并与撤销开给了编辑者的写令牌
//!   （合并过闸门，见 `review_token_merge_tests.rs`；撤销见 `review_token_revert_tests.rs`），
//!   只读令牌与 Viewer 的令牌在两条合并路由和撤销路由上照旧被拒绝；批量仍只认会话。
//!   图上的行前后相等，台账没有行
//! - **分开一对**：写令牌加编辑者——落地、进台账、审计行带令牌；拒绝各有各的 code，
//!   拒绝什么都不改；不排治理任务，这一对上开着的建议原样留着
//!
//! 夹具在 `review_session_unchanged_tests.rs`。没有 `UTOPIA_DATABASE_URL` 时跳过。
use axum::http::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;

use super::review_session_unchanged_tests::Fx;

const DUPLICATES: &str = "review?queue=duplicates";

fn ids(body: &Value) -> Vec<String> {
    body["items"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|i| i["id"].as_str().unwrap_or_default().to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// 这个库在图上的全部行：实体、事实、审核行、合并日志。前后各读一次，比是否相等
async fn graph_rows(f: &Fx) -> anyhow::Result<Value> {
    let mut out = serde_json::Map::new();
    for table in ["entities", "facts", "resolution_reviews", "entity_merges"] {
        // 表名是上面写死的四个，不是外面来的
        let rows: Value = sqlx::query_scalar(&format!(
            "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY t.id), '[]'::jsonb)
               FROM {table} t WHERE t.kb_id = $1"
        ))
        .bind(f.kb)
        .fetch_one(&f.pool)
        .await?;
        out.insert(table.to_string(), rows);
    }
    Ok(Value::Object(out))
}

async fn ledger(
    f: &Fx,
) -> anyhow::Result<Vec<utopia_store::token_agent_actions::TokenAgentAction>> {
    Ok(utopia_store::token_agent_actions::list(&f.pool, f.kb, 50, 0).await?)
}

async fn keep(f: &Fx, bearer: &str, review: Uuid) -> anyhow::Result<(StatusCode, Value)> {
    f.call(
        bearer,
        "POST",
        &format!("/kbs/{}/review/{review}", f.kb),
        Some(json!({ "action": "keep", "rationale": "different people" })),
    )
    .await
}

#[tokio::test]
async fn a_read_token_of_a_viewer_lists_duplicates_and_overview_as_a_session_editor_sees_them(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        f.pair("Zhang Wei", "Zhang Wei").await?;
        f.pair("Li Na", "Li Na").await?;
        f.pair("Acme Ltd", "Acme Limited").await?;
        f.conflicting_pair("Mercury", "Mercury").await?;
        // 主人正好是 Viewer，令牌只读：读的下限
        let (_, token) = f.token(f.viewer, "reader", "read", None).await?;

        for path in [
            format!("/kbs/{kb}/{DUPLICATES}"),
            format!("/kbs/{kb}/review"),
            format!("/kbs/{kb}/{DUPLICATES}&types=same"),
            format!("/kbs/{kb}/{DUPLICATES}&types=conflict"),
        ] {
            let (status, mine) = f.call(&token, "GET", &path, None).await?;
            anyhow::ensure!(status == StatusCode::OK, "token {path}: {status} {mine}");
            let (status, theirs) = f.call(&f.editor_session, "GET", &path, None).await?;
            anyhow::ensure!(status == StatusCode::OK, "session {path}: {status}");
            anyhow::ensure!(
                mine["items"] == theirs["items"],
                "{path}: items differ\n{}\n{}",
                mine["items"],
                theirs["items"]
            );
            anyhow::ensure!(mine["counts"] == theirs["counts"], "{path}: counts differ");
            anyhow::ensure!(mine == theirs, "{path}: bodies differ");
        }
        let (_, all) = f
            .call(&token, "GET", &format!("/kbs/{kb}/{DUPLICATES}"), None)
            .await?;
        anyhow::ensure!(ids(&all).len() == 4, "four pending pairs: {all}");
        anyhow::ensure!(all["counts"]["duplicates"] == 4, "{all}");
        anyhow::ensure!(all["counts"]["duplicates_type_conflict"] == 1, "{all}");

        let path = format!("/kbs/{kb}/review/summary");
        let (status, mine) = f.call(&token, "GET", &path, None).await?;
        anyhow::ensure!(status == StatusCode::OK, "token summary: {status} {mine}");
        let (status, theirs) = f.call(&f.editor_session, "GET", &path, None).await?;
        anyhow::ensure!(status == StatusCode::OK, "session summary: {status}");
        anyhow::ensure!(mine == theirs, "summary differs\n{mine}\n{theirs}");
        anyhow::ensure!(mine["waiting"].is_object(), "{mine}");

        // 限定在这个库上的令牌也一样读得到
        let (_, scoped) = f.token(f.viewer, "scoped", "read", Some(&[kb])).await?;
        let (status, body) = f
            .call(&scoped, "GET", &format!("/kbs/{kb}/{DUPLICATES}"), None)
            .await?;
        anyhow::ensure!(status == StatusCode::OK && body == all, "{status} {body}");
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_token_whose_kb_ids_exclude_the_base_gets_404_on_review_reads() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        f.pair("Zhang Wei", "Zhang Wei").await?;
        // 同一个人的两枚令牌：一枚够得着这个库，一枚只列了另一个库
        let (_, reaches) = f.token(f.viewer, "reaches", "read", Some(&[kb])).await?;
        let (_, excluded) = f
            .token(f.viewer, "excluded", "read", Some(&[f.other_kb]))
            .await?;
        let nowhere = Uuid::now_v7();
        for tail in [DUPLICATES, "review/summary"] {
            let (status, _) = f
                .call(&reaches, "GET", &format!("/kbs/{kb}/{tail}"), None)
                .await?;
            anyhow::ensure!(status == StatusCode::OK, "{tail}: {status}");

            let (status, body) = f
                .call(&excluded, "GET", &format!("/kbs/{kb}/{tail}"), None)
                .await?;
            anyhow::ensure!(status == StatusCode::NOT_FOUND, "{tail}: {status} {body}");
            for key in [
                "items", "counts", "queue", "waiting", "decided", "health", "agent",
            ] {
                anyhow::ensure!(body.get(key).is_none(), "{tail}: {key} leaked in {body}");
            }
            // 与一个不存在的库同一个回答
            let (missing_status, missing) = f
                .call(&excluded, "GET", &format!("/kbs/{nowhere}/{tail}"), None)
                .await?;
            anyhow::ensure!(
                missing_status == status && missing == body,
                "{tail}: an excluded base must read like a missing one: {body} vs {missing}"
            );
        }
        // 写令牌也一样：读路由问的是够不够得着，不是 scope
        let (_, excluded_write) = f
            .token(f.editor, "excluded write", "write", Some(&[f.other_kb]))
            .await?;
        for tail in [DUPLICATES, "review/summary"] {
            let (status, _) = f
                .call(&excluded_write, "GET", &format!("/kbs/{kb}/{tail}"), None)
                .await?;
            anyhow::ensure!(status == StatusCode::NOT_FOUND, "{tail}: {status}");
        }
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn an_empty_duplicates_section_reads_the_same_for_a_token_and_a_session() -> anyhow::Result<()>
{
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let path = format!("/kbs/{}/{DUPLICATES}", f.kb);
        let (_, token) = f.token(f.viewer, "reader", "read", None).await?;
        let (status, mine) = f.call(&token, "GET", &path, None).await?;
        anyhow::ensure!(status == StatusCode::OK, "token: {status} {mine}");
        let (status, theirs) = f.call(&f.editor_session, "GET", &path, None).await?;
        anyhow::ensure!(status == StatusCode::OK, "session: {status} {theirs}");
        anyhow::ensure!(mine["items"] == json!([]), "{mine}");
        anyhow::ensure!(theirs["items"] == json!([]), "{theirs}");
        anyhow::ensure!(mine["counts"] == theirs["counts"], "counts differ");
        anyhow::ensure!(mine["counts"]["duplicates"] == 0, "{mine}");
        anyhow::ensure!(mine["counts"]["duplicates_same_type"] == 0, "{mine}");
        anyhow::ensure!(mine["counts"]["duplicates_type_conflict"] == 0, "{mine}");
        anyhow::ensure!(mine == theirs, "bodies differ");
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_token_receives_duplicates_in_the_session_order() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        let mut seeded = Vec::new();
        for (i, name) in ["Zhang Wei", "Li Na", "Wang Fang", "Liu Yang", "Chen Jie"]
            .iter()
            .enumerate()
        {
            seeded.push(f.pair(name, name).await?);
            if i % 2 == 0 {
                seeded.push(f.conflicting_pair(name, name).await?);
            }
        }
        let (_, token) = f.token(f.viewer, "reader", "read", None).await?;
        // 五对同类，三对异类，一共八对
        for (types, total) in [
            ("", 8),
            ("&types=any", 8),
            ("&types=same", 5),
            ("&types=conflict", 3),
        ] {
            let mut walked: Vec<String> = Vec::new();
            for offset in [0, 2] {
                let path = format!("/kbs/{kb}/{DUPLICATES}{types}&limit=2&offset={offset}");
                let (status, mine) = f.call(&token, "GET", &path, None).await?;
                anyhow::ensure!(status == StatusCode::OK, "token {path}: {status}");
                let (status, theirs) = f.call(&f.editor_session, "GET", &path, None).await?;
                anyhow::ensure!(status == StatusCode::OK, "session {path}: {status}");
                let (mine, theirs) = (ids(&mine), ids(&theirs));
                anyhow::ensure!(mine == theirs, "{path}: {mine:?} vs {theirs:?}");
                anyhow::ensure!(
                    mine.len() == (total - offset).clamp(0, 2) as usize,
                    "{path}: page of {}",
                    mine.len()
                );
                walked.extend(mine);
            }
            // 两页不重叠，而且就是会话那个顺序里的前几条
            let mut unique = walked.clone();
            unique.sort();
            unique.dedup();
            anyhow::ensure!(unique.len() == walked.len(), "{types}: pages overlap");
            let path = format!("/kbs/{kb}/{DUPLICATES}{types}&limit=200");
            let (_, whole) = f.call(&f.editor_session, "GET", &path, None).await?;
            let whole = ids(&whole);
            anyhow::ensure!(whole.len() == total as usize, "{types}: {}", whole.len());
            anyhow::ensure!(
                whole[..walked.len()] == walked[..],
                "{types}: two pages must be the head of the session's list"
            );
        }
        // 没有筛选时最新的在前
        let (_, whole) = f
            .call(
                &token,
                "GET",
                &format!("/kbs/{kb}/{DUPLICATES}&limit=200"),
                None,
            )
            .await?;
        let newest_first: Vec<String> = seeded.iter().rev().map(Uuid::to_string).collect();
        anyhow::ensure!(ids(&whole) == newest_first, "{:?}", ids(&whole));
        // 同一个 clamp：0 当 1，负的 offset 当 0
        let path = format!("/kbs/{kb}/{DUPLICATES}&limit=0&offset=-3");
        let (_, mine) = f.call(&token, "GET", &path, None).await?;
        let (_, theirs) = f.call(&f.editor_session, "GET", &path, None).await?;
        anyhow::ensure!(
            ids(&mine).len() == 1 && ids(&mine) == ids(&theirs),
            "{:?}",
            ids(&mine)
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_token_refused_on_merge_manual_merge_revert_or_batch_leaves_the_graph_equal(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        let left = f.entity(f.person_type, "Zhang Wei").await?;
        let right = f.entity(f.person_type, "Zhang Wei").await?;
        let pair = f.pair_of(left, right).await?;
        let other = f.pair("Li Na", "Li Na").await?;
        // 一条事实挂在这一对上：合并真落地的话它会被挪走
        sqlx::query(
            "INSERT INTO facts (id, kb_id, subject_id, object_id, layer, phrase)
             VALUES ($1, $2, $3, $4, 'open', 'knows')",
        )
        .bind(Uuid::now_v7())
        .bind(kb)
        .bind(left)
        .bind(right)
        .execute(&f.pool)
        .await?;
        // 一笔人做的合并，给撤销路由一个真的目标
        let merged = f.pair("Acme Ltd", "Acme Limited").await?;
        let (status, _) = f
            .call(
                &f.editor_session,
                "POST",
                &format!("/kbs/{kb}/review/{merged}"),
                Some(json!({ "action": "merge" })),
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "seeding a merge: {status}");
        let merge_id: Uuid = sqlx::query_scalar("SELECT id FROM entity_merges WHERE kb_id = $1")
            .bind(kb)
            .fetch_one(&f.pool)
            .await?;

        // 能做的最多的令牌：写 scope，主人是编辑者
        let (_, token) = f.token(f.editor, "writer", "write", None).await?;
        let before = graph_rows(&f).await?;
        for table in ["entities", "facts", "resolution_reviews", "entity_merges"] {
            anyhow::ensure!(
                before[table].as_array().is_some_and(|a| !a.is_empty()),
                "{table} must have rows for the comparison to mean anything"
            );
        }

        // 队列里的合并、手动合并与撤销开给了编辑者的写令牌；留下来的拒绝是这两种——只读
        // 令牌（`token_read_only`）与主人是 Viewer 的写令牌（`token_owner_below_editor`），
        // 两条合并路由和撤销路由上都一样
        let (_, read_only) = f.token(f.editor, "reader", "read", None).await?;
        let (_, viewer_write) = f.token(f.viewer, "viewer writer", "write", None).await?;
        for (refused, code) in [
            (&read_only, "token_read_only"),
            (&viewer_write, "token_owner_below_editor"),
        ] {
            for (path, request) in [
                (
                    format!("/kbs/{kb}/review/{pair}"),
                    json!({ "action": "merge", "rationale": "same person" }),
                ),
                (
                    format!("/kbs/{kb}/entities/merge"),
                    json!({ "source": left, "target": right }),
                ),
                (format!("/kbs/{kb}/merges/{merge_id}/revert"), json!({})),
            ] {
                let (status, body) = f.call(refused, "POST", &path, Some(request)).await?;
                anyhow::ensure!(
                    status == StatusCode::FORBIDDEN,
                    "{code} {path}: {status} {body}"
                );
                anyhow::ensure!(body["code"] == code, "{code} {path}: {body}");
            }
        }
        // 批量（不论动作）：这条路由仍只认会话
        let closed = [
            (
                format!("/kbs/{kb}/review/batch"),
                json!({ "ids": [pair, other], "action": "merge" }),
            ),
            (
                format!("/kbs/{kb}/review/batch"),
                json!({ "ids": [pair, other], "action": "keep" }),
            ),
        ];
        for (path, body) in closed {
            let (status, answer) = f.call(&token, "POST", &path, Some(body.clone())).await?;
            anyhow::ensure!(
                status == StatusCode::UNAUTHORIZED,
                "{path} {body}: {status} {answer}"
            );
        }

        let after = graph_rows(&f).await?;
        for table in ["entities", "facts", "resolution_reviews", "entity_merges"] {
            anyhow::ensure!(
                before[table] == after[table],
                "{table} changed under a refused token call"
            );
        }
        anyhow::ensure!(
            before == after,
            "the graph changed under a refused token call"
        );
        anyhow::ensure!(
            ledger(&f).await?.is_empty(),
            "a refusal writes no ledger row"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_write_token_of_an_editor_keeps_a_pair_over_the_api() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        let pair = f.pair("Zhang Wei", "Zhang Wei").await?;
        let stays = f.pair("Li Na", "Li Na").await?;
        // 主人正好是 Editor：写的下限
        let (token_id, token) = f.token(f.editor, "cleanup agent", "write", None).await?;

        let (status, body) = keep(&f, &token, pair).await?;
        anyhow::ensure!(status == StatusCode::OK, "keep: {status} {body}");
        anyhow::ensure!(body["ok"] == true && body["outcome"] == "applied", "{body}");
        let action_id: Uuid = body["action_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("no action_id in {body}"))?
            .parse()?;
        anyhow::ensure!(
            body == json!({ "ok": true, "outcome": "applied", "action_id": action_id }),
            "keep body: {body}"
        );

        // 这一对不在队列里了，令牌看不到，会话也看不到
        let path = format!("/kbs/{kb}/{DUPLICATES}");
        for bearer in [&token, &f.editor_session] {
            let (status, list) = f.call(bearer, "GET", &path, None).await?;
            anyhow::ensure!(status == StatusCode::OK, "{status}");
            anyhow::ensure!(ids(&list) == [stays.to_string()], "{:?}", ids(&list));
            anyhow::ensure!(list["counts"]["duplicates"] == 1, "{list}");
        }
        anyhow::ensure!(
            f.review_row(pair).await? == ("kept".to_string(), Some(f.editor), Some(token_id)),
            "decided by the owner, through the token"
        );

        let rows = ledger(&f).await?;
        anyhow::ensure!(rows.len() == 1, "one ledger row: {rows:?}");
        let row = &rows[0];
        anyhow::ensure!(row.id == action_id, "action_id names the ledger row");
        anyhow::ensure!(row.outcome == "applied" && row.operation == "review.keep");
        anyhow::ensure!(row.target_kind == "review" && row.target_id == pair);
        anyhow::ensure!(row.owner_id == f.editor && row.token_id == token_id);
        anyhow::ensure!(row.token_name == "cleanup agent", "{}", row.token_name);

        let audits = f.audits("review.keep", pair).await?;
        anyhow::ensure!(audits.len() == 1, "one audit row: {audits:?}");
        anyhow::ensure!(audits[0].0 == Some(f.editor), "the owner is the actor");
        anyhow::ensure!(
            audits[0].1
                == json!({ "left": "Zhang Wei", "right": "Zhang Wei", "score": 0.5,
                           "why": "different people", "token_id": token_id,
                           "token_name": "cleanup agent" }),
            "{}",
            audits[0].1
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_keep_over_the_api_is_refused_with_its_code() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let pair = f.pair("Zhang Wei", "Zhang Wei").await?;
        let untouched = ("pending".to_string(), None, None);

        let (_, read_only) = f.token(f.editor, "reader", "read", None).await?;
        let (_, out_of_scope) = f
            .token(f.editor, "elsewhere", "write", Some(&[f.other_kb]))
            .await?;
        let (_, viewer_write) = f.token(f.viewer, "viewer writer", "write", None).await?;
        // 只读、主人又是 Viewer：scope 先于角色（E13）
        let (_, viewer_read) = f.token(f.viewer, "viewer reader", "read", None).await?;
        // 只读、又够不着：scope 先于 kb_ids
        let (_, read_elsewhere) = f
            .token(f.editor, "reader elsewhere", "read", Some(&[f.other_kb]))
            .await?;
        for (token, code) in [
            (&read_only, "token_read_only"),
            (&out_of_scope, "token_kb_out_of_scope"),
            (&viewer_write, "token_owner_below_editor"),
            (&viewer_read, "token_read_only"),
            (&read_elsewhere, "token_read_only"),
        ] {
            let (status, body) = keep(&f, token, pair).await?;
            anyhow::ensure!(status == StatusCode::FORBIDDEN, "{code}: {status} {body}");
            anyhow::ensure!(body["code"] == code, "expected {code}: {body}");
            anyhow::ensure!(
                f.review_row(pair).await? == untouched,
                "{code}: the pair must stay pending"
            );
            anyhow::ensure!(ledger(&f).await?.is_empty(), "{code}: no ledger row");
            anyhow::ensure!(f.audits("review.keep", pair).await?.is_empty(), "{code}");
        }
        // 同样的拒绝也挡在「没开的动作」前面：只读令牌问合并，答的是 token_read_only
        let (status, body) = f
            .call(
                &read_only,
                "POST",
                &format!("/kbs/{}/review/{pair}", f.kb),
                Some(json!({ "action": "merge" })),
            )
            .await?;
        anyhow::ensure!(
            status == StatusCode::FORBIDDEN && body["code"] == "token_read_only",
            "{status} {body}"
        );
        anyhow::ensure!(f.review_row(pair).await? == untouched);

        // 落地一次，再来一次就是 already_decided——同一枚令牌、另一枚令牌都一样
        let (first_id, first) = f.token(f.editor, "first", "write", None).await?;
        let (_, second) = f.token(f.editor, "second", "write", None).await?;
        let (status, body) = keep(&f, &first, pair).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let decided = ("kept".to_string(), Some(f.editor), Some(first_id));
        for token in [&first, &second] {
            let (status, body) = keep(&f, token, pair).await?;
            anyhow::ensure!(status == StatusCode::CONFLICT, "repeat: {status} {body}");
            anyhow::ensure!(body["code"] == "already_decided", "repeat: {body}");
            anyhow::ensure!(
                f.review_row(pair).await? == decided,
                "a repeat must not change who decided"
            );
            anyhow::ensure!(
                ledger(&f).await?.len() == 1,
                "a repeat writes no ledger row"
            );
            anyhow::ensure!(f.audits("review.keep", pair).await?.len() == 1);
        }
        // 这个库里没有的 id：404，不是 already_decided
        let (status, _) = keep(&f, &first, Uuid::now_v7()).await?;
        anyhow::ensure!(status == StatusCode::NOT_FOUND, "unknown id: {status}");
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_token_keep_over_the_api_starts_no_govern_round() -> anyhow::Result<()> {
    // 治理开着的库
    let Some(f) = Fx::new(true).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        let pair = f.pair("Zhang Wei", "Zhang Wei").await?;
        let by_person = f.pair("Li Na", "Li Na").await?;
        // 这一对上开着一条治理建议，等人答
        let proposal = utopia_store::governance::record(
            &f.pool,
            kb,
            utopia_store::governance::NewDecision {
                run_id: Uuid::now_v7(),
                target_id: pair,
                action: "merge",
                confidence: 0.7,
                reason: None,
                precedents: json!([]),
                status: "proposed",
                merge_id: None,
                question: None,
                trace: json!([]),
                calls: 0,
            },
        )
        .await?;
        let govern_jobs = || async {
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM jobs WHERE kind = 'govern' AND payload->>'kb_id' = $1",
            )
            .bind(kb.to_string())
            .fetch_one(&f.pool)
            .await
        };
        let proposal_status = || async {
            sqlx::query_scalar::<_, String>("SELECT status FROM agent_decisions WHERE id = $1")
                .bind(proposal)
                .fetch_one(&f.pool)
                .await
        };
        anyhow::ensure!(govern_jobs().await? == 0);

        let (_, token) = f.token(f.editor, "cleanup agent", "write", None).await?;
        let (status, body) = keep(&f, &token, pair).await?;
        anyhow::ensure!(status == StatusCode::OK, "keep: {status} {body}");
        anyhow::ensure!(f.review_row(pair).await?.0 == "kept");

        anyhow::ensure!(
            govern_jobs().await? == 0,
            "a token keep must not queue a govern round"
        );
        anyhow::ensure!(
            proposal_status().await? == "proposed",
            "the governor's proposal on the pair must stay open for a person"
        );

        // 对照：人在同一个库里分开一对，照旧排一轮——上面的零不是因为这个库排不了
        let (status, _) = keep(&f, &f.editor_session, by_person).await?;
        anyhow::ensure!(status == StatusCode::OK, "session keep: {status}");
        anyhow::ensure!(
            govern_jobs().await? == 1,
            "a person's keep still queues a govern round"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}
