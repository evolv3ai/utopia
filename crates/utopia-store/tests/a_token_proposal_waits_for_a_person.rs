//! 令牌 agent 在 Review 其余各档的提议与落地的行（0076），打在真库上：路由还没开之前，
//! 先在 store 里把后面每一刀都要踩的六件事钉住。
//!
//! 1. **一条提议就是两行**：台账一行（`proposed`，理由 `propose_only`）加 `agent.proposal`
//!    审计一行，这个库别的表一行不变、不排任何任务
//! 2. **一个目标一条开着的提议**：重复的、同时到的调用拿回开着的那一条（`existing`）
//! 3. **同一个目标上不同的操作各开各的**：一条事实的确认与拒绝可以同时等着
//! 4. **目标的状态从带类型的行读**：每一种操作的 `Open` / `Decided` / `Missing`，别的库的
//!    id 一律 `Missing`
//! 5. **落地的行跟着调用方的事务走**：回滚就没有，提交才在
//! 6. **Agent 一档与徽标**：目标不是一对的行用话说出目标；分开与合并的行一个键不多；
//!    开着的令牌提议数为零时计数里没有那个键
//!
//! 种数据只用直接插入与 store 函数，不经模型任务。每个测试自建自己的库（kb）、自己拆，
//! 互不相干，所以可以并着跑。没有 `UTOPIA_DATABASE_URL` 时跳过而不是失败。

use serde_json::{json, Value};
use sqlx::PgPool;
use utopia_core::models::AgentProposalView;
use utopia_store::review_targets::{self, TargetState};
use utopia_store::token_agent_actions::{self, Proposal, TokenAgentAction, TokenWrite};
use uuid::Uuid;

const TOKEN_NAME: &str = "review agent";

struct Base {
    kb: Uuid,
    org: Uuid,
    user: Uuid,
    token: Uuid,
    company_type: Uuid,
    based_in: Uuid,
}

async fn base(pool: &PgPool) -> anyhow::Result<Base> {
    let [org, ws, kb, user, company_type, based_in] = std::array::from_fn(|_| Uuid::now_v7());
    sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, 'token-proposal-test')")
        .bind(org)
        .execute(pool)
        .await?;
    sqlx::query(
        "INSERT INTO users (id, org_id, email, display_name, password_hash)
         VALUES ($1, $2, $3, 'Token Owner', 'x')",
    )
    .bind(user)
    .bind(org)
    .bind(format!("token-proposal-{}@test.local", user.simple()))
    .execute(pool)
    .await?;
    sqlx::query("INSERT INTO workspaces (id, org_id, name) VALUES ($1, $2, 'token-proposal-test')")
        .bind(ws)
        .bind(org)
        .execute(pool)
        .await?;
    sqlx::query(
        "INSERT INTO knowledge_bases (id, workspace_id, name) VALUES ($1, $2, 'Proposal Base')",
    )
    .bind(kb)
    .bind(ws)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO entity_types (id, kb_id, key, label) VALUES ($1, $2, 'org', 'Organization')",
    )
    .bind(company_type)
    .bind(kb)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO relation_types (id, kb_id, key, label, functional)
         VALUES ($1, $2, 'based_in', 'based in', false)",
    )
    .bind(based_in)
    .bind(kb)
    .execute(pool)
    .await?;
    let (view, _plain) =
        utopia_store::tokens::issue(pool, user, TOKEN_NAME, "write", None, None).await?;
    Ok(Base {
        kb,
        org,
        user,
        token: view.id,
        company_type,
        based_in,
    })
}

async fn teardown(pool: &PgPool, b: &Base) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM knowledge_bases WHERE id = $1")
        .bind(b.kb)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM organizations WHERE id = $1")
        .bind(b.org)
        .execute(pool)
        .await?;
    Ok(())
}

fn write(b: &Base) -> TokenWrite {
    TokenWrite {
        kb_id: b.kb,
        owner_id: b.user,
        token_id: b.token,
        rationale: None,
        batch_id: None,
        breaker: false,
    }
}

async fn entity(pool: &PgPool, b: &Base, name: &str) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO entities (id, kb_id, type_id, canonical_name) VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(b.kb)
    .bind(b.company_type)
    .bind(name)
    .execute(pool)
    .await?;
    Ok(id)
}

/// 一条活着、没有终点的事实：`s based in o`
async fn fact(pool: &PgPool, b: &Base, s: Uuid, o: Uuid) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO facts (id, kb_id, subject_id, predicate_id, object_id, attested_from)
         VALUES ($1, $2, $3, $4, $5, NULL)",
    )
    .bind(id)
    .bind(b.kb)
    .bind(s)
    .bind(b.based_in)
    .bind(o)
    .execute(pool)
    .await?;
    Ok(id)
}

/// 一条新的活事实，主语与宾语各是一个新实体
async fn new_fact(pool: &PgPool, b: &Base, name: &str) -> anyhow::Result<Uuid> {
    let s = entity(pool, b, name).await?;
    let o = entity(pool, b, &format!("{name} City")).await?;
    fact(pool, b, s, o).await
}

async fn invalidate(pool: &PgPool, fact_id: Uuid) -> anyhow::Result<()> {
    sqlx::query("UPDATE facts SET invalidated_at = now() WHERE id = $1")
        .bind(fact_id)
        .execute(pool)
        .await?;
    Ok(())
}

async fn end(pool: &PgPool, fact_id: Uuid) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE facts SET valid_to = '2024-01-01T00:00:00Z', valid_to_precision = 'day'
          WHERE id = $1",
    )
    .bind(fact_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// 一份文档里的一个分块，回（文档，分块）
async fn chunk(pool: &PgPool, b: &Base) -> anyhow::Result<(Uuid, Uuid)> {
    let (document, chunk) = (Uuid::now_v7(), Uuid::now_v7());
    sqlx::query(
        "INSERT INTO documents (id, kb_id, filename, sha256) VALUES ($1, $2, 'd', $1::text)",
    )
    .bind(document)
    .bind(b.kb)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO chunks (id, kb_id, document_id, seq, text) VALUES ($1, $2, $3, 0, 'acme')",
    )
    .bind(chunk)
    .bind(b.kb)
    .bind(document)
    .execute(pool)
    .await?;
    Ok((document, chunk))
}

async fn conflict(pool: &PgPool, b: &Base, old: Uuid, new: Uuid) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO fact_conflicts (id, kb_id, old_fact_id, new_fact_id, reason)
         VALUES ($1, $2, $3, $4, 'functional')",
    )
    .bind(id)
    .bind(b.kb)
    .bind(old)
    .bind(new)
    .execute(pool)
    .await?;
    Ok(id)
}

async fn violation(pool: &PgPool, b: &Base, left: Uuid, right: Uuid) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO axiom_violations (id, kb_id, kind, left_fact, right_fact)
         VALUES ($1, $2, 'functional', $3, $4)",
    )
    .bind(id)
    .bind(b.kb)
    .bind(left)
    .bind(right)
    .execute(pool)
    .await?;
    Ok(id)
}

async fn defect(pool: &PgPool, b: &Base) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO ontology_defects (id, kb_id, kind, subject)
         VALUES ($1, $2, 'inverse_of_itself', $3)",
    )
    .bind(id)
    .bind(b.kb)
    .bind(b.based_in)
    .execute(pool)
    .await?;
    Ok(id)
}

