//! 令牌读还开着的本体提案、提议拒绝或采纳其中一条：只留提议，人答了才算（0076）。
//!
//! 照的是 `review_token_errata_tests`。本体提案不在 Review 的列表里：它有自己的三条路由
//! （读、拒绝、采纳），按 section 与 key 认。要钉住的行为：
//!
//! - **读**：够得着这个库的令牌（主人至少是 Viewer）从列表路由、从 `review_ontology_list`
//!   拿到的就是登录的人拿到的那一份；够不着的令牌是 404。一条提案上有开着的令牌提议时，
//!   那一项多一个 `agent_proposals`，别的项一个键也不多
//! - **令牌的调用什么都不改**（SC1 / E1）：拒绝、采纳都回 200、`proposed`、`propose_only`；
//!   提案行、本体、这个库的每一条事实与实体、排着的任务数前后相等。一条本体提案上只有
//!   一条开着的提议，拒绝还是采纳都算它
//! - **目标与请求体照人的路由校验**（E3 / E4）：没有存着的提案是 404，定过的是
//!   `already_decided`，拒绝路由仍然答 `adoption_endpoint`；写的检查排在前面
//! - **人批准之后才落地，而且是这个人的决定**（SC2 / E7 / N2）：拒绝同这个人自己拒的一样，
//!   多一条 `ontology_proposal.rejected` 审计行；采纳跑的是 `ontology_agent::adopt`，
//!   `ontology_proposal.adopted` 那一行多指着提议。actor 都是这个人，不带 `token_id`
//! - **过时**（E8）：提案在等的时候被定了、或者 payload 被代理刷成了别的样子——批准这样
//!   的提议什么都不改，答 `superseded`
//! - **人自己的拒绝一个字没变**：仍然不写审计行，没有这条提案也仍然答 ok
//!
//! 提案直接种进 `ontology_proposals`，不跑本体代理的模型任务。
//!
//! 夹具在 `review_token_write_support.rs`。没有 `UTOPIA_DATABASE_URL` 时跳过。
use axum::http::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;

use super::review_token_fact_tests::{audits_of, ledger_row, proposed};
use super::review_token_write_support::{
    answer, audits, ensure_tool_refused, keys, ledger, open_proposals, post, row, snapshot,
    tool_call, tools_list, Fx,
};

const TOKEN_NAME: &str = "ontology agent's helper";

/// 一条提案的 payload：一个新类
fn class(key: &str, parents: &[&str]) -> Value {
    json!({ "key": key, "label": format!("The {key}"), "description": format!("A {key}."),
            "parents": parents })
}

/// 一条提案直接种进 `ontology_proposals`。`by` 是 agent（本体代理提的）或 suggest（退役的
/// 一次性建议留下的）。回那一行的 id
async fn seed(
    f: &Fx,
    kb: Uuid,
    section: &str,
    key: &str,
    payload: Value,
    by: &str,
) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO ontology_proposals (id, kb_id, section, key, payload, proposed_by)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(kb)
    .bind(section)
    .bind(key)
    .bind(payload)
    .bind(by)
    .execute(&f.pool)
    .await?;
    Ok(id)
}

/// 一个新类的提案，本体代理提的
async fn seed_class(f: &Fx, key: &str) -> anyhow::Result<Uuid> {
    seed(f, f.kb, "entity_types", key, class(key, &[]), "agent").await
}

/// 读还开着的提案：人的那条路由，令牌与会话都从这里进
async fn read(f: &Fx, bearer: &str) -> anyhow::Result<(StatusCode, Value)> {
    f.call(
        bearer,
        "GET",
        &format!("/kbs/{}/ontology/proposals", f.kb),
        None,
    )
    .await
}

/// 拒绝的那条路由
async fn reject(f: &Fx, bearer: &str, body: Value) -> anyhow::Result<(StatusCode, Value)> {
    post(f, bearer, "ontology/proposals", body).await
}

/// 采纳的那条路由
async fn adopt(f: &Fx, bearer: &str, body: Value) -> anyhow::Result<(StatusCode, Value)> {
    post(f, bearer, "ontology/proposals/adopt", body).await
}

fn rejecting(key: &str) -> Value {
    json!({ "section": "entity_types", "key": key, "status": "rejected" })
}

fn adopting(key: &str) -> Value {
    json!({ "section": "entity_types", "key": key })
}

/// 本体与提案此刻的样子：这个库的每一个类、每一条属性、每一条父边、每一条提案的每一列
/// （向量两列不比：测试里没有嵌入模型，它们恒为空）
async fn ontology(f: &Fx) -> anyhow::Result<Value> {
    Ok(sqlx::query_scalar(
        "SELECT jsonb_build_object(
            'classes', (SELECT coalesce(jsonb_agg(to_jsonb(t) - 'embedding' - 'label_embedding'
                                                  ORDER BY t.id), '[]'::jsonb)
                          FROM entity_types t WHERE t.kb_id = $1),
            'properties', (SELECT coalesce(jsonb_agg(to_jsonb(r) - 'embedding' - 'label_embedding'
                                                     ORDER BY r.id), '[]'::jsonb)
                             FROM relation_types r WHERE r.kb_id = $1),
            'parents', (SELECT count(*) FROM entity_type_parents p
                          JOIN entity_types t ON t.id = p.child_id WHERE t.kb_id = $1),
            'proposals', (SELECT coalesce(jsonb_agg(to_jsonb(o) ORDER BY o.id), '[]'::jsonb)
                            FROM ontology_proposals o WHERE o.kb_id = $1))",
    )
    .bind(f.kb)
    .fetch_one(&f.pool)
    .await?)
}

/// 这个键的类：除了 id、键、名字、颜色与时刻之外的每一列，加上它有几条父边。没有就是 null
async fn class_of(f: &Fx, key: &str) -> anyhow::Result<Value> {
    let found: Option<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(t) - 'id' - 'key' - 'label' - 'description' - 'color' - 'created_at'
                            - 'updated_at' - 'embedding' - 'label_embedding'
                || jsonb_build_object('parents',
                       (SELECT count(*) FROM entity_type_parents p WHERE p.child_id = t.id))
           FROM entity_types t WHERE t.kb_id = $1 AND t.key = $2",
    )
    .bind(f.kb)
    .bind(key)
    .fetch_optional(&f.pool)
    .await?;
    Ok(found.unwrap_or(Value::Null))
}

/// 提案行里人的决定：状态、谁定的、理由、改没改过
fn decision_of(proposal: &Value) -> Value {
    json!([
        proposal["status"],
        proposal["decided_by"],
        proposal["reason"],
        proposal["edited"]
    ])
}

