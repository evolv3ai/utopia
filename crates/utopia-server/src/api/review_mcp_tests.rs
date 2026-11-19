//! Review 的三个 MCP 工具（0076 决定 2）：`review_list`、`review_item`、`review_keep`。
//!
//! 要钉住的是**两个入口是同一个实现**：
//!
//! - 同一枚令牌，`review_list` 的 `structuredContent` 与令牌 API 的响应体相等（SC3）；
//!   `review_item` 回的就是列表里那一对的那个对象
//! - 页大小照会话路由的规矩夹到 1 到 200，越界不是错误（E6）；别的档是 `unknown_queue`（E7）；
//!   不是这个库里一对还在等的重复项，都是同一个 `not_found`，不带证据（E8）
//! - `kb_ids` 排除了这个库的令牌，从这两个读工具拿不到条目、计数或证据（N3）
//! - 分开一对：REST 与 MCP 各做一次，写下的东西一样（SC4）；被拒绝时两边同一个 code，
//!   而且写工具不列给被拒绝的调用方（SC5、E13）
//! - 同一对同时来两次，只落地一次（E10）；审计行写不进去，这一对就还在队列里（E11）
//! - 聊天助手拿不到任何 Review 工具
//!
//! 全部经路由进去（`POST /api/v1/kbs/{kb}/mcp` 与 REST 路由）。夹具在
//! `review_session_unchanged_tests.rs`。没有 `UTOPIA_DATABASE_URL` 时跳过。
use axum::http::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;

use super::review_session_unchanged_tests::Fx;

const DUPLICATES: &str = "review?queue=duplicates";

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
    anyhow::ensure!(status == StatusCode::OK, "{name}: {status} {body}");
    anyhow::ensure!(body.get("error").is_none(), "{name}: {body}");
    anyhow::ensure!(body["result"].is_object(), "{name}: {body}");
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

async fn listed_tools(f: &Fx, bearer: &str) -> anyhow::Result<Vec<Value>> {
    let (status, body) = rpc(f, f.kb, bearer, "tools/list", json!({})).await?;
    anyhow::ensure!(status == StatusCode::OK, "tools/list: {status} {body}");
    Ok(body["result"]["tools"]
        .as_array()
        .cloned()
        .unwrap_or_default())
}

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

/// Duplicates 这一档此刻的 id，经 MCP 读
async fn queue_ids(f: &Fx, bearer: &str) -> anyhow::Result<Vec<String>> {
    let page = read(f, bearer, "review_list", json!({ "section": "duplicates" })).await?;
    Ok(ids(&page))
}

async fn rest_keep(f: &Fx, bearer: &str, review: Uuid) -> anyhow::Result<(StatusCode, Value)> {
    f.call(
        bearer,
        "POST",
        &format!("/kbs/{}/review/{review}", f.kb),
        Some(json!({ "action": "keep", "rationale": "different people" })),
    )
    .await
}

async fn mcp_keep(f: &Fx, bearer: &str, review: Uuid) -> anyhow::Result<Value> {
    tool(
        f,
        bearer,
        "review_keep",
        json!({ "id": review, "rationale": "different people" }),
    )
    .await
}

/// 一个被拒绝的工具结果：`isError`，带这个 code，文字就是那句 message
fn ensure_refused(result: &Value, code: &str) -> anyhow::Result<()> {
    anyhow::ensure!(result["isError"] == true, "expected {code}: {result}");
    anyhow::ensure!(
        result["structuredContent"]["code"] == code,
        "expected {code}: {result}"
    );
    anyhow::ensure!(
        result["structuredContent"]["message"].is_string()
            && result["structuredContent"]["message"] == result["content"][0]["text"],
        "{result}"
    );
    Ok(())
}

/// 这一对在令牌 agent 台账里的行
async fn ledger_of(
    f: &Fx,
    review: Uuid,
) -> anyhow::Result<Vec<utopia_store::token_agent_actions::TokenAgentAction>> {
    Ok(
        utopia_store::token_agent_actions::list(&f.pool, f.kb, 500, 0)
            .await?
            .into_iter()
            .filter(|row| row.target_id == review)
            .collect(),
    )
}