async fn errata(pool: &PgPool, b: &Base, fact_id: Uuid, status: &str) -> anyhow::Result<Uuid> {
    let (document, _) = chunk(pool, b).await?;
    let (run, id) = (Uuid::now_v7(), Uuid::now_v7());
    sqlx::query("INSERT INTO errata_runs (id, kb_id, document_id) VALUES ($1, $2, $3)")
        .bind(run)
        .bind(b.kb)
        .bind(document)
        .execute(pool)
        .await?;
    sqlx::query(
        "INSERT INTO errata_actions (id, kb_id, run_id, document_id, fact_id, action, reason, status)
         VALUES ($1, $2, $3, $4, $5, 'retract', 'not in the text', $6)",
    )
    .bind(id)
    .bind(b.kb)
    .bind(run)
    .bind(document)
    .bind(fact_id)
    .bind(status)
    .execute(pool)
    .await?;
    Ok(id)
}

async fn phrase(pool: &PgPool, b: &Base, phrase: &str) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO phrase_bindings (id, kb_id, phrase, subject_type_id, status)
         VALUES ($1, $2, $3, $4, 'undecided')",
    )
    .bind(id)
    .bind(b.kb)
    .bind(phrase)
    .bind(b.company_type)
    .execute(pool)
    .await?;
    Ok(id)
}

async fn rule(pool: &PgPool, b: &Base, phrase: &str) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO implication_rules (id, kb_id, trigger, phrase, conclude_property_id, status)
         VALUES ($1, $2, 'phrase', $3, $4, 'proposed')",
    )
    .bind(id)
    .bind(b.kb)
    .bind(phrase)
    .bind(b.based_in)
    .execute(pool)
    .await?;
    Ok(id)
}

async fn pending(pool: &PgPool, b: &Base) -> anyhow::Result<Uuid> {
    let (_, chunk) = chunk(pool, b).await?;
    let s = entity(pool, b, "Mira").await?;
    let o = entity(pool, b, "Lisbon").await?;
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO pending_facts (id, kb_id, subject_id, predicate_id, object_id, chunk_id,
                                    proposed_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(id)
    .bind(b.kb)
    .bind(s)
    .bind(b.based_in)
    .bind(o)
    .bind(chunk)
    .bind(b.user)
    .execute(pool)
    .await?;
    Ok(id)
}

/// 一对等人的重复项，回（审核行，左，右）
async fn pair(pool: &PgPool, b: &Base, name: &str) -> anyhow::Result<(Uuid, Uuid, Uuid)> {
    let left = entity(pool, b, name).await?;
    let right = entity(pool, b, &format!("{name} Inc")).await?;
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO resolution_reviews (id, kb_id, left_id, right_id, score, reason, stage)
         VALUES ($1, $2, $3, $4, 0.5, 'namesake', 'human')",
    )
    .bind(id)
    .bind(b.kb)
    .bind(left)
    .bind(right)
    .execute(pool)
    .await?;
    Ok((id, left, right))
}

/// 治理 agent 的一笔，指着一条审核行
async fn governor(pool: &PgPool, b: &Base, review: Uuid, status: &str) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO agent_decisions (id, kb_id, run_id, target_kind, target_id, action, status)
         VALUES ($1, $2, $3, 'review', $4, 'merge', $5)",
    )
    .bind(id)
    .bind(b.kb)
    .bind(Uuid::now_v7())
    .bind(review)
    .bind(status)
    .execute(pool)
    .await?;
    Ok(id)
}

async fn mapping(pool: &PgPool, b: &Base, concept: &str) -> anyhow::Result<Uuid> {
    let concept = entity(pool, b, concept).await?;
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO concept_mappings (id, kb_id, concept_id, source, table_name)
         VALUES ($1, $2, $3, 'warehouse', 'orders')",
    )
    .bind(id)
    .bind(b.kb)
    .bind(concept)
    .execute(pool)
    .await?;
    Ok(id)
}

async fn ontology_proposal(
    pool: &PgPool,
    b: &Base,
    key: &str,
    proposed_by: &str,
) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO ontology_proposals (id, kb_id, section, key, payload, proposed_by)
         VALUES ($1, $2, 'entity_types', $3, $4, $5)",
    )
    .bind(id)
    .bind(b.kb)
    .bind(key)
    .bind(json!({ "key": key, "label": "City" }))
    .bind(proposed_by)
    .execute(pool)
    .await?;
    Ok(id)
}

async fn set_status(pool: &PgPool, table: &str, id: Uuid, status: &str) -> anyhow::Result<()> {
    sqlx::query(&format!("UPDATE {table} SET status = $2 WHERE id = $1"))
        .bind(id)
        .bind(status)
        .execute(pool)
        .await?;
    Ok(())
}

/// 这个库的台账，最早的在前
async fn ledger(pool: &PgPool, kb: Uuid) -> anyhow::Result<Vec<TokenAgentAction>> {
    let mut rows = token_agent_actions::list(pool, kb, 500, 0).await?;
    rows.reverse();
    Ok(rows)
}

/// 这个库的全部审计行：动作、actor、target_kind、target_id、detail，最早的在前
#[allow(clippy::type_complexity)]
async fn audits(
    pool: &PgPool,
    kb: Uuid,
) -> anyhow::Result<Vec<(String, Option<Uuid>, String, Option<Uuid>, Value)>> {
    Ok(sqlx::query_as(
        "SELECT action, actor_id, target_kind, target_id, detail FROM audit_events
          WHERE kb_id = $1 ORDER BY created_at, id",
    )
    .bind(kb)
    .fetch_all(pool)
    .await?)
}

/// 这个库在台账与审计**之外**的一切：每一张带 `kb_id` 的表里它的每一行（整行转成 jsonb
/// 文本），加上没有 `kb_id` 的证据表、库自己的那一行、令牌的那一行，和指着这个库的任务数。
/// 表是从目录里现查的，所以以后新加的表自动在里面
async fn everything_else(pool: &PgPool, b: &Base) -> anyhow::Result<Vec<(String, Vec<String>)>> {
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT c.table_name::text
           FROM information_schema.columns c
           JOIN information_schema.tables t
             ON t.table_schema = c.table_schema AND t.table_name = c.table_name
          WHERE c.table_schema = current_schema() AND c.column_name = 'kb_id'
            AND t.table_type = 'BASE TABLE'
            AND c.table_name NOT IN ('token_agent_actions', 'audit_events')
          ORDER BY 1",
    )
    .fetch_all(pool)
    .await?;
    assert!(
        tables.len() > 20 && tables.iter().any(|t| t == "facts"),
        "the snapshot must cover the base's tables, found {tables:?}"
    );
    let mut out = Vec::new();
    for table in tables {
        let rows: Vec<String> = sqlx::query_scalar(&format!(
            "SELECT to_jsonb(x)::text FROM \"{table}\" x WHERE x.kb_id = $1 ORDER BY 1"
        ))
        .bind(b.kb)
        .fetch_all(pool)
        .await?;
        out.push((table, rows));
    }
    for (name, sql, id) in [
        (
            "fact_evidence",
            "SELECT to_jsonb(fe)::text FROM fact_evidence fe JOIN facts f ON f.id = fe.fact_id
              WHERE f.kb_id = $1 ORDER BY 1",
            b.kb,
        ),
        (
            "knowledge_bases",
            "SELECT to_jsonb(k)::text FROM knowledge_bases k WHERE k.id = $1",
            b.kb,
        ),
        (
            "personal_tokens",
            "SELECT to_jsonb(p)::text FROM personal_tokens p WHERE p.id = $1",
            b.token,
        ),
        (
            "jobs",
            "SELECT count(*)::text FROM jobs WHERE payload ->> 'kb_id' = $1::text",
            b.kb,
        ),
    ] {
        let rows: Vec<String> = sqlx::query_scalar(sql).bind(id).fetch_all(pool).await?;
        out.push((name.to_string(), rows));
    }
    Ok(out)
}