/// 这个库排着的任务，按种类数
async fn queued(f: &Fx) -> anyhow::Result<Value> {
    Ok(sqlx::query_scalar(
        "SELECT coalesce(jsonb_object_agg(kind, n), '{}'::jsonb)
           FROM (SELECT kind, count(*) AS n FROM jobs
                  WHERE status = 'queued' AND payload ->> 'kb_id' = $1::text GROUP BY kind) k",
    )
    .bind(f.kb)
    .fetch_one(&f.pool)
    .await?)
}

/// 采纳排下的任务没有 worker 来领：测试完了删掉，不留给别的测试
async fn cleanup(f: Fx) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM jobs WHERE payload ->> 'kb_id' = ANY($1)")
        .bind([f.kb.to_string(), f.other_kb.to_string()])
        .execute(&f.pool)
        .await?;
    f.cleanup().await
}

/// 列表里这个键的那一项
fn listed<'a>(body: &'a Value, section: &str, key: &str) -> anyhow::Result<&'a Value> {
    body[section]
        .as_array()
        .into_iter()
        .flatten()
        .find(|item| item["key"] == key)
        .ok_or_else(|| anyhow::anyhow!("{section}/{key} is not listed in {body}"))
}

#[tokio::test]
async fn a_token_reads_the_open_ontology_proposals_a_person_sees() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let (_, viewer_read) = f.token(f.viewer, "reader", "read", None).await?;
        let (_, editor_write) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let here = [f.kb];
        let (_, scoped) = f.token(f.viewer, "scoped", "read", Some(&here)).await?;
        let elsewhere = [f.other_kb];
        let (_, out_of_scope) = f
            .token(f.editor, "elsewhere", "read", Some(&elsewhere))
            .await?;
        seed_class(&f, "city").await?;
        seed_class(&f, "harbour").await?;
        seed(
            &f,
            f.kb,
            "attribute_types",
            "founding_note",
            json!({ "key": "founding_note", "label": "founding note", "datatype": "text",
                    "domains": ["org"] }),
            "agent",
        )
        .await?;
        // 不列的三种：退役的建议留下的、已经拒掉的、别的库的
        seed(
            &f,
            f.kb,
            "entity_types",
            "relic",
            class("relic", &[]),
            "suggest",
        )
        .await?;
        let gone = seed_class(&f, "gone").await?;
        sqlx::query("UPDATE ontology_proposals SET status = 'rejected' WHERE id = $1")
            .bind(gone)
            .execute(&f.pool)
            .await?;
        seed(
            &f,
            f.other_kb,
            "entity_types",
            "abroad",
            class("abroad", &[]),
            "agent",
        )
        .await?;

        let (status, by_person) = read(&f, &f.editor_session).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {by_person}");
        anyhow::ensure!(
            keys(&by_person)
                == [
                    "attribute_types",
                    "entity_types",
                    "map_to",
                    "relation_types"
                ],
            "{by_person}"
        );
        let mut classes: Vec<&str> = by_person["entity_types"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|item| item["key"].as_str())
            .collect();
        classes.sort_unstable();
        anyhow::ensure!(classes == ["city", "harbour"], "{by_person}");
        let city = listed(&by_person, "entity_types", "city")?.clone();
        anyhow::ensure!(
            keys(&city)
                == [
                    "description",
                    "key",
                    "label",
                    "parents",
                    "proposed_by",
                    "serves",
                    "signatures"
                ],
            "an item without a token proposal has the keys it always had: {city}"
        );

        // 够得着这个库的令牌——只读的、Viewer 的、限定到这个库的、写的——拿到的是同一份
        for (who, bearer) in [
            ("a viewer's session", &f.viewer_session),
            ("a read token of a viewer", &viewer_read),
            ("a read token scoped to this base", &scoped),
            ("a write token of an editor", &editor_write),
        ] {
            let (status, body) = read(&f, bearer).await?;
            anyhow::ensure!(
                status == StatusCode::OK && body == by_person,
                "{who}: {status}\n{body}\n{by_person}"
            );
        }
        let by_tool = tool_call(&f, &viewer_read, "review_ontology_list", json!({})).await?;
        anyhow::ensure!(
            by_tool.get("isError").is_none_or(|e| *e == false)
                && by_tool["structuredContent"] == by_person,
            "{by_tool}"
        );
        // 够不着的令牌：404，与不存在的库同一个回答；工具同样读不到
        let (status, body) = read(&f, &out_of_scope).await?;
        anyhow::ensure!(status == StatusCode::NOT_FOUND, "{status} {body}");
        let (status, body) = f
            .call(
                &viewer_read,
                "GET",
                &format!("/kbs/{}/ontology/proposals", Uuid::now_v7()),
                None,
            )
            .await?;
        anyhow::ensure!(status == StatusCode::NOT_FOUND, "{status} {body}");

        // 令牌在一条提案上留了提议：那一项多一个 `agent_proposals`，别的项原样
        let (status, body) = reject(&f, &editor_write, rejecting("city")).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let proposal = proposed(&body)?;
        let (status, after) = read(&f, &f.editor_session).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {after}");
        let marked = listed(&after, "entity_types", "city")?;
        let open = &marked["agent_proposals"];
        anyhow::ensure!(
            open.as_array().is_some_and(|o| o.len() == 1)
                && keys(&open[0])
                    == [
                        "action",
                        "created_at",
                        "id",
                        "operation",
                        "owner_name",
                        "reason",
                        "token_name"
                    ]
                && open[0]["id"] == json!(proposal)
                && open[0]["operation"] == "ontology.proposal"
                && open[0]["action"] == "reject"
                && open[0]["reason"] == "propose_only"
                && open[0]["owner_name"] == "Editor"
                && open[0]["token_name"] == TOKEN_NAME,
            "{marked}"
        );
        let mut unmarked = marked.clone();
        if let Some(fields) = unmarked.as_object_mut() {
            fields.remove("agent_proposals");
        }
        anyhow::ensure!(unmarked == city, "only that key is added: {marked}");
        for (section, key) in [
            ("entity_types", "harbour"),
            ("attribute_types", "founding_note"),
        ] {
            anyhow::ensure!(
                listed(&after, section, key)? == listed(&by_person, section, key)?,
                "an item without a proposal is unchanged"
            );
        }
        let (status, by_token) = read(&f, &viewer_read).await?;
        let by_tool = tool_call(&f, &viewer_read, "review_ontology_list", json!({})).await?;
        anyhow::ensure!(
            status == StatusCode::OK && by_token == after && by_tool["structuredContent"] == after,
            "every reader sees the open proposal\n{by_token}\n{by_tool}"
        );
        // 答掉之后那个键就没了
        let (status, body) = answer(&f, &f.editor_session, proposal, "decline").await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let (_, closed) = read(&f, &viewer_read).await?;
        anyhow::ensure!(closed == by_person, "{closed}");
        Ok(())
    }
    .await;
    cleanup(f).await?;
    result
}

