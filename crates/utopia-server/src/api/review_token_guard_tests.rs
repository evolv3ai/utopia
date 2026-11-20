//! 令牌落地的写入的两道护栏，以及令牌的动作不动治理 agent 读到的历史（0076；SC9、SC11）。
//!
//! 要钉住的行为：
//!
//! - **限速**（E21）：一枚令牌在一个库里、滚动的一小时之内最多落地 200 笔。第 200 笔
//!   落地，下一笔答 429 `agent_rate_limited`，什么都不改、一行不写。留成提议的与被拒绝的
//!   不算；分开、合并、撤回都算；别的令牌、别的库、一小时以前的行不算；成批调用里过了
//!   线的每一条各拿各的 `agent_rate_limited`
//! - **熔断器**（E22）：七天之内，人第二次撤回某一枚令牌做出的合并，这枚令牌在这个库里
//!   的合并与撤回（成批的合并也算）就只留提议，理由 `breaker`；一次不跳；令牌自己撤的、
//!   别的令牌的合并被撤的都不算在它头上；它的分开照常落地；七天过了它重新直接落地
//! - **每一个入口**（N10）：队列合并、手动合并、成批的一笔、整批撤回，REST 与 MCP 的
//!   `review_merge` / `review_revert` / `review_batch` / `review_keep`，拿到的是同一个回答
//! - **先例**（E25）：一次活着的令牌合并、一次令牌合了又由令牌撤回的合并，治理 agent
//!   读到的历史一个字不变，也不排治理任务；人撤回令牌的合并，照任何一次撤回那样算
//!
//! 夹具在 `review_session_unchanged_tests.rs`。种子直接插表，不跑模型任务：到线靠直接插
//! 台账行，不靠真的合并两百次。没有 `UTOPIA_DATABASE_URL` 时跳过。
use axum::http::StatusCode;
use serde_json::{json, Value};
use utopia_core::models::{ReviewItem, ReviewSide};
use utopia_store::governance;
use uuid::Uuid;

use super::review_ops::TOKEN_WRITES_PER_HOUR;
use super::review_session_unchanged_tests::Fx;

const TOKEN_NAME: &str = "cleanup agent";

// ───────────────────────── 调用 ─────────────────────────

/// 队列里的一对，经 REST 裁：`action` 是 merge 或 keep
async fn rest_decide(
    f: &Fx,
    bearer: &str,
    review: Uuid,
    action: &str,
) -> anyhow::Result<(StatusCode, Value)> {
    f.call(
        bearer,
        "POST",
        &format!("/kbs/{}/review/{review}", f.kb),
        Some(json!({ "action": action, "rationale": "same thing" })),
    )
    .await
}

async fn rest_manual(
    f: &Fx,
    bearer: &str,
    source: Uuid,
    target: Uuid,
) -> anyhow::Result<(StatusCode, Value)> {
    f.call(
        bearer,
        "POST",
        &format!("/kbs/{}/entities/merge", f.kb),
        Some(json!({ "source": source, "target": target })),
    )
    .await
}

async fn rest_revert(f: &Fx, bearer: &str, merge: Uuid) -> anyhow::Result<(StatusCode, Value)> {
    f.call(
        bearer,
        "POST",
        &format!("/kbs/{}/merges/{merge}/revert", f.kb),
        None,
    )
    .await
}

async fn rest_batch(
    f: &Fx,
    bearer: &str,
    ids: &[Uuid],
    action: &str,
) -> anyhow::Result<(StatusCode, Value)> {
    f.call(
        bearer,
        "POST",
        &format!("/kbs/{}/review/batch", f.kb),
        Some(json!({ "ids": ids, "action": action })),
    )
    .await
}

async fn rest_unit_revert(
    f: &Fx,
    bearer: &str,
    batch_id: Uuid,
) -> anyhow::Result<(StatusCode, Value)> {
    f.call(
        bearer,
        "POST",
        &format!("/kbs/{}/review/agent-batches/{batch_id}/revert", f.kb),
        None,
    )
    .await
}

/// 一次工具调用，回工具结果。工具自己的拒绝是 `isError: true` 的正常结果
async fn tool(f: &Fx, bearer: &str, name: &str, args: Value) -> anyhow::Result<Value> {
    let (status, body) = f
        .call(
            bearer,
            "POST",
            &format!("/kbs/{}/mcp", f.kb),
            Some(json!({
                "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": { "name": name, "arguments": args },
            })),
        )
        .await?;
    anyhow::ensure!(status == StatusCode::OK, "{name} {args}: {status} {body}");
    anyhow::ensure!(body.get("error").is_none(), "{name} {args}: {body}");
    anyhow::ensure!(body["result"].is_object(), "{name} {args}: {body}");
    Ok(body["result"].clone())
}

/// 成功的工具结果里的 `structuredContent`
fn structured(result: &Value) -> anyhow::Result<Value> {
    anyhow::ensure!(result["isError"] == false, "a tool error: {result}");
    Ok(result["structuredContent"].clone())
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

fn uuid_at(body: &Value, key: &str) -> anyhow::Result<Uuid> {
    Ok(body[key]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("no {key} in {body}"))?
        .parse()?)
}

// ───────────────────────── 回答的样子 ─────────────────────────

/// 落地了：200、`applied`
fn ensure_applied(what: &str, answered: &(StatusCode, Value)) -> anyhow::Result<()> {
    let (status, body) = answered;
    anyhow::ensure!(
        *status == StatusCode::OK && body["ok"] == true && body["outcome"] == "applied",
        "{what} should apply: {status} {body}"
    );
    Ok(())
}

/// 落地的合并或撤回：回它的 `merge_id`
fn applied_merge(what: &str, answered: &(StatusCode, Value)) -> anyhow::Result<Uuid> {
    ensure_applied(what, answered)?;
    uuid_at(&answered.1, "merge_id")
}

