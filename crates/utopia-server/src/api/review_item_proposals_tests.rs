//! 开着的令牌提议挂在它指着的那一条上，左栏的数把它们算进去，Agent History 用话说出
//! 每一种操作的目标（0076）。
//!
//! 要钉住的行为：
//!
//! - **每一档的条目带着它自己开着的提议**（SC2 / E11）：等你点头、矛盾、未确认、低置信、
//!   公理、本体缺陷、勘误、对齐的短语与规则、Agent 一档里治理 agent 的行。键是
//!   `agent_proposals`，一列，最早提的在前；会话、令牌、MCP 的 `review_list` 读到的是
//!   同一页，`review_item` 读到的是同一条
//! - **没有提议的条目一个键也不多**（E11）：与这一档自己的 store 调用给出的逐字相等，
//!   键也正好是这一刀之前的那一组；提议答掉之后键跟着消失
//! - **重复项与合并历史不带这个键**（它们带的是 M003 的 `agent_proposal`），类别词、
//!   映射、令牌台账的行也不带
//! - **`counts.agent_token_proposals` 只在有开着的提议时出现**，值就是开着的条数；
//!   别的计数不动
//! - **Agent History 的行用话说出目标**（SC7 / E12）：这一刀的每一种操作各一行，带
//!   `target_label`、动作、主人与令牌名；不是一对的目标没有 `left` / `right`
//!
//! 各档的路由大多还没开给令牌，所以提议直接经 store 的 `propose` 种（落地的行经
//! `record_applied`），不经模型任务。夹具在 `review_token_write_support.rs`。
//! 没有 `UTOPIA_DATABASE_URL` 时跳过。
use std::collections::HashMap;

use axum::http::StatusCode;
use serde_json::{json, Value};
use utopia_store::token_agent_actions::{Proposal, TokenWrite};
use uuid::Uuid;

use super::review_token_write_support::{answer, keys, seeded, tool_call, Fx};

const TOKEN_NAME: &str = "section agent";

/// 列表路由认的十二个档名：十一个 Review 分档，加只开给会话的 `mappings`
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

/// 一条提议在卡片上的键：有动作的提议正好这七个
const PROPOSAL_KEYS: [&str; 7] = [
    "action",
    "created_at",
    "id",
    "operation",
    "owner_name",
    "reason",
    "token_name",
];

/// 没有提议的条目在这一刀之前（`d5d2356`）的键。对齐一档按 `kind` 分三种，Agent 一档
/// 这里说的是治理 agent 的行
fn keys_before(section: &str, item: &Value) -> Option<Vec<&'static str>> {
    let fact = vec![
        "confidence",
        "evidence_count",
        "id",
        "object_name",
        "predicate_label",
        "quote",
        "subject_name",
        "valid_from",
        "valid_to",
    ];
    Some(match (section, item["kind"].as_str()) {
        ("pending", _) => vec![
            "chunk_id",
            "confidence",
            "created_at",
            "id",
            "object_id",
            "object_name",
            "object_value",
            "phrase",
            "predicate_id",
            "predicate_label",
            "proposed_by",
            "proposed_by_name",
            "proposed_predicate",
            "proposed_token_name",
            "qualifiers",
            "quote",
            "quote_end",
            "quote_start",
            "subject_id",
            "subject_name",
            "time_words",
            "valid_from",
            "valid_from_precision",
            "valid_to",
            "valid_to_precision",
        ],
        ("conflicts", _) => vec![
            "created_at",
            "id",
            "new_confidence",
            "new_fact_id",
            "new_object",
            "new_subject",
            "new_valid_from",
            "new_valid_from_precision",
            "old_fact_id",
            "old_object",
            "old_subject",
            "old_valid_from",
            "old_valid_from_precision",
            "predicate_label",
            "reason",
        ],
        ("unconfirmed" | "lowconf", _) => fact,
        ("violations", _) => vec![
            "detail",
            "detected_at",
            "hint",
            "id",
            "kind",
            "left_fact",
            "left_text",
            "path",
            "path_len",
            "predicate",
            "right_fact",
            "right_text",
        ],
        ("defects", _) => vec![
            "detail",
            "detected_at",
            "id",
            "kind",
            "other_label",
            "path_labels",
            "subject_label",
        ],
        ("errata", _) => vec![
            "action",
            "created_at",
            "detail",
            "document",
            "document_id",
            "fact_id",
            "flag",
            "id",
            "proposed",
            "quote",
            "reason",
        ],
        ("alignment", Some("phrase")) => vec![
            "bound_to",
            "decided_at",
            "direction",
            "examples",
            "id",
            "kind",
            "object_class",
            "object_is_value",
            "phrase",
            "statement_count",
            "subject_class",
            "votes",
        ],
        ("alignment", Some("rule")) => vec![
            "decided_at",
            "examples",
            "id",
            "kind",
            "object_class",
            "object_is_value",
            "phrase",
            "property",
            "property_label",
            "reading",
            "statement_count",
            "subject_class",
            "trigger",
            "votes",
        ],
        ("agent", _) if item.get("source").is_none() => vec![
            "action",
            "calls",
            "confidence",
            "created_at",
            "decided_at",
            "decided_by_name",
            "id",
            "left",
            "merge_id",
            "precedents",
            "question",
            "reason",
            "right",
            "run_id",
            "status",
            "target_id",
            "target_kind",
            "trace",
        ],
        _ => return None,
    })
}