#[tokio::test]
async fn a_token_ontology_rejection_and_adoption_wait_as_proposals_and_change_nothing(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
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
        let city = seed_class(&f, "city").await?;
        let harbour = seed_class(&f, "harbour").await?;
        seed(&f, f.kb, "entity_types", "relic", class("relic", &[]), "suggest").await?;
        seed(
            &f,
            f.other_kb,
            "entity_types",
            "abroad",
            class("abroad", &[]),
            "agent",
        )
        .await?;
        seed(
            &f,
            f.kb,
            "entity_types",
            "suburb",
            class("suburb", &["nowhere"]),
            "agent",
        )
        .await?;
        // 人已经定过的两条：拒掉的、采纳了的
        seed_class(&f, "rejected").await?;
        seed_class(&f, "adopted").await?;
        let (status, body) = reject(&f, &f.editor_session, rejecting("rejected")).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let (status, body) = adopt(&f, &f.editor_session, adopting("adopted")).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");

        let before = snapshot(&f).await?;
        let ontology_before = ontology(&f).await?;
        let rows_before = ledger(&f).await?.len();
        let audits_before = audits(&f).await?.len();

        // ── 没过检查的调用：哪一种都一行不写 ──
        // 请求体解不出来：提取器先答，对谁都一样
        for bearer in [&token, &editor_read] {
            let (status, body) = reject(&f, bearer, json!({ "section": "entity_types" })).await?;
            anyhow::ensure!(
                status == StatusCode::UNPROCESSABLE_ENTITY,
                "{status} {body}"
            );
            let (status, body) = adopt(&f, bearer, json!({ "key": 7 })).await?;
            anyhow::ensure!(
                status == StatusCode::UNPROCESSABLE_ENTITY,
                "{status} {body}"
            );
        }
        // 写的检查，次序是 scope、库、角色；排在请求体与「提案在不在」前面
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
            for key in ["city", "no_such_key"] {
                for (route, answered) in [
                    ("reject", reject(&f, bearer, rejecting(key)).await?),
                    (
                        "reject with adopted",
                        reject(
                            &f,
                            bearer,
                            json!({ "section": "entity_types", "key": key, "status": "adopted" }),
                        )
                        .await?,
                    ),
                    ("adopt", adopt(&f, bearer, adopting(key)).await?),
                ] {
                    anyhow::ensure!(
                        answered.0 == StatusCode::FORBIDDEN && answered.1["code"] == code,
                        "{who}, {route} {key}: {answered:?}"
                    );
                }
            }
        }
        // 拒绝路由的请求体：与人拿到的是同一个 code、同一句话
        for (status_given, code) in [("adopted", "adoption_endpoint"), ("open", "bad_status")] {
            let body = json!({ "section": "entity_types", "key": "city", "status": status_given });
            let by_token = reject(&f, &token, body.clone()).await?;
            let by_person = reject(&f, &f.editor_session, body).await?;
            anyhow::ensure!(
                by_token.0 == StatusCode::UNPROCESSABLE_ENTITY
                    && by_token.1["code"] == code
                    && by_token == by_person,
                "{status_given}: {by_token:?} {by_person:?}"
            );
        }
        // 没有存着的提案（认不出的键、认不出的 section、别的库的）、退役的建议留下的：404
        for (section, key) in [
            ("entity_types", "no_such_key"),
            ("no_such_section", "city"),
            ("relation_types", "city"),
            ("entity_types", "abroad"),
            ("entity_types", "relic"),
        ] {
            for answered in [
                reject(
                    &f,
                    &token,
                    json!({ "section": section, "key": key, "status": "rejected" }),
                )
                .await?,
                adopt(&f, &token, json!({ "section": section, "key": key })).await?,
            ] {
                anyhow::ensure!(
                    answered.0 == StatusCode::NOT_FOUND,
                    "{section}/{key}: {answered:?}"
                );
            }
        }
        // 人对退役的建议留下的那一条的采纳，同一个 404
        let (status, body) = adopt(&f, &f.editor_session, adopting("relic")).await?;
        anyhow::ensure!(status == StatusCode::NOT_FOUND, "{status} {body}");
        // 定过的：already_decided
        for key in ["rejected", "adopted"] {
            for answered in [
                reject(&f, &token, rejecting(key)).await?,
                adopt(&f, &token, adopting(key)).await?,
            ] {
                anyhow::ensure!(
                    answered.0 == StatusCode::CONFLICT && answered.1["code"] == "already_decided",
                    "{key}: {answered:?}"
                );
            }
        }
        // 采纳点了名的类不存在：提案自己写着的、请求里改成的，都是人采纳时的那个 code
        let by_person = adopt(&f, &f.editor_session, adopting("suburb")).await?;
        let by_token = adopt(&f, &token, adopting("suburb")).await?;
        anyhow::ensure!(
            by_token.0 == StatusCode::UNPROCESSABLE_ENTITY
                && by_token.1["code"] == "unknown_class"
                && by_token == by_person,
            "{by_token:?} {by_person:?}"
        );
        let (status, body) = adopt(
            &f,
            &token,
            json!({ "section": "entity_types", "key": "city", "parents": ["nowhere"] }),
        )
        .await?;
        anyhow::ensure!(
            status == StatusCode::UNPROCESSABLE_ENTITY && body["code"] == "unknown_class",
            "{status} {body}"
        );
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before
                && audits(&f).await?.len() == audits_before
                && snapshot(&f).await? == before
                && ontology(&f).await? == ontology_before,
            "a refusal wrote something"
        );

        // ── 拒绝与采纳：各留一条提议，什么都没动 ──
        let rejection =
            json!({ "section": "entity_types", "key": "city", "status": "rejected",
                    "reason": "a city is a place, and place is already a class" });
        let adoption = json!({ "section": "entity_types", "key": "harbour",
                               "label": "Harbour", "parents": ["org"] });
        for (action, target, key, body, answered) in [
            (
                "reject",
                city,
                "city",
                rejection.clone(),
                reject(&f, &token, rejection.clone()).await?,
            ),
            (
                "adopt",
                harbour,
                "harbour",
                adoption.clone(),
                adopt(&f, &token, adoption.clone()).await?,
            ),
        ] {
            anyhow::ensure!(
                answered.0 == StatusCode::OK,
                "{action}: {} {}",
                answered.0,
                answered.1
            );
            let proposal = proposed(&answered.1)?;
            anyhow::ensure!(
                snapshot(&f).await? == before,
                "an ontology {action} proposal changed a fact, an entity, evidence or the queued jobs"
            );
            anyhow::ensure!(
                ontology(&f).await? == ontology_before,
                "an ontology {action} proposal changed the ontology or the stored proposal"
            );
            let open: Vec<_> = open_proposals(&f)
                .await?
                .into_iter()
                .filter(|p| p.target_id == target)
                .collect();
            anyhow::ensure!(open.len() == 1, "one open proposal: {open:?}");
            let p = &open[0];
            anyhow::ensure!(
                p.id == proposal
                    && p.operation == "ontology.proposal"
                    && p.target_kind == "ontology_proposal"
                    && p.outcome == "proposed"
                    && p.gate_reason.as_deref() == Some("propose_only")
                    && p.owner_id == f.editor
                    && p.token_id == token_id
                    && p.token_name == TOKEN_NAME,
                "{p:?}"
            );
            let mut expected = vec!["action", "fingerprint", "request", "target_label"];
            if action == "reject" {
                expected.push("why");
            }
            anyhow::ensure!(keys(&p.payload) == expected, "{}", p.payload);
            anyhow::ensure!(
                p.payload["request"] == body
                    && p.payload["action"] == action
                    && p.payload["fingerprint"] == class(key, &[])
                    && p.payload["target_label"] == format!("entity_types: {key}"),
                "{}",
                p.payload
            );
            // 一条本体提案上只有一条开着的提议：再提一次、换一个动作提，都是那一条
            for again in [
                reject(&f, &token, rejecting(key)).await?,
                adopt(&f, &token, adopting(key)).await?,
            ] {
                anyhow::ensure!(
                    again.0 == StatusCode::OK
                        && again.1["code"] == "already_proposed"
                        && again.1["action_id"] == json!(proposal)
                        && again.1["outcome"] == "proposed",
                    "{action}, a repeat: {again:?}"
                );
            }
        }
        anyhow::ensure!(
            open_proposals(&f).await?.len() == 2
                && ledger(&f).await?.len() == rows_before + 2
                && snapshot(&f).await? == before
                && ontology(&f).await? == ontology_before,
            "two proposals and nothing else"
        );
        // 提议的审计行是令牌 agent 的：带着令牌，不是人的决定
        let rows = audits(&f).await?;
        anyhow::ensure!(
            audits_of(&rows, "ontology_proposal.rejected").is_empty()
                && audits_of(&rows, "ontology_proposal.adopted").len() == 1,
            "no decision was recorded for a proposal"
        );
        Ok(())
    }
    .await;
    cleanup(f).await?;
    result
}

