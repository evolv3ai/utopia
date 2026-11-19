//! 个人令牌读 Review 的十三档与两条证明（0076）。
//!
//! 要钉住的行为：
//!
//! - **十三档读到的和编辑者在 Review 页看到的一样**（SC1）：Viewer 的只读令牌对十一个列表档、
//!   总览、决策台账发同一个请求，响应体与会话编辑者的逐字相等；空档是空列表和 0（E1），
//!   顺序相同（E2），页大小照旧夹到 1 到 200、翻过头是空页和真实的 `total`（E3）
//! - **不在十三档里的名字是 `unknown_queue`**（E4），`mappings` 也算：它只开给会话
//! - **`total` 是列表翻到底的行数**（E5）；按它翻对齐一档能翻到最后一条蕴含规则（SC4 / E10），
//!   时间相同的条目各出现一次（E11）
//! - **够不着的库是 404**（SC5），与不存在的库同一个回答；撤销了的令牌是 401；MCP 端点
//!   对够不着的库照旧是 403（E12）——这里只看它，不改它
//! - **Agent 一档里令牌的那一行写着主人和令牌**（SC6），治理 agent 的行原样；令牌名是动作
//!   那一刻存下的快照（E13）
//! - **两条证明读到的与会话一样**；读的一侧开了，写的一侧一条没多开（N4）：令牌读得到
//!   「等你点头」，点不了头
//!
//! 夹具在 `review_session_unchanged_tests.rs`，种子在 `review_totals_tests.rs`。
//! 没有 `UTOPIA_DATABASE_URL` 时跳过。自建自拆，只碰自己建的库。
use axum::http::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;

use super::review_session_unchanged_tests::Fx;
use super::review_totals_tests::seed_every_section;

/// Review 的十一个列表档。加上总览与决策台账就是十三档
const SECTIONS: [&str; 11] = [
    "pending",
    "duplicates",
    "conflicts",
    "unconfirmed",
    "lowconf",
    "violations",
    "defects",
    "alignment",
    "errata",
    "agent",
    "merges",
];

/// 哪一种响应里都不该漏给够不着的调用方的键
const DATA_KEYS: [&str; 6] = ["items", "total", "counts", "events", "steps", "proof"];

async fn get(f: &Fx, bearer: &str, path: &str) -> anyhow::Result<(StatusCode, Value)> {
    f.call(bearer, "GET", path, None).await
}

/// `f.kb` 上的一次读，必须是 200。`tail` 接在 `/kbs/{kb}/` 后面
async fn ok(f: &Fx, bearer: &str, tail: &str) -> anyhow::Result<Value> {
    let path = format!("/kbs/{}/{tail}", f.kb);
    let (status, body) = get(f, bearer, &path).await?;
    anyhow::ensure!(status == StatusCode::OK, "{tail}: {status} {body}");
    Ok(body)
}

/// 同一个请求，令牌与会话编辑者各发一次，两个响应体必须逐字相等；回那个响应体
async fn same(f: &Fx, token: &str, tail: &str) -> anyhow::Result<Value> {
    let mine = ok(f, token, tail).await?;
    let theirs = ok(f, &f.editor_session, tail).await?;
    anyhow::ensure!(
        mine == theirs,
        "{tail}: the token and the session differ\n{mine}\n{theirs}"
    );
    Ok(mine)
}

fn items(body: &Value) -> Vec<Value> {
    body["items"].as_array().cloned().unwrap_or_default()
}

fn events(body: &Value) -> Vec<Value> {
    body["events"].as_array().cloned().unwrap_or_default()
}

fn total(body: &Value) -> anyhow::Result<i64> {
    body["total"]
        .as_i64()
        .ok_or_else(|| anyhow::anyhow!("total is not an integer: {body}"))
}

/// 一条的标识：有 `id` 用 `id`，类别词用 `kind_word`；两样都没有就拿整条当标识
fn identifier(item: &Value) -> String {
    item["id"]
        .as_str()
        .or_else(|| item["kind_word"].as_str())
        .map(str::to_string)
        .unwrap_or_else(|| item.to_string())
}

fn identifiers(rows: &[Value]) -> Vec<String> {
    rows.iter().map(identifier).collect()
}

/// 令牌按 `limit` 从头翻一档，翻 `total` 报的那么多行就停：回翻到的条目和 `total`。
/// 每一页都得是 200，报的 `total` 都得相同
async fn token_pages(
    f: &Fx,
    token: &str,
    queue: &str,
    limit: i64,
) -> anyhow::Result<(Vec<Value>, i64)> {
    let expected = total(&ok(f, token, &format!("review?queue={queue}&limit=1")).await?)?;
    let mut rows = Vec::new();
    let mut offset = 0;
    while offset < expected {
        let body = ok(
            f,
            token,
            &format!("review?queue={queue}&limit={limit}&offset={offset}"),
        )
        .await?;
        anyhow::ensure!(
            total(&body)? == expected,
            "{queue}: total changed from {expected} at offset {offset}"
        );
        rows.extend(items(&body));
        offset += limit;
    }
    Ok((rows, expected))
}

/// 响应体里没有任何一个数据键
fn ensure_no_data(body: &Value, what: &str) -> anyhow::Result<()> {
    for key in DATA_KEYS {
        anyhow::ensure!(body.get(key).is_none(), "{what}: `{key}` in {body}");
    }
    Ok(())
}