/// 每一档自己的 store 调用——列表路由在挂提议之前拿到的那一页，原样抄在这里当对照
async fn items_before(f: &Fx, queue: &str) -> anyhow::Result<Vec<Value>> {
    let (pool, kb, limit, offset) = (&f.pool, f.kb, 200, 0);
    let page = match queue {
        "pending" => json!(utopia_store::pending::list(pool, kb, limit, offset).await?),
        "duplicates" => json!(
            utopia_store::resolution::list_reviews(
                pool,
                kb,
                utopia_store::resolution::TypeFilter::parse(None)?,
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
    };
    Ok(page.as_array().cloned().unwrap_or_default())
}

/// 列表路由的一页（最多 200 条），必须是 200。`bearer` 是会话或令牌
async fn list(f: &Fx, bearer: &str, queue: &str) -> anyhow::Result<Value> {
    let (status, body) = f
        .call(
            bearer,
            "GET",
            &format!("/kbs/{}/review?queue={queue}&limit=200", f.kb),
            None,
        )
        .await?;
    anyhow::ensure!(status == StatusCode::OK, "{queue}: {status} {body}");
    Ok(body)
}

/// MCP 的 `review_list` 读同一页
async fn mcp_list(f: &Fx, token: &str, queue: &str) -> anyhow::Result<Value> {
    let result = tool_call(
        f,
        token,
        "review_list",
        json!({ "section": queue, "limit": 200 }),
    )
    .await?;
    anyhow::ensure!(result["isError"] != true, "review_list {queue}: {result}");
    Ok(result["structuredContent"].clone())
}

fn items(body: &Value) -> Vec<Value> {
    body["items"].as_array().cloned().unwrap_or_default()
}

fn id_of(item: &Value) -> Option<Uuid> {
    item["id"].as_str().and_then(|s| s.parse().ok())
}

/// 这一档的列表里第一条满足 `pick` 的条目的 id
async fn first_id(f: &Fx, queue: &str, pick: impl Fn(&Value) -> bool) -> anyhow::Result<Uuid> {
    items_before(f, queue)
        .await?
        .iter()
        .find(|item| pick(item))
        .and_then(id_of)
        .ok_or_else(|| anyhow::anyhow!("the seed left nothing to pick in {queue}"))
}

fn write(f: &Fx, token_id: Uuid) -> TokenWrite {
    TokenWrite {
        kb_id: f.kb,
        owner_id: f.editor,
        token_id,
        rationale: None,
        batch_id: None,
        breaker: false,
    }
}

/// payload 照 `review_ops::land_proposal` 的样子拼：请求体、目标的那句话、有动作时的动作
async fn payload(
    f: &Fx,
    operation: &str,
    target_id: Uuid,
    request: Value,
    action: Option<&str>,
) -> anyhow::Result<Value> {
    let label =
        utopia_store::review_targets::target_label(&f.pool, f.kb, operation, target_id, &request)
            .await?;
    let mut payload = json!({ "request": request, "target_label": label });
    if let Some(action) = action {
        payload["action"] = json!(action);
    }
    Ok(payload)
}

/// 经 store 种一条只能提议的提议，回台账行的 id。目标的钥匙就是它的 id
async fn propose(
    f: &Fx,
    token_id: Uuid,
    operation: &str,
    target_kind: &str,
    target_id: Uuid,
    action: &str,
) -> anyhow::Result<Uuid> {
    let payload = payload(
        f,
        operation,
        target_id,
        json!({ "seeded": action }),
        Some(action),
    )
    .await?;
    let done = utopia_store::token_agent_actions::propose(
        &f.pool,
        &write(f, token_id),
        &Proposal {
            operation,
            target_kind,
            target_id,
            target_key: &target_id.to_string(),
            payload,
        },
    )
    .await?;
    anyhow::ensure!(
        !done.existing,
        "{operation} on {target_id} was already open"
    );
    Ok(done.action.id)
}

/// 经 store 种一行直接落地的记录，回台账行的 id
#[allow(clippy::too_many_arguments)]
async fn applied(
    f: &Fx,
    token_id: Uuid,
    operation: &str,
    target_kind: &str,
    target_id: Uuid,
    target_key: &str,
    request: Value,
    action: Option<&str>,
) -> anyhow::Result<Uuid> {
    let payload = payload(f, operation, target_id, request, action).await?;
    let mut conn = f.pool.acquire().await?;
    let row = utopia_store::token_agent_actions::record_applied(
        &mut conn,
        &write(f, token_id),
        &Proposal {
            operation,
            target_kind,
            target_id,
            target_key,
            payload,
        },
    )
    .await?;
    Ok(row.id)
}

async fn editor_name(f: &Fx) -> anyhow::Result<String> {
    Ok(
        sqlx::query_scalar("SELECT display_name FROM users WHERE id = $1")
            .bind(f.editor)
            .fetch_one(&f.pool)
            .await?,
    )
}

/// 一条种下的提议：挂在哪一档的哪一条上，是什么操作、什么动作
struct Seeded {
    section: &'static str,
    target: Uuid,
    proposal: Uuid,
    operation: &'static str,
    action: &'static str,
}

/// 每一档种一条（未确认那一条上种两条：同一条事实的拒绝与闭合是两种操作）。
/// 回种下的提议，按种的先后
async fn seed_proposals(f: &Fx, token_id: Uuid) -> anyhow::Result<Vec<Seeded>> {
    let any = |_: &Value| true;
    let stale = first_id(f, "unconfirmed", any).await?;
    let plan: Vec<(&'static str, &'static str, &'static str, Uuid, &'static str)> = vec![
        (
            "pending",
            "pending.nod",
            "pending_fact",
            first_id(f, "pending", any).await?,
            "confirm",
        ),
        (
            "conflicts",
            "conflict.resolve",
            "conflict",
            first_id(f, "conflicts", any).await?,
            "keep",
        ),
        ("unconfirmed", "fact.reject", "fact", stale, "reject"),
        ("unconfirmed", "fact.close", "fact", stale, "close"),
        (
            "lowconf",
            "fact.confirm",
            "fact",
            first_id(f, "lowconf", any).await?,
            "confirm",
        ),
        (
            "violations",
            "violation.decide",
            "axiom_violation",
            first_id(f, "violations", any).await?,
            "accepted",
        ),
        (
            "defects",
            "defect.decide",
            "ontology_defect",
            first_id(f, "defects", any).await?,
            "fixed",
        ),
        (
            "errata",
            "errata.decide",
            "errata_action",
            first_id(f, "errata", any).await?,
            "approve",
        ),
        (
            "alignment",
            "alignment.phrase",
            "phrase_binding",
            first_id(f, "alignment", |item| item["kind"] == "phrase").await?,
            "none",
        ),
        (
            "alignment",
            "alignment.rule",
            "implication_rule",
            first_id(f, "alignment", |item| item["kind"] == "rule").await?,
            "approve",
        ),
        (
            "agent",
            "governor.answer",
            "agent_decision",
            first_id(f, "agent", |item| item.get("source").is_none()).await?,
            "keep",
        ),
    ];
    let mut seeded = Vec::with_capacity(plan.len());
    for (section, operation, target_kind, target, action) in plan {
        let proposal = propose(f, token_id, operation, target_kind, target, action).await?;
        seeded.push(Seeded {
            section,
            target,
            proposal,
            operation,
            action,
        });
    }
    Ok(seeded)
}

/// 一页的条目对着 store 的对照逐条比：没有提议的条目逐字相等，带提议的条目正好多一个
/// `agent_proposals`，里面就是 `expected` 给这一条排好的那一列
fn ensure_page(
    who: &str,
    section: &str,
    got: &[Value],
    before: &[Value],
    expected: &HashMap<Uuid, Vec<Value>>,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        got.len() == before.len(),
        "{who}, {section}: {} items, the store lists {}",
        got.len(),
        before.len()
    );
    for (item, plain) in got.iter().zip(before) {
        let proposals = id_of(plain).and_then(|id| expected.get(&id));
        let Some(proposals) = proposals else {
            anyhow::ensure!(
                item == plain,
                "{who}, {section}: an item without a proposal changed\n{item}\n{plain}"
            );
            anyhow::ensure!(item.get("agent_proposals").is_none(), "{item}");
            if let Some(known) = keys_before(section, item) {
                anyhow::ensure!(
                    keys(item) == known,
                    "{who}, {section}: keys {:?}",
                    keys(item)
                );
            }
            continue;
        };
        let mut with_key = keys(plain);
        with_key.push("agent_proposals".to_string());
        with_key.sort();
        anyhow::ensure!(
            keys(item) == with_key,
            "{who}, {section}: keys {:?}",
            keys(item)
        );
        if let Some(known) = keys_before(section, plain) {
            anyhow::ensure!(
                keys(plain) == known,
                "{section}: the store item's keys {:?}",
                keys(plain)
            );
        }
        anyhow::ensure!(
            item["agent_proposals"] == json!(proposals),
            "{who}, {section}: {}\nexpected {}",
            item["agent_proposals"],
            json!(proposals)
        );
        let mut without = item.clone();
        if let Some(fields) = without.as_object_mut() {
            fields.remove("agent_proposals");
        }
        anyhow::ensure!(
            without == *plain,
            "{who}, {section}: more than the key changed\n{item}\n{plain}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn every_section_item_carries_its_open_proposals_and_an_item_without_one_keeps_its_keys(
) -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (token_id, _) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        // 读的一侧：主人只是 Viewer 的只读令牌，读得到的与编辑者的会话相同
        let (_, reader) = f.token(f.viewer, "reader", "read", None).await?;
        let owner = editor_name(&f).await?;
        let nothing = HashMap::new();

        // ── 还没有提议：每一档的每一条都与 store 给的逐字相等，键是从前那一组 ──
        for queue in LIST_QUEUES {
            let before = items_before(&f, queue).await?;
            anyhow::ensure!(!before.is_empty(), "the seed left {queue} empty");
            let body = list(&f, &f.editor_session, queue).await?;
            ensure_page("a session", queue, &items(&body), &before, &nothing)?;
            if queue != "mappings" {
                let body = list(&f, &reader, queue).await?;
                ensure_page("a token", queue, &items(&body), &before, &nothing)?;
                let body = mcp_list(&f, &reader, queue).await?;
                ensure_page("review_list", queue, &items(&body), &before, &nothing)?;
            }
        }

        // ── 每一档种一条 ──
        let seeded = seed_proposals(&f, token_id).await?;
        let mut expected: HashMap<Uuid, Vec<Value>> = HashMap::new();
        for s in &seeded {
            let created_at: chrono::DateTime<chrono::Utc> =
                sqlx::query_scalar("SELECT created_at FROM token_agent_actions WHERE id = $1")
                    .bind(s.proposal)
                    .fetch_one(&f.pool)
                    .await?;
            // 同一个目标上的几条按种的先后排：`seeded` 本来就是那个次序
            expected.entry(s.target).or_default().push(json!({
                "id": s.proposal,
                "operation": s.operation,
                "reason": "propose_only",
                "owner_name": owner,
                "token_name": TOKEN_NAME,
                "created_at": created_at,
                "action": s.action,
            }));
        }
        anyhow::ensure!(
            expected.values().map(Vec::len).max() == Some(2),
            "one item carries two proposals"
        );

        for queue in LIST_QUEUES {
            // 对照要在种完之后再取：Agent 一档里多了令牌台账的行（提议自己）
            let before = items_before(&f, queue).await?;
            let session = list(&f, &f.editor_session, queue).await?;
            ensure_page("a session", queue, &items(&session), &before, &expected)?;
            let marked: Vec<Value> = items(&session)
                .into_iter()
                .filter(|item| item.get("agent_proposals").is_some())
                .collect();
            let wanted = seeded
                .iter()
                .filter(|s| s.section == queue)
                .map(|s| s.target)
                .collect::<std::collections::HashSet<_>>();
            anyhow::ensure!(
                marked.len() == wanted.len()
                    && marked
                        .iter()
                        .all(|item| id_of(item).is_some_and(|id| wanted.contains(&id))),
                "{queue}: {} items carry proposals, {} were seeded",
                marked.len(),
                wanted.len()
            );
            for item in &marked {
                for proposal in item["agent_proposals"].as_array().into_iter().flatten() {
                    anyhow::ensure!(keys(proposal) == PROPOSAL_KEYS, "{queue}: {proposal}");
                }
            }
            if queue == "mappings" {
                continue;
            }
            // 令牌与 MCP 读到的是会话读到的那一页
            let token = list(&f, &reader, queue).await?;
            anyhow::ensure!(
                token["items"] == session["items"],
                "{queue}: a token reads other items than a session"
            );
            let mcp = mcp_list(&f, &reader, queue).await?;
            anyhow::ensure!(
                mcp["items"] == session["items"],
                "{queue}: review_list reads other items than a session"
            );
            // 单条读拿到的就是列表里那一条，提议也在
            for item in &marked {
                let one = tool_call(
                    &f,
                    &reader,
                    "review_item",
                    json!({ "section": queue, "id": item["id"] }),
                )
                .await?;
                anyhow::ensure!(one["isError"] != true, "review_item {queue}: {one}");
                anyhow::ensure!(
                    one["structuredContent"]["agent_proposals"] == item["agent_proposals"],
                    "review_item {queue}: {one}"
                );
            }
        }
        // 不带这个键的那几种：重复项、合并历史、映射、类别词、令牌台账的行
        for queue in ["duplicates", "merges", "mappings"] {
            let body = list(&f, &f.editor_session, queue).await?;
            anyhow::ensure!(
                items(&body)
                    .iter()
                    .all(|item| item.get("agent_proposals").is_none()),
                "{queue}: {body}"
            );
        }
        let alignment = items(&list(&f, &f.editor_session, "alignment").await?);
        anyhow::ensure!(
            alignment
                .iter()
                .any(|item| item["kind"] == "kind_word" && item.get("agent_proposals").is_none()),
            "a kind word carries no proposal"
        );
        let agent = items(&list(&f, &f.editor_session, "agent").await?);
        anyhow::ensure!(
            agent
                .iter()
                .filter(|row| row["source"] == "token")
                .all(|row| row.get("agent_proposals").is_none()),
            "a token ledger row carries no proposal"
        );

        // ── 别的库里同一种操作的提议不挂到这个库的条目上 ──
        // （提议按库找：这里只确认另一个库的列表没有被这些提议碰到）
        let (status, other) = f
            .call(
                &f.editor_session,
                "GET",
                &format!("/kbs/{}/review?queue=defects&limit=200", f.other_kb),
                None,
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {other}");
        anyhow::ensure!(other["counts"].get("agent_token_proposals").is_none());

        // ── 答掉一条：那一条上的键跟着少一项，答完了就没有这个键 ──
        let stale: Vec<&Seeded> = seeded
            .iter()
            .filter(|s| s.section == "unconfirmed")
            .collect();
        anyhow::ensure!(stale.len() == 2);
        let (status, body) = answer(&f, &f.editor_session, stale[0].proposal, "decline").await?;
        anyhow::ensure!(
            status == StatusCode::OK && body["state"] == "declined",
            "{status} {body}"
        );
        let left = items(&list(&f, &reader, "unconfirmed").await?);
        let item = left
            .iter()
            .find(|item| id_of(item) == Some(stale[0].target))
            .ok_or_else(|| anyhow::anyhow!("the fact left the list"))?;
        anyhow::ensure!(
            item["agent_proposals"]
                .as_array()
                .is_some_and(|open| open.len() == 1 && open[0]["id"] == json!(stale[1].proposal)),
            "{item}"
        );
        for s in &seeded {
            // 已经答过的那一条再答是 `already_decided`，别的都关上
            let closed = utopia_store::token_agent_actions::answer(
                &f.pool, f.kb, s.proposal, "declined", f.editor, None,
            )
            .await?;
            anyhow::ensure!(closed == (s.proposal != stale[0].proposal));
        }
        for queue in LIST_QUEUES {
            let before = items_before(&f, queue).await?;
            let body = list(&f, &f.editor_session, queue).await?;
            ensure_page("a session", queue, &items(&body), &before, &nothing)?;
        }
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

/// `counts` 去掉两样之后的样子：开着的令牌提议数（要比的就是它），与 Agent 一档的总行数
/// （提议本身是台账的行，会让它变）
fn counts_without(counts: &Value) -> Value {
    let mut rest = counts.clone();
    if let Some(fields) = rest.as_object_mut() {
        fields.remove("agent_token_proposals");
        fields.remove("agent_rows");
    }
    rest
}

/// 三种读法读到的 `counts`：会话、令牌、MCP 的 `review_list`
async fn counts_seen(f: &Fx, token: &str, queue: &str) -> anyhow::Result<[Value; 3]> {
    Ok([
        list(f, &f.editor_session, queue).await?["counts"].clone(),
        list(f, token, queue).await?["counts"].clone(),
        mcp_list(f, token, queue).await?["counts"].clone(),
    ])
}

#[tokio::test]
async fn the_counts_add_open_token_proposals_only_while_one_is_open() -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (token_id, _) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let (_, reader) = f.token(f.viewer, "reader", "read", None).await?;
        let sections: Vec<&str> = LIST_QUEUES
            .into_iter()
            .filter(|queue| *queue != "mappings")
            .collect();

        // ── 没有开着的提议：没有这个键，哪一档的回答、哪一种读法都一样 ──
        let plain = list(&f, &f.editor_session, "duplicates").await?["counts"].clone();
        let governor_open = plain["agent"].clone();
        anyhow::ensure!(
            governor_open.as_i64().is_some_and(|n| n > 0),
            "the seed leaves an open governor proposal: {plain}"
        );
        anyhow::ensure!(
            keys(&plain)
                == [
                    "agent",
                    "agent_queue",
                    "agent_rows",
                    "agent_running",
                    "alignment",
                    "conflicts",
                    "defects",
                    "duplicates",
                    "duplicates_same_type",
                    "duplicates_type_conflict",
                    "errata",
                    "lowconf",
                    "mappings",
                    "merges",
                    "pending",
                    "unconfirmed",
                    "violations",
                ],
            "the counts keys with no open proposal: {plain}"
        );
        for queue in &sections {
            for counts in counts_seen(&f, &reader, queue).await? {
                anyhow::ensure!(counts == plain, "{queue}: {counts}\n{plain}");
            }
        }

        // ── 种下提议：值就是开着的条数，治理 agent 的那个数与别的计数都不动 ──
        let seeded = seed_proposals(&f, token_id).await?;
        let mut open = seeded.len() as i64;
        for queue in &sections {
            for counts in counts_seen(&f, &reader, queue).await? {
                anyhow::ensure!(
                    counts["agent_token_proposals"] == json!(open),
                    "{queue}: {counts}"
                );
                anyhow::ensure!(counts["agent"] == governor_open, "{queue}: {counts}");
                anyhow::ensure!(
                    counts_without(&counts) == counts_without(&plain),
                    "{queue}: another count moved\n{counts}\n{plain}"
                );
            }
        }
        // 只开给会话的 `mappings` 一档的回答里也是同一个数
        let mappings = list(&f, &f.editor_session, "mappings").await?;
        anyhow::ensure!(mappings["counts"]["agent_token_proposals"] == json!(open));
        // 左栏 Agent 的徽标是这两个数的和：两个数都在同一个回答里
        let counts = list(&f, &f.editor_session, "agent").await?["counts"].clone();
        anyhow::ensure!(
            counts["agent"].as_i64().unwrap_or(0)
                + counts["agent_token_proposals"].as_i64().unwrap_or(0)
                == governor_open.as_i64().unwrap_or(0) + open,
            "{counts}"
        );

        // ── 一条一条答掉：数跟着减，减到零键就没了 ──
        for (i, s) in seeded.iter().enumerate() {
            // 头一条走人的路由（拒绝），其余经 store 关上：批准要各档自己的那一支
            if i == 0 {
                let (status, body) = answer(&f, &f.editor_session, s.proposal, "decline").await?;
                anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
            } else {
                let state = if i % 2 == 0 { "superseded" } else { "approved" };
                anyhow::ensure!(
                    utopia_store::token_agent_actions::answer(
                        &f.pool, f.kb, s.proposal, state, f.editor, None,
                    )
                    .await?
                );
            }
            open -= 1;
            let counts = list(&f, &reader, "agent").await?["counts"].clone();
            if open > 0 {
                anyhow::ensure!(counts["agent_token_proposals"] == json!(open), "{counts}");
            } else {
                anyhow::ensure!(counts.get("agent_token_proposals").is_none(), "{counts}");
            }
            anyhow::ensure!(counts["agent"] == governor_open, "{counts}");
        }
        for queue in &sections {
            for counts in counts_seen(&f, &reader, queue).await? {
                anyhow::ensure!(
                    counts.get("agent_token_proposals").is_none()
                        && counts_without(&counts) == counts_without(&plain),
                    "{queue}: {counts}"
                );
            }
        }
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

/// 一行台账记录该是什么样：哪种操作、落地还是提议、什么动作
struct Row {
    id: Uuid,
    operation: &'static str,
    target_kind: &'static str,
    outcome: &'static str,
    action: Option<&'static str>,
}

#[tokio::test]
async fn agent_history_names_each_target_in_words_for_every_operation() -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (token_id, _) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let (_, reader) = f.token(f.viewer, "reader", "read", None).await?;
        let owner = editor_name(&f).await?;

        // ── 只能提议的十二种：十一种挂在 Review 的条目上，本体提案在本体页 ──
        let mut rows: Vec<Row> = seed_proposals(&f, token_id)
            .await?
            .into_iter()
            .map(|s| Row {
                id: s.proposal,
                operation: s.operation,
                target_kind: match s.operation {
                    "pending.nod" => "pending_fact",
                    "conflict.resolve" => "conflict",
                    "violation.decide" => "axiom_violation",
                    "defect.decide" => "ontology_defect",
                    "errata.decide" => "errata_action",
                    "alignment.phrase" => "phrase_binding",
                    "alignment.rule" => "implication_rule",
                    "governor.answer" => "agent_decision",
                    _ => "fact",
                },
                outcome: "proposed",
                action: Some(s.action),
            })
            .collect();
        let ontology_proposal = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO ontology_proposals (id, kb_id, section, key, payload, proposed_by)
             VALUES ($1, $2, 'entity_types', 'seed_city', $3, 'agent')",
        )
        .bind(ontology_proposal)
        .bind(f.kb)
        .bind(json!({ "key": "seed_city", "label": "Seed city" }))
        .execute(&f.pool)
        .await?;
        let payload_of = payload(
            &f,
            "ontology.proposal",
            ontology_proposal,
            json!({ "section": "entity_types", "key": "seed_city" }),
            Some("adopt"),
        )
        .await?;
        let done = utopia_store::token_agent_actions::propose(
            &f.pool,
            &write(&f, token_id),
            &Proposal {
                operation: "ontology.proposal",
                target_kind: "ontology_proposal",
                target_id: ontology_proposal,
                target_key: "entity_types:seed_city",
                payload: payload_of,
            },
        )
        .await?;
        rows.push(Row {
            id: done.action.id,
            operation: "ontology.proposal",
            target_kind: "ontology_proposal",
            outcome: "proposed",
            action: Some("adopt"),
        });

        // ── 直接落地的四种：映射、类别词、一致性检查、推理 ──
        let mapping = first_id(&f, "mappings", |_| true).await?;
        let kb_key = f.kb.to_string();
        for (operation, target_kind, target, key, request, action) in [
            (
                "mapping.decide",
                "concept_mapping",
                mapping,
                mapping.to_string(),
                json!({ "status": "confirmed" }),
                Some("confirmed"),
            ),
            (
                "alignment.kind_word",
                "type_binding",
                f.kb,
                "seed startup".to_string(),
                json!({ "kind_word": "seed startup", "class": "seed_org" }),
                Some("bind"),
            ),
            (
                "consistency.check",
                "knowledge_base",
                f.kb,
                kb_key.clone(),
                json!({}),
                None,
            ),
            (
                "inference.run",
                "knowledge_base",
                f.kb,
                kb_key.clone(),
                json!({}),
                None,
            ),
        ] {
            let id = applied(
                &f,
                token_id,
                operation,
                target_kind,
                target,
                &key,
                request,
                action,
            )
            .await?;
            rows.push(Row {
                id,
                operation,
                target_kind,
                outcome: "applied",
                action,
            });
        }
        // 这一刀的每一种操作都在里面，各不相同的十六种
        let operations: std::collections::HashSet<&str> =
            rows.iter().map(|row| row.operation).collect();
        for operation in [
            "fact.confirm",
            "fact.reject",
            "fact.close",
            "conflict.resolve",
            "violation.decide",
            "defect.decide",
            "errata.decide",
            "alignment.phrase",
            "alignment.rule",
            "pending.nod",
            "governor.answer",
            "ontology.proposal",
            "mapping.decide",
            "alignment.kind_word",
            "consistency.check",
            "inference.run",
        ] {
            anyhow::ensure!(operations.contains(operation), "{operation} was not seeded");
        }
        anyhow::ensure!(operations.len() == 16 && rows.len() == 16);

        // ── Agent 一档：会话、令牌、MCP 读到同一页，每一种操作一行 ──
        let session = list(&f, &f.editor_session, "agent").await?;
        let token = list(&f, &reader, "agent").await?;
        let mcp = mcp_list(&f, &reader, "agent").await?;
        anyhow::ensure!(
            token["items"] == session["items"],
            "a token reads other rows"
        );
        anyhow::ensure!(
            mcp["items"] == session["items"],
            "review_list reads other rows"
        );
        let history = items(&session);
        let mut labels = std::collections::HashSet::new();
        for row in &rows {
            let shown = history
                .iter()
                .find(|item| id_of(item) == Some(row.id))
                .ok_or_else(|| anyhow::anyhow!("no Agent History row for {}", row.operation))?;
            let stored: Value =
                sqlx::query_scalar("SELECT payload FROM token_agent_actions WHERE id = $1")
                    .bind(row.id)
                    .fetch_one(&f.pool)
                    .await?;
            anyhow::ensure!(
                shown["source"] == "token"
                    && shown["operation"] == row.operation
                    && shown["target_kind"] == row.target_kind
                    && shown["outcome"] == row.outcome
                    && shown["owner_name"] == json!(owner)
                    && shown["token_name"] == TOKEN_NAME,
                "{}: {shown}",
                row.operation
            );
            // 目标用话说：存下的那一句，不是空的，也不是一个 id
            let label = shown["target_label"]
                .as_str()
                .map(str::trim)
                .unwrap_or_default();
            anyhow::ensure!(
                !label.is_empty()
                    && shown["target_label"] == stored["target_label"]
                    && label.parse::<Uuid>().is_err(),
                "{}: {shown}",
                row.operation
            );
            labels.insert(label.to_string());
            // 不是一对：没有哪一侧的名字可写
            anyhow::ensure!(
                shown["left"].is_null() && shown["right"].is_null(),
                "{}: {shown}",
                row.operation
            );
            match row.action {
                Some(action) => {
                    anyhow::ensure!(shown["action"] == action, "{}: {shown}", row.operation)
                }
                None => {
                    anyhow::ensure!(shown.get("action").is_none(), "{}: {shown}", row.operation)
                }
            }
            // 留成提议的行说得出为什么；落地的行没有理由
            if row.outcome == "proposed" {
                anyhow::ensure!(
                    shown["reason"] == "propose_only" && shown.get("state").is_none(),
                    "{}: {shown}",
                    row.operation
                );
            } else {
                anyhow::ensure!(shown.get("reason").is_none(), "{}: {shown}", row.operation);
            }
            anyhow::ensure!(shown.get("queued").is_none(), "{}: {shown}", row.operation);
            // 单条读拿到的就是这一行
            let one = tool_call(
                &f,
                &reader,
                "review_item",
                json!({ "section": "agent", "id": row.id }),
            )
            .await?;
            anyhow::ensure!(
                one["structuredContent"] == *shown,
                "review_item {}: {one}",
                row.operation
            );
        }
        // 目标各说各的：同一条事实上的两种操作说的是同一句，别的都不同
        anyhow::ensure!(labels.len() >= 13, "{labels:?}");

        // ── 令牌改了名字：行上还是动作那一刻的名字，主人的名字照旧 ──
        sqlx::query("UPDATE personal_tokens SET name = 'renamed' WHERE id = $1")
            .bind(token_id)
            .execute(&f.pool)
            .await?;
        let renamed = items(&list(&f, &f.editor_session, "agent").await?);
        for row in &rows {
            let shown = renamed
                .iter()
                .find(|item| id_of(item) == Some(row.id))
                .ok_or_else(|| anyhow::anyhow!("no Agent History row for {}", row.operation))?;
            anyhow::ensure!(
                shown["token_name"] == TOKEN_NAME && shown["owner_name"] == json!(owner),
                "{}: {shown}",
                row.operation
            );
        }

        // ── 批准了的短语与规则判定：行上说「排着」，别的批准没有这个键 ──
        for row in rows.iter().filter(|row| row.outcome == "proposed") {
            anyhow::ensure!(
                utopia_store::token_agent_actions::answer(
                    &f.pool, f.kb, row.id, "approved", f.editor, None,
                )
                .await?
            );
        }
        let answered = items(&list(&f, &reader, "agent").await?);
        for row in rows.iter().filter(|row| row.outcome == "proposed") {
            let shown = answered
                .iter()
                .find(|item| id_of(item) == Some(row.id))
                .ok_or_else(|| anyhow::anyhow!("no Agent History row for {}", row.operation))?;
            anyhow::ensure!(shown["state"] == "approved", "{}: {shown}", row.operation);
            let queued = matches!(row.operation, "alignment.phrase" | "alignment.rule");
            anyhow::ensure!(
                shown.get("queued") == queued.then_some(&json!(true)),
                "{}: {shown}",
                row.operation
            );
        }
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}