#[tokio::test]
async fn approving_an_ontology_rejection_rejects_it_as_the_person_and_writes_an_audit_row(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let by_person = seed_class(&f, "mine").await?;
        let by_proposal = seed_class(&f, "theirs").await?;
        let declined = seed_class(&f, "declined").await?;

        let mut body = rejecting("theirs");
        body["reason"] = json!("the token's own words");
        let (status, answered) = reject(&f, &token, body).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {answered}");
        let proposal = proposed(&answered)?;
        let (status, answered) = reject(&f, &token, rejecting("declined")).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {answered}");
        let declined_proposal = proposed(&answered)?;

        // 一枚令牌答不了提议，自己的也不行
        let (status, body) = answer(&f, &token, proposal, "approve").await?;
        anyhow::ensure!(status == StatusCode::UNAUTHORIZED, "{status} {body}");

        // 对照：这个人自己拒绝同样的一条
        let (status, answered) = reject(&f, &f.editor_session, rejecting("mine")).await?;
        anyhow::ensure!(
            status == StatusCode::OK && answered == json!({ "ok": true }),
            "{status} {answered}"
        );
        let mine = row(&f, "ontology_proposals", by_person).await?;
        anyhow::ensure!(
            decision_of(&mine) == json!(["rejected", f.editor, null, false]),
            "{mine}"
        );

        let before = snapshot(&f).await?;
        let audits_before = audits(&f).await?.len();
        let (status, answered) = answer(&f, &f.editor_session, proposal, "approve").await?;
        anyhow::ensure!(
            status == StatusCode::OK && answered == json!({ "ok": true, "state": "approved" }),
            "{status} {answered}"
        );
        // 提案同这个人自己拒的一模一样：令牌写的理由没有当成这个人的话存进去
        let theirs = row(&f, "ontology_proposals", by_proposal).await?;
        anyhow::ensure!(
            decision_of(&theirs) == decision_of(&mine) && !theirs["decided_at"].is_null(),
            "{theirs}\n{mine}"
        );
        anyhow::ensure!(
            snapshot(&f).await? == before,
            "a rejection changes no fact and queues no job"
        );
        let closed = ledger_row(&f, proposal).await?;
        anyhow::ensure!(
            closed.state.as_deref() == Some("approved")
                && closed.state_by == Some(f.editor)
                && closed.outcome == "proposed"
                && closed.payload["why"] == "the token's own words",
            "{closed:?}"
        );
        // 审计：正好多一行，记的是这个人，指着提案与提议，不带令牌
        let rows = audits(&f).await?;
        anyhow::ensure!(rows.len() == audits_before + 1, "one audit row");
        let said = audits_of(&rows, "ontology_proposal.rejected");
        anyhow::ensure!(
            said.len() == 1
                && said[0].actor == Some(f.editor)
                && said[0].target_kind == "ontology_proposal"
                && said[0].target_id == Some(by_proposal)
                && said[0].detail
                    == json!({ "section": "entity_types", "key": "theirs",
                               "agent_proposal": proposal }),
            "the approval is the person's act and names its proposal: {said:?}"
        );
        // 再答一次：已经答过了
        let (status, body) = answer(&f, &f.editor_session, proposal, "approve").await?;
        anyhow::ensure!(
            status == StatusCode::CONFLICT && body["code"] == "already_decided",
            "{status} {body}"
        );

        // 拒绝提议：什么都不改，提案还开着
        let declined_row = row(&f, "ontology_proposals", declined).await?;
        let ontology_before = ontology(&f).await?;
        let (status, answered) =
            answer(&f, &f.editor_session, declined_proposal, "decline").await?;
        anyhow::ensure!(
            status == StatusCode::OK && answered == json!({ "ok": true, "state": "declined" }),
            "{status} {answered}"
        );
        anyhow::ensure!(
            snapshot(&f).await? == before
                && ontology(&f).await? == ontology_before
                && row(&f, "ontology_proposals", declined).await? == declined_row
                && declined_row["status"] == "open",
            "a declined proposal changed something"
        );
        let rows = audits(&f).await?;
        let said: Vec<_> = audits_of(&rows, "agent.proposal.declined")
            .into_iter()
            .filter(|a| a.target_id == Some(declined_proposal))
            .collect();
        anyhow::ensure!(
            said.len() == 1
                && said[0].actor == Some(f.editor)
                && said[0].detail
                    == json!({ "agent_proposal": declined_proposal,
                               "operation": "ontology.proposal", "reason": "propose_only",
                               "proposed_by_token": TOKEN_NAME }),
            "{said:?}"
        );
        anyhow::ensure!(
            audits_of(&rows, "ontology_proposal.rejected").len() == 1
                && audits_of(&rows, "agent.proposal.superseded").is_empty(),
            "nothing else was recorded"
        );
        Ok(())
    }
    .await;
    cleanup(f).await?;
    result
}