/// 种过的库里那条展得开的违规、另一条别的种类的违规，以及一条列在低置信里的派生事实
async fn seeded_proof_targets(f: &Fx) -> anyhow::Result<(Uuid, Uuid, Uuid)> {
    let violations = items(&ok(f, &f.editor_session, "review?queue=violations&limit=200").await?);
    let id_of = |v: &Value| -> anyhow::Result<Uuid> {
        Ok(v["id"].as_str().unwrap_or_default().parse::<Uuid>()?)
    };
    let contradiction = violations
        .iter()
        .find(|v| v["kind"] == "derived_contradiction")
        .ok_or_else(|| anyhow::anyhow!("no derived_contradiction seeded"))?;
    let other = violations
        .iter()
        .find(|v| v["kind"] != "derived_contradiction")
        .ok_or_else(|| anyhow::anyhow!("no violation of another kind seeded"))?;
    let lowconf: Vec<Uuid> =
        items(&ok(f, &f.editor_session, "review?queue=lowconf&limit=200").await?)
            .iter()
            .filter_map(|i| i["id"].as_str().and_then(|s| s.parse().ok()))
            .collect();
    let derived: Uuid = sqlx::query_scalar(
        "SELECT id FROM facts WHERE id = ANY($1) AND derived_by_rule IS NOT NULL LIMIT 1",
    )
    .bind(&lowconf)
    .fetch_one(&f.pool)
    .await?;
    Ok((id_of(contradiction)?, id_of(other)?, derived))
}