/// 每一档都有东西等着的一个库，回其中一条事实与一处缺陷
async fn a_busy_base(pool: &PgPool, b: &Base) -> anyhow::Result<(Uuid, Uuid)> {
    let old = new_fact(pool, b, "Acme").await?;
    let new = new_fact(pool, b, "Acme Corp").await?;
    let (_, chunk) = chunk(pool, b).await?;
    sqlx::query("INSERT INTO fact_evidence (fact_id, chunk_id, quote) VALUES ($1, $2, 'acme')")
        .bind(old)
        .bind(chunk)
        .execute(pool)
        .await?;
    conflict(pool, b, old, new).await?;
    violation(pool, b, old, new).await?;
    let defect = defect(pool, b).await?;
    errata(pool, b, old, "held").await?;
    phrase(pool, b, "is based in").await?;
    rule(pool, b, "moved to").await?;
    pending(pool, b).await?;
    let (review, _, _) = pair(pool, b, "Orion").await?;
    governor(pool, b, review, "proposed").await?;
    mapping(pool, b, "Revenue").await?;
    ontology_proposal(pool, b, "city", "agent").await?;
    Ok((old, defect))
}

// ───────────────────────── 1. 一条提议就是两行 ─────────────────────────

async fn one_proposal_two_rows(pool: &PgPool, b: &Base) -> anyhow::Result<()> {
    let (_, defect) = a_busy_base(pool, b).await?;
    let before = everything_else(pool, b).await?;
    assert!(ledger(pool, b.kb).await?.is_empty());
    assert!(audits(pool, b.kb).await?.is_empty());

    let key = defect.to_string();
    let payload = json!({
        "request": { "resolution": "accepted" },
        "target_label": "inverse_of_itself: based in",
        "action": "accepted",
        "why": "the relation is symmetric in this corpus",
    });
    let out = token_agent_actions::propose(
        pool,
        &write(b),
        &Proposal {
            operation: "defect.decide",
            target_kind: "ontology_defect",
            target_id: defect,
            target_key: &key,
            payload: payload.clone(),
        },
    )
    .await?;
    assert!(!out.existing);
    assert_eq!(out.follow_up_error, None);

    // 台账：正好一行，proposed / propose_only，指着这处缺陷，请求原样存着
    let rows = ledger(pool, b.kb).await?;
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.id, out.action.id);
    assert_eq!(row.operation, "defect.decide");
    assert_eq!(row.target_kind, "ontology_defect");
    assert_eq!(row.target_id, defect);
    assert_eq!(row.outcome, "proposed");
    assert_eq!(row.gate_reason.as_deref(), Some("propose_only"));
    assert_eq!(row.owner_id, b.user);
    assert_eq!(row.owner_name.as_deref(), Some("Token Owner"));
    assert_eq!(row.token_id, b.token);
    assert_eq!(row.token_name, TOKEN_NAME);
    assert_eq!(row.payload, payload);
    assert_eq!((row.batch_id, row.merge_id), (None, None));
    assert_eq!(
        (row.state.clone(), row.state_at, row.state_by),
        (None, None, None)
    );
    let stored_key: Option<String> =
        sqlx::query_scalar("SELECT target_key FROM token_agent_actions WHERE id = $1")
            .bind(row.id)
            .fetch_one(pool)
            .await?;
    assert_eq!(stored_key.as_deref(), Some(key.as_str()));

    // 审计：正好一行 agent.proposal，actor 是令牌的主人，detail 正好那六个键
    assert_eq!(
        audits(pool, b.kb).await?,
        vec![(
            "agent.proposal".to_string(),
            Some(b.user),
            "agent_action".to_string(),
            Some(row.id),
            json!({
                "operation": "defect.decide",
                "outcome": "proposed",
                "reason": "propose_only",
                "token_id": b.token,
                "token_name": TOKEN_NAME,
                "action_id": row.id,
            })
        )]
    );

    // 别的表一行没变，一个任务没排（E1）
    let after = everything_else(pool, b).await?;
    for (was, is) in before.iter().zip(&after) {
        assert_eq!(
            was, is,
            "the table {} changed on a propose-only call",
            was.0
        );
    }
    assert_eq!(before.len(), after.len());

    // 按 id 读得到，别的库读不到
    let read = token_agent_actions::proposal(pool, b.kb, row.id).await?;
    assert_eq!(read.map(|r| r.id), Some(row.id));
    assert!(token_agent_actions::proposal(pool, Uuid::now_v7(), row.id)
        .await?
        .is_none());
    Ok(())
}

#[tokio::test]
async fn a_propose_only_proposal_writes_one_ledger_row_and_one_audit_row_and_nothing_else(
) -> anyhow::Result<()> {
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let pool = PgPool::connect(&url).await?;
    let b = base(&pool).await?;
    let outcome = one_proposal_two_rows(&pool, &b).await;
    teardown(&pool, &b).await?;
    outcome
}

// ───────────────────────── 2. 一个目标一条开着的提议 ─────────────────────────

fn on_fact<'a>(operation: &'a str, fact: Uuid, key: &'a str, action: &str) -> Proposal<'a> {
    Proposal {
        operation,
        target_kind: "fact",
        target_id: fact,
        target_key: key,
        payload: json!({ "request": {}, "target_label": "Acme based in Acme City", "action": action }),
    }
}

async fn one_open_proposal(pool: &PgPool, b: &Base) -> anyhow::Result<()> {
    let fact_id = new_fact(pool, b, "Acme").await?;
    let key = fact_id.to_string();
    let reject = on_fact("fact.reject", fact_id, &key, "reject");

    // 重复调用：第二次什么都不写，拿回第一条
    let first = token_agent_actions::propose(pool, &write(b), &reject).await?;
    assert!(!first.existing);
    let again = token_agent_actions::propose(pool, &write(b), &reject).await?;
    assert!(again.existing);
    assert_eq!(again.action.id, first.action.id);
    assert_eq!(again.action.outcome, "proposed");
    assert_eq!(ledger(pool, b.kb).await?.len(), 1);
    assert_eq!(audits(pool, b.kb).await?.len(), 1);

    // 同时到的一群调用（另一条事实）：正好一行，正好一个不是 existing，全都拿到同一条
    let other = new_fact(pool, b, "Orion").await?;
    let other_key = other.to_string();
    let close = on_fact("fact.close", other, &other_key, "close");
    let w = write(b);
    let calls = (0..8).map(|_| token_agent_actions::propose(pool, &w, &close));
    let outs: Vec<_> = futures_all(calls)
        .await
        .into_iter()
        .collect::<Result<_, _>>()?;
    assert_eq!(outs.iter().filter(|o| !o.existing).count(), 1, "{outs:?}");
    let winner = outs
        .iter()
        .find(|o| !o.existing)
        .expect("one wrote")
        .action
        .id;
    assert!(outs.iter().all(|o| o.action.id == winner));
    assert_eq!(ledger(pool, b.kb).await?.len(), 2);
    assert_eq!(audits(pool, b.kb).await?.len(), 2);

    // 别的库里同一把钥匙不相干：那边照样开得出自己的一条
    let elsewhere = base(pool).await?;
    let there = token_agent_actions::propose(pool, &write(&elsewhere), &reject).await;
    let there_fresh = there.as_ref().map(|o| !o.existing).unwrap_or(false);
    teardown(pool, &elsewhere).await?;
    there?;
    assert!(there_fresh, "another base holds its own open proposal");

    // 人答掉之后，这个目标上可以再开一条
    assert!(
        token_agent_actions::answer(pool, b.kb, first.action.id, "declined", b.user, None).await?
    );
    let reopened = token_agent_actions::propose(pool, &write(b), &reject).await?;
    assert!(!reopened.existing);
    assert_ne!(reopened.action.id, first.action.id);
    assert_eq!(ledger(pool, b.kb).await?.len(), 3);
    Ok(())
}