/// REST 上的限速：429，body 正好是 `{code, error}`，code 是 `agent_rate_limited`
fn ensure_limited(what: &str, answered: &(StatusCode, Value)) -> anyhow::Result<()> {
    let (status, body) = answered;
    anyhow::ensure!(
        *status == StatusCode::TOO_MANY_REQUESTS,
        "{what} should be rate limited: {status} {body}"
    );
    anyhow::ensure!(body["code"] == "agent_rate_limited", "{what}: {body}");
    anyhow::ensure!(keys(body) == ["code", "error"], "{what}: {body}");
    anyhow::ensure!(
        body["error"].as_str().is_some_and(|s| !s.is_empty()),
        "{what}: {body}"
    );
    Ok(())
}

/// MCP 上的限速：工具错误，`structuredContent` 正好是 `{code, message}`
fn ensure_tool_limited(what: &str, result: &Value) -> anyhow::Result<()> {
    anyhow::ensure!(result["isError"] == true, "{what}: {result}");
    let content = &result["structuredContent"];
    anyhow::ensure!(content["code"] == "agent_rate_limited", "{what}: {result}");
    anyhow::ensure!(keys(content) == ["code", "message"], "{what}: {result}");
    Ok(())
}

/// 熔断器留下的一次合并或撤回（单笔的回答）：`proposed`、理由 `breaker`、带提议的台账行
/// 与一句解释，没有 `merge_id`。回提议的 id
fn ensure_breaker(what: &str, body: &Value) -> anyhow::Result<Uuid> {
    anyhow::ensure!(
        body["ok"] == true && body["outcome"] == "proposed" && body["reason"] == "breaker",
        "{what} should land as a breaker proposal: {body}"
    );
    anyhow::ensure!(body.get("merge_id").is_none(), "{what}: {body}");
    anyhow::ensure!(
        body["explanation"].as_str().is_some_and(|s| !s.is_empty()),
        "{what}: {body}"
    );
    uuid_at(body, "action_id")
}

/// 同上，经 REST：还得是 200
fn ensure_breaker_rest(what: &str, answered: &(StatusCode, Value)) -> anyhow::Result<Uuid> {
    anyhow::ensure!(
        answered.0 == StatusCode::OK,
        "{what}: {} {}",
        answered.0,
        answered.1
    );
    ensure_breaker(what, &answered.1)
}

/// 成批调用里熔断器留下的一条
fn ensure_breaker_item(what: &str, row: &Value) -> anyhow::Result<()> {
    anyhow::ensure!(
        row["outcome"] == "proposed" && row["reason"] == "breaker",
        "{what} should be a breaker proposal: {row}"
    );
    anyhow::ensure!(
        row["merge_id"].is_null() && row["error"].is_null() && !row["action_id"].is_null(),
        "{what}: {row}"
    );
    Ok(())
}

/// 成批调用（或整批撤回）里被限速的一条：`outcome` 是 null，code 是 `agent_rate_limited`
fn ensure_limited_item(what: &str, row: &Value) -> anyhow::Result<()> {
    anyhow::ensure!(
        row["outcome"].is_null() && row["code"] == "agent_rate_limited",
        "{what} should be rate limited: {row}"
    );
    anyhow::ensure!(
        row["error"].as_str().is_some_and(|s| !s.is_empty()),
        "{what}: {row}"
    );
    Ok(())
}

fn outcomes(body: &Value) -> Vec<Value> {
    body["outcomes"].as_array().cloned().unwrap_or_default()
}

// ───────────────────────── 读库与种子 ─────────────────────────