#[tokio::test]
async fn approving_an_ontology_adoption_adopts_it_as_the_person() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let by_person = seed(
            &f,
            f.kb,
            "entity_types",
            "mine",
            class("mine", &["org"]),
            "agent",
        )
        .await?;
        let by_proposal = seed(
            &f,
            f.kb,
            "entity_types",
            "theirs",
            class("theirs", &["org"]),
            "agent",
        )
        .await?;
        seed_class(&f, "edited").await?;

        let (status, answered) = adopt(&f, &token, adopting("theirs")).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {answered}");
        let proposal = proposed(&answered)?;
        anyhow::ensure!(
            class_of(&f, "theirs").await?.is_null(),
            "a proposal adds no class"
        );

        // 对照：这个人自己采纳同样的一条
        let jobs_before = queued(&f).await?;
        let (status, answered) = adopt(&f, &f.editor_session, adopting("mine")).await?;
        anyhow::ensure!(
            status == StatusCode::OK && keys(&answered) == ["id", "skipped"],
            "{status} {answered}"
        );
        let jobs_between = queued(&f).await?;
        let own_jobs = |kind: &str| {
            jobs_between[kind].as_i64().unwrap_or(0) - jobs_before[kind].as_i64().unwrap_or(0)
        };
        anyhow::ensure!(
            own_jobs("embed_ontology") == 1 && own_jobs("align_types") == 1,
            "{jobs_before} {jobs_between}"
        );
        let mine = row(&f, "ontology_proposals", by_person).await?;
        anyhow::ensure!(
            decision_of(&mine) == json!(["adopted", f.editor, null, false]),
            "{mine}"
        );
        let mine_class = class_of(&f, "mine").await?;
        anyhow::ensure!(mine_class["parents"] == 1, "{mine_class}");

        let before = snapshot(&f).await?;
        let (status, answered) = answer(&f, &f.editor_session, proposal, "approve").await?;
        anyhow::ensure!(
            status == StatusCode::OK && answered == json!({ "ok": true, "state": "approved" }),
            "{status} {answered}"
        );
        // 本体同这个人自己采纳的一模一样：同样的类、同样的父边、同样排下的两份任务
        let theirs = row(&f, "ontology_proposals", by_proposal).await?;
        anyhow::ensure!(
            decision_of(&theirs) == decision_of(&mine) && !theirs["decided_at"].is_null(),
            "{theirs}\n{mine}"
        );
        let theirs_class = class_of(&f, "theirs").await?;
        anyhow::ensure!(
            !theirs_class.is_null() && theirs_class == mine_class,
            "an approved adoption builds the class exactly as the person's own\n{theirs_class}\n{mine_class}"
        );
        let jobs_after = queued(&f).await?;
        for kind in ["embed_ontology", "align_types"] {
            anyhow::ensure!(
                jobs_after[kind].as_i64().unwrap_or(0) - jobs_between[kind].as_i64().unwrap_or(0)
                    == own_jobs(kind),
                "{kind}: {jobs_between} {jobs_after}"
            );
        }
        let after = snapshot(&f).await?;
        anyhow::ensure!(
            after["facts"] == before["facts"] && after["entities"] == before["entities"],
            "an adoption changes no fact and no entity"
        );
        let closed = ledger_row(&f, proposal).await?;
        anyhow::ensure!(
            closed.state.as_deref() == Some("approved")
                && closed.state_by == Some(f.editor)
                && closed.outcome == "proposed",
            "{closed:?}"
        );
        // 审计：actor 都是这个人，同一个动作名；这个人自己的那一行正好是从前的四个键，
        // 批准的那一行多指着提议，都不带令牌
        let rows = audits(&f).await?;
        let adopted = audits_of(&rows, "ontology_proposal.adopted");
        anyhow::ensure!(adopted.len() == 2, "{adopted:?}");
        let (own, approved) = (adopted[0], adopted[1]);
        anyhow::ensure!(
            own.actor == Some(f.editor)
                && own.target_kind == "entity_type"
                && own.detail
                    == json!({ "key": "mine", "label": "The mine", "proposed_by": "agent",
                               "serves": [] }),
            "a session adoption records what it always did: {own:?}"
        );
        anyhow::ensure!(
            approved.actor == Some(f.editor)
                && approved.target_kind == "entity_type"
                && approved.detail
                    == json!({ "key": "theirs", "label": "The theirs", "proposed_by": "agent",
                               "serves": [], "agent_proposal": proposal }),
            "the approval is the person's act and names its proposal: {approved:?}"
        );

        // 令牌提的采纳带着改过的几格：批准时照它改，同这个人带着同样的请求体采纳一样
        let edits = json!({ "section": "entity_types", "key": "edited", "label": "Renamed",
                            "parents": ["person"] });
        let (status, answered) = adopt(&f, &token, edits).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {answered}");
        let edited = proposed(&answered)?;
        let (status, answered) = answer(&f, &f.editor_session, edited, "approve").await?;
        anyhow::ensure!(
            status == StatusCode::OK && answered == json!({ "ok": true, "state": "approved" }),
            "{status} {answered}"
        );
        let (label, parents): (String, i64) = sqlx::query_as(
            "SELECT t.label, (SELECT count(*) FROM entity_type_parents p
                               JOIN entity_types up ON up.id = p.parent_id
                              WHERE p.child_id = t.id AND up.key = 'person')
               FROM entity_types t WHERE t.kb_id = $1 AND t.key = 'edited'",
        )
        .bind(f.kb)
        .fetch_one(&f.pool)
        .await?;
        anyhow::ensure!(label == "Renamed" && parents == 1, "{label} {parents}");

        // 采纳在批准时报错：人的 code 原样报出去，提议留着，什么都没留下
        sqlx::query("INSERT INTO entity_types (id, kb_id, key, label) VALUES ($1, $2, $3, $4)")
            .bind(Uuid::now_v7())
            .bind(f.kb)
            .bind("district")
            .bind("District")
            .execute(&f.pool)
            .await?;
        seed(
            &f,
            f.kb,
            "entity_types",
            "ward",
            class("ward", &["district"]),
            "agent",
        )
        .await?;
        let (status, answered) = adopt(&f, &token, adopting("ward")).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {answered}");
        let stuck = proposed(&answered)?;
        sqlx::query("DELETE FROM entity_types WHERE kb_id = $1 AND key = 'district'")
            .bind(f.kb)
            .execute(&f.pool)
            .await?;
        let ontology_before = ontology(&f).await?;
        let jobs_before = queued(&f).await?;
        let audits_before = audits(&f).await?.len();
        let (status, body) = answer(&f, &f.editor_session, stuck, "approve").await?;
        anyhow::ensure!(
            status == StatusCode::UNPROCESSABLE_ENTITY && body["code"] == "unknown_class",
            "the person's error is returned as it is: {status} {body}"
        );
        let waiting = ledger_row(&f, stuck).await?;
        anyhow::ensure!(
            waiting.state.is_none() && waiting.state_by.is_none(),
            "the proposal stays open: {waiting:?}"
        );
        anyhow::ensure!(
            ontology(&f).await? == ontology_before
                && queued(&f).await? == jobs_before
                && audits(&f).await?.len() == audits_before,
            "a failed approval changed something"
        );
        Ok(())
    }
    .await;
    cleanup(f).await?;
    result
}