/// 把一群 future 同时推着跑，等全部回来（不引入新的依赖：每个各占一个任务）
async fn futures_all<F, T>(calls: impl Iterator<Item = F>) -> Vec<T>
where
    F: std::future::Future<Output = T>,
{
    let mut pinned: Vec<_> = calls.map(Box::pin).collect();
    let mut done: Vec<Option<T>> = pinned.iter().map(|_| None).collect();
    std::future::poll_fn(|cx| {
        let mut waiting = false;
        for (call, slot) in pinned.iter_mut().zip(done.iter_mut()) {
            if slot.is_none() {
                match call.as_mut().poll(cx) {
                    std::task::Poll::Ready(out) => *slot = Some(out),
                    std::task::Poll::Pending => waiting = true,
                }
            }
        }
        if waiting {
            std::task::Poll::Pending
        } else {
            std::task::Poll::Ready(())
        }
    })
    .await;
    done.into_iter()
        .map(|d| d.expect("polled to the end"))
        .collect()
}

#[tokio::test]
async fn a_repeated_or_simultaneous_proposal_returns_the_open_row() -> anyhow::Result<()> {
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let pool = PgPool::connect(&url).await?;
    let b = base(&pool).await?;
    let outcome = one_open_proposal(&pool, &b).await;
    teardown(&pool, &b).await?;
    outcome
}

// ───────────────────────── 3. 不同的操作各开各的 ─────────────────────────

async fn two_operations(pool: &PgPool, b: &Base) -> anyhow::Result<()> {
    let fact_id = new_fact(pool, b, "Acme").await?;
    let untouched = new_fact(pool, b, "Orion").await?;
    let key = fact_id.to_string();

    let confirm = token_agent_actions::propose(
        pool,
        &write(b),
        &on_fact("fact.confirm", fact_id, &key, "confirm"),
    )
    .await?;
    let reject = token_agent_actions::propose(
        pool,
        &write(b),
        &on_fact("fact.reject", fact_id, &key, "reject"),
    )
    .await?;
    assert!(!confirm.existing && !reject.existing);
    assert_ne!(confirm.action.id, reject.action.id);
    let open: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM token_agent_actions
          WHERE kb_id = $1 AND target_key = $2 AND outcome = 'proposed' AND state IS NULL",
    )
    .bind(b.kb)
    .bind(&key)
    .fetch_one(pool)
    .await?;
    assert_eq!(open, 2);

    // 挂到卡片上的那一列：两条都在，最早的在前，各带各的 action
    let on_cards =
        token_agent_actions::open_proposals_for(pool, b.kb, "fact", &[fact_id, untouched]).await?;
    assert_eq!(
        on_cards.len(),
        1,
        "a target with no open proposal is absent"
    );
    let views: &Vec<AgentProposalView> = &on_cards[&fact_id];
    assert_eq!(
        views
            .iter()
            .map(|v| (v.id, v.operation.as_str(), v.action.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            (confirm.action.id, "fact.confirm", Some("confirm")),
            (reject.action.id, "fact.reject", Some("reject")),
        ]
    );
    assert_eq!(views[0].reason.as_deref(), Some("propose_only"));
    assert_eq!(views[0].owner_name.as_deref(), Some("Token Owner"));
    assert_eq!(views[0].token_name, TOKEN_NAME);
    assert_eq!(json!(views[0])["action"], json!("confirm"));

    // 别的目标种类、别的库、空的 id 列表：什么都没有
    assert!(
        token_agent_actions::open_proposals_for(pool, b.kb, "conflict", &[fact_id])
            .await?
            .is_empty()
    );
    assert!(
        token_agent_actions::open_proposals_for(pool, Uuid::now_v7(), "fact", &[fact_id])
            .await?
            .is_empty()
    );
    assert!(
        token_agent_actions::open_proposals_for(pool, b.kb, "fact", &[])
            .await?
            .is_empty()
    );

    // 答掉一条，卡片上只剩另一条
    assert!(
        token_agent_actions::answer(pool, b.kb, confirm.action.id, "approved", b.user, None)
            .await?
    );
    let left = token_agent_actions::open_proposals_for(pool, b.kb, "fact", &[fact_id]).await?;
    assert_eq!(
        left[&fact_id].iter().map(|v| v.id).collect::<Vec<_>>(),
        vec![reject.action.id]
    );

    // 没有 action 的提议（合并、撤回那一类）序列化出来没有这个键
    let no_action = token_agent_actions::propose(
        pool,
        &write(b),
        &Proposal {
            operation: "fact.close",
            target_kind: "fact",
            target_id: untouched,
            target_key: &untouched.to_string(),
            payload: json!({ "request": {} }),
        },
    )
    .await?;
    let bare = token_agent_actions::open_proposals_for(pool, b.kb, "fact", &[untouched]).await?;
    let bare = json!(bare[&untouched][0]);
    assert_eq!(bare["id"], json!(no_action.action.id));
    assert_eq!(
        bare.as_object()
            .expect("an object")
            .keys()
            .collect::<Vec<_>>(),
        [
            "created_at",
            "id",
            "operation",
            "owner_name",
            "reason",
            "token_name"
        ]
    );
    Ok(())
}

#[tokio::test]
async fn two_operations_on_one_target_may_both_be_open() -> anyhow::Result<()> {
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let pool = PgPool::connect(&url).await?;
    let b = base(&pool).await?;
    let outcome = two_operations(&pool, &b).await;
    teardown(&pool, &b).await?;
    outcome
}

// ───────────────────────── 4. 目标的状态从带类型的行读 ─────────────────────────

use TargetState::{Decided, Missing, Open};