/// 「什么都没变」比的东西：这个库的实体、事实、审核行、合并日志，每一行的每一列
async fn graph(f: &Fx) -> anyhow::Result<Value> {
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

/// 这个库的台账一共多少行
async fn ledger_rows(f: &Fx) -> anyhow::Result<i64> {
    Ok(utopia_store::token_agent_actions::count(&f.pool, f.kb).await?)
}

/// 这枚令牌在这个库里最近一小时落地了多少笔——限速问的就是这个数
async fn finalized(f: &Fx, token_id: Uuid) -> anyhow::Result<i64> {
    Ok(utopia_store::token_agent_actions::finalized_since(
        &f.pool,
        f.kb,
        token_id,
        chrono::Utc::now() - chrono::Duration::hours(1),
    )
    .await?)
}

/// 直接往台账里插 `n` 行：哪个库、哪枚令牌、什么结果、多少分钟以前。落地的行轮着用四种
/// 操作（分开、队列合并、手动合并、撤回都算一笔）；提议的行各有各的钥匙，互不相撞
async fn seed_rows(
    f: &Fx,
    kb: Uuid,
    token_id: Uuid,
    outcome: &str,
    minutes_ago: i32,
    n: i32,
) -> anyhow::Result<()> {
    let reason = match outcome {
        "held" => Some("derived"),
        "proposed" => Some("merge_by_person"),
        _ => None,
    };
    sqlx::query(
        "INSERT INTO token_agent_actions
             (id, kb_id, operation, target_kind, target_id, owner_id, token_id, token_name,
              outcome, payload, gate_reason, target_key, created_at)
         SELECT gen_random_uuid(), $1,
                (ARRAY['review.keep', 'review.merge', 'merge.manual', 'merge.revert'])[1 + g % 4],
                'review', gen_random_uuid(), $2, $3, 'seeded', $4, '{}'::jsonb, $5,
                gen_random_uuid()::text, now() - make_interval(mins => $6)
           FROM generate_series(1, $7) g",
    )
    .bind(kb)
    .bind(f.editor)
    .bind(token_id)
    .bind(outcome)
    .bind(reason)
    .bind(minutes_ago)
    .bind(n)
    .execute(&f.pool)
    .await?;
    Ok(())
}

/// 两个公司配成一对等人的重复项：回 (审核行, 左, 右)。没有事实、没有派生，闸门不拦
async fn companies(f: &Fx, name: &str) -> anyhow::Result<(Uuid, Uuid, Uuid)> {
    let a = f.entity(f.company_type, name).await?;
    let b = f.entity(f.company_type, &format!("{name} Corp")).await?;
    Ok((f.pair_of(a, b).await?, a, b))
}

/// 只要审核行的那种
async fn pair(f: &Fx, name: &str) -> anyhow::Result<Uuid> {
    Ok(companies(f, name).await?.0)
}

/// 令牌合并队列里的一对，必须落地：回做出的合并
async fn token_merge(f: &Fx, bearer: &str, review: Uuid) -> anyhow::Result<Uuid> {
    applied_merge(
        "a token queue merge",
        &rest_decide(f, bearer, review, "merge").await?,
    )
}

/// 登录的编辑者从合并历史撤回一次合并：今天的那条会话路径
async fn person_reverts(f: &Fx, merge: Uuid) -> anyhow::Result<()> {
    let (status, body) = rest_revert(f, &f.editor_session, merge).await?;
    anyhow::ensure!(
        status == StatusCode::OK && body == json!({ "ok": true }),
        "a session revert: {status} {body}"
    );
    Ok(())
}

async fn merge_is_live(f: &Fx, merge: Uuid) -> anyhow::Result<bool> {
    Ok(sqlx::query_scalar(
        "SELECT reverted_at IS NULL FROM entity_merges WHERE id = $1 AND kb_id = $2",
    )
    .bind(merge)
    .bind(f.kb)
    .fetch_one(&f.pool)
    .await?)
}

/// 台账行的 (结果, 理由, 批号)
async fn ledger_row(f: &Fx, id: Uuid) -> anyhow::Result<(String, Option<String>, Option<Uuid>)> {
    Ok(sqlx::query_as(
        "SELECT outcome, gate_reason, batch_id FROM token_agent_actions
          WHERE id = $1 AND kb_id = $2",
    )
    .bind(id)
    .bind(f.kb)
    .fetch_one(&f.pool)
    .await?)
}

// ───────────────────────── E21：限速 ─────────────────────────

#[tokio::test]
async fn the_200th_finalized_write_applies_and_the_next_answers_agent_rate_limited_with_429(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        anyhow::ensure!(TOKEN_WRITES_PER_HOUR == 200, "the limit is 200");
        let (token_id, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let (other_id, other) = f.token(f.editor, "second agent", "write", None).await?;

        // 不算数的行：一小时以前落地的、留成提议的、别的令牌的、别的库的。哪一种要是
        // 被数了进去，下面那几笔就落不了地
        seed_rows(&f, f.kb, token_id, "applied", 61, 30).await?;
        seed_rows(&f, f.kb, token_id, "proposed", 5, 30).await?;
        seed_rows(&f, f.kb, token_id, "held", 5, 30).await?;
        seed_rows(&f, f.kb, other_id, "applied", 5, 150).await?;
        seed_rows(&f, f.other_kb, token_id, "applied", 5, 150).await?;
        // 算数的：最近一小时里这枚令牌在这个库落地的 196 笔
        seed_rows(&f, f.kb, token_id, "applied", 5, 196).await?;
        anyhow::ensure!(finalized(&f, token_id).await? == 196);

        let keep_me = pair(&f, "Keep").await?;
        let merge_a = pair(&f, "Merge A").await?;
        let merge_b = pair(&f, "Merge B").await?;
        let (s1, s1_left, s1_right) = companies(&f, "Spare 1").await?;
        let s2 = pair(&f, "Spare 2").await?;
        let s3 = pair(&f, "Spare 3").await?;
        let s4 = pair(&f, "Spare 4").await?;
        // 人分开过的一对：令牌再去手动合并这两个实体，闸门留下（`kept_by_person`）
        let (kept, kept_left, kept_right) = companies(&f, "Kept").await?;
        let (status, _) = rest_decide(&f, &f.editor_session, kept, "keep").await?;
        anyhow::ensure!(status == StatusCode::OK, "a session keep: {status}");

        // 被拒绝的调用与留成提议的调用：都不算一笔
        let (status, _) = rest_decide(&f, &token, Uuid::now_v7(), "merge").await?;
        anyhow::ensure!(status == StatusCode::NOT_FOUND, "an unknown pair: {status}");
        let (status, body) = rest_manual(&f, &token, kept_left, kept_right).await?;
        anyhow::ensure!(
            status == StatusCode::OK
                && body["outcome"] == "held"
                && body["reason"] == "kept_by_person",
            "a held manual merge: {status} {body}"
        );
        anyhow::ensure!(
            finalized(&f, token_id).await? == 196,
            "a refused call and a proposal are not finalized writes"
        );

        // 分开、合并、撤回各算一笔：197、198、199，第 200 笔照样落地
        ensure_applied(
            "keep (197th)",
            &rest_decide(&f, &token, keep_me, "keep").await?,
        )?;
        let first = token_merge(&f, &token, merge_a).await?;
        let second = token_merge(&f, &token, merge_b).await?;
        anyhow::ensure!(finalized(&f, token_id).await? == 199);
        ensure_applied(
            "the 200th finalized write (a revert)",
            &rest_revert(&f, &token, first).await?,
        )?;
        anyhow::ensure!(finalized(&f, token_id).await? == 200);

        // 下一笔：429 `agent_rate_limited`，哪一种写入都一样；什么都没改，一行没写
        let before = graph(&f).await?;
        let rows = ledger_rows(&f).await?;
        ensure_limited("keep", &rest_decide(&f, &token, s1, "keep").await?)?;
        ensure_limited("queue merge", &rest_decide(&f, &token, s1, "merge").await?)?;
        ensure_limited(
            "manual merge",
            &rest_manual(&f, &token, s1_left, s1_right).await?,
        )?;
        ensure_limited("revert", &rest_revert(&f, &token, second).await?)?;
        // 本来会被拒绝的、会留成提议的调用，过了线同样是 429：线在它们前面
        ensure_limited(
            "an unknown pair",
            &rest_decide(&f, &token, Uuid::now_v7(), "merge").await?,
        )?;
        ensure_limited(
            "a pair the gate would hold",
            &rest_manual(&f, &token, kept_left, kept_right).await?,
        )?;
        // 成批：整批是 200，过了线的每一条各拿各的 `agent_rate_limited`
        for action in ["keep", "merge"] {
            let (status, body) = rest_batch(&f, &token, &[s1, s2], action).await?;
            anyhow::ensure!(status == StatusCode::OK, "batch {action}: {status} {body}");
            anyhow::ensure!(body["decided"] == 0, "{body}");
            let items = outcomes(&body);
            anyhow::ensure!(items.len() == 2, "{body}");
            for (item, id) in items.iter().zip([s1, s2]) {
                anyhow::ensure!(item["id"] == json!(id), "{body}");
                ensure_limited_item("a batch member past the limit", item)?;
                anyhow::ensure!(
                    item["action_id"].is_null() && item["merge_id"].is_null(),
                    "{item}"
                );
            }
        }
        anyhow::ensure!(graph(&f).await? == before, "a limited call changes nothing");
        anyhow::ensure!(
            ledger_rows(&f).await? == rows,
            "a limited call writes no ledger row"
        );
        anyhow::ensure!(finalized(&f, token_id).await? == 200);
        anyhow::ensure!(merge_is_live(&f, second).await?, "the merge stays live");
        for id in [s1, s2] {
            anyhow::ensure!(f.review_row(id).await?.0 == "pending");
        }

        // 滚动的一小时：最早的一笔滑出窗口，就又有一笔的余地——成批的三条里第一条落地，
        // 后两条各自被限速
        sqlx::query(
            "UPDATE token_agent_actions SET created_at = now() - interval '61 minutes'
              WHERE id = (SELECT id FROM token_agent_actions
                           WHERE kb_id = $1 AND token_id = $2 AND outcome = 'applied'
                             AND token_name = 'seeded'
                             AND created_at > now() - interval '1 hour'
                           LIMIT 1)",
        )
        .bind(f.kb)
        .bind(token_id)
        .execute(&f.pool)
        .await?;
        anyhow::ensure!(finalized(&f, token_id).await? == 199);
        let (status, body) = rest_batch(&f, &token, &[s1, s2, s3], "keep").await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let items = outcomes(&body);
        anyhow::ensure!(
            body["decided"] == 1 && items.len() == 3 && items[0]["outcome"] == "applied",
            "the first member is the 200th and applies: {body}"
        );
        ensure_limited_item("the second member", &items[1])?;
        ensure_limited_item("the third member", &items[2])?;
        anyhow::ensure!(f.review_row(s1).await?.0 == "kept");
        anyhow::ensure!(f.review_row(s2).await?.0 == "pending");
        anyhow::ensure!(f.review_row(s3).await?.0 == "pending");

        // 线是每一枚令牌各有各的：同一个人的另一枚令牌在同一个库里照常落地
        ensure_applied(
            "another token's keep",
            &rest_decide(&f, &other, s2, "keep").await?,
        )?;
        ensure_limited(
            "the limited token",
            &rest_decide(&f, &token, s3, "keep").await?,
        )?;
        // 会话不走这条线：登录的编辑者照常裁
        let (status, body) = rest_decide(&f, &f.editor_session, s4, "keep").await?;
        anyhow::ensure!(
            status == StatusCode::OK && body == json!({ "ok": true }),
            "a session keep is not limited: {status} {body}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── E22：熔断器 ─────────────────────────

#[tokio::test]
async fn the_second_revert_by_a_person_within_seven_days_makes_the_tokens_merge_and_revert_proposals_and_its_keep_still_finalizes(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        anyhow::ensure!(governance::FUSE_REVERTS == 2 && governance::FUSE_WINDOW_DAYS == 7);
        let (token_id, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let (other_id, other) = f.token(f.editor, "second agent", "write", None).await?;
        let tripped = |id: Uuid| {
            let pool = f.pool.clone();
            let kb = f.kb;
            async move {
                anyhow::Ok(utopia_store::token_agent_actions::breaker_tripped(&pool, kb, id).await?)
            }
        };

        // 这枚令牌的四次合并，另一枚令牌的两次
        let mut mine = Vec::new();
        for name in ["Alpha", "Beta", "Gamma", "Delta"] {
            mine.push(token_merge(&f, &token, pair(&f, name).await?).await?);
        }
        let mut theirs = Vec::new();
        for name in ["Other 1", "Other 2"] {
            theirs.push(token_merge(&f, &other, pair(&f, name).await?).await?);
        }

        // 不算在这枚令牌头上的撤回：令牌自己撤的一次；别的令牌的合并被人撤的两次
        ensure_applied(
            "a token revert of its own merge",
            &rest_revert(&f, &token, mine[3]).await?,
        )?;
        for merge in &theirs {
            person_reverts(&f, *merge).await?;
        }
        anyhow::ensure!(!tripped(token_id).await?, "none of these is counted");
        anyhow::ensure!(
            tripped(other_id).await?,
            "the other token's two merges were reverted by a person"
        );

        // 人撤回它的一次合并：一次不跳，它的合并与撤回照常落地
        person_reverts(&f, mine[0]).await?;
        anyhow::ensure!(!tripped(token_id).await?, "one revert does not trip");
        let fifth = token_merge(&f, &token, pair(&f, "Epsilon").await?).await?;
        ensure_applied(
            "a token revert after one revert by a person",
            &rest_revert(&f, &token, fifth).await?,
        )?;
        anyhow::ensure!(
            !tripped(token_id).await?,
            "reverts made by a token do not count"
        );

        // 第二次：跳了。此后它的合并与撤回都只留提议，理由 `breaker`，图不动
        person_reverts(&f, mine[1]).await?;
        anyhow::ensure!(tripped(token_id).await?, "the second revert trips");
        // 人的撤回与令牌的撤回同时跑到同一次合并上时，令牌那边会随后写下一行落地的
        // `merge.revert`。那一行不抹掉人的这一次：审计里人的那一行还在，照样算人撤的
        sqlx::query(
            "INSERT INTO token_agent_actions
                 (id, kb_id, operation, target_kind, target_id, owner_id, token_id, token_name,
                  outcome, payload, merge_id, target_key)
             VALUES ($1, $2, 'merge.revert', 'merge', $3, $4, $5, 'second agent', 'applied',
                     '{}'::jsonb, $3, $3::text)",
        )
        .bind(Uuid::now_v7())
        .bind(f.kb)
        .bind(mine[1])
        .bind(f.editor)
        .bind(other_id)
        .execute(&f.pool)
        .await?;
        anyhow::ensure!(
            tripped(token_id).await?,
            "a token's ledger row beside a person's revert does not erase that revert"
        );
        let queue = pair(&f, "Zeta").await?;
        let (_, manual_source, manual_target) = companies(&f, "Eta").await?;
        let batch_a = pair(&f, "Theta").await?;
        let batch_b = pair(&f, "Iota").await?;
        let before = graph(&f).await?;

        let proposal = ensure_breaker_rest(
            "queue merge",
            &rest_decide(&f, &token, queue, "merge").await?,
        )?;
        anyhow::ensure!(
            ledger_row(&f, proposal).await? == ("proposed".into(), Some("breaker".into()), None)
        );
        ensure_breaker_rest(
            "manual merge",
            &rest_manual(&f, &token, manual_source, manual_target).await?,
        )?;
        // 它还活着的那次合并（令牌 agent 做的，平时直接撤）：现在也只留提议
        ensure_breaker_rest("revert", &rest_revert(&f, &token, mine[2]).await?)?;
        anyhow::ensure!(merge_is_live(&f, mine[2]).await?);
        // 成批的合并，每一条都是
        let (status, body) = rest_batch(&f, &token, &[batch_a, batch_b], "merge").await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        anyhow::ensure!(body["decided"] == 0, "{body}");
        let batch_id = uuid_at(&body, "batch_id")?;
        let items = outcomes(&body);
        anyhow::ensure!(items.len() == 2, "{body}");
        for item in &items {
            ensure_breaker_item("a batch merge member", item)?;
            anyhow::ensure!(
                ledger_row(&f, uuid_at(item, "action_id")?).await?
                    == ("proposed".into(), Some("breaker".into()), Some(batch_id))
            );
        }
        anyhow::ensure!(
            graph(&f).await? == before,
            "a breaker proposal changes nothing"
        );
        // 重复的调用拿回同一条提议
        let (status, body) = rest_decide(&f, &token, queue, "merge").await?;
        anyhow::ensure!(
            status == StatusCode::OK
                && body["code"] == "already_proposed"
                && body["reason"] == "breaker"
                && body["action_id"] == json!(proposal),
            "{status} {body}"
        );

        // 分开照常落地：单笔与成批
        ensure_applied(
            "a tripped token's keep",
            &rest_decide(&f, &token, pair(&f, "Kappa").await?, "keep").await?,
        )?;
        let kept = pair(&f, "Lambda").await?;
        let (status, body) = rest_batch(&f, &token, &[kept], "keep").await?;
        anyhow::ensure!(
            status == StatusCode::OK
                && body["decided"] == 1
                && body["outcomes"][0]["outcome"] == "applied",
            "a tripped token's batch keep: {status} {body}"
        );

        // 七天过了：头一次撤回滑出窗口，窗口里只剩一次，它重新直接落地
        sqlx::query(
            "UPDATE entity_merges SET reverted_at = now() - interval '8 days' WHERE id = $1",
        )
        .bind(mine[0])
        .execute(&f.pool)
        .await?;
        anyhow::ensure!(!tripped(token_id).await?, "the window has passed");
        let again = token_merge(&f, &token, pair(&f, "Mu").await?).await?;
        ensure_applied(
            "a revert after the window",
            &rest_revert(&f, &token, again).await?,
        )?;
        // 另一枚令牌的两次都还在窗口里：它照旧只能提议
        ensure_breaker_rest(
            "the other token, still inside its window",
            &rest_decide(&f, &other, pair(&f, "Nu").await?, "merge").await?,
        )?;
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── N10：每一个入口 ─────────────────────────

#[tokio::test]
async fn the_limit_and_the_breaker_hold_for_the_queue_merge_the_manual_merge_batch_members_and_the_mcp_tools(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let (token_id, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;

        // 跳之前：一批三次合并，再加两次单笔的合并（留着给撤回用）
        let batch_pairs = [
            pair(&f, "Batch 1").await?,
            pair(&f, "Batch 2").await?,
            pair(&f, "Batch 3").await?,
        ];
        let (status, body) = rest_batch(&f, &token, &batch_pairs, "merge").await?;
        anyhow::ensure!(
            status == StatusCode::OK && body["decided"] == 3,
            "{status} {body}"
        );
        let batch_id = uuid_at(&body, "batch_id")?;
        let batch_merges: Vec<Uuid> = outcomes(&body)
            .iter()
            .map(|o| uuid_at(o, "merge_id"))
            .collect::<anyhow::Result<_>>()?;
        let live_a = token_merge(&f, &token, pair(&f, "Live A").await?).await?;
        let live_b = token_merge(&f, &token, pair(&f, "Live B").await?).await?;
        // 人撤回这一批里的两次：熔断器跳了。第三次还活着
        person_reverts(&f, batch_merges[0]).await?;
        person_reverts(&f, batch_merges[1]).await?;
        anyhow::ensure!(
            utopia_store::token_agent_actions::breaker_tripped(&f.pool, f.kb, token_id).await?
        );

        // ── 熔断器：哪个入口进来都只留提议 ──
        let rest_queue = pair(&f, "Rest Queue").await?;
        let (_, rest_source, rest_target) = companies(&f, "Rest Manual").await?;
        let rest_member = pair(&f, "Rest Member").await?;
        let mcp_queue = pair(&f, "Mcp Queue").await?;
        let (_, mcp_source, mcp_target) = companies(&f, "Mcp Manual").await?;
        let mcp_member = pair(&f, "Mcp Member").await?;
        let before = graph(&f).await?;

        ensure_breaker_rest(
            "REST queue merge",
            &rest_decide(&f, &token, rest_queue, "merge").await?,
        )?;
        ensure_breaker_rest(
            "REST manual merge",
            &rest_manual(&f, &token, rest_source, rest_target).await?,
        )?;
        let (status, body) = rest_batch(&f, &token, &[rest_member], "merge").await?;
        anyhow::ensure!(
            status == StatusCode::OK && body["decided"] == 0,
            "{status} {body}"
        );
        ensure_breaker_item("REST batch member", &body["outcomes"][0])?;
        ensure_breaker_rest("REST revert", &rest_revert(&f, &token, live_a).await?)?;
        // 整批撤回：撤过的两次是 `already_decided`，还活着的那一次只留提议
        let (status, body) = rest_unit_revert(&f, &token, batch_id).await?;
        anyhow::ensure!(
            status == StatusCode::OK && body["reverted"] == 0,
            "{status} {body}"
        );
        let items = outcomes(&body);
        anyhow::ensure!(items.len() == 3, "{body}");
        anyhow::ensure!(
            items[0]["code"] == "already_decided" && items[1]["code"] == "already_decided",
            "{body}"
        );
        anyhow::ensure!(
            items[2]["outcome"] == "proposed" && items[2]["merge_id"] == json!(batch_merges[2]),
            "the live merge of the batch is only proposed: {body}"
        );

        let answer =
            structured(&tool(&f, &token, "review_merge", json!({ "id": mcp_queue })).await?)?;
        ensure_breaker("MCP review_merge by id", &answer)?;
        let answer = structured(
            &tool(
                &f,
                &token,
                "review_merge",
                json!({ "source": mcp_source, "target": mcp_target }),
            )
            .await?,
        )?;
        ensure_breaker("MCP review_merge by source and target", &answer)?;
        let answer = structured(
            &tool(
                &f,
                &token,
                "review_batch",
                json!({ "ids": [mcp_member], "action": "merge" }),
            )
            .await?,
        )?;
        anyhow::ensure!(answer["decided"] == 0, "{answer}");
        ensure_breaker_item("MCP review_batch member", &answer["outcomes"][0])?;
        let answer =
            structured(&tool(&f, &token, "review_revert", json!({ "merge_id": live_b })).await?)?;
        ensure_breaker("MCP review_revert", &answer)?;
        let answer =
            structured(&tool(&f, &token, "review_revert", json!({ "batch_id": batch_id })).await?)?;
        anyhow::ensure!(
            answer["reverted"] == 0
                && answer["outcomes"][2]["outcome"] == "proposed"
                && answer["outcomes"][2]["code"] == "already_proposed",
            "MCP review_revert of the batch: {answer}"
        );

        anyhow::ensure!(
            graph(&f).await? == before,
            "no entry let the tripped token change the graph"
        );
        for merge in [batch_merges[2], live_a, live_b] {
            anyhow::ensure!(merge_is_live(&f, merge).await?, "{merge} stays live");
        }
        let proposed: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM token_agent_actions
              WHERE kb_id = $1 AND outcome = 'proposed' AND gate_reason = 'breaker'",
        )
        .bind(f.kb)
        .fetch_one(&f.pool)
        .await?;
        anyhow::ensure!(
            proposed == 9,
            "six merge proposals and three revert proposals, got {proposed}"
        );
        // 分开照常落地，两个入口都是
        ensure_applied(
            "REST keep by a tripped token",
            &rest_decide(&f, &token, pair(&f, "Rest Keep").await?, "keep").await?,
        )?;
        let mcp_keep = pair(&f, "Mcp Keep").await?;
        let answer =
            structured(&tool(&f, &token, "review_keep", json!({ "id": mcp_keep })).await?)?;
        anyhow::ensure!(answer["outcome"] == "applied", "MCP review_keep: {answer}");

        // ── 限速：到线之后哪个入口进来都是 `agent_rate_limited` ──
        seed_rows(&f, f.kb, token_id, "applied", 5, 200).await?;
        let limited = pair(&f, "Limited").await?;
        let limited_too = pair(&f, "Limited Too").await?;
        let (_, lim_source, lim_target) = companies(&f, "Limited Manual").await?;
        let before = graph(&f).await?;
        let rows = ledger_rows(&f).await?;

        ensure_limited(
            "REST keep",
            &rest_decide(&f, &token, limited, "keep").await?,
        )?;
        ensure_limited(
            "REST queue merge",
            &rest_decide(&f, &token, limited, "merge").await?,
        )?;
        ensure_limited(
            "REST manual merge",
            &rest_manual(&f, &token, lim_source, lim_target).await?,
        )?;
        ensure_limited("REST revert", &rest_revert(&f, &token, live_a).await?)?;
        for action in ["keep", "merge"] {
            let (status, body) = rest_batch(&f, &token, &[limited, limited_too], action).await?;
            anyhow::ensure!(
                status == StatusCode::OK && body["decided"] == 0,
                "{status} {body}"
            );
            for item in outcomes(&body) {
                ensure_limited_item("REST batch member", &item)?;
            }
        }
        let (status, body) = rest_unit_revert(&f, &token, batch_id).await?;
        anyhow::ensure!(
            status == StatusCode::OK && body["reverted"] == 0,
            "{status} {body}"
        );
        anyhow::ensure!(outcomes(&body).len() == 3, "{body}");
        for item in outcomes(&body) {
            ensure_limited_item("REST unit revert member", &item)?;
        }

        ensure_tool_limited(
            "MCP review_keep",
            &tool(&f, &token, "review_keep", json!({ "id": limited })).await?,
        )?;
        ensure_tool_limited(
            "MCP review_merge by id",
            &tool(&f, &token, "review_merge", json!({ "id": limited })).await?,
        )?;
        ensure_tool_limited(
            "MCP review_merge by source and target",
            &tool(
                &f,
                &token,
                "review_merge",
                json!({ "source": lim_source, "target": lim_target }),
            )
            .await?,
        )?;
        ensure_tool_limited(
            "MCP review_revert",
            &tool(&f, &token, "review_revert", json!({ "merge_id": live_a })).await?,
        )?;
        for action in ["keep", "merge"] {
            let answer = structured(
                &tool(
                    &f,
                    &token,
                    "review_batch",
                    json!({ "ids": [limited, limited_too], "action": action }),
                )
                .await?,
            )?;
            anyhow::ensure!(
                answer["decided"] == 0 && outcomes(&answer).len() == 2,
                "{answer}"
            );
            for item in outcomes(&answer) {
                ensure_limited_item("MCP review_batch member", &item)?;
            }
        }
        let answer =
            structured(&tool(&f, &token, "review_revert", json!({ "batch_id": batch_id })).await?)?;
        anyhow::ensure!(
            answer["reverted"] == 0 && outcomes(&answer).len() == 3,
            "{answer}"
        );
        for item in outcomes(&answer) {
            ensure_limited_item("MCP review_revert batch member", &item)?;
        }

        anyhow::ensure!(
            graph(&f).await? == before,
            "no entry let the limited token change the graph"
        );
        anyhow::ensure!(
            ledger_rows(&f).await? == rows,
            "no entry wrote a ledger row for the limited token"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

// ───────────────────────── E25：治理 agent 读到的历史 ─────────────────────────

/// 治理 agent 读先例用的那一条：只认两边的 id 与名字
fn item(review: Uuid, left: (Uuid, &str), right: (Uuid, &str)) -> ReviewItem {
    let side = |(id, name): (Uuid, &str)| ReviewSide {
        id,
        name: name.to_string(),
        type_label: Some("Organization".into()),
        color: String::new(),
        disambiguator: None,
        degree: 0,
        top_facts: vec![],
    };
    ReviewItem {
        id: review,
        score: 0.5,
        reason: Some("namesake".into()),
        stage: "human".into(),
        created_at: chrono::Utc::now(),
        left: side(left),
        right: side(right),
        proposal: None,
        agent_proposal: None,
    }
}

/// 治理 agent 对一对会读到的全部「人的历史」，拼成一个值，前后好比较：先例三族与类型对的
/// 习惯（`precedents_for`，里面就是 `type_pair_stats`）、台账检索、「人裁过没有」
/// （`decided_before`，两个方向）、「人已经定了」（`settled_by_people`）、闸门（`gate`）
/// 在两条线之间与之上的判决
async fn history(f: &Fx, it: &ReviewItem) -> anyhow::Result<Value> {
    let p = governance::precedents_for(&f.pool, f.kb, it).await?;
    let searched = governance::ledger_search(&f.pool, f.kb, &it.left.name, 50).await?;
    let prior = governance::decided_before(&f.pool, f.kb, it.left.id, it.right.id).await?;
    let swapped = governance::decided_before(&f.pool, f.kb, it.right.id, it.left.id).await?;
    let shape = governance::name_shape(&it.left.name, &it.right.name);
    let gate = |same: bool, conf: f32| {
        format!("{:?}", governance::gate(Some(same), conf, false, shape, &p))
    };
    Ok(json!({
        "precedents": governance::precedents_json(&p),
        "lines": governance::render_lines(&p),
        "type_pair": p.type_pair.as_ref().map(|t| (t.merged, t.kept, t.reverted)),
        "supports_merge": p.supports(true),
        "supports_keep": p.supports(false),
        "ledger_search": searched.iter().map(|x| x.event_id).collect::<Vec<_>>(),
        "decided_before": prior,
        "decided_before_swapped": swapped,
        "settled_by_people":
            governance::settled_by_people(&it.left.name, &it.right.name, &p, prior),
        "gate_keep_0.80": gate(false, 0.8),
        "gate_merge_0.80": gate(true, 0.8),
        "gate_merge_0.99": gate(true, 0.99),
    }))
}

async fn govern_jobs(f: &Fx) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "SELECT count(*) FROM jobs WHERE kind = 'govern' AND payload->>'kb_id' = $1",
    )
    .bind(f.kb.to_string())
    .fetch_one(&f.pool)
    .await?)
}

#[tokio::test]
async fn a_token_merge_and_a_token_reverted_token_merge_move_no_governor_read_and_a_persons_revert_of_one_counts_as_any_revert(
) -> anyhow::Result<()> {
    // 治理开着：令牌的动作要是排了一轮，这里看得见
    let Some(f) = Fx::new(true).await? else {
        return Ok(());
    };
    let result = async {
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        // 五笔人的合并垫底（直接走 store，不排治理任务）：类型对的习惯够数了，0.8 的 merge
        // 因此够治理 agent 自己动手。令牌的动作只要漏进任何一处，这条线就会动
        for i in 0..governance::TYPE_PAIR_MIN {
            let l = f.entity(f.company_type, &format!("Habit {i}")).await?;
            let r = f.entity(f.company_type, &format!("Habit {i} Inc.")).await?;
            let review = f.pair_of(l, r).await?;
            utopia_store::resolution::decide_review(&f.pool, f.kb, review, "merge", f.editor, None)
                .await?;
        }

        // 令牌要动的三对，加一对同名的旁观者（看「这个名字对别的名字」那一族）
        let company = |name: &'static str| f.entity(f.company_type, name);
        let (a1, a2) = (company("Acme").await?, company("Acme Corp").await?);
        let (o1, o2) = (company("Orion").await?, company("Orion Labs").await?);
        let (v1, v2) = (company("Vega").await?, company("Vega Inc.").await?);
        let records = company("Acme Records").await?;
        let queue_pair = f.pair_of(a1, a2).await?;
        let manual_pair = f.pair_of(o1, o2).await?;
        let person_pair = f.pair_of(v1, v2).await?;
        let bystander = f.pair_of(records, a2).await?;
        let items = [
            item(queue_pair, (a1, "Acme"), (a2, "Acme Corp")),
            item(manual_pair, (o1, "Orion"), (o2, "Orion Labs")),
            item(person_pair, (v1, "Vega"), (v2, "Vega Inc.")),
            item(bystander, (records, "Acme Records"), (a2, "Acme Corp")),
        ];

        let mut before = Vec::new();
        for it in &items {
            before.push(history(&f, it).await?);
        }
        for h in &before {
            anyhow::ensure!(
                h["type_pair"] == json!([5, 0, 0]),
                "five merges by people, none reverted: {h}"
            );
            anyhow::ensure!(h["supports_merge"] == json!(true), "{h}");
            anyhow::ensure!(
                h["gate_merge_0.80"] == json!("Apply"),
                "the habit lowers the bar: {h}"
            );
            anyhow::ensure!(h["decided_before"].is_null(), "{h}");
            anyhow::ensure!(h["settled_by_people"].is_null(), "{h}");
            anyhow::ensure!(h["ledger_search"] == json!([]), "{h}");
        }
        anyhow::ensure!(govern_jobs(&f).await? == 0);
        let unchanged = |what: &'static str| {
            let (f, items, before) = (&f, &items, &before);
            async move {
                for (it, was) in items.iter().zip(before) {
                    anyhow::ensure!(
                        &history(f, it).await? == was,
                        "{what}: {} / {} reads a different human history",
                        it.left.name,
                        it.right.name
                    );
                }
                anyhow::ensure!(
                    govern_jobs(f).await? == 0,
                    "{what}: no govern round is queued"
                );
                anyhow::Ok(())
            }
        };

        // ── 活着的令牌合并（队列、手动），经 API：哪一处都不算 ──
        let queue_merge = token_merge(&f, &token, queue_pair).await?;
        unchanged("a live token queue merge").await?;
        // 审核行是 merged、decided_by 是这个人——分得出来的只有 decided_token
        let row = f.review_row(queue_pair).await?;
        anyhow::ensure!(
            row.0 == "merged" && row.1 == Some(f.editor) && row.2.is_some(),
            "{row:?}"
        );
        let manual_merge = applied_merge(
            "a token manual merge",
            &rest_manual(&f, &token, o1, o2).await?,
        )?;
        unchanged("a live token manual merge").await?;

        // ── 令牌合的、令牌撤的：仍然哪一处都不算，类型对的撤回数还是零 ──
        for merge in [queue_merge, manual_merge] {
            ensure_applied("a token revert", &rest_revert(&f, &token, merge).await?)?;
            unchanged("a merge made and reverted by tokens").await?;
        }
        let p = governance::precedents_for(&f.pool, f.kb, &items[0]).await?;
        anyhow::ensure!(
            p.reverts.is_empty() && p.same_pair.is_empty() && p.same_name.is_empty(),
            "no token action is a precedent"
        );
        anyhow::ensure!(
            p.type_pair.as_ref().map(|t| t.reverted) == Some(0),
            "two merges of this type pair are reverted, both made and reverted by tokens"
        );

        // ── 人撤回令牌的合并：照任何一次撤回那样算 ──
        let third = token_merge(&f, &token, person_pair).await?;
        unchanged("a third live token merge").await?;
        person_reverts(&f, third).await?;
        let p = governance::precedents_for(&f.pool, f.kb, &items[2]).await?;
        anyhow::ensure!(
            p.type_pair.as_ref().map(|t| (t.merged, t.kept, t.reverted)) == Some((5, 0, 1)),
            "a person's revert of a token merge counts in the type-pair habit: {:?}",
            p.type_pair
        );
        anyhow::ensure!(
            p.reverts.len() == 1,
            "and is a revert precedent for these names: {:?}",
            p.reverts
        );
        anyhow::ensure!(
            !p.supports(true),
            "a reverted type pair is no longer a merge habit"
        );
        let gate = governance::gate(
            Some(true),
            0.99,
            false,
            governance::name_shape("Vega", "Vega Inc."),
            &p,
        );
        anyhow::ensure!(
            gate == governance::Gate::Propose,
            "a revert precedent on either name makes the governor propose"
        );
        // 别的对上，类型对的习惯同样看得见这一次人的撤回——别的都没变
        let p = governance::precedents_for(&f.pool, f.kb, &items[0]).await?;
        anyhow::ensure!(p.type_pair.as_ref().map(|t| t.reverted) == Some(1));
        anyhow::ensure!(p.reverts.is_empty());
        // 人撤回令牌的合并也不排治理任务：那一步只管治理 agent 自己裁的合并
        anyhow::ensure!(govern_jobs(&f).await? == 0);
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}