#[tokio::test]
async fn an_ontology_proposal_changed_or_decided_meanwhile_is_superseded() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let mut waiting = Vec::new();
        for (what, key, propose_adoption) in [
            (
                "a rejection of a proposal a person rejected",
                "r_rejected",
                false,
            ),
            (
                "an adoption of a proposal a person rejected",
                "a_rejected",
                true,
            ),
            (
                "a rejection of a proposal a person adopted",
                "r_adopted",
                false,
            ),
            (
                "an adoption of a proposal a person adopted",
                "a_adopted",
                true,
            ),
            (
                "a rejection of a proposal the agent rewrote",
                "r_rewritten",
                false,
            ),
            (
                "an adoption of a proposal the agent rewrote",
                "a_rewritten",
                true,
            ),
            ("a rejection of a proposal that is gone", "r_gone", false),
            ("an adoption of a proposal that is gone", "a_gone", true),
        ] {
            let target = seed_class(&f, key).await?;
            let (status, body) = if propose_adoption {
                adopt(&f, &token, adopting(key)).await?
            } else {
                reject(&f, &token, rejecting(key)).await?
            };
            anyhow::ensure!(status == StatusCode::OK, "{what}: {status} {body}");
            waiting.push((what, key, target, proposed(&body)?));
        }
        // 等着的时候：这个人拒掉两条、采纳两条；代理把两条刷成了别的样子（走的是它落库的
        // 那个函数，行还开着）；两条行没了
        for key in ["r_rejected", "a_rejected"] {
            let (status, body) = reject(&f, &f.editor_session, rejecting(key)).await?;
            anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        }
        for key in ["r_adopted", "a_adopted"] {
            let (status, body) = adopt(&f, &f.editor_session, adopting(key)).await?;
            anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        }
        let rewritten: Vec<_> = ["r_rewritten", "a_rewritten"]
            .into_iter()
            .map(|key| utopia_store::ontology::AgentProposal {
                section: "entity_types".into(),
                key: key.into(),
                payload: json!({ "key": key, "label": "Something else", "parents": ["org"] }),
                serves: Vec::new(),
                signatures: json!({}),
            })
            .collect();
        utopia_store::ontology::save_agent_proposals(&f.pool, f.kb, &rewritten).await?;
        sqlx::query("DELETE FROM ontology_proposals WHERE kb_id = $1 AND key = ANY($2)")
            .bind(f.kb)
            .bind(vec!["r_gone".to_string(), "a_gone".to_string()])
            .execute(&f.pool)
            .await?;

        let before = snapshot(&f).await?;
        let ontology_before = ontology(&f).await?;
        let audits_before = audits(&f).await?.len();
        for (what, key, target, proposal) in &waiting {
            let (status, body) = answer(&f, &f.editor_session, *proposal, "approve").await?;
            anyhow::ensure!(
                status == StatusCode::OK && body == json!({ "ok": true, "state": "superseded" }),
                "{what}: {status} {body}"
            );
            anyhow::ensure!(
                snapshot(&f).await? == before && ontology(&f).await? == ontology_before,
                "{what}: a superseded proposal changed something"
            );
            let closed = ledger_row(&f, *proposal).await?;
            anyhow::ensure!(
                closed.state.as_deref() == Some("superseded")
                    && closed.state_by == Some(f.editor)
                    && closed.operation == "ontology.proposal"
                    && closed.target_id == *target,
                "{what}: {closed:?}"
            );
            let (status, body) = answer(&f, &f.editor_session, *proposal, "approve").await?;
            anyhow::ensure!(
                status == StatusCode::CONFLICT && body["code"] == "already_decided",
                "{what}: {status} {body}"
            );
            // 刷过的那两条还开着、还是代理写的样子：过时的批准没有去拒它、采纳它
            if key.ends_with("rewritten") {
                let stored = row(&f, "ontology_proposals", *target).await?;
                anyhow::ensure!(
                    stored["status"] == "open" && stored["payload"]["label"] == "Something else",
                    "{what}: {stored}"
                );
                anyhow::ensure!(class_of(&f, key).await?.is_null(), "{what}");
            }
        }
        // 审计：只多了八行「过时」，记的是这个人；没有哪一行是人的决定
        let rows = audits(&f).await?;
        anyhow::ensure!(rows.len() == audits_before + 8, "eight audit rows");
        let superseded = audits_of(&rows, "agent.proposal.superseded");
        anyhow::ensure!(superseded.len() == 8, "{superseded:?}");
        for said in superseded {
            anyhow::ensure!(
                said.actor == Some(f.editor)
                    && keys(&said.detail)
                        == ["agent_proposal", "operation", "proposed_by_token", "reason"],
                "{said:?}"
            );
        }
        anyhow::ensure!(
            audits_of(&rows, "ontology_proposal.rejected").is_empty()
                && audits_of(&rows, "ontology_proposal.adopted").len() == 2,
            "only the person's own two adoptions were recorded"
        );
        // 定过的提不了；刷过的那一条还开着，可以照它现在的样子再提
        for key in ["r_rejected", "a_adopted"] {
            let (status, body) = reject(&f, &token, rejecting(key)).await?;
            anyhow::ensure!(
                status == StatusCode::CONFLICT && body["code"] == "already_decided",
                "{key}: {status} {body}"
            );
        }
        let (status, body) = adopt(&f, &token, adopting("r_gone")).await?;
        anyhow::ensure!(status == StatusCode::NOT_FOUND, "{status} {body}");
        let (status, body) = reject(&f, &token, rejecting("r_rewritten")).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        proposed(&body)?;
        Ok(())
    }
    .await;
    cleanup(f).await?;
    result
}

