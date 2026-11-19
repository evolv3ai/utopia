//! Review 列表响应里的 `total`、对齐一档的第二排序键，以及十三档都有东西的种子（0076）。
//!
//! 要钉住的行为：
//!
//! - **会话的列表响应只多了 `total` 一个键**（SC7 / E14）：`counts`、`queue`、`items`
//!   与这一刀之前同一组 store 调用给出的逐字相等
//! - **`total` 是这一档的列表翻到底的行数**（E5）：一页一条翻到空页，拿到的正好是 `total`
//!   条；对齐、低置信、Agent 三档上它与 `counts` 里同名的徽标不等，徽标的值不动
//! - **对齐一档时间相同的条目各出现一次**（E11）：第二键是条目自己的标识
//! - **空档是空列表和 0，翻过头是空列表和真实的 `total`**（E1 / E3）
//! - **令牌读重复项拿到同一个 `total`**，别的档这一刀还是 `unknown_queue`
//!
//! [`seed_every_section`] 也给下一波的令牌读测试用：它跑的就是
//! `scripts/seed-review-sections.sql`，终审走查用 psql 跑的是同一个文件。
//!
//! 夹具在 `review_session_unchanged_tests.rs`。没有 `UTOPIA_DATABASE_URL` 时跳过。
use axum::http::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;

use super::review_session_unchanged_tests::Fx;

/// 种子脚本本身：编译时读进来，夹具跑的与仓库里那个文件是同一份字节
const SEED_SQL: &str = include_str!("../../../../scripts/seed-review-sections.sql");

/// 列表路由认的十二个档名。十一个是 Review 的分档，`mappings` 只开给会话
const LIST_QUEUES: [&str; 12] = [
    "pending",
    "duplicates",
    "conflicts",
    "unconfirmed",
    "lowconf",
    "mappings",
    "alignment",
    "errata",
    "violations",
    "defects",
    "merges",
    "agent",
];

/// 这一刀之前 `counts.alignment` 用的短语判据，照 `904b2a2` 的
/// `alignment_queue::WAITING_PHRASE` 抄下来：测试要对着「之前的数」比，不能引用会跟着改的常量
const WAITING_PHRASE_BEFORE: &str = "(b.status = 'undecided'
       OR (b.status = 'bound' AND b.marks IS NULL
           AND (b.decided_by = 'person' OR b.marks_asked_at IS NOT NULL)
           AND EXISTS (SELECT 1 FROM relation_types rt
                        WHERE rt.id = b.relation_type_id AND rt.temporal = 'state')))";

/// 把库 `f.kb` 种成十三档都有东西的样子。
///
/// 先在一条连接上 `SET seed.kb`，再整个跑 `scripts/seed-review-sections.sql`（全是直接插入，
/// 不排任务、不调模型）；SQL 做不出来的只有一样——Agent 一档里令牌的那一行要一枚真的
/// 令牌——所以最后用 `f.editor` 的写令牌经 `keep_duplicate_pair` 把留给它的那一对分开。
/// `f.other_kb` 不碰
pub(super) async fn seed_every_section(f: &Fx) -> anyhow::Result<()> {
    let mut conn = f.pool.acquire().await?;
    // 拼进去的只有夹具自己生成的 UUID
    sqlx::raw_sql(&format!("SET seed.kb = '{}'", f.kb))
        .execute(&mut *conn)
        .await?;
    // 多条语句一次发过去是一个隐式事务：中途出错整个不落，连接也回到空闲
    let seeded = sqlx::raw_sql(SEED_SQL).execute(&mut *conn).await;
    // 连接要还给池：不把这个库的 id 留在上面
    sqlx::raw_sql("RESET seed.kb").execute(&mut *conn).await?;
    seeded?;
    drop(conn);

    let (token_id, _) = f.token(f.editor, "seed-agent", "write", None).await?;
    let review: Uuid = sqlx::query_scalar(
        "SELECT id FROM resolution_reviews
          WHERE kb_id = $1 AND status = 'pending' AND reason = 'seed: token keep'",
    )
    .bind(f.kb)
    .fetch_one(&f.pool)
    .await?;
    utopia_store::token_agent_actions::keep_duplicate_pair(
        &f.pool,
        f.kb,
        review,
        f.editor,
        token_id,
        Some("seed: different people"),
    )
    .await?;
    Ok(())
}