/// 种一条治理 agent 的行，与 `review_attribution_tests.rs` 同一个写法
async fn governor_row(f: &Fx, review: Uuid, status: &str) -> anyhow::Result<Uuid> {
    Ok(utopia_store::governance::record(
        &f.pool,
        f.kb,
        utopia_store::governance::NewDecision {
            run_id: Uuid::now_v7(),
            target_id: review,
            action: "keep",
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

/// 一次 MCP 的 JSON-RPC 调用，回 HTTP 状态与整个响应体
async fn rpc(
    f: &Fx,
    bearer: &str,
    method: &str,
    params: Value,
) -> anyhow::Result<(StatusCode, Value)> {
    f.call(
        bearer,
        "POST",
        &format!("/kbs/{}/mcp", f.kb),
        Some(json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params })),
    )
    .await
}

#[tokio::test]
async fn a_read_token_of_a_viewer_lists_each_of_the_thirteen_sections_as_a_session_editor_sees_them(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        seed_every_section(&f).await?;
        // 主人正好是 Viewer，令牌只读：读的下限
        let (_, token) = f.token(f.viewer, "reader", "read", None).await?;
        for queue in SECTIONS {
            // 缺省的一页，和一页装得下整档的
            for tail in [
                format!("review?queue={queue}"),
                format!("review?queue={queue}&limit=200"),
            ] {
                let body = same(&f, &token, &tail).await?;
                anyhow::ensure!(body["queue"] == queue, "{tail}: {}", body["queue"]);
                anyhow::ensure!(body["counts"].is_object(), "{tail}: {body}");
                anyhow::ensure!(!items(&body).is_empty(), "{tail}: nothing listed");
                anyhow::ensure!(total(&body)? >= 1, "{tail}: {body}");
            }
        }
        // 重复项的分档照旧作用在列表上
        for types in ["same", "conflict"] {
            same(
                &f,
                &token,
                &format!("review?queue=duplicates&types={types}"),
            )
            .await?;
        }
        // 总览：整个响应体
        let summary = same(&f, &token, "review/summary").await?;
        anyhow::ensure!(summary["waiting"].is_object(), "summary: {summary}");
        // 决策台账：events 与 total
        let history = same(&f, &token, "review/history").await?;
        anyhow::ensure!(
            !events(&history).is_empty() && total(&history)? >= 2,
            "decisions: {history}"
        );
        same(&f, &token, "review/history?page=1&per=1").await?;
        // 写令牌读到的也是同一页：scope 不收窄读
        let (_, write) = f.token(f.editor, "writer", "write", None).await?;
        same(&f, &write, "review?queue=conflicts").await?;
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn an_empty_section_reads_as_an_empty_list_with_total_0_for_a_token_as_for_a_session(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let (_, token) = f.token(f.viewer, "reader", "read", None).await?;
        for queue in SECTIONS {
            let body = same(&f, &token, &format!("review?queue={queue}")).await?;
            anyhow::ensure!(
                body["items"] == json!([]) && body["total"] == 0,
                "{queue} on an empty base: {body}"
            );
        }
        let history = same(&f, &token, "review/history").await?;
        anyhow::ensure!(
            history["events"] == json!([]) && history["total"] == 0,
            "decisions on an empty base: {history}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_token_receives_each_sections_items_in_the_session_order() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        seed_every_section(&f).await?;
        let (_, token) = f.token(f.viewer, "reader", "read", None).await?;
        for queue in SECTIONS {
            let mut mine = Vec::new();
            let mut theirs = Vec::new();
            for offset in [0, 1] {
                let tail = format!("review?queue={queue}&limit=1&offset={offset}");
                mine.extend(items(&ok(&f, &token, &tail).await?));
                theirs.extend(items(&ok(&f, &f.editor_session, &tail).await?));
            }
            anyhow::ensure!(mine.len() == 2, "{queue}: two pages of one, got {mine:?}");
            anyhow::ensure!(
                identifiers(&mine) == identifiers(&theirs),
                "{queue}: order differs\n{:?}\n{:?}",
                identifiers(&mine),
                identifiers(&theirs)
            );
            // 两页是两条不同的：比的是顺序，不是同一条比两遍
            anyhow::ensure!(
                identifier(&mine[0]) != identifier(&mine[1]),
                "{queue}: both pages returned {}",
                identifier(&mine[0])
            );
            // 一页两条的顺序，就是两页一条接起来的顺序
            let both = items(&ok(&f, &token, &format!("review?queue={queue}&limit=2")).await?);
            anyhow::ensure!(
                identifiers(&both) == identifiers(&mine),
                "{queue}: limit=2 order differs from two pages of one"
            );
        }
        // 决策台账有自己的 page / per
        let mut mine = Vec::new();
        let mut theirs = Vec::new();
        for page in [0, 1] {
            let tail = format!("review/history?per=1&page={page}");
            mine.extend(events(&ok(&f, &token, &tail).await?));
            theirs.extend(events(&ok(&f, &f.editor_session, &tail).await?));
        }
        anyhow::ensure!(mine.len() == 2, "decisions: two pages of one, got {mine:?}");
        anyhow::ensure!(
            identifiers(&mine) == identifiers(&theirs)
                && identifier(&mine[0]) != identifier(&mine[1]),
            "decisions: order differs\n{:?}\n{:?}",
            identifiers(&mine),
            identifiers(&theirs)
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_token_page_size_is_clamped_and_an_offset_at_or_past_the_total_returns_no_items(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        seed_every_section(&f).await?;
        let (_, token) = f.token(f.viewer, "reader", "read", None).await?;
        for queue in SECTIONS {
            // 小于 1 的页大小当 1
            for limit in [0, -5] {
                let body = ok(&f, &token, &format!("review?queue={queue}&limit={limit}")).await?;
                anyhow::ensure!(
                    items(&body).len() == 1,
                    "{queue} limit={limit}: {} items",
                    items(&body).len()
                );
            }
            // 翻到 total 和翻过头：空页，total 不变，不是错误
            let all = total(&ok(&f, &token, &format!("review?queue={queue}")).await?)?;
            anyhow::ensure!(all >= 2, "{queue}: {all} seeded");
            for offset in [all, all + 1, all + 1000] {
                let body = ok(&f, &token, &format!("review?queue={queue}&offset={offset}")).await?;
                anyhow::ensure!(
                    body["items"] == json!([]) && body["total"] == all,
                    "{queue} at offset {offset} of {all}: {body}"
                );
            }
            // 负的 offset 当 0
            let negative = ok(&f, &token, &format!("review?queue={queue}&offset=-3")).await?;
            let first = ok(&f, &token, &format!("review?queue={queue}")).await?;
            anyhow::ensure!(negative == first, "{queue}: offset=-3 is served as 0");
        }

        // 大于 200 的页大小当 200：重复项一档种到两百条以上
        for i in 0..205 {
            f.pair(&format!("Clamp {i}"), &format!("Clamp {i}")).await?;
        }
        let all = total(&ok(&f, &token, "review?queue=duplicates").await?)?;
        anyhow::ensure!(all > 200, "more than 200 pairs wait, got {all}");
        for limit in [201_i64, 1_000_000] {
            let body = same(
                &f,
                &token,
                &format!("review?queue=duplicates&limit={limit}"),
            )
            .await?;
            anyhow::ensure!(
                items(&body).len() == 200 && body["total"] == all,
                "limit={limit}: {} items of {}",
                items(&body).len(),
                body["total"]
            );
        }
        // 刚好 200 不受影响，剩下的在下一页
        let rest = ok(&f, &token, "review?queue=duplicates&limit=200&offset=200").await?;
        anyhow::ensure!(
            items(&rest).len() as i64 == all - 200,
            "the rest after 200: {} of {all}",
            items(&rest).len()
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_token_asking_for_mappings_or_an_unknown_section_is_refused_with_unknown_queue(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        seed_every_section(&f).await?;
        let (_, read) = f.token(f.viewer, "reader", "read", None).await?;
        let (_, write) = f.token(f.editor, "writer", "write", None).await?;
        // 这个库里有一条等着的映射：拒绝不是因为那一档空
        let mappings = ok(&f, &f.editor_session, "review?queue=mappings").await?;
        anyhow::ensure!(
            mappings["queue"] == "mappings" && !items(&mappings).is_empty(),
            "the session editor still lists mappings: {mappings}"
        );
        // `overview` 与 `decisions` 不是列表档：在列表路由上对令牌和对会话是同一个拒绝
        for queue in [
            "mappings",
            "no-such-section",
            "overview",
            "decisions",
            "Pending",
            "",
        ] {
            let path = format!("/kbs/{kb}/review?queue={queue}");
            for token in [&read, &write] {
                let (status, body) = get(&f, token, &path).await?;
                anyhow::ensure!(
                    status == StatusCode::UNPROCESSABLE_ENTITY && body["code"] == "unknown_queue",
                    "`{queue}`: {status} {body}"
                );
                anyhow::ensure!(
                    body.get("items").is_none()
                        && body.get("total").is_none()
                        && body.get("counts").is_none(),
                    "`{queue}`: {body}"
                );
                anyhow::ensure!(
                    body["error"] == format!("no review queue named {queue}"),
                    "`{queue}`: {body}"
                );
            }
            if queue != "mappings" {
                let (status, session) = get(&f, &f.editor_session, &path).await?;
                let (_, mine) = get(&f, &read, &path).await?;
                anyhow::ensure!(
                    status == StatusCode::UNPROCESSABLE_ENTITY && session == mine,
                    "`{queue}`: the session gets the same refusal: {status} {session}"
                );
            }
        }
        // 够不着的库先答 404：不告诉它那里有哪些档
        let (_, excluded) = f
            .token(f.viewer, "excluded", "read", Some(&[f.other_kb]))
            .await?;
        let (status, _) = get(&f, &excluded, &format!("/kbs/{kb}/review?queue=mappings")).await?;
        anyhow::ensure!(status == StatusCode::NOT_FOUND, "excluded: {status}");
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_tokens_total_counts_the_listed_rows_and_counts_keep_their_values() -> anyhow::Result<()>
{
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        seed_every_section(&f).await?;
        let (_, token) = f.token(f.viewer, "reader", "read", None).await?;
        let session_counts = ok(&f, &f.editor_session, "review?queue=duplicates").await?["counts"].clone();
        let stored_counts = json!(utopia_store::review::counts(&f.pool, kb).await?);
        anyhow::ensure!(session_counts == stored_counts, "the session's counts");

        let mut listed = std::collections::HashMap::new();
        for queue in SECTIONS {
            let (rows, total) = token_pages(&f, &token, queue, 1).await?;
            anyhow::ensure!(
                rows.len() as i64 == total && total > 0,
                "{queue}: total {total}, pages returned {} rows",
                rows.len()
            );
            // 一页一条翻到底，没有哪一条出现两次
            let mut distinct = identifiers(&rows);
            distinct.sort();
            distinct.dedup();
            anyhow::ensure!(distinct.len() == rows.len(), "{queue}: a row was listed twice");
            // total 之后是空页
            let past = ok(&f, &token, &format!("review?queue={queue}&limit=1&offset={total}")).await?;
            anyhow::ensure!(items(&past).is_empty(), "{queue}: a row past total");
            // 徽标的键和值都不动
            anyhow::ensure!(past["counts"] == session_counts, "{queue}: counts differ");
            listed.insert(queue, rows);
        }

        // 低置信：total 数列出来的行，推出来的事实在内；徽标不数它
        let lowconf: Vec<Uuid> = listed["lowconf"]
            .iter()
            .filter_map(|i| i["id"].as_str().and_then(|s| s.parse().ok()))
            .collect();
        let derived: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM facts WHERE id = ANY($1) AND derived_by_rule IS NOT NULL",
        )
        .bind(&lowconf)
        .fetch_one(&f.pool)
        .await?;
        anyhow::ensure!(derived >= 1, "a derived fact is listed");
        anyhow::ensure!(
            session_counts["lowconf"] == lowconf.len() as i64 - derived,
            "counts.lowconf {} leaves out the {derived} derived of {} listed",
            session_counts["lowconf"],
            lowconf.len()
        );

        // Agent：total 是两种来源的行数，徽标还是开着的建议数
        let (governor, ledger, proposed): (i64, i64, i64) = sqlx::query_as(
            "SELECT (SELECT count(*) FROM agent_decisions WHERE kb_id = $1),
                    (SELECT count(*) FROM token_agent_actions WHERE kb_id = $1),
                    (SELECT count(*) FROM agent_decisions WHERE kb_id = $1 AND status = 'proposed')",
        )
        .bind(kb)
        .fetch_one(&f.pool)
        .await?;
        let agent = &listed["agent"];
        anyhow::ensure!(governor >= 1 && ledger == 1, "{governor} governor, {ledger} ledger");
        anyhow::ensure!(
            agent.len() as i64 == governor + ledger,
            "agent total {} = {governor} + {ledger}",
            agent.len()
        );
        anyhow::ensure!(
            agent.iter().filter(|r| r["source"] == "token").count() as i64 == ledger
                && agent.iter().filter(|r| r.get("source").is_none()).count() as i64 == governor,
            "rows of both sources: {agent:?}"
        );
        anyhow::ensure!(
            session_counts["agent"] == proposed && proposed != governor + ledger,
            "counts.agent stays the badge: {} vs {proposed}",
            session_counts["agent"]
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_token_paging_alignment_by_the_total_reaches_the_last_implication_rule(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        seed_every_section(&f).await?;
        let (_, token) = f.token(f.viewer, "reader", "read", None).await?;
        let rules: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM implication_rules WHERE kb_id = $1 AND status = 'proposed'",
        )
        .bind(kb)
        .fetch_all(&f.pool)
        .await?;
        anyhow::ensure!(
            rules.len() >= 2,
            "two proposed rules seeded, got {}",
            rules.len()
        );

        let first = ok(&f, &token, "review?queue=alignment").await?;
        let all = total(&first)?;
        // 徽标不数蕴含规则：按它翻页，正好差规则那几条
        anyhow::ensure!(
            first["counts"]["alignment"] == all - rules.len() as i64,
            "counts.alignment {} is lower than total {all} by {} rules",
            first["counts"]["alignment"],
            rules.len()
        );
        let mut seen = Vec::new();
        for offset in 0..all {
            let body = ok(
                &f,
                &token,
                &format!("review?queue=alignment&limit=1&offset={offset}"),
            )
            .await?;
            let page = items(&body);
            anyhow::ensure!(
                page.len() == 1,
                "offset {offset} of {all}: {} items",
                page.len()
            );
            anyhow::ensure!(
                body["total"] == all,
                "offset {offset}: total {}",
                body["total"]
            );
            seen.push(page[0].clone());
        }
        // 每一条等着的蕴含规则都翻到了
        for rule in &rules {
            anyhow::ensure!(
                seen.iter()
                    .any(|i| i["kind"] == "rule" && i["id"] == rule.to_string()),
                "rule {rule} was not reached"
            );
        }
        // 顺序里最后的那一条是规则，它在 offset = total − 1
        let whole = items(&ok(&f, &f.editor_session, "review?queue=alignment&limit=200").await?);
        let last = whole
            .last()
            .ok_or_else(|| anyhow::anyhow!("nothing listed"))?;
        anyhow::ensure!(
            last["kind"] == "rule",
            "a rule is last in the order: {last}"
        );
        anyhow::ensure!(
            seen.last() == Some(last),
            "offset total − 1 returns the last rule: {:?} vs {last}",
            seen.last()
        );
        anyhow::ensure!(seen == whole, "the pages are the session's list");
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_token_paging_alignment_with_equal_timestamps_visits_each_waiting_item_exactly_once(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        seed_every_section(&f).await?;
        let (_, token) = f.token(f.viewer, "reader", "read", None).await?;
        // 一轮对齐写下的行时间相同：把这个库里等着的条目全部拨到同一刻
        for table in ["phrase_bindings", "type_bindings", "implication_rules"] {
            // 表名是上面写死的三个
            sqlx::query(&format!(
                "UPDATE {table} SET decided_at = '2026-01-01T00:00:00Z' WHERE kb_id = $1"
            ))
            .bind(kb)
            .execute(&f.pool)
            .await?;
        }
        let waiting = identifiers(&items(
            &ok(&f, &f.editor_session, "review?queue=alignment&limit=200").await?,
        ));
        anyhow::ensure!(waiting.len() >= 5, "five waiting items seeded: {waiting:?}");
        let mut expected = waiting.clone();
        expected.sort();
        for limit in [1, 2] {
            let (rows, all) = token_pages(&f, &token, "alignment", limit).await?;
            let got = identifiers(&rows);
            anyhow::ensure!(
                got.len() as i64 == all && all == waiting.len() as i64,
                "limit={limit}: {} rows, total {all}",
                got.len()
            );
            anyhow::ensure!(
                got.iter().all(|id| !id.is_empty()),
                "limit={limit}: {got:?}"
            );
            let mut sorted = got.clone();
            sorted.sort();
            // 排过序与等着的那一组逐个相等：每一条正好一次，不多不少
            anyhow::ensure!(
                sorted == expected,
                "limit={limit}: each waiting item exactly once\n{got:?}\n{waiting:?}"
            );
            anyhow::ensure!(got == waiting, "limit={limit}: the session's order");
        }
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_token_whose_kb_ids_exclude_the_base_gets_404_on_every_review_read() -> anyhow::Result<()>
{
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        seed_every_section(&f).await?;
        let (contradiction, _, derived) = seeded_proof_targets(&f).await?;
        // 同一个人的两枚令牌：一枚够得着这个库，一枚只列了另一个库。再加一枚编辑者的写令牌
        let (_, reaches) = f.token(f.viewer, "reaches", "read", Some(&[kb])).await?;
        let (_, excluded) = f
            .token(f.viewer, "excluded", "read", Some(&[f.other_kb]))
            .await?;
        let (_, excluded_write) = f
            .token(f.editor, "excluded write", "write", Some(&[f.other_kb]))
            .await?;
        let mut tails: Vec<String> = SECTIONS
            .iter()
            .map(|q| format!("review?queue={q}"))
            .collect();
        tails.push("review/summary".into());
        tails.push("review/history".into());
        tails.push(format!("violations/{contradiction}/proof"));
        tails.push(format!("derived/{derived}/proof"));
        anyhow::ensure!(tails.len() == 15);
        let missing = Uuid::now_v7();
        for tail in &tails {
            // 对照：够得着的那一枚读得到
            ok(&f, &reaches, tail).await?;
            for token in [&excluded, &excluded_write] {
                let (status, body) = get(&f, token, &format!("/kbs/{kb}/{tail}")).await?;
                anyhow::ensure!(status == StatusCode::NOT_FOUND, "{tail}: {status} {body}");
                // 与一个不存在的库同一个回答
                let (gone_status, gone) = get(&f, token, &format!("/kbs/{missing}/{tail}")).await?;
                anyhow::ensure!(
                    gone_status == StatusCode::NOT_FOUND && gone == body,
                    "{tail}: {body} vs a missing base {gone_status} {gone}"
                );
                ensure_no_data(&body, tail)?;
                let text = body.to_string();
                for leaked in ["waiting", "decided", "health", &contradiction.to_string()] {
                    anyhow::ensure!(!text.contains(leaked), "{tail}: {leaked} in {body}");
                }
            }
        }
        // 这枚令牌够得着的那个库照常读：404 是 `kb_ids` 的事，不是令牌坏了
        let (status, other) = get(
            &f,
            &excluded,
            &format!("/kbs/{}/review?queue=pending", f.other_kb),
        )
        .await?;
        anyhow::ensure!(
            status == StatusCode::OK && other["total"] == 0,
            "the base it reaches: {status} {other}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_dead_token_gets_401_and_an_excluded_token_gets_404_on_the_api_and_403_on_the_mcp_endpoint(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        seed_every_section(&f).await?;
        let (contradiction, _, derived) = seeded_proof_targets(&f).await?;
        let pair = items(&ok(&f, &f.editor_session, "review?queue=duplicates").await?)[0]["id"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let tails = [
            "review?queue=pending".to_string(),
            "review?queue=agent".to_string(),
            "review/summary".to_string(),
            "review/history".to_string(),
            format!("violations/{contradiction}/proof"),
            format!("derived/{derived}/proof"),
        ];
        let mcp_calls = [
            ("tools/list", json!({})),
            (
                "tools/call",
                json!({ "name": "review_list", "arguments": { "section": "duplicates" } }),
            ),
            (
                "tools/call",
                json!({ "name": "review_list", "arguments": { "section": "pending" } }),
            ),
            (
                "tools/call",
                json!({ "name": "review_item", "arguments": { "id": pair } }),
            ),
        ];
        // 响应里不带任何条目、计数或证据：按键名查，也按种子里的名字查
        let ensure_bare = |body: &Value, what: &str| -> anyhow::Result<()> {
            ensure_no_data(body, what)?;
            anyhow::ensure!(body.get("result").is_none(), "{what}: {body}");
            let text = body.to_string();
            for key in DATA_KEYS.iter().copied().chain([
                "evidence",
                "top_facts",
                "waiting",
                "left",
                "right",
            ]) {
                anyhow::ensure!(
                    !text.contains(&format!("\"{key}\"")),
                    "{what}: `{key}` in {body}"
                );
            }
            anyhow::ensure!(!text.contains(&pair), "{what}: an item id in {body}");
            Ok(())
        };

        // 撤销了的令牌：先确认它活着的时候读得到，撤销之后处处 401
        let (dead_id, dead) = f.token(f.viewer, "revoked", "read", None).await?;
        for tail in &tails {
            ok(&f, &dead, tail).await?;
        }
        let (status, alive) = rpc(&f, &dead, mcp_calls[1].0, mcp_calls[1].1.clone()).await?;
        anyhow::ensure!(
            status == StatusCode::OK && alive["result"]["isError"] == false,
            "before the revoke: {status} {alive}"
        );
        utopia_store::tokens::revoke(&f.pool, f.viewer, dead_id).await?;
        for tail in &tails {
            let (status, body) = get(&f, &dead, &format!("/kbs/{kb}/{tail}")).await?;
            anyhow::ensure!(
                status == StatusCode::UNAUTHORIZED,
                "dead {tail}: {status} {body}"
            );
            ensure_bare(&body, tail)?;
        }
        for (method, params) in &mcp_calls {
            let (status, body) = rpc(&f, &dead, method, params.clone()).await?;
            anyhow::ensure!(
                status == StatusCode::UNAUTHORIZED,
                "dead {method} {params}: {status} {body}"
            );
            ensure_bare(&body, method)?;
        }
        // 认不出的令牌同样是 401
        let (status, _) = get(
            &f,
            "utp_pat_not-a-token",
            &format!("/kbs/{kb}/review/history"),
        )
        .await?;
        anyhow::ensure!(
            status == StatusCode::UNAUTHORIZED,
            "unknown token: {status}"
        );

        // `kb_ids` 排除了这个库的令牌：令牌 API 是 404，MCP 端点照旧是工具跑之前的 403
        let (_, excluded) = f
            .token(f.viewer, "excluded", "read", Some(&[f.other_kb]))
            .await?;
        for tail in &tails {
            let (status, body) = get(&f, &excluded, &format!("/kbs/{kb}/{tail}")).await?;
            anyhow::ensure!(
                status == StatusCode::NOT_FOUND,
                "excluded {tail}: {status} {body}"
            );
            ensure_bare(&body, tail)?;
        }
        for (method, params) in &mcp_calls {
            let (status, body) = rpc(&f, &excluded, method, params.clone()).await?;
            anyhow::ensure!(
                status == StatusCode::FORBIDDEN,
                "excluded {method} {params}: {status} {body}"
            );
            ensure_bare(&body, method)?;
        }
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn agent_history_read_by_a_token_names_the_owner_and_the_token_of_a_keep_beside_governor_rows(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        seed_every_section(&f).await?;
        let (_, token) = f.token(f.viewer, "reader", "read", None).await?;
        // 令牌的响应体就是会话编辑者的
        let body = same(&f, &token, "review?queue=agent&limit=200").await?;
        let rows = items(&body);
        let from_token: Vec<&Value> = rows.iter().filter(|r| r["source"] == "token").collect();
        anyhow::ensure!(from_token.len() == 1, "exactly one token row: {rows:?}");
        let keep = from_token[0];
        // 种子里那一对是 `f.editor` 用名叫 seed-agent 的写令牌分开的
        anyhow::ensure!(
            keep["operation"] == "review.keep"
                && keep["owner_name"] == "Editor"
                && keep["owner_id"] == f.editor.to_string()
                && keep["token_name"] == "seed-agent",
            "the keep names its owner and its token: {keep}"
        );
        // 治理 agent 的行没有 `source` 键，也没有令牌的那几个字段：不会被读成令牌的行
        let governor: Vec<&Value> = rows.iter().filter(|r| r.get("source").is_none()).collect();
        anyhow::ensure!(!governor.is_empty(), "governor rows beside it: {rows:?}");
        anyhow::ensure!(
            governor.len() + 1 == rows.len(),
            "every row is a governor row or the token row: {rows:?}"
        );
        for row in &governor {
            anyhow::ensure!(
                row.get("token_name").is_none() && row.get("owner_name").is_none(),
                "a governor row carries no token attribution: {row}"
            );
        }
        anyhow::ensure!(body["total"] == rows.len() as i64, "{}", body["total"]);
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn agent_history_without_a_token_action_reads_governor_rows_only_and_a_ledger_row_keeps_its_stored_token_name(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        let first = f.pair("Zhang Wei", "Zhang Wei").await?;
        let second = f.pair("Li Na", "Li Na").await?;
        let third = f.pair("Wang Fang", "Wang Fang").await?;
        governor_row(&f, first, "proposed").await?;
        governor_row(&f, second, "applied").await?;
        let (_, token) = f.token(f.viewer, "reader", "read", None).await?;

        // 没有令牌动作的库：只有治理 agent 的行，与会话列表逐字相等
        let body = same(&f, &token, "review?queue=agent").await?;
        let rows = items(&body);
        anyhow::ensure!(
            rows.len() == 2 && body["total"] == 2,
            "two governor rows: {body}"
        );
        anyhow::ensure!(
            rows.iter().all(|r| r.get("source").is_none()),
            "no token row without a token action: {rows:?}"
        );
        anyhow::ensure!(
            utopia_store::token_agent_actions::count(&f.pool, kb).await? == 0,
            "reading writes no ledger row"
        );

        // 一枚写令牌分开一对，然后改名：行上的名字还是分开那一刻的
        let (curator_id, curator) = f.token(f.editor, "curator", "write", None).await?;
        let (status, kept) = f
            .call(
                &curator,
                "POST",
                &format!("/kbs/{kb}/review/{third}"),
                Some(json!({ "action": "keep", "rationale": "different people" })),
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "keep: {status} {kept}");
        let renamed = sqlx::query("UPDATE personal_tokens SET name = 'renamed' WHERE id = $1")
            .bind(curator_id)
            .execute(&f.pool)
            .await?
            .rows_affected();
        anyhow::ensure!(renamed == 1, "the token row was not renamed");
        let body = same(&f, &token, "review?queue=agent").await?;
        let rows = items(&body);
        anyhow::ensure!(rows.len() == 3 && body["total"] == 3, "{body}");
        let from_token: Vec<&Value> = rows.iter().filter(|r| r["source"] == "token").collect();
        anyhow::ensure!(from_token.len() == 1, "one ledger row: {rows:?}");
        anyhow::ensure!(
            from_token[0]["token_name"] == "curator"
                && from_token[0]["token_id"] == curator_id.to_string()
                && from_token[0]["owner_name"] == "Editor",
            "the stored token name survives the rename: {}",
            from_token[0]
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_read_token_of_a_viewer_reads_both_proof_routes_as_a_session_does() -> anyhow::Result<()>
{
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        seed_every_section(&f).await?;
        let (contradiction, other, derived) = seeded_proof_targets(&f).await?;
        let (_, token) = f.token(f.viewer, "reader", "read", None).await?;

        // 违规的证明：`derived_contradiction` 展得开，别的种类是 null，不是错误
        let proof = same(&f, &token, &format!("violations/{contradiction}/proof")).await?;
        anyhow::ensure!(
            proof["steps"].as_array().is_some_and(|s| !s.is_empty()),
            "steps of the derived contradiction: {proof}"
        );
        let none = same(&f, &token, &format!("violations/{other}/proof")).await?;
        anyhow::ensure!(none == json!({ "steps": null }), "another kind: {none}");
        let unknown = same(&f, &token, &format!("violations/{}/proof", Uuid::now_v7())).await?;
        anyhow::ensure!(
            unknown == json!({ "steps": null }),
            "an unknown id: {unknown}"
        );

        // 派生事实的证明：种子里那一条，和一个编出来的 id
        let proof = same(&f, &token, &format!("derived/{derived}/proof")).await?;
        anyhow::ensure!(
            proof.as_object().is_some_and(|o| o.len() == 1) && proof.get("proof").is_some(),
            "the body is `proof` and nothing else: {proof}"
        );
        let unknown = same(&f, &token, &format!("derived/{}/proof", Uuid::now_v7())).await?;
        anyhow::ensure!(
            unknown == json!({ "proof": null }),
            "a made-up id: {unknown}"
        );

        // 另一个库的 id 在这个库的路由上展不开：证明是按库取的
        let (status, crossed) = get(
            &f,
            &token,
            &format!("/kbs/{}/violations/{contradiction}/proof", f.other_kb),
        )
        .await?;
        anyhow::ensure!(
            status == StatusCode::OK && crossed == json!({ "steps": null }),
            "another base's violation: {status} {crossed}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_token_cannot_nod_and_no_review_write_but_keep_answers_a_token() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        seed_every_section(&f).await?;
        // 写得最多的那一种令牌：编辑者的写令牌，不限库
        let (_, token) = f.token(f.editor, "writer", "write", None).await?;
        let listed = |queue: &'static str| {
            let (f, token) = (&f, &token);
            async move {
                Ok::<_, anyhow::Error>(identifiers(&items(
                    &ok(f, token, &format!("review?queue={queue}&limit=200")).await?,
                )))
            }
        };
        let pending = listed("pending").await?;
        let duplicates = listed("duplicates").await?;
        let agent = listed("agent").await?;
        anyhow::ensure!(pending.len() >= 2 && duplicates.len() >= 2 && !agent.is_empty());
        let pending_rows: i64 =
            sqlx::query_scalar("SELECT count(*) FROM pending_facts WHERE kb_id = $1")
                .bind(kb)
                .fetch_one(&f.pool)
                .await?;
        let facts: i64 = sqlx::query_scalar("SELECT count(*) FROM facts WHERE kb_id = $1")
            .bind(kb)
            .fetch_one(&f.pool)
            .await?;

        // 读得到「等你点头」，点不了头：确认和否决都是 401
        for action in ["confirm", "reject"] {
            let (status, body) = f
                .call(
                    &token,
                    "POST",
                    &format!("/kbs/{kb}/review/pending/{}", pending[0]),
                    Some(json!({ "action": action })),
                )
                .await?;
            anyhow::ensure!(
                status == StatusCode::UNAUTHORIZED,
                "nod {action}: {status} {body}"
            );
        }
        // 批量、回答治理 agent 的建议：同样没开给令牌
        let writes = [
            (
                format!("/kbs/{kb}/review/batch"),
                json!({ "ids": [duplicates[0], duplicates[1]], "action": "keep" }),
            ),
            (
                format!("/kbs/{kb}/review/agent/{}", agent[0]),
                json!({ "action": "keep" }),
            ),
            // 别的档的裁决路由也一条没开
            (
                format!("/kbs/{kb}/review/violations/{}", Uuid::now_v7()),
                json!({ "action": "dismiss" }),
            ),
            (
                format!("/kbs/{kb}/review/defects/{}", Uuid::now_v7()),
                json!({ "action": "dismiss" }),
            ),
            (
                format!("/kbs/{kb}/review/errata/{}", Uuid::now_v7()),
                json!({ "action": "approve" }),
            ),
            (
                format!("/kbs/{kb}/review/alignment/rules/{}", Uuid::now_v7()),
                json!({ "action": "approve" }),
            ),
            (
                format!("/kbs/{kb}/review/mappings/{}", Uuid::now_v7()),
                json!({ "status": "confirmed" }),
            ),
            (
                format!("/kbs/{kb}/conflicts/{}", Uuid::now_v7()),
                json!({ "action": "keep_both" }),
            ),
            (
                format!("/kbs/{kb}/facts/{}/confirm", Uuid::now_v7()),
                json!({}),
            ),
            (
                format!("/kbs/{kb}/facts/{}/reject", Uuid::now_v7()),
                json!({}),
            ),
            (
                format!("/kbs/{kb}/merges/{}/revert", Uuid::now_v7()),
                json!({}),
            ),
        ];
        for (path, body) in &writes {
            let (status, answer) = f.call(&token, "POST", path, Some(body.clone())).await?;
            anyhow::ensure!(
                status == StatusCode::UNAUTHORIZED,
                "{path}: {status} {answer}"
            );
        }
        // 按句取待确认项的那条读路由不在这一刀里：仍只给会话
        let (status, _) = get(
            &f,
            &token,
            &format!("/kbs/{kb}/review/pending?chunk_id={}", Uuid::now_v7()),
        )
        .await?;
        anyhow::ensure!(
            status == StatusCode::UNAUTHORIZED,
            "pending_for_chunk: {status}"
        );

        // 什么都没动：三档列出来的还是那些，表里的行数没变
        anyhow::ensure!(listed("pending").await? == pending, "pending changed");
        anyhow::ensure!(
            listed("duplicates").await? == duplicates,
            "duplicates changed"
        );
        anyhow::ensure!(listed("agent").await? == agent, "agent history changed");
        let pending_after: i64 =
            sqlx::query_scalar("SELECT count(*) FROM pending_facts WHERE kb_id = $1")
                .bind(kb)
                .fetch_one(&f.pool)
                .await?;
        let facts_after: i64 = sqlx::query_scalar("SELECT count(*) FROM facts WHERE kb_id = $1")
            .bind(kb)
            .fetch_one(&f.pool)
            .await?;
        anyhow::ensure!(
            pending_after == pending_rows && facts_after == facts,
            "pending_facts {pending_rows} -> {pending_after}, facts {facts} -> {facts_after}"
        );
        // 会话编辑者照旧点得了头：401 是因为来的是令牌，不是这一条坏了
        let (status, body) = f
            .call(
                &f.editor_session,
                "POST",
                &format!("/kbs/{kb}/review/pending/{}", pending[0]),
                Some(json!({ "action": "reject" })),
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "a person's nod: {status} {body}");
        anyhow::ensure!(
            !listed("pending").await?.contains(&pending[0]),
            "the person's decision takes the item off the queue"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}
