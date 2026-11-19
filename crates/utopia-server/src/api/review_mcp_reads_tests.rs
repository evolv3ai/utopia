//! MCP 的两个 Review 读工具管十三档（0076）：`review_list` 读一档的一页，`review_item` 读
//! 某一档里列着的一条。
//!
//! 要钉住的行为：
//!
//! - **`review_list` 回的就是令牌 API 回的**（SC2）：同一枚令牌，十一个列表档、总览、决策台账，
//!   `structuredContent` 与响应体逐字相等
//! - **各档用各自的翻页参数**（E6）：决策台账是 `page` / `per`，回 `{events, total}`；列表档是
//!   `limit` / `offset`；总览两样都不用。越界的值照路由的规矩夹，不是错误；这一档不用的参数
//!   不作用在它上面
//! - **`review_item` 回的是列表里那一条自己的字段**（SC3）：十一个列表档和决策台账各取一条，
//!   与令牌 API 的列表里那一条相等，后者又与会话编辑者看到的相等
//! - **违规一档的条目带证明**（E8）：`derived_contradiction` 是证明路由的 `steps`，别的种类是
//!   `null` 而不是错误；本体缺陷就是它列表里的那几个键
//! - **「没有」只有一种回答**（E7）：编出来的 id、别的库的 id、已经不在这一档里等着的，
//!   同一个 `not_found`，不带任何字段
//! - **`section` 不给就是重复项；类别词按存着的词原样认；Agent 的行不分来源按 `id` 认**（E9）
//! - **十三档以外的名字是 `unknown_queue`**，`mappings` 也算；总览没有条目，是 `not_found`
//! - **Review 的工具还是那三个**（N4）：读开了，写的一个没多
//!
//! 全部经路由进去（`POST /api/v1/kbs/{kb}/mcp` 与 REST 路由）。夹具在
//! `review_session_unchanged_tests.rs`，种子在 `review_totals_tests.rs`。
//! 没有 `UTOPIA_DATABASE_URL` 时跳过。自建自拆，只碰自己建的库。
use axum::http::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;

use super::review_session_unchanged_tests::Fx;
use super::review_totals_tests::seed_every_section;

/// 种子脚本：`seed_every_section` 只种 `f.kb`，「别的库的 id」那一条要另一个库也有东西，
/// 所以这里把同一份脚本对着 `f.other_kb` 再跑一遍
const SEED_SQL: &str = include_str!("../../../../scripts/seed-review-sections.sql");

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

/// 一次 JSON-RPC 调用，回 HTTP 状态与整个响应体
async fn rpc(
    f: &Fx,
    kb: Uuid,
    bearer: &str,
    method: &str,
    params: Value,
) -> anyhow::Result<(StatusCode, Value)> {
    f.call(
        bearer,
        "POST",
        &format!("/kbs/{kb}/mcp"),
        Some(json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params })),
    )
    .await
}

/// 一次工具调用的 `result`。传输层或 JSON-RPC 层出错在这里就是测试失败——
/// 工具自己的拒绝是 `isError: true` 的正常结果，不走那条路
async fn tool_on(f: &Fx, kb: Uuid, bearer: &str, name: &str, args: Value) -> anyhow::Result<Value> {
    let (status, body) = rpc(
        f,
        kb,
        bearer,
        "tools/call",
        json!({ "name": name, "arguments": args }),
    )
    .await?;
    anyhow::ensure!(status == StatusCode::OK, "{name} {args}: {status} {body}");
    anyhow::ensure!(body.get("error").is_none(), "{name} {args}: {body}");
    anyhow::ensure!(body["result"].is_object(), "{name} {args}: {body}");
    Ok(body["result"].clone())
}

async fn tool(f: &Fx, bearer: &str, name: &str, args: Value) -> anyhow::Result<Value> {
    tool_on(f, f.kb, bearer, name, args).await
}

/// 成功的工具结果里的 `structuredContent`
async fn read(f: &Fx, bearer: &str, name: &str, args: Value) -> anyhow::Result<Value> {
    let result = tool(f, bearer, name, args.clone()).await?;
    anyhow::ensure!(result["isError"] == false, "{name} {args}: {result}");
    Ok(result["structuredContent"].clone())
}

/// 某个库上的一次 REST 读，必须是 200。`tail` 接在 `/kbs/{kb}/` 后面
async fn rest_on(f: &Fx, kb: Uuid, bearer: &str, tail: &str) -> anyhow::Result<Value> {
    let (status, body) = f
        .call(bearer, "GET", &format!("/kbs/{kb}/{tail}"), None)
        .await?;
    anyhow::ensure!(status == StatusCode::OK, "{tail}: {status} {body}");
    Ok(body)
}

async fn rest(f: &Fx, bearer: &str, tail: &str) -> anyhow::Result<Value> {
    rest_on(f, f.kb, bearer, tail).await
}

fn items(body: &Value) -> Vec<Value> {
    body["items"].as_array().cloned().unwrap_or_default()
}