/// 列表路由的一次调用。`query` 是 `queue=...` 起头的查询串
async fn get_list(
    f: &Fx,
    bearer: &str,
    kb: Uuid,
    query: &str,
) -> anyhow::Result<(StatusCode, Value)> {
    f.call(bearer, "GET", &format!("/kbs/{kb}/review?{query}"), None)
        .await
}

/// 会话编辑者读 `f.kb` 的一页，必须是 200
async fn page(f: &Fx, query: &str) -> anyhow::Result<Value> {
    let (status, body) = get_list(f, &f.editor_session, f.kb, query).await?;
    anyhow::ensure!(status == StatusCode::OK, "{query}: {status} {body}");
    Ok(body)
}

fn items(body: &Value) -> Vec<Value> {
    body["items"].as_array().cloned().unwrap_or_default()
}

fn total(body: &Value) -> anyhow::Result<i64> {
    body["total"]
        .as_i64()
        .ok_or_else(|| anyhow::anyhow!("total is not an integer: {body}"))
}

/// 对象的键，排好序
fn keys(v: &Value) -> Vec<String> {
    let mut k: Vec<String> = v
        .as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    k.sort();
    k
}

/// 一页一条从头翻到空页：回每一页的条目，并确认每一页报的 `total` 都相同
async fn page_through(f: &Fx, query: &str, limit: i64) -> anyhow::Result<(Vec<Value>, i64)> {
    let first = page(f, &format!("{query}&limit={limit}&offset=0")).await?;
    let expected = total(&first)?;
    let mut rows = Vec::new();
    let mut offset = 0;
    loop {
        let body = page(f, &format!("{query}&limit={limit}&offset={offset}")).await?;
        anyhow::ensure!(
            total(&body)? == expected,
            "{query}: total changed from {expected} at offset {offset}: {body}"
        );
        let got = items(&body);
        if got.is_empty() {
            break;
        }
        anyhow::ensure!(
            got.len() as i64 <= limit,
            "{query}: a page of {limit} returned {} rows",
            got.len()
        );
        rows.extend(got);
        offset += limit;
        // 翻不到头就是列表不认 offset：停下来报错，不要转到超时
        anyhow::ensure!(offset <= 500, "{query}: no empty page by offset {offset}");
    }
    Ok((rows, expected))
}

/// 这一刀之前会话列表路由对每一档做的那次 store 调用（`904b2a2` 的 `match`），原样抄在这里
/// 当对照：响应里的 `items` 得与它逐字相等
async fn items_before(
    f: &Fx,
    queue: &str,
    types: Option<&str>,
    limit: i64,
    offset: i64,
) -> anyhow::Result<Value> {
    let (pool, kb) = (&f.pool, f.kb);
    Ok(match queue {
        "pending" => json!(utopia_store::pending::list(pool, kb, limit, offset).await?),
        "duplicates" => json!(
            utopia_store::resolution::list_reviews(
                pool,
                kb,
                utopia_store::resolution::TypeFilter::parse(types)?,
                limit,
                offset,
            )
            .await?
        ),
        "conflicts" => {
            json!(utopia_store::temporal::list_conflicts(pool, kb, limit, offset).await?)
        }
        "unconfirmed" => json!(utopia_store::graph::stale_facts(pool, kb, limit, offset).await?),
        "lowconf" => json!(
            utopia_store::graph::low_confidence_facts(
                pool,
                kb,
                utopia_store::review::LOW_CONFIDENCE_BELOW,
                limit,
                offset,
            )
            .await?
        ),
        "mappings" => json!(utopia_store::mappings::proposed(pool, kb, limit, offset).await?),
        "alignment" => json!(utopia_store::alignment_queue::list(pool, kb, limit, offset).await?),
        "errata" => json!(utopia_store::errata::held(pool, kb, limit, offset).await?),
        "violations" => {
            json!(utopia_store::reasoning::open_violations(pool, kb, limit, offset).await?)
        }
        "defects" => json!(utopia_store::reasoning::open_defects(pool, kb, limit, offset).await?),
        "merges" => json!(utopia_store::resolution::list_merges(pool, kb, limit, offset).await?),
        "agent" => {
            json!(utopia_store::token_agent_actions::agent_history(pool, kb, limit, offset).await?)
        }
        other => anyhow::bail!("not a list queue: {other}"),
    })
}