#[tokio::test]
async fn a_persons_own_ontology_rejection_still_writes_no_audit_row() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let target = seed_class(&f, "city").await?;
        let kept = seed_class(&f, "harbour").await?;
        // 退役的建议留下的那一条：人的拒绝从来不看是谁提的
        let relic = seed(
            &f,
            f.kb,
            "entity_types",
            "relic",
            class("relic", &[]),
            "suggest",
        )
        .await?;
        let before = snapshot(&f).await?;
        let rows_before = ledger(&f).await?.len();
        let audits_before = audits(&f).await?.len();

        // 拒绝路由仍然不收采纳，也不收别的状态
        let kept_row = row(&f, "ontology_proposals", kept).await?;
        for (status_given, code) in [("adopted", "adoption_endpoint"), ("open", "bad_status")] {
            let (status, body) = reject(
                &f,
                &f.editor_session,
                json!({ "section": "entity_types", "key": "harbour", "status": status_given }),
            )
            .await?;
            anyhow::ensure!(
                status == StatusCode::UNPROCESSABLE_ENTITY && body["code"] == code,
                "{status_given}: {status} {body}"
            );
        }
        anyhow::ensure!(row(&f, "ontology_proposals", kept).await? == kept_row);
        // Viewer 仍然拒不了
        let (status, body) = reject(&f, &f.viewer_session, rejecting("city")).await?;
        anyhow::ensure!(status == StatusCode::FORBIDDEN, "{status} {body}");

        // 拒绝：正好 `ok`，理由是这个人的话，原样（去了两头的空白）存进提案
        let mut body = rejecting("city");
        body["reason"] = json!("  not a class of its own  ");
        let (status, answered) = reject(&f, &f.editor_session, body).await?;
        anyhow::ensure!(
            status == StatusCode::OK && answered == json!({ "ok": true }),
            "{status} {answered}"
        );
        let rejected = row(&f, "ontology_proposals", target).await?;
        anyhow::ensure!(
            decision_of(&rejected)
                == json!(["rejected", f.editor, "not a class of its own", false])
                && !rejected["decided_at"].is_null(),
            "{rejected}"
        );
        let (status, answered) = reject(&f, &f.editor_session, rejecting("relic")).await?;
        anyhow::ensure!(
            status == StatusCode::OK && answered == json!({ "ok": true }),
            "{status} {answered}"
        );
        anyhow::ensure!(row(&f, "ontology_proposals", relic).await?["status"] == "rejected");
        // 没有这条提案、已经拒过的那一条：仍然答 ok，什么都不改
        for key in ["no_such_key", "city"] {
            let (status, answered) = reject(&f, &f.editor_session, rejecting(key)).await?;
            anyhow::ensure!(
                status == StatusCode::OK && answered == json!({ "ok": true }),
                "{key}: {status} {answered}"
            );
        }
        anyhow::ensure!(row(&f, "ontology_proposals", target).await? == rejected);
        // 一行审计都没写，台账一行都没写，图没动
        anyhow::ensure!(
            audits(&f).await?.len() == audits_before,
            "a person's own rejection writes no audit row: {:?}",
            audits(&f).await?
        );
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before && snapshot(&f).await? == before,
            "a session rejection writes no ledger row and changes no fact"
        );
        Ok(())
    }
    .await;
    cleanup(f).await?;
    result
}