fn events(body: &Value) -> Vec<Value> {
    body["events"].as_array().cloned().unwrap_or_default()
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

/// `review_item` 认的那个标识：有 `id` 用 `id`，类别词用 `kind_word`
fn identifier(item: &Value) -> anyhow::Result<String> {
    item["id"]
        .as_str()
        .or_else(|| item["kind_word"].as_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("an item with neither id nor kind_word: {item}"))
}

/// 一个被拒绝的工具结果：`isError`，带这个 code，文字就是那句 message，
/// `structuredContent` 里除了这两样什么都没有
fn ensure_refused(result: &Value, code: &str) -> anyhow::Result<()> {
    anyhow::ensure!(result["isError"] == true, "expected {code}: {result}");
    let content = &result["structuredContent"];
    anyhow::ensure!(content["code"] == code, "expected {code}: {result}");
    anyhow::ensure!(
        content["message"].is_string() && content["message"] == result["content"][0]["text"],
        "{result}"
    );
    anyhow::ensure!(
        keys(content) == ["code", "message"],
        "a refusal carries a code and a message and nothing else: {result}"
    );
    Ok(())
}

/// 令牌 API 上某一档的全部条目（种过的库每一档都不到 200 条）
async fn listed(f: &Fx, kb: Uuid, bearer: &str, section: &str) -> anyhow::Result<Vec<Value>> {
    let body = rest_on(f, kb, bearer, &format!("review?queue={section}&limit=200")).await?;
    Ok(items(&body))
}

/// 把 `f.other_kb` 也种成每一档都有东西的样子，外加一个只有它才有的类别词。
/// 脚本里的 id 都由库的 id 推出来，两个库的条目各有各的 id
async fn seed_other_base(f: &Fx) -> anyhow::Result<()> {
    let mut conn = f.pool.acquire().await?;
    // 拼进去的只有夹具自己生成的 UUID
    sqlx::raw_sql(&format!("SET seed.kb = '{}'", f.other_kb))
        .execute(&mut *conn)
        .await?;
    let seeded = sqlx::raw_sql(SEED_SQL).execute(&mut *conn).await;
    sqlx::raw_sql("RESET seed.kb").execute(&mut *conn).await?;
    seeded?;
    drop(conn);
    sqlx::query(
        "INSERT INTO type_bindings (id, kb_id, kind_word, words, status, votes, decided_at)
         VALUES ($1, $2, 'elsewhere venture', ARRAY['Elsewhere Venture'], 'undecided',
                 '{\"first\": null, \"second\": null}'::jsonb, now())",
    )
    .bind(Uuid::now_v7())
    .bind(f.other_kb)
    .execute(&f.pool)
    .await?;
    Ok(())
}

async fn listed_tools(f: &Fx, bearer: &str) -> anyhow::Result<Vec<Value>> {
    let (status, body) = rpc(f, f.kb, bearer, "tools/list", json!({})).await?;
    anyhow::ensure!(status == StatusCode::OK, "tools/list: {status} {body}");
    Ok(body["result"]["tools"]
        .as_array()
        .cloned()
        .unwrap_or_default())
}

#[tokio::test]
async fn review_list_returns_what_the_token_api_returns_for_each_of_the_thirteen_sections(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        seed_every_section(&f).await?;
        // 主人正好是 Viewer，令牌只读：读的下限
        let (_, token) = f.token(f.viewer, "reader", "read", None).await?;

        for section in SECTIONS {
            // 缺省的一页，和指定大小的一页：两边同一组参数
            for (query, args) in [
                (
                    format!("review?queue={section}"),
                    json!({ "section": section }),
                ),
                (
                    format!("review?queue={section}&limit=3&offset=0"),
                    json!({ "section": section, "limit": 3, "offset": 0 }),
                ),
            ] {
                let api = rest(&f, &token, &query).await?;
                let mine = read(&f, &token, "review_list", args).await?;
                anyhow::ensure!(
                    mine == api,
                    "{query}: MCP differs from the token API\n{mine}\n{api}"
                );
                anyhow::ensure!(
                    keys(&mine) == ["counts", "items", "queue", "total"],
                    "{query}: {mine}"
                );
                anyhow::ensure!(mine["queue"] == section, "{query}: {mine}");
                anyhow::ensure!(
                    !items(&mine).is_empty(),
                    "{query}: nothing listed on the seeded base"
                );
                anyhow::ensure!(
                    mine["total"].as_i64().is_some_and(|t| t >= 2),
                    "{query}: total {}",
                    mine["total"]
                );
            }
        }

        let api = rest(&f, &token, "review/summary").await?;
        let mine = read(&f, &token, "review_list", json!({ "section": "overview" })).await?;
        anyhow::ensure!(mine == api, "overview differs over MCP\n{mine}\n{api}");
        anyhow::ensure!(
            keys(&mine) == ["agent", "decided", "health", "waiting"],
            "{mine}"
        );

        let api = rest(&f, &token, "review/history").await?;
        let mine = read(&f, &token, "review_list", json!({ "section": "decisions" })).await?;
        anyhow::ensure!(mine == api, "decisions differ over MCP\n{mine}\n{api}");
        anyhow::ensure!(keys(&mine) == ["events", "total"], "{mine}");
        anyhow::ensure!(
            events(&mine).len() >= 2 && mine["total"].as_i64().is_some_and(|t| t >= 2),
            "the seeded history has at least two events: {mine}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn review_list_pages_decisions_by_page_and_per_and_the_queue_sections_by_limit_and_offset(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        seed_every_section(&f).await?;
        let (_, token) = f.token(f.viewer, "reader", "read", None).await?;

        // 决策台账：它自己的 `page` / `per`，它自己的 `{events, total}`
        let api = rest(&f, &token, "review/history?page=1&per=1").await?;
        let mine = read(
            &f,
            &token,
            "review_list",
            json!({ "section": "decisions", "page": 1, "per": 1 }),
        )
        .await?;
        anyhow::ensure!(mine == api, "decisions page 1 per 1\n{mine}\n{api}");
        anyhow::ensure!(keys(&mine) == ["events", "total"], "{mine}");
        anyhow::ensure!(events(&mine).len() == 1, "one event per page: {mine}");
        let first = read(
            &f,
            &token,
            "review_list",
            json!({ "section": "decisions", "page": 0, "per": 1 }),
        )
        .await?;
        anyhow::ensure!(
            events(&first).len() == 1 && events(&first) != events(&mine),
            "page 0 and page 1 are different events\n{first}\n{mine}"
        );
        // 越界的 `per` 照路由的规矩夹，不是错误
        for per in [0, 1000, -4] {
            let api = rest(&f, &token, &format!("review/history?per={per}")).await?;
            let result = tool(
                &f,
                &token,
                "review_list",
                json!({ "section": "decisions", "per": per }),
            )
            .await?;
            anyhow::ensure!(
                result["isError"] == false,
                "per {per} must not be a tool error: {result}"
            );
            anyhow::ensure!(
                result["structuredContent"] == api,
                "per {per}\n{result}\n{api}"
            );
        }
        let one = read(
            &f,
            &token,
            "review_list",
            json!({ "section": "decisions", "per": 0 }),
        )
        .await?;
        anyhow::ensure!(events(&one).len() == 1, "per 0 is served as 1: {one}");
        let all = read(
            &f,
            &token,
            "review_list",
            json!({ "section": "decisions", "per": 1000 }),
        )
        .await?;
        anyhow::ensure!(
            events(&all).len() as i64 == all["total"].as_i64().unwrap_or(-1),
            "per 1000 is served as 100, which holds the whole seeded history: {all}"
        );
        // 列表档的 `limit` / `offset` 不作用在台账上
        let default = rest(&f, &token, "review/history").await?;
        let mine = read(
            &f,
            &token,
            "review_list",
            json!({ "section": "decisions", "limit": 1, "offset": 1 }),
        )
        .await?;
        anyhow::ensure!(
            mine == default,
            "limit and offset do not page decisions\n{mine}\n{default}"
        );

        // 列表档：`limit` / `offset`，与令牌 API 同一页
        for section in SECTIONS {
            let api = rest(
                &f,
                &token,
                &format!("review?queue={section}&limit=1&offset=1"),
            )
            .await?;
            let mine = read(
                &f,
                &token,
                "review_list",
                json!({ "section": section, "limit": 1, "offset": 1 }),
            )
            .await?;
            anyhow::ensure!(mine == api, "{section} limit 1 offset 1\n{mine}\n{api}");
            anyhow::ensure!(items(&mine).len() == 1, "{section}: {mine}");
            // 台账的 `page` / `per` 不作用在列表档上
            let default = rest(&f, &token, &format!("review?queue={section}")).await?;
            let mine = read(
                &f,
                &token,
                "review_list",
                json!({ "section": section, "page": 1, "per": 1 }),
            )
            .await?;
            anyhow::ensure!(
                mine == default,
                "{section}: page and per do not page a queue section\n{mine}\n{default}"
            );
        }

        // 总览：两样都不用，给了也是同一个总览
        let summary = rest(&f, &token, "review/summary").await?;
        for args in [
            json!({ "section": "overview" }),
            json!({ "section": "overview", "limit": 1, "offset": 1 }),
            json!({ "section": "overview", "page": 1, "per": 1 }),
            json!({ "section": "overview", "limit": 1, "offset": 1, "page": 1, "per": 1 }),
        ] {
            let mine = read(&f, &token, "review_list", args.clone()).await?;
            anyhow::ensure!(mine == summary, "{args}\n{mine}\n{summary}");
        }

        // 这一档用的参数给错了类型：纯文字的参数错误，没有执行，不带 code
        for args in [
            json!({ "section": "decisions", "per": "many" }),
            json!({ "section": "decisions", "page": 1.5 }),
            json!({ "section": "conflicts", "limit": "many" }),
        ] {
            let result = tool(&f, &token, "review_list", args.clone()).await?;
            anyhow::ensure!(result["isError"] == true, "{args}: {result}");
            anyhow::ensure!(
                result["structuredContent"].get("code").is_none()
                    && result["structuredContent"].get("events").is_none()
                    && result["structuredContent"].get("items").is_none(),
                "{args}: {result}"
            );
        }

        // 清单里的 schema 没有把越界的值拦在客户端
        let tools = listed_tools(&f, &token).await?;
        let list = tools
            .iter()
            .find(|t| t["name"] == "review_list")
            .ok_or_else(|| anyhow::anyhow!("review_list is not listed"))?;
        for key in ["limit", "offset", "page", "per"] {
            let schema = &list["inputSchema"]["properties"][key];
            anyhow::ensure!(schema["type"] == "integer", "{key}: {schema}");
            anyhow::ensure!(
                schema.get("minimum").is_none() && schema.get("maximum").is_none(),
                "{key}: {schema}"
            );
        }
        anyhow::ensure!(
            list["inputSchema"]["required"] == json!(["section"]),
            "{list}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn review_item_returns_the_listed_items_own_fields_for_one_item_of_each_section(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        seed_every_section(&f).await?;
        let (_, token) = f.token(f.viewer, "reader", "read", None).await?;

        for section in SECTIONS {
            let mine = listed(&f, f.kb, &token, section).await?;
            let theirs = listed(&f, f.kb, &f.editor_session, section).await?;
            anyhow::ensure!(
                mine == theirs,
                "{section}: the token's list is the session editor's list"
            );
            anyhow::ensure!(mine.len() >= 2, "{section}: {} items", mine.len());
            // 不只取一条：这一档列着的每一条都取一遍
            for listed_item in &mine {
                let id = identifier(listed_item)?;
                let mut item = read(
                    &f,
                    &token,
                    "review_item",
                    json!({ "section": section, "id": id }),
                )
                .await?;
                if section == "violations" {
                    // 违规多一个 `proof`，放到一边之后就是列表里那一条
                    let proof = item
                        .as_object_mut()
                        .and_then(|o| o.remove("proof"))
                        .ok_or_else(|| anyhow::anyhow!("a violation carries proof: {item}"))?;
                    anyhow::ensure!(proof.is_null() || proof.is_array(), "{proof}");
                }
                anyhow::ensure!(
                    item == *listed_item,
                    "{section} {id}: review_item must be the listed object\n{item}\n{listed_item}"
                );
            }

            if section == "alignment" {
                // 三种条目都在：短语、类别词、蕴含规则
                for kind in ["phrase", "kind_word", "rule"] {
                    anyhow::ensure!(
                        mine.iter().any(|i| i["kind"] == kind),
                        "alignment lists no {kind}"
                    );
                }
            }
            if section == "agent" {
                // 两种来源都在：治理 agent 的行不带 `source`，令牌的行带
                anyhow::ensure!(
                    mine.iter().any(|i| i["source"] == "token")
                        && mine.iter().any(|i| i.get("source").is_none()),
                    "agent lists both sources: {mine:?}"
                );
            }
        }

        // 决策台账：事件按它的 id 取
        let mine = rest(&f, &token, "review/history?per=100").await?;
        let theirs = rest(&f, &f.editor_session, "review/history?per=100").await?;
        anyhow::ensure!(mine == theirs, "the token's history is the session's");
        anyhow::ensure!(events(&mine).len() >= 2, "{mine}");
        for event in events(&mine) {
            let item = read(
                &f,
                &token,
                "review_item",
                json!({ "section": "decisions", "id": event["id"] }),
            )
            .await?;
            anyhow::ensure!(
                item == event,
                "review_item must be the listed event\n{item}\n{event}"
            );
        }
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn review_item_carries_proof_steps_for_a_derived_contradiction_and_a_null_proof_for_another_kind(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        seed_every_section(&f).await?;
        let (_, token) = f.token(f.viewer, "reader", "read", None).await?;

        let violations = listed(&f, f.kb, &token, "violations").await?;
        let derived = violations
            .iter()
            .find(|v| v["kind"] == "derived_contradiction")
            .ok_or_else(|| anyhow::anyhow!("no derived_contradiction seeded"))?;
        let other = violations
            .iter()
            .find(|v| v["kind"] != "derived_contradiction")
            .ok_or_else(|| anyhow::anyhow!("no violation of another kind seeded"))?;

        // 展得开的那一条：`proof` 就是证明路由给同一枚令牌的 `steps`
        let id = identifier(derived)?;
        let route = rest(&f, &token, &format!("violations/{id}/proof")).await?;
        let item = read(
            &f,
            &token,
            "review_item",
            json!({ "section": "violations", "id": id }),
        )
        .await?;
        anyhow::ensure!(
            item["proof"] == route["steps"],
            "proof is the proof route's steps\n{}\n{}",
            item["proof"],
            route["steps"]
        );
        anyhow::ensure!(
            item["proof"].as_array().is_some_and(|s| !s.is_empty()),
            "the seeded derived_contradiction has proof steps: {item}"
        );
        // 多出来的只有 `proof` 这一个键
        let mut expected = keys(derived);
        expected.push("proof".into());
        expected.sort();
        anyhow::ensure!(keys(&item) == expected, "{:?}", keys(&item));

        // 别的种类：`proof` 是 null，不是错误
        let id = identifier(other)?;
        let route = rest(&f, &token, &format!("violations/{id}/proof")).await?;
        anyhow::ensure!(route == json!({ "steps": null }), "{route}");
        let result = tool(
            &f,
            &token,
            "review_item",
            json!({ "section": "violations", "id": id }),
        )
        .await?;
        anyhow::ensure!(
            result["isError"] == false,
            "a violation of another kind is not an error: {result}"
        );
        let item = &result["structuredContent"];
        anyhow::ensure!(
            item.get("proof").is_some_and(Value::is_null),
            "proof is present and null: {item}"
        );
        let mut expected = keys(other);
        expected.push("proof".into());
        expected.sort();
        anyhow::ensure!(keys(item) == expected, "{:?}", keys(item));

        // 本体缺陷：就是列表里那一条的键，不带 `proof`
        let defects = listed(&f, f.kb, &token, "defects").await?;
        anyhow::ensure!(!defects.is_empty(), "no defect seeded");
        for defect in &defects {
            let item = read(
                &f,
                &token,
                "review_item",
                json!({ "section": "defects", "id": identifier(defect)? }),
            )
            .await?;
            anyhow::ensure!(
                keys(&item) == keys(defect),
                "a defect is exactly its list fields: {:?} vs {:?}",
                keys(&item),
                keys(defect)
            );
            anyhow::ensure!(item.get("proof").is_none(), "{item}");
            anyhow::ensure!(item == *defect, "{item}\n{defect}");
        }
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn review_item_answers_one_not_found_for_an_unknown_id_another_bases_id_and_an_item_no_longer_waiting(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        seed_every_section(&f).await?;
        seed_other_base(&f).await?;
        // 令牌两个库都够得着：它被拒绝不是因为够不着那个库
        let (_, token) = f.token(f.viewer, "reader", "read", None).await?;

        // 编出来的 id：每一档都是同一个回答，以它为准
        let unknown = tool(
            &f,
            &token,
            "review_item",
            json!({ "section": "duplicates", "id": Uuid::now_v7() }),
        )
        .await?;
        ensure_refused(&unknown, "not_found")?;

        let mut sections: Vec<&str> = SECTIONS.to_vec();
        sections.push("decisions");
        for section in sections {
            let made_up = tool(
                &f,
                &token,
                "review_item",
                json!({ "section": section, "id": Uuid::now_v7() }),
            )
            .await?;
            anyhow::ensure!(
                made_up == unknown,
                "{section}: a made-up id\n{made_up}\n{unknown}"
            );

            // 另一个库的这一档里真的列着的一条
            let elsewhere = if section == "decisions" {
                events(&rest_on(&f, f.other_kb, &token, "review/history").await?)
            } else {
                listed(&f, f.other_kb, &token, section).await?
            };
            let foreign = elsewhere
                .iter()
                .find(|i| i["id"].is_string())
                .ok_or_else(|| anyhow::anyhow!("{section}: nothing listed in the other base"))?;
            let id = identifier(foreign)?;
            let there = tool_on(
                &f,
                f.other_kb,
                &token,
                "review_item",
                json!({ "section": section, "id": id }),
            )
            .await?;
            anyhow::ensure!(
                there["isError"] == false && there["structuredContent"]["id"] == id.as_str(),
                "{section}: the other base's item is real: {there}"
            );
            let here = tool(
                &f,
                &token,
                "review_item",
                json!({ "section": section, "id": id }),
            )
            .await?;
            anyhow::ensure!(
                here == unknown,
                "{section}: an id of another base must read like an unknown id\n{here}\n{unknown}"
            );
            anyhow::ensure!(
                !here.to_string().contains(&id),
                "{section}: the id is not echoed back: {here}"
            );
        }

        // 类别词也一样：只有另一个库才有的词，和编出来的词
        let there = tool_on(
            &f,
            f.other_kb,
            &token,
            "review_item",
            json!({ "section": "alignment", "id": "elsewhere venture" }),
        )
        .await?;
        anyhow::ensure!(
            there["isError"] == false
                && there["structuredContent"]["kind_word"] == "elsewhere venture",
            "the other base's kind word is real: {there}"
        );
        for word in ["elsewhere venture", "no such kind word"] {
            let here = tool(
                &f,
                &token,
                "review_item",
                json!({ "section": "alignment", "id": word }),
            )
            .await?;
            anyhow::ensure!(here == unknown, "{word}\n{here}\n{unknown}");
        }
        // 不是 uuid 的 id 在别的档里只能是「没有」
        for section in ["duplicates", "violations", "agent", "decisions"] {
            let here = tool(
                &f,
                &token,
                "review_item",
                json!({ "section": section, "id": "seed startup" }),
            )
            .await?;
            anyhow::ensure!(here == unknown, "{section}\n{here}\n{unknown}");
        }

        // 一档的 id 拿到另一档去问：也是「没有」
        let pair = identifier(&listed(&f, kb, &token, "duplicates").await?[0])?;
        let violation = identifier(&listed(&f, kb, &token, "violations").await?[0])?;
        let event = identifier(&events(&rest(&f, &token, "review/history").await?)[0])?;
        for (section, id) in [
            ("conflicts", &pair),
            ("merges", &pair),
            ("decisions", &pair),
            ("defects", &violation),
            ("duplicates", &violation),
            ("agent", &event),
            ("alignment", &event),
        ] {
            let here = tool(
                &f,
                &token,
                "review_item",
                json!({ "section": section, "id": id }),
            )
            .await?;
            anyhow::ensure!(
                here == unknown,
                "{section} asked for an id of another section\n{here}\n{unknown}"
            );
        }

        // 已经不在这一档里等着的：裁过的一对重复项
        let found = tool(&f, &token, "review_item", json!({ "id": pair })).await?;
        anyhow::ensure!(found["isError"] == false, "{found}");
        let (status, body) = f
            .call(
                &f.editor_session,
                "POST",
                &format!("/kbs/{kb}/review/{pair}"),
                Some(json!({ "action": "keep", "rationale": "different people" })),
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "keep: {status} {body}");
        let gone = tool(
            &f,
            &token,
            "review_item",
            json!({ "section": "duplicates", "id": pair }),
        )
        .await?;
        anyhow::ensure!(gone == unknown, "a decided pair\n{gone}\n{unknown}");

        // 裁过的一处违规（接受它）
        let found = tool(
            &f,
            &token,
            "review_item",
            json!({ "section": "violations", "id": violation }),
        )
        .await?;
        anyhow::ensure!(found["isError"] == false, "{found}");
        let (status, body) = f
            .call(
                &f.editor_session,
                "POST",
                &format!("/kbs/{kb}/review/violations/{violation}"),
                Some(json!({ "resolution": "accepted" })),
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "violation: {status} {body}");
        let gone = tool(
            &f,
            &token,
            "review_item",
            json!({ "section": "violations", "id": violation }),
        )
        .await?;
        anyhow::ensure!(gone == unknown, "a decided violation\n{gone}\n{unknown}");

        // 定过的对齐条目：类别词定成「没有类对得上」
        let found = tool(
            &f,
            &token,
            "review_item",
            json!({ "section": "alignment", "id": "seed startup" }),
        )
        .await?;
        anyhow::ensure!(found["isError"] == false, "{found}");
        let (status, body) = f
            .call(
                &f.editor_session,
                "POST",
                &format!("/kbs/{kb}/review/alignment/kind-words/seed%20startup"),
                Some(json!({ "class": null })),
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "kind word: {status} {body}");
        let gone = tool(
            &f,
            &token,
            "review_item",
            json!({ "section": "alignment", "id": "seed startup" }),
        )
        .await?;
        anyhow::ensure!(gone == unknown, "a decided kind word\n{gone}\n{unknown}");

        // 没有一个回答带着条目的字段或证据
        anyhow::ensure!(
            !unknown.to_string().contains("Seed"),
            "evidence leaked: {unknown}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn review_item_defaults_to_duplicates_and_finds_a_kind_word_by_its_exact_word_and_an_agent_row_by_its_row_id(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        seed_every_section(&f).await?;
        let (_, token) = f.token(f.viewer, "reader", "read", None).await?;

        // `section` 不给就是重复项
        let pairs = listed(&f, f.kb, &token, "duplicates").await?;
        let pair = pairs
            .first()
            .ok_or_else(|| anyhow::anyhow!("no pending pair seeded"))?;
        let id = identifier(pair)?;
        let omitted = read(&f, &token, "review_item", json!({ "id": id })).await?;
        anyhow::ensure!(
            omitted == *pair,
            "no section means duplicates\n{omitted}\n{pair}"
        );
        let null = read(
            &f,
            &token,
            "review_item",
            json!({ "id": id, "section": null }),
        )
        .await?;
        let named = read(
            &f,
            &token,
            "review_item",
            json!({ "section": "duplicates", "id": id }),
        )
        .await?;
        anyhow::ensure!(null == omitted && named == omitted, "{null}\n{named}");
        // 缺省的是重复项，不是「哪一档都找一遍」：别的档的 id 不给 `section` 就找不到
        let violation = identifier(&listed(&f, f.kb, &token, "violations").await?[0])?;
        let result = tool(&f, &token, "review_item", json!({ "id": violation })).await?;
        ensure_refused(&result, "not_found")?;

        // 类别词：按存着的词原样认
        let alignment = listed(&f, f.kb, &token, "alignment").await?;
        let word_item = alignment
            .iter()
            .find(|i| i["kind"] == "kind_word")
            .ok_or_else(|| anyhow::anyhow!("no kind word seeded"))?;
        anyhow::ensure!(
            word_item["kind_word"] == "seed startup" && word_item.get("id").is_none(),
            "{word_item}"
        );
        let found = read(
            &f,
            &token,
            "review_item",
            json!({ "section": "alignment", "id": "seed startup" }),
        )
        .await?;
        anyhow::ensure!(found == *word_item, "{found}\n{word_item}");
        // 换了大小写、添了空格，就不是那个词
        for word in [
            "Seed Startup",
            "SEED STARTUP",
            "seed startup ",
            " seed startup",
            "seed  startup",
        ] {
            let result = tool(
                &f,
                &token,
                "review_item",
                json!({ "section": "alignment", "id": word }),
            )
            .await?;
            ensure_refused(&result, "not_found")
                .map_err(|e| anyhow::anyhow!("{word:?} must not be found: {e}"))?;
        }

        // Agent 的行：治理 agent 的和令牌台账的，都按行的 `id` 认
        let agent = listed(&f, f.kb, &token, "agent").await?;
        let governor = agent
            .iter()
            .find(|r| r.get("source").is_none())
            .ok_or_else(|| anyhow::anyhow!("no governor row seeded"))?;
        let ledger = agent
            .iter()
            .find(|r| r["source"] == "token")
            .ok_or_else(|| anyhow::anyhow!("no token row seeded"))?;
        for row in [governor, ledger] {
            let found = read(
                &f,
                &token,
                "review_item",
                json!({ "section": "agent", "id": identifier(row)? }),
            )
            .await?;
            anyhow::ensure!(found == *row, "{found}\n{row}");
        }
        anyhow::ensure!(
            ledger["operation"] == "review.keep" && ledger["token_name"] == "seed-agent",
            "{ledger}"
        );
        // 行的 id 才是标识：它指向的那一对（`target_id`）不是 Agent 一档里的一条
        let target = ledger["target_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("{ledger}"))?;
        let result = tool(
            &f,
            &token,
            "review_item",
            json!({ "section": "agent", "id": target }),
        )
        .await?;
        ensure_refused(&result, "not_found")?;
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn review_item_refuses_mappings_and_an_unknown_section_with_unknown_queue(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        seed_every_section(&f).await?;
        let (_, read_token) = f.token(f.viewer, "reader", "read", None).await?;
        let (_, write_token) = f.token(f.editor, "writer", "write", None).await?;
        // 会话列得出来的一条映射：它是真的，令牌照样问不到
        let mappings = listed(&f, f.kb, &f.editor_session, "mappings").await?;
        let mapping = identifier(
            mappings
                .first()
                .ok_or_else(|| anyhow::anyhow!("no mapping seeded"))?,
        )?;
        let pair = identifier(&listed(&f, f.kb, &read_token, "duplicates").await?[0])?;

        for token in [&read_token, &write_token] {
            for (section, id) in [
                ("mappings", mapping.as_str()),
                ("no-such-section", pair.as_str()),
                ("", pair.as_str()),
                ("Duplicates", pair.as_str()),
            ] {
                let result = tool(
                    &f,
                    token,
                    "review_item",
                    json!({ "section": section, "id": id }),
                )
                .await?;
                ensure_refused(&result, "unknown_queue")
                    .map_err(|e| anyhow::anyhow!("section {section:?}: {e}"))?;
                // 与 `review_list` 对同一个名字的拒绝是同一个 code、同一句话
                if !section.is_empty() {
                    let list =
                        tool(&f, token, "review_list", json!({ "section": section })).await?;
                    anyhow::ensure!(
                        list["structuredContent"] == result["structuredContent"],
                        "{section}: {list}\n{result}"
                    );
                }
            }
            // 总览没有条目：问哪一个 id 都是「没有」，不是「没有这一档」
            for id in [pair.clone(), Uuid::now_v7().to_string(), "waiting".into()] {
                let result = tool(
                    &f,
                    token,
                    "review_item",
                    json!({ "section": "overview", "id": id }),
                )
                .await?;
                ensure_refused(&result, "not_found")?;
            }
        }
        // `section` 给了别的类型：参数错误，没有执行
        let result = tool(
            &f,
            &read_token,
            "review_item",
            json!({ "section": 7, "id": pair }),
        )
        .await?;
        anyhow::ensure!(
            result["isError"] == true && result["structuredContent"].get("code").is_none(),
            "{result}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn the_mcp_endpoint_offers_no_review_tool_beyond_list_item_and_keep() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        seed_every_section(&f).await?;
        let (_, viewer_read) = f.token(f.viewer, "reader", "read", None).await?;
        let (_, editor_read) = f.token(f.editor, "editor reader", "read", None).await?;
        let (_, editor_write) = f.token(f.editor, "writer", "write", None).await?;
        for (who, token, expected) in [
            (
                "read token of a viewer",
                &viewer_read,
                vec!["review_item", "review_list"],
            ),
            (
                "read token of an editor",
                &editor_read,
                vec!["review_item", "review_list"],
            ),
            (
                "write token of an editor",
                &editor_write,
                vec!["review_item", "review_keep", "review_list"],
            ),
        ] {
            let tools = listed_tools(&f, token).await?;
            let mut review: Vec<&str> = tools
                .iter()
                .filter_map(|t| t["name"].as_str())
                .filter(|n| n.starts_with("review"))
                .collect();
            review.sort_unstable();
            anyhow::ensure!(review == expected, "{who}: {review:?}");
            for tool in tools.iter().filter(|t| {
                t["name"]
                    .as_str()
                    .is_some_and(|n| n.starts_with("review") && n != "review_keep")
            }) {
                anyhow::ensure!(
                    tool["annotations"] == json!({ "readOnlyHint": true }),
                    "{who}: {tool}"
                );
            }
        }

        let tools = listed_tools(&f, &editor_write).await?;
        let find = |name: &str| {
            tools
                .iter()
                .find(|t| t["name"] == name)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("{name} is not listed"))
        };
        // `review_item`：`section` 可省，`id` 必填，而且不再说它是 uuid——类别词不是
        let item = find("review_item")?;
        let schema = &item["inputSchema"];
        anyhow::ensure!(schema["required"] == json!(["id"]), "{schema}");
        anyhow::ensure!(keys(&schema["properties"]) == ["id", "section"], "{schema}");
        anyhow::ensure!(
            schema["properties"]["id"]["type"] == "string"
                && schema["properties"]["id"].get("format").is_none(),
            "{schema}"
        );
        anyhow::ensure!(
            schema["properties"]["section"]["type"] == "string",
            "{schema}"
        );
        // `review_keep` 原样：一个 uuid 的 `id`，一句可省的理由
        let keep = find("review_keep")?;
        anyhow::ensure!(
            keep["inputSchema"]["required"] == json!(["id"])
                && keys(&keep["inputSchema"]["properties"]) == ["id", "rationale"]
                && keep["inputSchema"]["properties"]["id"]["format"] == "uuid",
            "{keep}"
        );
        // 两个读工具的说明写着十三档，不再说只有两档
        let list = find("review_list")?;
        let list_text = format!("{} {}", list["description"], list["inputSchema"]);
        let item_text = format!("{} {}", item["description"], item["inputSchema"]);
        for section in SECTIONS.iter().chain(["overview", "decisions"].iter()) {
            anyhow::ensure!(
                list_text.contains(&format!("\\\"{section}\\\"")),
                "review_list does not name {section}: {list_text}"
            );
            anyhow::ensure!(
                item_text.contains(&format!("\\\"{section}\\\"")),
                "review_item does not name {section}: {item_text}"
            );
        }
        for word in ["limit", "offset", "page", "per", "total", "thirteen"] {
            anyhow::ensure!(list_text.contains(word), "review_list: {word}");
        }
        for word in ["section", "id", "proof", "kind_word"] {
            anyhow::ensure!(item_text.contains(word), "review_item: {word}");
        }
        anyhow::ensure!(
            !list_text.contains("No other section is served"),
            "{list_text}"
        );

        // 读「等你点头」给不了令牌点头的办法：条目里没有能调的东西，工具里也没有第四个
        let pending = read(
            &f,
            &editor_write,
            "review_list",
            json!({ "section": "pending" }),
        )
        .await?;
        let first = identifier(&items(&pending)[0])?;
        for name in [
            "review_decide",
            "review_confirm",
            "review_merge",
            "review_pending",
        ] {
            let (status, body) = rpc(
                &f,
                f.kb,
                &editor_write,
                "tools/call",
                json!({ "name": name, "arguments": { "id": first } }),
            )
            .await?;
            anyhow::ensure!(status == StatusCode::OK, "{name}: {status}");
            anyhow::ensure!(
                body["error"]["code"] == -32601 && body.get("result").is_none(),
                "{name} must not exist: {body}"
            );
        }
        // `review_keep` 还是只分重复项：拿一条等人点头的事实去调，什么也没裁
        let result = tool(&f, &editor_write, "review_keep", json!({ "id": first })).await?;
        ensure_refused(&result, "not_found")?;
        let after = read(
            &f,
            &editor_write,
            "review_list",
            json!({ "section": "pending" }),
        )
        .await?;
        anyhow::ensure!(
            after == pending,
            "the pending section is unchanged\n{after}\n{pending}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}