async fn typed_states(pool: &PgPool, b: &Base, other: &Base) -> anyhow::Result<()> {
    let none = json!({});
    // 一次读：这个库、这种操作、这个目标、这份请求、（可选的）指纹
    macro_rules! state {
        ($op:expr, $id:expr) => {
            review_targets::target_state(pool, b.kb, $op, $id, &none, None).await?
        };
        ($op:expr, $id:expr, $req:expr) => {
            review_targets::target_state(pool, b.kb, $op, $id, &$req, None).await?
        };
        ($op:expr, $id:expr, $req:expr, $fp:expr) => {
            review_targets::target_state(pool, b.kb, $op, $id, &$req, Some(&$fp)).await?
        };
    }
    // 每一种操作拿一个此刻开着的目标：别的库读它是 Missing，也说不出它的名字
    let mut checked_elsewhere = 0;
    macro_rules! elsewhere {
        ($op:expr, $id:expr, $req:expr) => {
            assert_eq!(
                review_targets::target_state(pool, b.kb, $op, $id, &$req, None).await?,
                Open,
                "{} is open in its own base",
                $op
            );
            assert_eq!(
                review_targets::target_state(pool, other.kb, $op, $id, &$req, None).await?,
                Missing,
                "{}: an id of another base is missing",
                $op
            );
            assert!(
                matches!(
                    review_targets::target_label(pool, other.kb, $op, $id, &$req).await,
                    Err(utopia_core::AppError::NotFound)
                ),
                "{}: another base cannot name the target",
                $op
            );
            checked_elsewhere += 1;
        };
    }

    // ── 事实：确认、拒绝、闭合 ──
    let live = new_fact(pool, b, "Acme").await?;
    let ended = new_fact(pool, b, "Borealis").await?;
    end(pool, ended).await?;
    let gone = new_fact(pool, b, "Cygnus").await?;
    invalidate(pool, gone).await?;
    for op in ["fact.confirm", "fact.reject", "fact.close"] {
        assert_eq!(state!(op, live), Open, "{op} on a live open fact");
        assert_eq!(state!(op, Uuid::now_v7()), Missing, "{op} on an unknown id");
        elsewhere!(op, live, none);
    }
    // 确认没有「定了」：作废的就是没有；有终点的活事实照样能确认
    assert_eq!(state!("fact.confirm", gone), Missing);
    assert_eq!(state!("fact.confirm", ended), Open);
    assert_eq!(state!("fact.reject", gone), Decided);
    assert_eq!(state!("fact.reject", ended), Open);
    // 闭合：作废的是定了；活着但已经有终点的，人的路由答 not found
    assert_eq!(state!("fact.close", gone), Decided);
    assert_eq!(state!("fact.close", ended), Missing);

    // ── 矛盾 ──
    let (old, new) = (
        new_fact(pool, b, "Draco").await?,
        new_fact(pool, b, "Draco Ltd").await?,
    );
    let open_conflict = conflict(pool, b, old, new).await?;
    assert_eq!(state!("conflict.resolve", open_conflict), Open);
    assert_eq!(state!("conflict.resolve", Uuid::now_v7()), Missing);
    elsewhere!("conflict.resolve", open_conflict, none);
    let print = review_targets::fingerprint(pool, b.kb, "conflict.resolve", open_conflict)
        .await?
        .expect("a conflict has a fingerprint");
    assert_eq!(print, json!({ "old_fact_id": old, "new_fact_id": new }));
    assert_eq!(state!("conflict.resolve", open_conflict, none, print), Open);
    // 同一个 id、两条事实换了：内容已经不是提议时的那一份
    let swapped = json!({ "old_fact_id": new, "new_fact_id": old });
    assert_eq!(
        state!("conflict.resolve", open_conflict, none, swapped),
        Decided
    );
    let replaced = new_fact(pool, b, "Draco GmbH").await?;
    sqlx::query("UPDATE fact_conflicts SET new_fact_id = $2 WHERE id = $1")
        .bind(open_conflict)
        .bind(replaced)
        .execute(pool)
        .await?;
    assert_eq!(
        state!("conflict.resolve", open_conflict, none, print),
        Decided
    );
    assert_eq!(state!("conflict.resolve", open_conflict), Open);
    // 一条事实被撤了：没有可裁的了
    invalidate(pool, replaced).await?;
    assert_eq!(state!("conflict.resolve", open_conflict), Decided);
    for status in ["resolved", "withdrawn"] {
        let (o, n) = (
            new_fact(pool, b, "Eris").await?,
            new_fact(pool, b, "Eris Co").await?,
        );
        let c = conflict(pool, b, o, n).await?;
        set_status(pool, "fact_conflicts", c, status).await?;
        assert_eq!(
            state!("conflict.resolve", c),
            Decided,
            "a {status} conflict"
        );
    }

    // ── 公理违规 ──
    let (left, right) = (
        new_fact(pool, b, "Fornax").await?,
        new_fact(pool, b, "Fornax AG").await?,
    );
    let open_violation = violation(pool, b, left, right).await?;
    let retract = |fact: Uuid| json!({ "resolution": "fact_retracted", "fact_id": fact });
    let close = json!({ "resolution": "fact_closed", "close_at": "2024-01-01T00:00:00Z" });
    let accept = json!({ "resolution": "accepted" });
    let relax = json!({ "resolution": "axiom_relaxed" });
    assert_eq!(state!("violation.decide", Uuid::now_v7(), accept), Missing);
    for req in [&accept, &relax, &close, &retract(left), &retract(right)] {
        assert_eq!(
            state!("violation.decide", open_violation, req),
            Open,
            "{req}"
        );
    }
    elsewhere!("violation.decide", open_violation, accept);
    // 请求指着的那条事实有了终点：撤它、闭它都不成立了，认可与改公理照旧
    end(pool, left).await?;
    assert_eq!(
        state!("violation.decide", open_violation, retract(left)),
        Decided
    );
    assert_eq!(state!("violation.decide", open_violation, close), Decided);
    assert_eq!(
        state!("violation.decide", open_violation, retract(right)),
        Open
    );
    assert_eq!(state!("violation.decide", open_violation, accept), Open);
    assert_eq!(state!("violation.decide", open_violation, relax), Open);
    // 另一条被撤了
    invalidate(pool, right).await?;
    assert_eq!(
        state!("violation.decide", open_violation, retract(right)),
        Decided
    );
    assert_eq!(state!("violation.decide", open_violation, accept), Open);
    // 单事实的违规：不点名也知道撤哪条
    let lone = new_fact(pool, b, "Gemini").await?;
    let single = violation(pool, b, lone, lone).await?;
    let unnamed = json!({ "resolution": "fact_retracted" });
    assert_eq!(state!("violation.decide", single, unnamed), Open);
    invalidate(pool, lone).await?;
    assert_eq!(state!("violation.decide", single, unnamed), Decided);
    set_status(pool, "axiom_violations", open_violation, "resolved").await?;
    assert_eq!(state!("violation.decide", open_violation, accept), Decided);

    // ── 本体缺陷 ──
    let open_defect = defect(pool, b).await?;
    assert_eq!(state!("defect.decide", open_defect), Open);
    assert_eq!(state!("defect.decide", Uuid::now_v7()), Missing);
    elsewhere!("defect.decide", open_defect, none);

    // ── 勘误 ──
    let held = errata(pool, b, live, "held").await?;
    assert_eq!(state!("errata.decide", held), Open);
    assert_eq!(state!("errata.decide", Uuid::now_v7()), Missing);
    elsewhere!("errata.decide", held, none);
    for status in ["applied", "refused", "rejected"] {
        let answered = errata(pool, b, live, status).await?;
        assert_eq!(
            state!("errata.decide", answered),
            Decided,
            "a {status} errata action"
        );
    }

    // ── 本体提案 ──
    let by_agent = ontology_proposal(pool, b, "city", "agent").await?;
    assert_eq!(
        review_targets::ontology_proposal_id(pool, b.kb, "entity_types", "city").await?,
        Some(by_agent)
    );
    assert_eq!(
        review_targets::ontology_proposal_id(pool, b.kb, "entity_types", "town").await?,
        None
    );
    assert_eq!(
        review_targets::ontology_proposal_id(pool, other.kb, "entity_types", "city").await?,
        None
    );
    assert_eq!(state!("ontology.proposal", by_agent), Open);
    assert_eq!(state!("ontology.proposal", Uuid::now_v7()), Missing);
    elsewhere!("ontology.proposal", by_agent, none);
    let stored = review_targets::fingerprint(pool, b.kb, "ontology.proposal", by_agent)
        .await?
        .expect("an ontology proposal has a fingerprint");
    assert_eq!(stored, json!({ "key": "city", "label": "City" }));
    assert_eq!(state!("ontology.proposal", by_agent, none, stored), Open);
    // 代理下一轮把同一条刷新了：id 没变，内容变了
    sqlx::query("UPDATE ontology_proposals SET payload = $2 WHERE id = $1")
        .bind(by_agent)
        .bind(json!({ "key": "city", "label": "Town" }))
        .execute(pool)
        .await?;
    assert_eq!(state!("ontology.proposal", by_agent, none, stored), Decided);
    assert_eq!(state!("ontology.proposal", by_agent), Open);
    // 不是代理提的：采纳的那条路不认它
    let by_suggest = ontology_proposal(pool, b, "harbour", "suggest").await?;
    assert_eq!(state!("ontology.proposal", by_suggest), Missing);
    for status in ["adopted", "rejected"] {
        let p = ontology_proposal(pool, b, status, "agent").await?;
        set_status(pool, "ontology_proposals", p, status).await?;
        assert_eq!(
            state!("ontology.proposal", p),
            Decided,
            "a {status} proposal"
        );
    }

    // ── 短语、规则、等点头的事实：行在就是开着，没有「定了」 ──
    let a_phrase = phrase(pool, b, "is based in").await?;
    let a_rule = rule(pool, b, "moved to").await?;
    let a_pending = pending(pool, b).await?;
    for (op, id) in [
        ("alignment.phrase", a_phrase),
        ("alignment.rule", a_rule),
        ("pending.nod", a_pending),
    ] {
        assert_eq!(state!(op, id), Open, "{op}");
        assert_eq!(state!(op, Uuid::now_v7()), Missing, "{op} on an unknown id");
        elsewhere!(op, id, none);
    }
    // 人已经定过的短语与规则照样是开着：批准时照做（E8）
    sqlx::query("UPDATE phrase_bindings SET status = 'none', decided_by = 'person' WHERE id = $1")
        .bind(a_phrase)
        .execute(pool)
        .await?;
    set_status(pool, "implication_rules", a_rule, "approved").await?;
    assert_eq!(state!("alignment.phrase", a_phrase), Open);
    assert_eq!(state!("alignment.rule", a_rule), Open);
    // 点过头的事实，行就没了
    let nodded = pending(pool, b).await?;
    sqlx::query("DELETE FROM pending_facts WHERE id = $1")
        .bind(nodded)
        .execute(pool)
        .await?;
    assert_eq!(state!("pending.nod", nodded), Missing);

    // ── 治理 agent 的一笔 ──
    let (review, _, _) = pair(pool, b, "Hydra").await?;
    let proposed = governor(pool, b, review, "proposed").await?;
    assert_eq!(state!("governor.answer", proposed), Open);
    assert_eq!(state!("governor.answer", Uuid::now_v7()), Missing);
    elsewhere!("governor.answer", proposed, none);
    let (settled_review, _, _) = pair(pool, b, "Indus").await?;
    let applied = governor(pool, b, settled_review, "applied").await?;
    set_status(pool, "resolution_reviews", settled_review, "merged").await?;
    assert_eq!(
        state!("governor.answer", applied),
        Open,
        "an applied decision can be reverted"
    );
    for status in ["accepted", "overridden", "reverted", "superseded"] {
        let d = governor(pool, b, review, status).await?;
        assert_eq!(state!("governor.answer", d), Decided, "a {status} decision");
    }
    // 建议还开着，它指着的那一对已经有人裁了
    set_status(pool, "resolution_reviews", review, "kept").await?;
    assert_eq!(state!("governor.answer", proposed), Decided);

    // ── 映射：等于请求要的状态就是定了 ──
    let a_mapping = mapping(pool, b, "Revenue").await?;
    let confirmed = json!({ "status": "confirmed" });
    let rejected = json!({ "status": "rejected" });
    assert_eq!(state!("mapping.decide", a_mapping, confirmed), Open);
    assert_eq!(state!("mapping.decide", a_mapping, rejected), Open);
    assert_eq!(state!("mapping.decide", Uuid::now_v7(), confirmed), Missing);
    elsewhere!("mapping.decide", a_mapping, confirmed);

    // ── 目标是库本身的三种 ──
    for op in ["alignment.kind_word", "consistency.check", "inference.run"] {
        assert_eq!(state!(op, b.kb), Open, "{op}");
        assert_eq!(state!(op, other.kb), Missing, "{op} naming another base");
    }

    // 十三种操作都验过「别的库读不到」
    assert_eq!(checked_elsewhere, 13);
    assert_eq!(
        review_targets::fingerprint(pool, other.kb, "conflict.resolve", open_conflict).await?,
        None
    );
    assert_eq!(
        review_targets::fingerprint(pool, other.kb, "ontology.proposal", by_agent).await?,
        None
    );
    assert_eq!(
        review_targets::fingerprint(pool, b.kb, "fact.reject", live).await?,
        None
    );

    // 映射定了之后
    set_status(pool, "concept_mappings", a_mapping, "confirmed").await?;
    assert_eq!(state!("mapping.decide", a_mapping, confirmed), Decided);
    assert_eq!(state!("mapping.decide", a_mapping, rejected), Open);
    set_status(pool, "ontology_defects", open_defect, "resolved").await?;
    assert_eq!(state!("defect.decide", open_defect), Decided);

    // 认不出的操作名是错误，不是一个猜出来的状态
    assert!(
        review_targets::target_state(pool, b.kb, "review.merge", live, &none, None)
            .await
            .is_err()
    );

    // ── 用话说出目标 ──
    macro_rules! label {
        ($op:expr, $id:expr) => {
            review_targets::target_label(pool, b.kb, $op, $id, &none).await?
        };
    }
    assert_eq!(label!("fact.confirm", live), "Acme based in Acme City");
    assert_eq!(
        label!("conflict.resolve", open_conflict),
        "Draco based in Draco City / Draco GmbH based in Draco GmbH City"
    );
    assert_eq!(
        label!("violation.decide", open_violation),
        "functional: Fornax based in Fornax City / Fornax AG based in Fornax AG City"
    );
    assert_eq!(
        label!("defect.decide", open_defect),
        "inverse_of_itself: based in"
    );
    assert_eq!(
        label!("errata.decide", held),
        "retract: Acme based in Acme City"
    );
    assert_eq!(
        label!("alignment.phrase", a_phrase),
        "is based in (Organization → any)"
    );
    assert_eq!(label!("alignment.rule", a_rule), "moved to ⇒ based in");
    assert_eq!(
        label!("pending.nod", a_pending),
        "Mira based in Lisbon (remembered by Token Owner)"
    );
    assert_eq!(label!("governor.answer", proposed), "Hydra / Hydra Inc");
    assert_eq!(
        label!("mapping.decide", a_mapping),
        "Revenue → warehouse.orders"
    );
    assert_eq!(label!("ontology.proposal", by_agent), "entity_types: city");
    assert_eq!(label!("consistency.check", b.kb), "Proposal Base");
    assert_eq!(label!("inference.run", b.kb), "Proposal Base");
    assert_eq!(
        review_targets::target_label(
            pool,
            b.kb,
            "alignment.kind_word",
            b.kb,
            &json!({ "kind_word": "startup", "class": "org" })
        )
        .await?,
        "startup"
    );
    assert!(matches!(
        review_targets::target_label(pool, b.kb, "fact.reject", Uuid::now_v7(), &none).await,
        Err(utopia_core::AppError::NotFound)
    ));

    // 读状态、取指纹、说名字都只读：台账与审计里一行没有
    assert!(ledger(pool, b.kb).await?.is_empty());
    assert!(audits(pool, b.kb).await?.is_empty());
    Ok(())
}