#[tokio::test]
async fn the_mcp_ontology_tools_read_and_propose() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let (_, editor_read) = f.token(f.editor, "reader", "read", None).await?;
        let (_, viewer_write) = f.token(f.viewer, "viewer writer", "write", None).await?;
        let city = seed_class(&f, "city").await?;
        let harbour = seed_class(&f, "harbour").await?;
        seed_class(&f, "decided").await?;
        let (status, body) = reject(&f, &f.editor_session, rejecting("decided")).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");

        // ── 列出来的工具 ──
        let names = |tools: &[Value]| -> Vec<String> {
            tools
                .iter()
                .filter_map(|t| t["name"].as_str())
                .filter(|n| n.starts_with("review_ontology"))
                .map(str::to_string)
                .collect()
        };
        for (who, bearer) in [
            ("a read token", &editor_read),
            ("a write token of a viewer", &viewer_write),
        ] {
            let tools = tools_list(&f, bearer).await?;
            anyhow::ensure!(
                names(&tools) == ["review_ontology_list"],
                "{who} is offered the read and not the write: {:?}",
                names(&tools)
            );
        }
        let tools = tools_list(&f, &token).await?;
        let mut offered = names(&tools);
        offered.sort_unstable();
        anyhow::ensure!(
            offered == ["review_ontology", "review_ontology_list"],
            "{offered:?}"
        );
        let find = |name: &str| {
            tools
                .iter()
                .find(|t| t["name"] == name)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("{name} is not listed"))
        };
        let list = find("review_ontology_list")?;
        anyhow::ensure!(
            list["annotations"] == json!({ "readOnlyHint": true })
                && list["inputSchema"].get("required").is_none()
                && keys(&list["inputSchema"]["properties"]).is_empty(),
            "{list}"
        );
        let write = find("review_ontology")?;
        anyhow::ensure!(
            write["annotations"]
                == json!({ "readOnlyHint": false, "destructiveHint": false,
                           "idempotentHint": false }),
            "{write}"
        );
        anyhow::ensure!(
            write["inputSchema"]["required"] == json!(["section", "key", "action"])
                && keys(&write["inputSchema"]["properties"])
                    == [
                        "action",
                        "datatype",
                        "description",
                        "domains",
                        "key",
                        "label",
                        "parents",
                        "ranges",
                        "reason",
                        "section"
                    ],
            "{write}"
        );
        let said = write["description"].as_str().unwrap_or_default();
        anyhow::ensure!(
            said.starts_with("Propose that a person")
                && said.ends_with("No tool answers a proposal."),
            "{said}"
        );
        for words in [
            "The call changes nothing",
            "a signed-in editor answers in the Review tab",
            "{ok, outcome: \"proposed\", action_id, reason: \"propose_only\", explanation}",
            "already_proposed",
            "already_decided",
            "not_found",
            "superseded",
        ] {
            anyhow::ensure!(
                said.contains(words),
                "review_ontology does not say: {words}"
            );
        }
        for tool in [&list, &write] {
            let name = tool["name"].as_str().unwrap_or_default();
            for word in ["proposal", "approve", "decline", "answer"] {
                anyhow::ensure!(!name.contains(word), "{name}");
            }
        }

        // ── 读：就是 REST 回的那一份 ──
        let (status, by_rest) = read(&f, &editor_read).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {by_rest}");
        let by_tool = tool_call(&f, &editor_read, "review_ontology_list", json!({})).await?;
        anyhow::ensure!(
            by_tool["structuredContent"] == by_rest && by_tool["content"][0]["text"].is_string(),
            "{by_tool}"
        );

        // ── 写：没列给它的调用方照样拿到带 REST code 的工具错误，不是 -32601 ──
        let before = snapshot(&f).await?;
        let ontology_before = ontology(&f).await?;
        let rows_before = ledger(&f).await?.len();
        let rejecting_city =
            json!({ "section": "entity_types", "key": "city", "action": "reject" });
        for (bearer, code) in [
            (&editor_read, "token_read_only"),
            (&viewer_write, "token_owner_below_editor"),
        ] {
            let result = tool_call(&f, bearer, "review_ontology", rejecting_city.clone()).await?;
            ensure_tool_refused(&result, code)?;
        }
        // 操作自己的拒绝：与 REST 同一个 code
        for (args, code) in [
            (
                json!({ "section": "entity_types", "key": "no_such_key", "action": "reject" }),
                "not_found",
            ),
            (
                json!({ "section": "entity_types", "key": "no_such_key", "action": "adopt" }),
                "not_found",
            ),
            (
                json!({ "section": "entity_types", "key": "decided", "action": "adopt" }),
                "already_decided",
            ),
            (
                json!({ "section": "entity_types", "key": "city", "action": "approve" }),
                "unknown_action",
            ),
            (
                json!({ "section": "entity_types", "key": "city", "action": "adopt",
                        "parents": ["nowhere"] }),
                "unknown_class",
            ),
        ] {
            let result = tool_call(&f, &token, "review_ontology", args.clone()).await?;
            ensure_tool_refused(&result, code).map_err(|e| anyhow::anyhow!("{args}: {e}"))?;
        }
        // 参数没说清：没有执行，不带 code
        for args in [
            json!({ "section": "entity_types", "key": "city" }),
            json!({ "section": "entity_types", "key": "city", "action": "adopt",
                    "parents": "org" }),
            json!({ "section": "entity_types", "key": "city", "action": "reject",
                    "reason": 7 }),
        ] {
            let result = tool_call(&f, &token, "review_ontology", args.clone()).await?;
            anyhow::ensure!(
                result["isError"] == true && result["structuredContent"].get("code").is_none(),
                "{args}: {result}"
            );
        }
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before
                && snapshot(&f).await? == before
                && ontology(&f).await? == ontology_before,
            "a refused tool call wrote something"
        );

        // ── 提议：`structuredContent` 就是 REST 的那个回答，什么都没动 ──
        let mut with_reason = rejecting_city.clone();
        with_reason["reason"] = json!("already covered");
        let result = tool_call(&f, &token, "review_ontology", with_reason).await?;
        anyhow::ensure!(
            result.get("isError").is_none_or(|e| *e == false),
            "{result}"
        );
        let proposal = proposed(&result["structuredContent"])?;
        let stored = ledger_row(&f, proposal).await?;
        anyhow::ensure!(
            stored.target_id == city
                && stored.payload["action"] == "reject"
                && stored.payload["request"]
                    == json!({ "section": "entity_types", "key": "city", "status": "rejected",
                               "reason": "already covered" }),
            "the stored request is the REST route's body: {}",
            stored.payload
        );
        // 同一条提案上 REST 再提：就是工具留下的那一条，回答也是同一个样子
        let (status, by_rest) = reject(&f, &token, rejecting("city")).await?;
        let by_tool = tool_call(&f, &token, "review_ontology", rejecting_city).await?;
        anyhow::ensure!(
            status == StatusCode::OK
                && by_rest["code"] == "already_proposed"
                && by_rest["action_id"] == json!(proposal)
                && by_tool["structuredContent"] == by_rest,
            "{by_rest}\n{by_tool}"
        );
        let result = tool_call(
            &f,
            &token,
            "review_ontology",
            json!({ "section": "entity_types", "key": "harbour", "action": "adopt",
                    "label": "Harbour", "parents": ["org"], "reason": "not read for adopt" }),
        )
        .await?;
        let adoption = proposed(&result["structuredContent"])?;
        let stored = ledger_row(&f, adoption).await?;
        anyhow::ensure!(
            stored.target_id == harbour
                && stored.payload["action"] == "adopt"
                && stored.payload["request"]
                    == json!({ "section": "entity_types", "key": "harbour", "label": "Harbour",
                               "parents": ["org"] }),
            "{}",
            stored.payload
        );
        anyhow::ensure!(
            snapshot(&f).await? == before && ontology(&f).await? == ontology_before,
            "a tool proposal changed something"
        );
        // 工具留下的提议，人批准：照 REST 留下的一样落地
        let (status, body) = answer(&f, &f.editor_session, adoption, "approve").await?;
        anyhow::ensure!(
            status == StatusCode::OK && body == json!({ "ok": true, "state": "approved" }),
            "{status} {body}"
        );
        let label: Option<String> =
            sqlx::query_scalar("SELECT label FROM entity_types WHERE kb_id = $1 AND key = $2")
                .bind(f.kb)
                .bind("harbour")
                .fetch_optional(&f.pool)
                .await?;
        anyhow::ensure!(label.as_deref() == Some("Harbour"), "{label:?}");
        // 没有哪个工具能答提议
        for name in ["review_approve", "review_ontology_answer"] {
            let (status, body) = post(
                &f,
                &token,
                "mcp",
                json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                        "params": { "name": name, "arguments": { "id": proposal } } }),
            )
            .await?;
            anyhow::ensure!(
                status == StatusCode::OK && body["error"]["code"] == -32601,
                "{name} must not exist: {status} {body}"
            );
        }
        Ok(())
    }
    .await;
    cleanup(f).await?;
    result
}