/// 对齐条目的标识：短语与规则是 `id`，类别词是 `kind_word`
fn alignment_identifier(item: &Value) -> String {
    item["id"]
        .as_str()
        .or_else(|| item["kind_word"].as_str())
        .unwrap_or_default()
        .to_string()
}

#[tokio::test]
async fn a_session_list_response_differs_from_before_only_by_the_added_total_key(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        seed_every_section(&f).await?;
        let counts = json!(utopia_store::review::counts(&f.pool, kb).await?);
        for queue in LIST_QUEUES {
            // 缺省的一页（10 条，从头），和一页指定大小、跳过一条的
            for (query, limit, offset) in [
                (format!("queue={queue}"), 10, 0),
                (format!("queue={queue}&limit=2&offset=1"), 2, 1),
            ] {
                let body = page(&f, &query).await?;
                anyhow::ensure!(
                    keys(&body) == ["counts", "items", "queue", "total"],
                    "{query}: keys {:?}",
                    keys(&body)
                );
                anyhow::ensure!(body["queue"] == queue, "{query}: {}", body["queue"]);
                anyhow::ensure!(
                    body["counts"] == counts,
                    "{query}: counts differ\n{}\n{counts}",
                    body["counts"]
                );
                let before = items_before(&f, queue, None, limit, offset).await?;
                anyhow::ensure!(
                    body["items"] == before,
                    "{query}: items differ\n{}\n{before}",
                    body["items"]
                );
                anyhow::ensure!(
                    body["total"].is_i64() || body["total"].is_u64(),
                    "{query}: total is not an integer: {}",
                    body["total"]
                );
            }
        }
        // 重复项的分档照旧作用在列表上
        for types in ["any", "same", "conflict"] {
            let query = format!("queue=duplicates&types={types}");
            let body = page(&f, &query).await?;
            let before = items_before(&f, "duplicates", Some(types), 10, 0).await?;
            anyhow::ensure!(body["items"] == before, "{query}: items differ");
            anyhow::ensure!(body["counts"] == counts, "{query}: counts differ");
        }
        // 缺省档还是 duplicates；clamp 照旧：0 当 1，1000 当 200，负的 offset 当 0
        let (status, default) = get_list(&f, &f.editor_session, kb, "").await?;
        anyhow::ensure!(status == StatusCode::OK, "default: {status} {default}");
        anyhow::ensure!(default == page(&f, "queue=duplicates").await?, "{default}");
        let clamped = page(&f, "queue=lowconf&limit=0&offset=-3").await?;
        anyhow::ensure!(
            clamped["items"] == items_before(&f, "lowconf", None, 1, 0).await?,
            "limit=0 is served as 1: {clamped}"
        );
        let wide = page(&f, "queue=lowconf&limit=1000").await?;
        anyhow::ensure!(
            wide["items"] == items_before(&f, "lowconf", None, 200, 0).await?,
            "limit=1000 is served as 200"
        );
        // 认不出的档名还是同一个带 code 的错误
        let (status, unknown) = get_list(&f, &f.editor_session, kb, "queue=nope").await?;
        anyhow::ensure!(
            status == StatusCode::UNPROCESSABLE_ENTITY
                && unknown["code"] == "unknown_queue"
                && unknown.get("total").is_none(),
            "{status} {unknown}"
        );

        // 总览与决策台账的形状不动
        let (status, summary) = f
            .call(
                &f.editor_session,
                "GET",
                &format!("/kbs/{kb}/review/summary"),
                None,
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "summary: {status} {summary}");
        anyhow::ensure!(
            keys(&summary) == ["agent", "decided", "health", "waiting"],
            "summary body: {summary}"
        );
        let (status, history) = f
            .call(
                &f.editor_session,
                "GET",
                &format!("/kbs/{kb}/review/history"),
                None,
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "history: {status} {history}");
        anyhow::ensure!(
            keys(&history) == ["events", "total"],
            "history body: {history}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn the_total_of_every_list_queue_is_the_number_of_rows_its_pages_return() -> anyhow::Result<()>
{
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        seed_every_section(&f).await?;
        let mut queries: Vec<String> = LIST_QUEUES.iter().map(|q| format!("queue={q}")).collect();
        queries.push("queue=duplicates&types=same".into());
        queries.push("queue=duplicates&types=conflict".into());
        for query in &queries {
            let (rows, total) = page_through(&f, query, 1).await?;
            anyhow::ensure!(
                rows.len() as i64 == total,
                "{query}: total {total}, pages returned {} rows",
                rows.len()
            );
            // 种过的库每一档都有东西：数的不是一串零
            anyhow::ensure!(total > 0, "{query}: nothing listed on the seeded base");
        }

        // 列表内连接丢掉的行不进 total（INTENT Risks）：一对冲突的旧事实是没有谓词的开放
        // 陈述，列表连不上 `relation_types`，不列它；`fact_conflicts` 里却多了一行
        let (_, before) = page_through(&f, "queue=conflicts", 1).await?;
        let counted_before = page(&f, "queue=conflicts").await?["counts"]["conflicts"]
            .as_i64()
            .unwrap_or(-1);
        let subject = f.entity(f.person_type, "Totals Join Probe").await?;
        let (statement, typed, conflict) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
        sqlx::query(
            "INSERT INTO facts (id, kb_id, subject_id, object_value, layer, phrase)
             VALUES ($1, $2, $3, '{\"value\": \"x\"}', 'open', 'was seen at'),
                    ($4, $2, $3, '{\"value\": \"y\"}', 'open', 'was seen at')",
        )
        .bind(statement)
        .bind(kb)
        .bind(subject)
        .bind(typed)
        .execute(&f.pool)
        .await?;
        sqlx::query(
            "INSERT INTO fact_conflicts (id, kb_id, old_fact_id, new_fact_id, reason)
             VALUES ($1, $2, $3, $4, 'no_time')",
        )
        .bind(conflict)
        .bind(kb)
        .bind(statement)
        .bind(typed)
        .execute(&f.pool)
        .await?;
        let (rows, after) = page_through(&f, "queue=conflicts", 1).await?;
        anyhow::ensure!(
            after == before && rows.len() as i64 == after,
            "a conflict the list's join drops is not in total: {before} -> {after}, {} rows",
            rows.len()
        );
        let counted_after = page(&f, "queue=conflicts").await?["counts"]["conflicts"]
            .as_i64()
            .unwrap_or(-1);
        anyhow::ensure!(
            counted_after == counted_before + 1 && counted_after == after + 1,
            "counts.conflicts keeps its own query: {counted_before} -> {counted_after}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn the_alignment_low_confidence_and_agent_totals_follow_the_list_while_counts_keep_their_values(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        seed_every_section(&f).await?;

        // 这一刀之前计数查询里的三段，逐字抄自 `904b2a2` 的 `review::counts`
        let alignment_before: i64 = sqlx::query_scalar(&format!(
            "SELECT count(*) FROM (SELECT 1 FROM phrase_bindings b WHERE b.kb_id = $1 AND {WAITING_PHRASE_BEFORE}
                                   UNION ALL SELECT 1 FROM type_bindings WHERE kb_id = $1 AND status = 'undecided') a"
        ))
        .bind(kb)
        .fetch_one(&f.pool)
        .await?;
        let lowconf_before: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM facts
              WHERE kb_id = $1 AND invalidated_at IS NULL
                AND confidence < $2 AND derived_by_rule IS NULL",
        )
        .bind(kb)
        .bind(utopia_store::review::LOW_CONFIDENCE_BELOW)
        .fetch_one(&f.pool)
        .await?;
        let agent_before: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM agent_decisions WHERE kb_id = $1 AND status = 'proposed'",
        )
        .bind(kb)
        .fetch_one(&f.pool)
        .await?;
        let agent_rows_before: i64 = sqlx::query_scalar(
            "SELECT (SELECT count(*) FROM agent_decisions WHERE kb_id = $1)
                  + (SELECT count(*) FROM token_agent_actions WHERE kb_id = $1)",
        )
        .bind(kb)
        .fetch_one(&f.pool)
        .await?;
        let proposed_rules: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM implication_rules WHERE kb_id = $1 AND status = 'proposed'",
        )
        .bind(kb)
        .fetch_one(&f.pool)
        .await?;

        let alignment = page(&f, "queue=alignment&limit=200").await?;
        let lowconf = page(&f, "queue=lowconf&limit=200").await?;
        let agent = page(&f, "queue=agent&limit=200").await?;
        let counts = &alignment["counts"];

        // 徽标的值不动
        anyhow::ensure!(
            counts["alignment"] == alignment_before && alignment_before == 3,
            "counts.alignment: {} vs {alignment_before}",
            counts["alignment"]
        );
        anyhow::ensure!(
            counts["lowconf"] == lowconf_before && lowconf_before == 2,
            "counts.lowconf: {} vs {lowconf_before}",
            counts["lowconf"]
        );
        anyhow::ensure!(
            counts["agent"] == agent_before && agent_before == 1,
            "counts.agent: {} vs {agent_before}",
            counts["agent"]
        );
        anyhow::ensure!(
            counts["agent_rows"] == agent_rows_before && agent_rows_before == 3,
            "counts.agent_rows: {} vs {agent_rows_before}",
            counts["agent_rows"]
        );
        anyhow::ensure!(lowconf["counts"] == *counts && agent["counts"] == *counts);

        // 对齐：徽标不数蕴含规则，total 数
        anyhow::ensure!(proposed_rules == 2, "two proposed rules, got {proposed_rules}");
        anyhow::ensure!(
            total(&alignment)? == alignment_before + proposed_rules,
            "alignment total {} = {alignment_before} + {proposed_rules}",
            alignment["total"]
        );
        anyhow::ensure!(items(&alignment).len() as i64 == total(&alignment)?);

        // 低置信：徽标不数推出来的事实，列表列它们，total 跟列表
        let listed: Vec<Uuid> = items(&lowconf)
            .iter()
            .filter_map(|i| i["id"].as_str().and_then(|s| s.parse().ok()))
            .collect();
        let listed_derived: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM facts WHERE id = ANY($1) AND derived_by_rule IS NOT NULL",
        )
        .bind(&listed)
        .fetch_one(&f.pool)
        .await?;
        anyhow::ensure!(listed_derived == 1, "one listed derived fact, got {listed_derived}");
        anyhow::ensure!(
            total(&lowconf)? == lowconf_before + listed_derived,
            "lowconf total {} = {lowconf_before} + {listed_derived}",
            lowconf["total"]
        );
        anyhow::ensure!(listed.len() as i64 == total(&lowconf)?);

        // Agent：徽标是开着的建议数，total 是两种来源的行数
        anyhow::ensure!(
            total(&agent)? == agent_rows_before,
            "agent total {} = counts.agent_rows {agent_rows_before}",
            agent["total"]
        );
        anyhow::ensure!(items(&agent).len() as i64 == total(&agent)?);
        anyhow::ensure!(total(&agent)? != agent_before, "the badge is not the row count");
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn alignment_items_with_one_timestamp_are_each_listed_once_across_pages() -> anyhow::Result<()>
{
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        seed_every_section(&f).await?;
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

        let (one_by_one, total) = page_through(&f, "queue=alignment", 1).await?;
        let (two_by_two, _) = page_through(&f, "queue=alignment", 2).await?;
        let (again, _) = page_through(&f, "queue=alignment", 1).await?;
        let ids = |rows: &[Value]| rows.iter().map(alignment_identifier).collect::<Vec<_>>();
        let first = ids(&one_by_one);
        anyhow::ensure!(total == 5, "five waiting alignment items, got {total}");
        anyhow::ensure!(first.len() as i64 == total, "{first:?}");
        anyhow::ensure!(first.iter().all(|id| !id.is_empty()), "{first:?}");
        let mut distinct = first.clone();
        distinct.sort();
        distinct.dedup();
        anyhow::ensure!(
            distinct.len() == first.len(),
            "an item was listed twice: {first:?}"
        );
        anyhow::ensure!(
            ids(&two_by_two) == first,
            "limit=2 order differs: {:?} vs {first:?}",
            ids(&two_by_two)
        );
        anyhow::ensure!(ids(&again) == first, "a repeat differs: {:?}", ids(&again));
        // 时间相同，先后就是标识的先后
        anyhow::ensure!(
            first.windows(2).all(|w| w[0] < w[1]),
            "ties are broken by identifier, ascending: {first:?}"
        );
        // 三种条目都在：种类、字段照旧
        let kinds: std::collections::BTreeSet<String> = one_by_one
            .iter()
            .map(|i| i["kind"].as_str().unwrap_or_default().to_string())
            .collect();
        anyhow::ensure!(
            kinds.iter().map(String::as_str).collect::<Vec<_>>() == ["kind_word", "phrase", "rule"],
            "{kinds:?}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn an_empty_queue_has_total_0_and_an_offset_past_the_end_returns_no_items_with_the_true_total(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        // 什么都没种的库：每一档 200、空列表、0
        for queue in LIST_QUEUES {
            let body = page(&f, &format!("queue={queue}")).await?;
            anyhow::ensure!(
                body["items"] == json!([]) && body["total"] == 0,
                "{queue} on an empty base: {body}"
            );
        }
        seed_every_section(&f).await?;
        for queue in LIST_QUEUES {
            let total = total(&page(&f, &format!("queue={queue}")).await?)?;
            anyhow::ensure!(total > 0, "{queue}: nothing seeded");
            // 最后一条在 offset = total - 1
            let last = page(&f, &format!("queue={queue}&limit=1&offset={}", total - 1)).await?;
            anyhow::ensure!(items(&last).len() == 1, "{queue}: the last item: {last}");
            for offset in [total, total + 7] {
                let body = page(&f, &format!("queue={queue}&offset={offset}")).await?;
                anyhow::ensure!(
                    body["items"] == json!([]) && body["total"] == total,
                    "{queue} at offset {offset} of {total}: {body}"
                );
            }
        }
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_token_reading_duplicates_receives_the_same_total_as_a_session() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        seed_every_section(&f).await?;
        // 主人正好是 Viewer，令牌只读：读的下限
        let (_, token) = f.token(f.viewer, "reader", "read", None).await?;
        for query in [
            "queue=duplicates",
            "",
            "queue=duplicates&types=same",
            "queue=duplicates&types=conflict",
            "queue=duplicates&limit=1&offset=1",
        ] {
            let (status, mine) = get_list(&f, &token, kb, query).await?;
            anyhow::ensure!(status == StatusCode::OK, "token {query}: {status} {mine}");
            let theirs = page(&f, query).await?;
            anyhow::ensure!(
                keys(&mine) == ["counts", "items", "queue", "total"],
                "{query}: {mine}"
            );
            anyhow::ensure!(
                mine["total"] == theirs["total"] && mine["total"].is_i64(),
                "{query}: total {} vs {}",
                mine["total"],
                theirs["total"]
            );
            anyhow::ensure!(mine["counts"] == theirs["counts"], "{query}: counts differ");
            anyhow::ensure!(mine["items"] == theirs["items"], "{query}: items differ");
            anyhow::ensure!(mine == theirs, "{query}: bodies differ");
        }
        let all = page(&f, "queue=duplicates").await?;
        anyhow::ensure!(all["total"] == 3, "three pairs still wait: {all}");
        // `mappings` 不是 Review 的分档，不开给令牌（E4）：还是 unknown_queue，不带条目和 total
        let (status, body) = get_list(&f, &token, kb, "queue=mappings").await?;
        anyhow::ensure!(
            status == StatusCode::UNPROCESSABLE_ENTITY
                && body["code"] == "unknown_queue"
                && body.get("items").is_none()
                && body.get("total").is_none(),
            "token mappings: {status} {body}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn the_seed_puts_items_in_every_one_of_the_thirteen_sections() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        // 夹具跑的就是仓库里的那个脚本：只有直接插入，没有 DDL，也不改不删
        anyhow::ensure!(SEED_SQL.contains("current_setting('seed.kb')"));
        let statements = SEED_SQL
            .lines()
            .filter(|l| !l.trim_start().starts_with("--"))
            .collect::<Vec<_>>()
            .join("\n")
            .to_uppercase();
        for forbidden in [
            "UPDATE ",
            "DELETE ",
            "CREATE ",
            "ALTER ",
            "DROP ",
            "TRUNCATE ",
        ] {
            anyhow::ensure!(
                !statements.contains(forbidden),
                "the seed only inserts, found {forbidden}"
            );
        }

        seed_every_section(&f).await?;
        let mut listed = std::collections::HashMap::new();
        for queue in LIST_QUEUES {
            let body = page(&f, &format!("queue={queue}&limit=200")).await?;
            listed.insert(queue, items(&body));
        }
        // 十档各至少两条，映射至少一条
        for queue in [
            "pending",
            "duplicates",
            "conflicts",
            "unconfirmed",
            "lowconf",
            "violations",
            "defects",
            "errata",
            "merges",
            "agent",
        ] {
            anyhow::ensure!(
                listed[queue].len() >= 2,
                "{queue}: {} listed",
                listed[queue].len()
            );
        }
        anyhow::ensure!(!listed["mappings"].is_empty(), "one proposed mapping");

        // 对齐：两条短语、一个类别词、两条规则，规则在最后
        let alignment = &listed["alignment"];
        let of_kind = |kind: &str| alignment.iter().filter(|i| i["kind"] == kind).count();
        anyhow::ensure!(
            of_kind("phrase") >= 2 && of_kind("kind_word") >= 1 && of_kind("rule") >= 2,
            "alignment kinds: {alignment:?}"
        );
        anyhow::ensure!(
            alignment.last().is_some_and(|i| i["kind"] == "rule"),
            "a rule is last in the list order"
        );

        // 低置信：推出来的和不是推出来的各至少一条
        let lowconf: Vec<Uuid> = listed["lowconf"]
            .iter()
            .filter_map(|i| i["id"].as_str().and_then(|s| s.parse().ok()))
            .collect();
        let (derived, asserted): (i64, i64) = sqlx::query_as(
            "SELECT count(*) FILTER (WHERE derived_by_rule IS NOT NULL),
                    count(*) FILTER (WHERE derived_by_rule IS NULL)
               FROM facts WHERE id = ANY($1)",
        )
        .bind(&lowconf)
        .fetch_one(&f.pool)
        .await?;
        anyhow::ensure!(
            derived >= 1 && asserted >= 1,
            "lowconf: {derived} derived, {asserted} not"
        );

        // 公理：一条 derived_contradiction，它的证明展得开；另有一条别的种类
        let violations = &listed["violations"];
        let contradiction = violations
            .iter()
            .find(|v| v["kind"] == "derived_contradiction")
            .ok_or_else(|| anyhow::anyhow!("no derived_contradiction: {violations:?}"))?;
        anyhow::ensure!(
            violations
                .iter()
                .any(|v| v["kind"] != "derived_contradiction"),
            "a violation of another kind"
        );
        let id: Uuid = contradiction["id"].as_str().unwrap_or_default().parse()?;
        let steps = utopia_store::reasoning::blocked_proof(&f.pool, kb, id)
            .await?
            .unwrap_or_default();
        anyhow::ensure!(!steps.is_empty(), "blocked_proof returns steps");
        let (status, proof) = f
            .call(
                &f.editor_session,
                "GET",
                &format!("/kbs/{kb}/violations/{id}/proof"),
                None,
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "proof: {status} {proof}");
        anyhow::ensure!(
            proof["steps"].as_array().is_some_and(|s| !s.is_empty()),
            "proof steps: {proof}"
        );

        // Agent：治理 agent 的行没有 `source` 键；令牌的行正好一条，是 f.editor 的写令牌分开的
        let agent = &listed["agent"];
        let tokens: Vec<&Value> = agent.iter().filter(|r| r["source"] == "token").collect();
        anyhow::ensure!(tokens.len() == 1, "exactly one token row: {agent:?}");
        anyhow::ensure!(
            tokens[0]["owner_id"] == f.editor.to_string()
                && tokens[0]["token_name"] == "seed-agent"
                && tokens[0]["operation"] == "review.keep"
                && tokens[0]["outcome"] == "applied",
            "{}",
            tokens[0]
        );
        anyhow::ensure!(
            agent.iter().any(|r| r.get("source").is_none()),
            "a governor row"
        );
        let scope: String = sqlx::query_scalar(
            "SELECT t.scope FROM personal_tokens t WHERE t.id = $1 AND t.user_id = $2",
        )
        .bind(
            tokens[0]["token_id"]
                .as_str()
                .unwrap_or_default()
                .parse::<Uuid>()?,
        )
        .bind(f.editor)
        .fetch_one(&f.pool)
        .await?;
        anyhow::ensure!(
            scope == "write",
            "the keep was made with a write token, not {scope}"
        );

        // 总览：200，等着的不全是零
        let (status, summary) = f
            .call(
                &f.editor_session,
                "GET",
                &format!("/kbs/{kb}/review/summary"),
                None,
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "summary: {status} {summary}");
        let waiting = summary["waiting"].as_object().cloned().unwrap_or_default();
        anyhow::ensure!(
            waiting
                .values()
                .any(|q| q["count"].as_i64().unwrap_or(0) > 0),
            "waiting counts are all zero: {summary}"
        );
        // 决策台账：至少两条
        let (status, history) = f
            .call(
                &f.editor_session,
                "GET",
                &format!("/kbs/{kb}/review/history"),
                None,
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "history: {status} {history}");
        anyhow::ensure!(
            history["total"].as_i64().unwrap_or(0) >= 2
                && history["events"].as_array().map(Vec::len).unwrap_or(0) >= 2,
            "decisions: {history}"
        );

        // 再跑一遍脚本什么都不加：每一档的 total 不变
        let mut totals = Vec::new();
        for queue in LIST_QUEUES {
            totals.push(total(&page(&f, &format!("queue={queue}")).await?)?);
        }
        let mut conn = f.pool.acquire().await?;
        sqlx::raw_sql(&format!("SET seed.kb = '{kb}'"))
            .execute(&mut *conn)
            .await?;
        let again = sqlx::raw_sql(SEED_SQL).execute(&mut *conn).await;
        sqlx::raw_sql("RESET seed.kb").execute(&mut *conn).await?;
        again?;
        drop(conn);
        for (queue, before) in LIST_QUEUES.iter().zip(&totals) {
            let after = total(&page(&f, &format!("queue={queue}")).await?)?;
            anyhow::ensure!(
                after == *before,
                "{queue}: a second run changed {before} to {after}"
            );
        }

        // 另一个库没被碰：每一档空、台账空、图上没有行
        for queue in LIST_QUEUES {
            let (status, body) =
                get_list(&f, &f.editor_session, f.other_kb, &format!("queue={queue}")).await?;
            anyhow::ensure!(
                status == StatusCode::OK && body["items"] == json!([]) && body["total"] == 0,
                "other base, {queue}: {status} {body}"
            );
        }
        let (status, history) = f
            .call(
                &f.editor_session,
                "GET",
                &format!("/kbs/{}/review/history", f.other_kb),
                None,
            )
            .await?;
        anyhow::ensure!(
            status == StatusCode::OK && history["total"] == 0,
            "other base history: {status} {history}"
        );
        let rows: i64 = sqlx::query_scalar(
            "SELECT (SELECT count(*) FROM entities WHERE kb_id = $1)
                  + (SELECT count(*) FROM facts WHERE kb_id = $1)
                  + (SELECT count(*) FROM documents WHERE kb_id = $1)
                  + (SELECT count(*) FROM relation_types WHERE kb_id = $1)",
        )
        .bind(f.other_kb)
        .fetch_one(&f.pool)
        .await?;
        anyhow::ensure!(rows == 0, "the other base holds {rows} rows");
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}