/// 读状态不改任何东西：同一个库前后两份快照之间只隔着读
async fn reading_changes_nothing(pool: &PgPool, b: &Base, other: &Base) -> anyhow::Result<()> {
    let (old, defect) = a_busy_base(pool, b).await?;
    let before = everything_else(pool, b).await?;
    let targets: Vec<(String, Uuid)> = sqlx::query_as(
        "SELECT 'conflict.resolve', id FROM fact_conflicts WHERE kb_id = $1
         UNION ALL SELECT 'violation.decide', id FROM axiom_violations WHERE kb_id = $1
         UNION ALL SELECT 'errata.decide', id FROM errata_actions WHERE kb_id = $1
         UNION ALL SELECT 'alignment.phrase', id FROM phrase_bindings WHERE kb_id = $1
         UNION ALL SELECT 'alignment.rule', id FROM implication_rules WHERE kb_id = $1
         UNION ALL SELECT 'pending.nod', id FROM pending_facts WHERE kb_id = $1
         UNION ALL SELECT 'governor.answer', id FROM agent_decisions WHERE kb_id = $1
         UNION ALL SELECT 'mapping.decide', id FROM concept_mappings WHERE kb_id = $1
         UNION ALL SELECT 'ontology.proposal', id FROM ontology_proposals WHERE kb_id = $1",
    )
    .bind(b.kb)
    .fetch_all(pool)
    .await?;
    assert_eq!(targets.len(), 9);
    let mut all: Vec<(String, Uuid)> = targets;
    for op in ["fact.confirm", "fact.reject", "fact.close"] {
        all.push((op.to_string(), old));
    }
    all.push(("defect.decide".to_string(), defect));
    for op in ["alignment.kind_word", "consistency.check", "inference.run"] {
        all.push((op.to_string(), b.kb));
    }
    let request = json!({ "resolution": "fact_retracted", "fact_id": old, "status": "confirmed" });
    for (op, id) in &all {
        for kb in [b.kb, other.kb] {
            review_targets::target_state(pool, kb, op, *id, &request, None).await?;
            review_targets::fingerprint(pool, kb, op, *id).await?;
            let _ = review_targets::target_label(pool, kb, op, *id, &request).await;
        }
    }
    let after = everything_else(pool, b).await?;
    for (was, is) in before.iter().zip(&after) {
        assert_eq!(was, is, "the table {} changed on a read of a target", was.0);
    }
    assert!(ledger(pool, b.kb).await?.is_empty());
    assert!(audits(pool, b.kb).await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn a_target_state_is_read_from_typed_rows_for_every_operation() -> anyhow::Result<()> {
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let pool = PgPool::connect(&url).await?;
    let b = base(&pool).await?;
    let quiet = base(&pool).await?;
    let other = base(&pool).await?;
    let mut outcome = typed_states(&pool, &b, &other).await;
    if outcome.is_ok() {
        outcome = reading_changes_nothing(&pool, &quiet, &other).await;
    }
    teardown(&pool, &b).await?;
    teardown(&pool, &quiet).await?;
    teardown(&pool, &other).await?;
    outcome
}

// ───────────────────────── 5. 落地的行跟着调用方的事务走 ─────────────────────────

async fn applied_with_the_change(pool: &PgPool, b: &Base) -> anyhow::Result<()> {
    let a_mapping = mapping(pool, b, "Revenue").await?;
    let key = a_mapping.to_string();
    let decided = Proposal {
        operation: "mapping.decide",
        target_kind: "concept_mapping",
        target_id: a_mapping,
        target_key: &key,
        payload: json!({
            "request": { "status": "confirmed" },
            "target_label": "Revenue → warehouse.orders",
            "action": "confirmed",
        }),
    };
    let status = || async {
        sqlx::query_scalar::<_, String>("SELECT status FROM concept_mappings WHERE id = $1")
            .bind(a_mapping)
            .fetch_one(pool)
            .await
    };
    let confirm = "UPDATE concept_mappings SET status = 'confirmed' WHERE id = $1";

    // 回滚：变更没了，台账行也没了
    let mut tx = pool.begin().await?;
    sqlx::query(confirm)
        .bind(a_mapping)
        .execute(&mut *tx)
        .await?;
    let unborn = token_agent_actions::record_applied(&mut tx, &write(b), &decided).await?;
    assert_eq!(unborn.outcome, "applied");
    tx.rollback().await?;
    assert_eq!(status().await?, "proposed");
    assert!(ledger(pool, b.kb).await?.is_empty());

    // 读不到令牌的名字：报错，什么都不写
    let mut tx = pool.begin().await?;
    let nameless = TokenWrite {
        token_id: Uuid::now_v7(),
        ..write(b)
    };
    assert!(
        token_agent_actions::record_applied(&mut tx, &nameless, &decided)
            .await
            .is_err()
    );
    tx.rollback().await?;
    assert!(ledger(pool, b.kb).await?.is_empty());

    // 提交：变更在，台账正好一行 applied
    let since = chrono::Utc::now() - chrono::Duration::minutes(5);
    assert_eq!(
        token_agent_actions::finalized_since(pool, b.kb, b.token, since).await?,
        0
    );
    let mut tx = pool.begin().await?;
    sqlx::query(confirm)
        .bind(a_mapping)
        .execute(&mut *tx)
        .await?;
    let action = token_agent_actions::record_applied(&mut tx, &write(b), &decided).await?;
    tx.commit().await?;
    assert_eq!(status().await?, "confirmed");
    let rows = ledger(pool, b.kb).await?;
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.id, action.id);
    assert_eq!(row.operation, "mapping.decide");
    assert_eq!(row.target_kind, "concept_mapping");
    assert_eq!(row.target_id, a_mapping);
    assert_eq!(row.outcome, "applied");
    assert_eq!(row.gate_reason, None);
    assert_eq!(row.state, None);
    assert_eq!(row.token_name, TOKEN_NAME);
    assert_eq!(row.payload, decided.payload);
    // 它数进这枚令牌的小时限额，不是一条提议，也不占「开着的提议」那个位子
    assert_eq!(
        token_agent_actions::finalized_since(pool, b.kb, b.token, since).await?,
        1
    );
    assert!(token_agent_actions::proposal(pool, b.kb, row.id)
        .await?
        .is_none());
    let mut tx = pool.begin().await?;
    let second = token_agent_actions::record_applied(&mut tx, &write(b), &decided).await?;
    tx.commit().await?;
    assert_ne!(second.id, action.id);
    assert_eq!(ledger(pool, b.kb).await?.len(), 2);

    // 它自己不写审计行；调用方的 detail 由 token_audit_detail 补上三项，别的键不动
    assert!(audits(pool, b.kb).await?.is_empty());
    assert_eq!(
        token_agent_actions::token_audit_detail(json!({ "status": "confirmed" }), &action),
        json!({
            "status": "confirmed",
            "token_id": b.token,
            "token_name": TOKEN_NAME,
            "action_id": action.id,
        })
    );
    Ok(())
}

#[tokio::test]
async fn an_applied_row_commits_or_rolls_back_with_the_callers_change() -> anyhow::Result<()> {
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let pool = PgPool::connect(&url).await?;
    let b = base(&pool).await?;
    let outcome = applied_with_the_change(&pool, &b).await;
    teardown(&pool, &b).await?;
    outcome
}

// ───────────────────────── 6. Agent 一档与徽标 ─────────────────────────

fn keys(row: &Value) -> Vec<String> {
    row.as_object()
        .expect("an object")
        .keys()
        .cloned()
        .collect()
}

fn sorted(keys: &[&str]) -> Vec<String> {
    let mut keys: Vec<String> = keys.iter().map(|k| k.to_string()).collect();
    keys.sort();
    keys
}

/// 分开一对的行今天的那十四个键
const KEEP_KEYS: [&str; 14] = [
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
];

async fn history_and_counts(pool: &PgPool, b: &Base) -> anyhow::Result<()> {
    // 治理 agent 有一条开着的建议：徽标里它那个数自始至终是 1
    let (governed, _, _) = pair(pool, b, "Hydra").await?;
    governor(pool, b, governed, "proposed").await?;

    // 零的时候计数里没有这个键
    let counts = utopia_store::review::counts(pool, b.kb).await?;
    assert_eq!(counts.agent_token_proposals, 0);
    assert_eq!(counts.agent, 1);
    let shown = json!(counts);
    assert!(shown.get("agent_token_proposals").is_none(), "{shown}");
    let keys_at_zero = keys(&shown);

    // 分开一对、合并一对：今天的行
    let (kept, _, _) = pair(pool, b, "Acme").await?;
    token_agent_actions::keep_duplicate_pair(pool, b.kb, kept, b.user, b.token, Some("two firms"))
        .await?;
    let (merged, _, _) = pair(pool, b, "Orion").await?;
    let merge = token_agent_actions::merge_duplicate_pair(pool, &write(b), merged).await?;
    assert_eq!(
        merge.action.outcome, "applied",
        "{:?}",
        merge.action.gate_reason
    );

    // 目标不是一对的提议：一处缺陷、一条短语、一条规则、一条没有 action 的
    let a_defect = defect(pool, b).await?;
    let a_phrase = phrase(pool, b, "is based in").await?;
    let a_rule = rule(pool, b, "moved to").await?;
    let declined_phrase = phrase(pool, b, "sits in").await?;
    let propose = |operation: &'static str, kind: &'static str, id: Uuid, payload: Value| async move {
        token_agent_actions::propose(
            pool,
            &write(b),
            &Proposal {
                operation,
                target_kind: kind,
                target_id: id,
                target_key: &id.to_string(),
                payload,
            },
        )
        .await
    };
    let on_defect = propose(
        "defect.decide",
        "ontology_defect",
        a_defect,
        json!({
            "request": { "resolution": "fixed" },
            "target_label": "inverse_of_itself: based in",
            "action": "fixed",
            "why": "the inverse was removed",
        }),
    )
    .await?;
    let bind = json!({ "request": { "property": "based_in", "direction": "forward" },
                       "target_label": "is based in (Organization → any)", "action": "bind" });
    let on_phrase = propose("alignment.phrase", "phrase_binding", a_phrase, bind.clone()).await?;
    let on_declined = propose("alignment.phrase", "phrase_binding", declined_phrase, bind).await?;
    let on_rule = propose(
        "alignment.rule",
        "implication_rule",
        a_rule,
        json!({ "request": { "approve": true }, "target_label": "moved to ⇒ based in",
                "action": "approve" }),
    )
    .await?;
    let on_run = propose(
        "consistency.check",
        "knowledge_base",
        b.kb,
        json!({ "request": {}, "target_label": "Proposal Base" }),
    )
    .await?;

    // 徽标：五条开着的令牌提议；治理的那个数没动；别的键与零的时候一样
    let counts = utopia_store::review::counts(pool, b.kb).await?;
    assert_eq!(counts.agent_token_proposals, 5);
    assert_eq!(counts.agent, 1);
    let shown = json!(counts);
    assert_eq!(shown["agent_token_proposals"], json!(5));
    let mut with_key = keys_at_zero.clone();
    with_key.push("agent_token_proposals".to_string());
    with_key.sort();
    let mut now_keys = keys(&shown);
    now_keys.sort();
    assert_eq!(now_keys, with_key);
    // 别的库的提议不算在这个库头上
    let elsewhere = base(pool).await?;
    let there = utopia_store::review::counts(pool, elsewhere.kb).await;
    teardown(pool, &elsewhere).await?;
    assert_eq!(there?.agent_token_proposals, 0);

    // 人答：两条批准、一条拒绝。开着的剩两条
    for (id, state) in [
        (on_phrase.action.id, "approved"),
        (on_rule.action.id, "approved"),
        (on_declined.action.id, "declined"),
    ] {
        assert!(token_agent_actions::answer(pool, b.kb, id, state, b.user, None).await?);
    }
    assert_eq!(
        utopia_store::review::counts(pool, b.kb)
            .await?
            .agent_token_proposals,
        2
    );

    let history = token_agent_actions::agent_history(pool, b.kb, 50, 0).await?;
    let row = |id: Uuid| {
        history
            .iter()
            .find(|r| r["id"] == json!(id))
            .cloned()
            .unwrap_or_else(|| panic!("no Agent History row for {id}"))
    };
    assert_eq!(history.len(), 8, "seven ledger rows and the governor's one");

    // 分开的行：正好今天的键。合并的行：多一个 merge_id，别的不多
    let keep_row = history
        .iter()
        .find(|r| r["operation"] == json!("review.keep"))
        .expect("the keep row");
    assert_eq!(keys(keep_row), sorted(&KEEP_KEYS));
    let merge_row = row(merge.action.id);
    let mut merge_keys = KEEP_KEYS.to_vec();
    merge_keys.push("merge_id");
    assert_eq!(keys(&merge_row), sorted(&merge_keys));

    // 缺陷的提议：说得出目标与动作，还开着所以没有 state，也没有 queued
    let defect_row = row(on_defect.action.id);
    let mut proposal_keys = KEEP_KEYS.to_vec();
    proposal_keys.extend(["reason", "target_label", "action"]);
    assert_eq!(keys(&defect_row), sorted(&proposal_keys));
    assert_eq!(
        defect_row["target_label"],
        json!("inverse_of_itself: based in")
    );
    assert_eq!(defect_row["action"], json!("fixed"));
    assert_eq!(defect_row["reason"], json!("propose_only"));
    assert_eq!(defect_row["rationale"], json!("the inverse was removed"));
    assert_eq!(defect_row["source"], json!("token"));
    assert_eq!(defect_row["target_kind"], json!("ontology_defect"));
    assert_eq!(defect_row["target_id"], json!(a_defect));
    assert_eq!(
        (&defect_row["left"], &defect_row["right"]),
        (&Value::Null, &Value::Null)
    );

    // 没有 action 的行不出 action 这个键
    let run_row = row(on_run.action.id);
    let mut run_keys = KEEP_KEYS.to_vec();
    run_keys.extend(["reason", "target_label"]);
    assert_eq!(keys(&run_row), sorted(&run_keys));
    assert_eq!(run_row["target_label"], json!("Proposal Base"));

    // 批准了的短语与规则：行上说「排着」。拒绝的不说
    for approved in [on_phrase.action.id, on_rule.action.id] {
        let r = row(approved);
        assert_eq!(r["state"], json!("approved"));
        assert_eq!(r["queued"], json!(true));
        let mut queued_keys = proposal_keys.clone();
        queued_keys.extend(["state", "state_at", "queued"]);
        assert_eq!(keys(&r), sorted(&queued_keys));
    }
    let declined_row = row(on_declined.action.id);
    assert_eq!(declined_row["state"], json!("declined"));
    assert!(declined_row.get("queued").is_none());
    // 批准了的别的操作也不说「排着」
    assert!(
        token_agent_actions::answer(pool, b.kb, on_defect.action.id, "approved", b.user, None)
            .await?
    );
    let history = token_agent_actions::agent_history(pool, b.kb, 50, 0).await?;
    let defect_row = history
        .iter()
        .find(|r| r["id"] == json!(on_defect.action.id))
        .expect("the defect row");
    assert_eq!(defect_row["state"], json!("approved"));
    assert!(defect_row.get("queued").is_none());
    assert_eq!(
        utopia_store::review::counts(pool, b.kb)
            .await?
            .agent_token_proposals,
        1
    );
    Ok(())
}

#[tokio::test]
async fn agent_history_names_a_target_that_is_not_a_pair_and_the_count_is_absent_at_zero(
) -> anyhow::Result<()> {
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let pool = PgPool::connect(&url).await?;
    let b = base(&pool).await?;
    let outcome = history_and_counts(&pool, &b).await;
    teardown(&pool, &b).await?;
    outcome
}