/// 这个库的实体、事实、合并日志的全部行：分开一对不该动其中任何一行
async fn graph_rows(f: &Fx) -> anyhow::Result<Value> {
    let mut out = serde_json::Map::new();
    for table in ["facts", "entities", "entity_merges"] {
        // 表名是上面写死的三个，不是外面来的
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

#[tokio::test]
async fn review_list_returns_what_the_token_api_returns_for_duplicates_and_overview(
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

        let (status, rest) = f
            .call(&token, "GET", &format!("/kbs/{kb}/{DUPLICATES}"), None)
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {rest}");
        anyhow::ensure!(ids(&rest).len() == 4, "four pending pairs: {rest}");
        let mine = read(
            &f,
            &token,
            "review_list",
            json!({ "section": "duplicates" }),
        )
        .await?;
        anyhow::ensure!(mine == rest, "duplicates differ over MCP\n{mine}\n{rest}");
        anyhow::ensure!(mine["counts"]["duplicates"] == 4, "{mine}");
        anyhow::ensure!(mine["queue"] == "duplicates", "{mine}");

        // 翻页也是同一页
        let (_, rest) = f
            .call(
                &token,
                "GET",
                &format!("/kbs/{kb}/{DUPLICATES}&limit=2&offset=1"),
                None,
            )
            .await?;
        let mine = read(
            &f,
            &token,
            "review_list",
            json!({ "section": "duplicates", "limit": 2, "offset": 1 }),
        )
        .await?;
        anyhow::ensure!(ids(&mine).len() == 2 && mine == rest, "{mine}\n{rest}");

        let (status, rest) = f
            .call(&token, "GET", &format!("/kbs/{kb}/review/summary"), None)
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {rest}");
        let mine = read(&f, &token, "review_list", json!({ "section": "overview" })).await?;
        anyhow::ensure!(mine == rest, "overview differs over MCP\n{mine}\n{rest}");
        anyhow::ensure!(mine["waiting"].is_object(), "{mine}");
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn review_item_returns_the_evidence_on_the_pairs_card() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        let left = f.entity(f.person_type, "Zhang Wei").await?;
        let right = f.entity(f.company_type, "Zhang Wei").await?;
        let employer = f.entity(f.company_type, "Acme Ltd").await?;
        sqlx::query("UPDATE entities SET disambiguator = 'the engineer' WHERE id = $1")
            .bind(left)
            .execute(&f.pool)
            .await?;
        // 卡片上的事实摘要：左边有一条，右边没有
        sqlx::query(
            "INSERT INTO facts (id, kb_id, subject_id, object_id, layer, phrase)
             VALUES ($1, $2, $3, $4, 'open', 'works at')",
        )
        .bind(Uuid::now_v7())
        .bind(kb)
        .bind(left)
        .bind(employer)
        .execute(&f.pool)
        .await?;
        let pair = f.pair_of(left, right).await?;
        f.pair("Li Na", "Li Na").await?;
        // 这一对上开着一条治理建议
        utopia_store::governance::record(
            &f.pool,
            kb,
            utopia_store::governance::NewDecision {
                run_id: Uuid::now_v7(),
                target_id: pair,
                action: "merge",
                confidence: 0.75,
                reason: Some("same name"),
                precedents: json!([]),
                status: "proposed",
                merge_id: None,
                question: None,
                trace: json!([]),
                calls: 0,
            },
        )
        .await?;
        let (_, token) = f.token(f.viewer, "reader", "read", None).await?;

        let list = read(
            &f,
            &token,
            "review_list",
            json!({ "section": "duplicates" }),
        )
        .await?;
        let listed = list["items"]
            .as_array()
            .and_then(|a| a.iter().find(|i| i["id"] == pair.to_string()))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("the pair is not listed: {list}"))?;
        let item = read(&f, &token, "review_item", json!({ "id": pair })).await?;
        anyhow::ensure!(
            item == listed,
            "review_item must be the listed object\n{item}\n{listed}"
        );

        anyhow::ensure!(item["id"] == pair.to_string(), "{item}");
        anyhow::ensure!(item["score"] == 0.5, "{item}");
        anyhow::ensure!(item["reason"] == "namesake", "{item}");
        anyhow::ensure!(item["stage"] == "human", "{item}");
        anyhow::ensure!(item["left"]["name"] == "Zhang Wei", "{item}");
        anyhow::ensure!(item["left"]["type_label"] == "Person", "{item}");
        anyhow::ensure!(item["left"]["disambiguator"] == "the engineer", "{item}");
        anyhow::ensure!(item["left"]["degree"] == 1, "{item}");
        anyhow::ensure!(
            item["left"]["top_facts"] == json!(["works at → Acme Ltd"]),
            "{item}"
        );
        anyhow::ensure!(item["right"]["name"] == "Zhang Wei", "{item}");
        anyhow::ensure!(item["right"]["type_label"] == "Organization", "{item}");
        anyhow::ensure!(item["right"]["disambiguator"].is_null(), "{item}");
        anyhow::ensure!(item["right"]["degree"] == 0, "{item}");
        anyhow::ensure!(item["right"]["top_facts"] == json!([]), "{item}");
        anyhow::ensure!(item["proposal"]["action"] == "merge", "{item}");
        anyhow::ensure!(item["proposal"]["confidence"] == 0.75, "{item}");
        anyhow::ensure!(item["proposal"]["reason"] == "same name", "{item}");

        // 没有建议的一对：proposal 是 null，照样与列表里那一条相等
        let other = list["items"]
            .as_array()
            .and_then(|a| a.iter().find(|i| i["id"] != pair.to_string()))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("{list}"))?;
        let item = read(&f, &token, "review_item", json!({ "id": other["id"] })).await?;
        anyhow::ensure!(item == other && item["proposal"].is_null(), "{item}");
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn review_list_serves_a_page_size_below_1_as_1_and_above_200_as_200() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        // 205 对等人的重复项：一个共同的左边，205 个各不相同的右边
        let hub = f.entity(f.person_type, "Zhang Wei").await?;
        sqlx::query(
            "WITH e AS (
                 INSERT INTO entities (id, kb_id, type_id, canonical_name)
                 SELECT gen_random_uuid(), $1, $2, 'Zhang Wei ' || g
                   FROM generate_series(1, 205) g
                 RETURNING id)
             INSERT INTO resolution_reviews (id, kb_id, left_id, right_id, score, reason, stage)
             SELECT gen_random_uuid(), $1, $3, e.id, 0.5, 'namesake', 'human' FROM e",
        )
        .bind(kb)
        .bind(f.person_type)
        .bind(hub)
        .execute(&f.pool)
        .await?;
        let (_, token) = f.token(f.viewer, "reader", "read", None).await?;

        for (limit, served) in [
            (json!(0), 1usize),
            (json!(-5), 1),
            (json!(1), 1),
            (json!(200), 200),
            (json!(201), 200),
            (json!(1_000_000), 200),
        ] {
            let result = tool(
                &f,
                &token,
                "review_list",
                json!({ "section": "duplicates", "limit": limit }),
            )
            .await?;
            anyhow::ensure!(
                result["isError"] == false,
                "limit {limit} must not be a tool error: {result}"
            );
            let page = &result["structuredContent"];
            anyhow::ensure!(
                ids(page).len() == served,
                "limit {limit}: {} items, expected {served}",
                ids(page).len()
            );
            anyhow::ensure!(page["counts"]["duplicates"] == 205, "limit {limit}");
            // 与令牌 API 同一个请求是同一页
            let (status, rest) = f
                .call(
                    &token,
                    "GET",
                    &format!("/kbs/{kb}/{DUPLICATES}&limit={limit}"),
                    None,
                )
                .await?;
            anyhow::ensure!(status == StatusCode::OK, "limit {limit}: {status}");
            anyhow::ensure!(ids(page) == ids(&rest), "limit {limit}: pages differ");
        }
        // 不给就是缺省的 10；负的 offset 当 0
        let page = read(
            &f,
            &token,
            "review_list",
            json!({ "section": "duplicates", "offset": -3 }),
        )
        .await?;
        anyhow::ensure!(ids(&page).len() == 10, "{}", ids(&page).len());
        // 工具清单里的 schema 没有把越界的 limit 拦在客户端
        let tools = listed_tools(&f, &token).await?;
        let list = tools
            .iter()
            .find(|t| t["name"] == "review_list")
            .ok_or_else(|| anyhow::anyhow!("review_list is not listed"))?;
        let limit = &list["inputSchema"]["properties"]["limit"];
        anyhow::ensure!(limit["type"] == "integer", "{limit}");
        anyhow::ensure!(
            limit.get("minimum").is_none() && limit.get("maximum").is_none(),
            "{limit}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn review_list_refuses_mappings_and_an_unknown_section_with_unknown_queue(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        f.pair("Zhang Wei", "Zhang Wei").await?;
        let (_, read_token) = f.token(f.viewer, "reader", "read", None).await?;
        let (_, write_token) = f.token(f.editor, "writer", "write", None).await?;
        for section in ["mappings", "no-such-section"] {
            for token in [&read_token, &write_token] {
                let result = tool(&f, token, "review_list", json!({ "section": section })).await?;
                ensure_refused(&result, "unknown_queue")?;
                let content = &result["structuredContent"];
                anyhow::ensure!(
                    content.get("items").is_none()
                        && content.get("counts").is_none()
                        && content.get("queue").is_none(),
                    "{section}: {result}"
                );
                // 与令牌 API 同一个 code、同一句话
                let (status, rest) = f
                    .call(
                        token,
                        "GET",
                        &format!("/kbs/{kb}/review?queue={section}"),
                        None,
                    )
                    .await?;
                anyhow::ensure!(status == StatusCode::UNPROCESSABLE_ENTITY, "{status}");
                anyhow::ensure!(
                    rest["code"] == content["code"] && rest["error"] == content["message"],
                    "{section}: {rest} vs {content}"
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
async fn review_item_answers_one_not_found_for_an_unknown_id_and_another_bases_id(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let mine = f.pair("Zhang Wei", "Zhang Wei").await?;
        let decided = f.pair("Li Na", "Li Na").await?;
        // 另一个库里一对真的、还在等的重复项
        let (a, b, elsewhere) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
        sqlx::query(
            "INSERT INTO entities (id, kb_id, canonical_name)
             VALUES ($1, $3, 'Wang Fang'), ($2, $3, 'Wang Fang')",
        )
        .bind(a)
        .bind(b)
        .bind(f.other_kb)
        .execute(&f.pool)
        .await?;
        sqlx::query(
            "INSERT INTO resolution_reviews (id, kb_id, left_id, right_id, score, reason, stage)
             VALUES ($1, $2, $3, $4, 0.5, 'namesake', 'human')",
        )
        .bind(elsewhere)
        .bind(f.other_kb)
        .bind(a)
        .bind(b)
        .execute(&f.pool)
        .await?;
        // 令牌两个库都够得着：它被拒绝不是因为够不着那个库
        let (_, token) = f.token(f.viewer, "reader", "read", None).await?;
        let there = tool_on(
            &f,
            f.other_kb,
            &token,
            "review_item",
            json!({ "id": elsewhere }),
        )
        .await?;
        anyhow::ensure!(
            there["isError"] == false && there["structuredContent"]["id"] == elsewhere.to_string(),
            "the other base's pair is real: {there}"
        );
        let here = tool(&f, &token, "review_item", json!({ "id": mine })).await?;
        anyhow::ensure!(here["isError"] == false, "{here}");

        let unknown = tool(&f, &token, "review_item", json!({ "id": Uuid::now_v7() })).await?;
        let other_base = tool(&f, &token, "review_item", json!({ "id": elsewhere })).await?;
        ensure_refused(&unknown, "not_found")?;
        anyhow::ensure!(
            unknown == other_base,
            "an id of another base must read like an unknown id\n{unknown}\n{other_base}"
        );
        // 裁过的一对也不再是「还在等的重复项」：同一个回答
        let (status, _) = rest_keep(&f, &f.editor_session, decided).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status}");
        let gone = tool(&f, &token, "review_item", json!({ "id": decided })).await?;
        anyhow::ensure!(gone == unknown, "{gone}");
        for result in [&unknown, &other_base, &gone] {
            let content = &result["structuredContent"];
            for key in [
                "id",
                "left",
                "right",
                "score",
                "reason",
                "stage",
                "proposal",
                "top_facts",
            ] {
                anyhow::ensure!(content.get(key).is_none(), "{key} leaked: {result}");
            }
            anyhow::ensure!(
                !result.to_string().contains("Wang Fang"),
                "evidence leaked: {result}"
            );
        }
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_token_whose_kb_ids_exclude_the_base_reads_no_review_data_over_mcp() -> anyhow::Result<()>
{
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        let pair = f.pair("Zhang Wei", "Zhang Wei").await?;
        // 同一个人的两枚令牌：一枚够得着这个库，一枚只列了另一个库
        let (_, reaches) = f.token(f.viewer, "reaches", "read", Some(&[kb])).await?;
        let (_, excluded) = f
            .token(f.viewer, "excluded", "read", Some(&[f.other_kb]))
            .await?;
        let (_, excluded_write) = f
            .token(f.editor, "excluded write", "write", Some(&[f.other_kb]))
            .await?;
        let calls = [
            ("review_list", json!({ "section": "duplicates" })),
            ("review_list", json!({ "section": "overview" })),
            ("review_item", json!({ "id": pair })),
        ];
        for (name, args) in &calls {
            // 对照：够得着的那一枚读得到
            let ok = tool(&f, &reaches, name, args.clone()).await?;
            anyhow::ensure!(ok["isError"] == false, "{name} {args}: {ok}");

            for token in [&excluded, &excluded_write] {
                let (status, body) = rpc(
                    &f,
                    kb,
                    token,
                    "tools/call",
                    json!({ "name": name, "arguments": args }),
                )
                .await?;
                anyhow::ensure!(status != StatusCode::OK, "{name} {args}: {status} {body}");
                anyhow::ensure!(body.get("result").is_none(), "{name} {args}: {body}");
                let text = body.to_string();
                for leaked in [
                    "items",
                    "counts",
                    "waiting",
                    "decided",
                    "health",
                    "top_facts",
                    "Zhang Wei",
                    &pair.to_string(),
                ] {
                    anyhow::ensure!(!text.contains(leaked), "{name} {args}: {leaked} in {body}");
                }
            }
        }
        // 清单同样拿不到
        for token in [&excluded, &excluded_write] {
            let (status, body) = rpc(&f, kb, token, "tools/list", json!({})).await?;
            anyhow::ensure!(
                status != StatusCode::OK && body.get("result").is_none(),
                "{status} {body}"
            );
        }
        // 写令牌也分不开它够不着的库里的一对
        let (status, body) = rpc(
            &f,
            kb,
            &excluded_write,
            "tools/call",
            json!({ "name": "review_keep", "arguments": { "id": pair } }),
        )
        .await?;
        anyhow::ensure!(
            status != StatusCode::OK && body.get("result").is_none(),
            "{status} {body}"
        );
        anyhow::ensure!(f.review_row(pair).await?.0 == "pending");
        anyhow::ensure!(ledger_of(&f, pair).await?.is_empty());
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_write_token_of_an_editor_keeps_a_pair_over_the_api_and_over_mcp() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        let left = f.entity(f.person_type, "Zhang Wei").await?;
        let right = f.entity(f.person_type, "Zhang Wei").await?;
        let over_api = f.pair_of(left, right).await?;
        let over_mcp = f.pair("Li Na", "Li Na").await?;
        let stays = f.pair("Wang Fang", "Wang Fang").await?;
        // 一条事实、一笔人做的合并：三张表都有行，前后相等才说明问题
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
        // 主人正好是 Editor：写的下限
        let (token_id, token) = f.token(f.editor, "cleanup agent", "write", None).await?;

        let before = graph_rows(&f).await?;
        for table in ["facts", "entities", "entity_merges"] {
            anyhow::ensure!(
                before[table].as_array().is_some_and(|a| !a.is_empty()),
                "{table} must have rows for the comparison to mean anything"
            );
        }
        anyhow::ensure!(
            queue_ids(&f, &token).await?.len() == 3,
            "three pending pairs"
        );

        // 一对经令牌 API
        let (status, body) = rest_keep(&f, &token, over_api).await?;
        anyhow::ensure!(status == StatusCode::OK, "api keep: {status} {body}");
        anyhow::ensure!(body["ok"] == true && body["outcome"] == "applied", "{body}");
        let api_action = body["action_id"].clone();
        anyhow::ensure!(
            queue_ids(&f, &token).await? == [stays.to_string(), over_mcp.to_string()],
            "the pair kept over the API must leave the queue"
        );
        anyhow::ensure!(
            graph_rows(&f).await? == before,
            "the API keep changed the graph"
        );

        // 另一对经 MCP
        let result = mcp_keep(&f, &token, over_mcp).await?;
        anyhow::ensure!(result["isError"] == false, "mcp keep: {result}");
        let content = &result["structuredContent"];
        anyhow::ensure!(
            content["ok"] == true && content["outcome"] == "applied",
            "{result}"
        );
        let mcp_action = content["action_id"].clone();
        anyhow::ensure!(
            *content == json!({ "ok": true, "outcome": "applied", "action_id": mcp_action }),
            "the MCP keep answers what the API answers: {content}"
        );
        anyhow::ensure!(
            queue_ids(&f, &token).await? == [stays.to_string()],
            "the pair kept over MCP must leave the queue"
        );
        let after = graph_rows(&f).await?;
        for table in ["facts", "entities", "entity_merges"] {
            anyhow::ensure!(
                before[table] == after[table],
                "{table} changed under a keep"
            );
        }

        // 两对各自：一行台账、一行带令牌的审计，写下的东西一样
        for (pair, action_id, names) in [
            (over_api, &api_action, "Zhang Wei"),
            (over_mcp, &mcp_action, "Li Na"),
        ] {
            anyhow::ensure!(
                f.review_row(pair).await? == ("kept".to_string(), Some(f.editor), Some(token_id)),
                "decided by the owner, through the token"
            );
            let rows = ledger_of(&f, pair).await?;
            anyhow::ensure!(rows.len() == 1, "one ledger row: {rows:?}");
            let row = &rows[0];
            anyhow::ensure!(
                json!(row.id) == *action_id,
                "action_id names the ledger row"
            );
            anyhow::ensure!(row.outcome == "applied" && row.operation == "review.keep");
            anyhow::ensure!(row.owner_id == f.editor && row.token_id == token_id);
            anyhow::ensure!(row.token_name == "cleanup agent", "{}", row.token_name);

            let audits = f.audits("review.keep", pair).await?;
            anyhow::ensure!(audits.len() == 1, "one audit row: {audits:?}");
            anyhow::ensure!(audits[0].0 == Some(f.editor), "the owner is the actor");
            anyhow::ensure!(
                audits[0].1
                    == json!({ "left": names, "right": names, "score": 0.5,
                               "why": "different people", "token_id": token_id,
                               "token_name": "cleanup agent" }),
                "{}",
                audits[0].1
            );
        }
        // MCP 那一次调用另有它每次都记的那一行，也只有一行
        let called: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM audit_events
              WHERE kb_id = $1 AND action = 'mcp.tool_called' AND target_id = $2
                AND detail->>'tool' = 'review_keep'",
        )
        .bind(kb)
        .bind(token_id)
        .fetch_one(&f.pool)
        .await?;
        anyhow::ensure!(
            called == 1,
            "one mcp.tool_called row for the keep, got {called}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_refused_keep_carries_the_same_code_over_the_api_and_over_mcp() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let pair = f.pair("Zhang Wei", "Zhang Wei").await?;
        let second = f.pair("Li Na", "Li Na").await?;
        let untouched = ("pending".to_string(), None, None);

        let (_, read_only) = f.token(f.editor, "reader", "read", None).await?;
        let (_, viewer_write) = f.token(f.viewer, "viewer writer", "write", None).await?;
        for (token, code) in [
            (&read_only, "token_read_only"),
            (&viewer_write, "token_owner_below_editor"),
        ] {
            let (status, rest) = rest_keep(&f, token, pair).await?;
            anyhow::ensure!(status == StatusCode::FORBIDDEN, "{code}: {status} {rest}");
            anyhow::ensure!(rest["code"] == code, "expected {code}: {rest}");

            let result = mcp_keep(&f, token, pair).await?;
            ensure_refused(&result, code)?;
            anyhow::ensure!(
                result["structuredContent"]
                    == json!({ "code": rest["code"], "message": rest["error"] }),
                "{code}: the tool error must carry the API's code and message\n{result}\n{rest}"
            );
            anyhow::ensure!(
                f.review_row(pair).await? == untouched,
                "{code}: the pair must stay pending"
            );
            anyhow::ensure!(
                ledger_of(&f, pair).await?.is_empty(),
                "{code}: no ledger row"
            );
            anyhow::ensure!(f.audits("review.keep", pair).await?.is_empty(), "{code}");
        }

        // 落地一次，再来一次就是 already_decided——哪个入口先、哪个入口后都一样
        let (first_id, first) = f.token(f.editor, "first", "write", None).await?;
        let (_, other) = f.token(f.editor, "second", "write", None).await?;
        let (status, body) = rest_keep(&f, &first, pair).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let kept = mcp_keep(&f, &first, second).await?;
        anyhow::ensure!(kept["isError"] == false, "{kept}");
        let decided = ("kept".to_string(), Some(f.editor), Some(first_id));
        for decided_pair in [pair, second] {
            for token in [&first, &other] {
                let (status, rest) = rest_keep(&f, token, decided_pair).await?;
                anyhow::ensure!(status == StatusCode::CONFLICT, "repeat: {status} {rest}");
                anyhow::ensure!(rest["code"] == "already_decided", "repeat: {rest}");

                let result = mcp_keep(&f, token, decided_pair).await?;
                ensure_refused(&result, "already_decided")?;
                anyhow::ensure!(
                    result["structuredContent"]
                        == json!({ "code": rest["code"], "message": rest["error"] }),
                    "{result}\n{rest}"
                );
            }
            anyhow::ensure!(
                f.review_row(decided_pair).await? == decided,
                "a repeat must not change who decided"
            );
            anyhow::ensure!(
                ledger_of(&f, decided_pair).await?.len() == 1,
                "a repeat writes no ledger row"
            );
            anyhow::ensure!(f.audits("review.keep", decided_pair).await?.len() == 1);
        }
        // 这个库里没有的 id：not_found，不是 already_decided
        let result = mcp_keep(&f, &first, Uuid::now_v7()).await?;
        ensure_refused(&result, "not_found")?;
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn review_keep_is_listed_only_to_a_write_token_of_an_editor() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let (_, read_only) = f.token(f.editor, "reader", "read", None).await?;
        let (_, viewer_write) = f.token(f.viewer, "viewer writer", "write", None).await?;
        let (_, editor_write) = f.token(f.editor, "writer", "write", None).await?;
        for (who, token, writes) in [
            ("read token of an editor", &read_only, false),
            ("write token of a viewer", &viewer_write, false),
            ("write token of an editor", &editor_write, true),
        ] {
            let tools = listed_tools(&f, token).await?;
            let find = |name: &str| tools.iter().find(|t| t["name"] == name);
            for name in ["review_list", "review_item"] {
                let tool = find(name).ok_or_else(|| anyhow::anyhow!("{who}: {name} missing"))?;
                anyhow::ensure!(
                    tool["annotations"] == json!({ "readOnlyHint": true }),
                    "{who}: {tool}"
                );
                anyhow::ensure!(
                    tool["inputSchema"]["type"] == "object"
                        && tool["description"].as_str().is_some_and(|d| !d.is_empty()),
                    "{who}: {tool}"
                );
            }
            anyhow::ensure!(
                find("review_keep").is_some() == writes,
                "{who}: review_keep listed = {}",
                find("review_keep").is_some()
            );
            // `remember` 的列法没变：同一条规矩
            anyhow::ensure!(find("remember").is_some() == writes, "{who}: remember");
            anyhow::ensure!(find("search_chunks").is_some(), "{who}: search_chunks");
            // 每个工具只列一次
            let mut names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
            let total = names.len();
            names.sort_unstable();
            names.dedup();
            anyhow::ensure!(names.len() == total, "{who}: a tool is listed twice");
        }
        let tools = listed_tools(&f, &editor_write).await?;
        let keep = tools
            .iter()
            .find(|t| t["name"] == "review_keep")
            .ok_or_else(|| anyhow::anyhow!("review_keep missing"))?;
        anyhow::ensure!(
            keep["annotations"]
                == json!({ "readOnlyHint": false, "destructiveHint": false,
                           "idempotentHint": false }),
            "{keep}"
        );
        anyhow::ensure!(keep["inputSchema"]["required"] == json!(["id"]), "{keep}");

        // 没列出来的 `remember` 照旧是 -32601，不是工具错误
        let (status, body) = rpc(
            &f,
            f.kb,
            &read_only,
            "tools/call",
            json!({ "name": "remember", "arguments": { "text": "Zhang Wei joined Acme" } }),
        )
        .await?;
        anyhow::ensure!(status == StatusCode::OK, "{status}");
        anyhow::ensure!(
            body["error"]["code"] == -32601 && body.get("result").is_none(),
            "{body}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_keeps_of_one_pair_at_once_apply_exactly_one() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let (_, rest_token) = f.token(f.editor, "rest agent", "write", None).await?;
        let (_, mcp_token) = f.token(f.editor, "mcp agent", "write", None).await?;
        let names = [
            "Zhang Wei",
            "Li Na",
            "Wang Fang",
            "Liu Yang",
            "Chen Jie",
            "Zhao Lei",
            "Sun Li",
            "Zhou Min",
        ];
        let mut winners = (0, 0);
        for (round, name) in names.iter().enumerate() {
            let pair = f.pair(name, name).await?;
            // 两个入口同时去分同一对；先后顺序每轮换一次
            let (rest, mcp) = if round % 2 == 0 {
                tokio::join!(
                    rest_keep(&f, &rest_token, pair),
                    mcp_keep(&f, &mcp_token, pair)
                )
            } else {
                let (mcp, rest) = tokio::join!(
                    mcp_keep(&f, &mcp_token, pair),
                    rest_keep(&f, &rest_token, pair)
                );
                (rest, mcp)
            };
            let (status, rest) = rest?;
            let mcp = mcp?;

            let rest_applied = status == StatusCode::OK;
            if rest_applied {
                anyhow::ensure!(rest["outcome"] == "applied", "{name}: {rest}");
            } else {
                anyhow::ensure!(
                    status == StatusCode::CONFLICT && rest["code"] == "already_decided",
                    "{name}: the losing API call answers already_decided: {status} {rest}"
                );
            }
            let mcp_applied = mcp["isError"] == false;
            if mcp_applied {
                anyhow::ensure!(mcp["structuredContent"]["outcome"] == "applied", "{mcp}");
            } else {
                ensure_refused(&mcp, "already_decided")?;
            }
            anyhow::ensure!(
                rest_applied != mcp_applied,
                "{name}: exactly one keep applies (api {rest_applied}, mcp {mcp_applied})"
            );
            if rest_applied {
                winners.0 += 1;
            } else {
                winners.1 += 1;
            }

            let rows = ledger_of(&f, pair).await?;
            anyhow::ensure!(
                rows.len() == 1 && rows[0].outcome == "applied",
                "{name}: exactly one applied ledger row: {rows:?}"
            );
            let winner = if rest_applied {
                "rest agent"
            } else {
                "mcp agent"
            };
            anyhow::ensure!(
                rows[0].token_name == winner,
                "{name}: {}",
                rows[0].token_name
            );
            anyhow::ensure!(
                f.audits("review.keep", pair).await?.len() == 1,
                "{name}: exactly one review.keep audit row"
            );
            anyhow::ensure!(f.review_row(pair).await?.0 == "kept", "{name}");
        }
        anyhow::ensure!(winners.0 + winners.1 == names.len(), "{winners:?}");

        // 同一个入口的两次调用也一样
        let pair = f.pair("Wu Qiang", "Wu Qiang").await?;
        let (a, b) = tokio::join!(
            mcp_keep(&f, &mcp_token, pair),
            mcp_keep(&f, &rest_token, pair)
        );
        let (a, b) = (a?, b?);
        anyhow::ensure!(
            (a["isError"] == false) != (b["isError"] == false),
            "exactly one MCP keep applies\n{a}\n{b}"
        );
        ensure_refused(
            if a["isError"] == true { &a } else { &b },
            "already_decided",
        )?;
        let rows = ledger_of(&f, pair).await?;
        anyhow::ensure!(rows.len() == 1 && rows[0].outcome == "applied", "{rows:?}");
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_keep_whose_audit_row_fails_leaves_the_pair_in_the_queue() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    // 让这个库的 `review.keep` 审计行写不进去：一个只认这个库的触发器。名字带上这个库的
    // id，别的测试、别的库不受影响；下面不论成败都把它拆掉
    let name = format!("t005_refuse_keep_audit_{}", f.kb.simple());
    let drop_sql =
        format!("DROP TRIGGER IF EXISTS {name} ON audit_events; DROP FUNCTION IF EXISTS {name}();");
    let result = async {
        let kb = f.kb;
        let pair = f.pair("Zhang Wei", "Zhang Wei").await?;
        let other = f.pair("Li Na", "Li Na").await?;
        let (_, token) = f.token(f.editor, "cleanup agent", "write", None).await?;
        // 插进去的只有本地生成的名字和这个库的 UUID
        sqlx::raw_sql(&format!(
            "CREATE FUNCTION {name}() RETURNS trigger LANGUAGE plpgsql AS $fn$
             BEGIN
                 RAISE EXCEPTION 'audit write refused for this test';
             END $fn$;
             CREATE TRIGGER {name} BEFORE INSERT ON audit_events
                 FOR EACH ROW
                 WHEN (NEW.kb_id = '{kb}' AND NEW.action = 'review.keep')
                 EXECUTE FUNCTION {name}();"
        ))
        .execute(&f.pool)
        .await?;

        let both = [other.to_string(), pair.to_string()];

        // 经 MCP：工具错误，不带内部细节
        let result = mcp_keep(&f, &token, pair).await?;
        anyhow::ensure!(result["isError"] == true, "the keep must fail: {result}");
        anyhow::ensure!(
            result["structuredContent"].get("ok").is_none()
                && result["structuredContent"].get("action_id").is_none(),
            "{result}"
        );
        anyhow::ensure!(
            !result.to_string().contains("audit write refused"),
            "the database's message must stay in the log: {result}"
        );
        anyhow::ensure!(
            queue_ids(&f, &token).await? == both,
            "the pair must stay listed"
        );
        anyhow::ensure!(
            f.review_row(pair).await? == ("pending".to_string(), None, None),
            "the review row must roll back"
        );
        anyhow::ensure!(ledger_of(&f, pair).await?.is_empty(), "no ledger row");
        anyhow::ensure!(f.audits("review.keep", pair).await?.is_empty());

        // 经令牌 API：同一条路，同一个结果
        let (status, body) = rest_keep(&f, &token, pair).await?;
        anyhow::ensure!(
            status == StatusCode::INTERNAL_SERVER_ERROR,
            "api keep: {status} {body}"
        );
        anyhow::ensure!(
            queue_ids(&f, &token).await? == both,
            "the pair must stay listed"
        );
        anyhow::ensure!(ledger_of(&f, pair).await?.is_empty(), "no ledger row");

        // 拆掉之后同一个调用落地：上面失败的就是那一行审计，不是别的
        sqlx::raw_sql(&drop_sql).execute(&f.pool).await?;
        let result = mcp_keep(&f, &token, pair).await?;
        anyhow::ensure!(result["isError"] == false, "{result}");
        anyhow::ensure!(queue_ids(&f, &token).await? == [other.to_string()]);
        anyhow::ensure!(ledger_of(&f, pair).await?.len() == 1);
        anyhow::ensure!(f.audits("review.keep", pair).await?.len() == 1);
        Ok(())
    }
    .await;
    let dropped = sqlx::raw_sql(&drop_sql).execute(&f.pool).await;
    f.cleanup().await?;
    dropped?;
    result
}

#[tokio::test]
async fn a_read_token_of_a_viewer_is_refused_with_token_read_only() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let pair = f.pair("Zhang Wei", "Zhang Wei").await?;
        // 只读，主人又是 Viewer：两条都不够，答的是先查的那一条
        let (_, token) = f.token(f.viewer, "viewer reader", "read", None).await?;

        let (status, rest) = rest_keep(&f, &token, pair).await?;
        anyhow::ensure!(status == StatusCode::FORBIDDEN, "{status} {rest}");
        anyhow::ensure!(rest["code"] == "token_read_only", "{rest}");

        let result = mcp_keep(&f, &token, pair).await?;
        ensure_refused(&result, "token_read_only")?;
        anyhow::ensure!(
            result["structuredContent"]
                == json!({ "code": rest["code"], "message": rest["error"] }),
            "{result}\n{rest}"
        );
        anyhow::ensure!(f.review_row(pair).await? == ("pending".to_string(), None, None));
        anyhow::ensure!(ledger_of(&f, pair).await?.is_empty());
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

/// 应用内的聊天助手与 MCP 共用 `chat::tools_schema`。Review 的工具不在那一份里：
/// 助手替登录的人说话，不该拿到一个能替他裁决的工具
#[test]
fn the_chat_assistant_is_offered_no_review_tool() {
    let sources = ["warehouse".to_string()];
    for (who, schema) in [
        ("a writer", super::chat::tools_schema(true, &[])),
        ("a reader", super::chat::tools_schema(false, &[])),
        (
            "a writer with a data source",
            super::chat::tools_schema(true, &sources),
        ),
    ] {
        let names: Vec<&str> = schema
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|t| t["function"]["name"].as_str())
            .collect();
        assert!(names.contains(&"search_chunks"), "{who}: {names:?}");
        for review in ["review_list", "review_item", "review_keep"] {
            assert!(!names.contains(&review), "{who} is offered {review}");
        }
        assert!(
            !names.iter().any(|n| n.starts_with("review")),
            "{who}: {names:?}"
        );
    }
}
