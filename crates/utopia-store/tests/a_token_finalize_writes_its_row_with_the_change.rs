//! 令牌 agent 直接落地的两种决定（0076）——语义层映射、类别词——打在真库上，钉一件事：
//! **台账行跟着变更走**。
//!
//! 1. 映射：新状态、台账行（`mapping.decide`，`applied`）、审计行（`mapping.decided`）是
//!    一个事务。写不进台账（读不到令牌的名字），状态也不留下；状态已经是请求要的那个，
//!    一行不写
//! 2. 类别词：判定、它在实体上的投影、排下的短语对齐任务、台账行（`alignment.kind_word`，
//!    `applied`）是一个事务。写不进台账，判定也不留下。台账行的请求里存着那个词，Agent
//!    一档那一行因此说得出目标
//!
//! 种数据只用直接插入，不经模型任务。每个测试自建自己的库（kb）、自己拆，互不相干，所以
//! 可以并着跑。没有 `UTOPIA_DATABASE_URL` 时跳过而不是失败。

use serde_json::{json, Value};
use sqlx::PgPool;
use utopia_core::AppError;
use utopia_store::token_agent_actions::{self, TokenAgentAction, TokenWrite};
use uuid::Uuid;

const TOKEN_NAME: &str = "finalizing agent";

struct Base {
    kb: Uuid,
    org: Uuid,
    user: Uuid,
    token: Uuid,
    company_type: Uuid,
}

async fn base(pool: &PgPool) -> anyhow::Result<Base> {
    let [org, ws, kb, user, company_type] = std::array::from_fn(|_| Uuid::now_v7());
    sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, 'token-finalize-test')")
        .bind(org)
        .execute(pool)
        .await?;
    sqlx::query(
        "INSERT INTO users (id, org_id, email, display_name, password_hash)
         VALUES ($1, $2, $3, 'Token Owner', 'x')",
    )
    .bind(user)
    .bind(org)
    .bind(format!("token-finalize-{}@test.local", user.simple()))
    .execute(pool)
    .await?;
    sqlx::query("INSERT INTO workspaces (id, org_id, name) VALUES ($1, $2, 'token-finalize-test')")
        .bind(ws)
        .bind(org)
        .execute(pool)
        .await?;
    sqlx::query(
        "INSERT INTO knowledge_bases (id, workspace_id, name) VALUES ($1, $2, 'Finalize Base')",
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
    let (view, _plain) =
        utopia_store::tokens::issue(pool, user, TOKEN_NAME, "write", None, None).await?;
    Ok(Base {
        kb,
        org,
        user,
        token: view.id,
        company_type,
    })
}

async fn teardown(pool: &PgPool, b: &Base) -> anyhow::Result<()> {
    // 任务按载荷里的 kb_id 挂在库上，没有外键，删库带不走它们
    sqlx::query("DELETE FROM jobs WHERE payload ->> 'kb_id' = $1")
        .bind(b.kb.to_string())
        .execute(pool)
        .await?;
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

/// 一枚没有 `personal_tokens` 行的令牌：读不到它的名字，所以台账行写不进去
fn nameless(b: &Base) -> TokenWrite {
    TokenWrite {
        token_id: Uuid::now_v7(),
        ..write(b)
    }
}

/// 这个库的台账，最早的在前
async fn ledger(pool: &PgPool, kb: Uuid) -> anyhow::Result<Vec<TokenAgentAction>> {
    let mut rows = token_agent_actions::list(pool, kb, 500, 0).await?;
    rows.reverse();
    Ok(rows)
}

/// 这个库某个动作的审计行：actor、target_kind、target_id、detail，最早的在前
#[allow(clippy::type_complexity)]
async fn audits(
    pool: &PgPool,
    kb: Uuid,
    action: &str,
) -> anyhow::Result<Vec<(Option<Uuid>, String, Option<Uuid>, Value)>> {
    Ok(sqlx::query_as(
        "SELECT actor_id, target_kind, target_id, detail FROM audit_events
          WHERE kb_id = $1 AND action = $2 ORDER BY created_at, id",
    )
    .bind(kb)
    .bind(action)
    .fetch_all(pool)
    .await?)
}

fn keys(v: &Value) -> Vec<String> {
    let mut k: Vec<String> = v
        .as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    k.sort();
    k
}

/// 这一小时里这枚令牌在这个库落地了几笔（限速数的就是它）
async fn finalized(pool: &PgPool, b: &Base) -> anyhow::Result<i64> {
    Ok(token_agent_actions::finalized_since(
        pool,
        b.kb,
        b.token,
        chrono::Utc::now() - chrono::Duration::hours(1),
    )
    .await?)
}

// ───────────────────────── 1. 映射 ─────────────────────────

/// 一条等人表态的映射：概念「Revenue」接在 warehouse.orders 上
async fn mapping(pool: &PgPool, kb: Uuid, type_id: Uuid) -> anyhow::Result<Uuid> {
    let concept = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO entities (id, kb_id, type_id, canonical_name)
         VALUES ($1, $2, $3, 'Revenue')",
    )
    .bind(concept)
    .bind(kb)
    .bind(type_id)
    .execute(pool)
    .await?;
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO concept_mappings (id, kb_id, concept_id, source, table_name)
         VALUES ($1, $2, $3, 'warehouse', 'orders')",
    )
    .bind(id)
    .bind(kb)
    .bind(concept)
    .execute(pool)
    .await?;
    Ok(id)
}

/// 映射此刻的状态与表态的人
async fn mapping_row(pool: &PgPool, id: Uuid) -> anyhow::Result<(String, Option<Uuid>)> {
    Ok(
        sqlx::query_as("SELECT status, decided_by FROM concept_mappings WHERE id = $1")
            .bind(id)
            .fetch_one(pool)
            .await?,
    )
}

async fn mapping_with_its_row(pool: &PgPool, b: &Base, other: &Base) -> anyhow::Result<()> {
    let id = mapping(pool, b.kb, b.company_type).await?;
    let foreign = mapping(pool, other.kb, other.company_type).await?;
    anyhow::ensure!(mapping_row(pool, id).await? == ("proposed".to_string(), None));

    // 写不进台账（读不到令牌的名字）：状态也不留下，审计也没有
    let failed = token_agent_actions::decide_mapping(pool, &nameless(b), id, "confirmed").await;
    anyhow::ensure!(
        matches!(failed, Err(AppError::Other(_))),
        "a token without a name must fail: {failed:?}"
    );
    anyhow::ensure!(
        mapping_row(pool, id).await? == ("proposed".to_string(), None),
        "the status rolled back with the ledger row"
    );
    anyhow::ensure!(ledger(pool, b.kb).await?.is_empty(), "no ledger row");
    anyhow::ensure!(audits(pool, b.kb, "mapping.decided").await?.is_empty());

    // 认不出的状态、这个库里没有的映射（别的库的也算）：一行不写
    let bad = token_agent_actions::decide_mapping(pool, &write(b), id, "proposed").await;
    anyhow::ensure!(
        matches!(&bad, Err(AppError::Invalid { code, .. }) if *code == "bad_status"),
        "{bad:?}"
    );
    for missing in [Uuid::now_v7(), foreign] {
        let gone = token_agent_actions::decide_mapping(pool, &write(b), missing, "confirmed").await;
        anyhow::ensure!(matches!(gone, Err(AppError::NotFound)), "{gone:?}");
    }
    anyhow::ensure!(
        ledger(pool, b.kb).await?.is_empty(),
        "refusals write no row"
    );
    anyhow::ensure!(
        mapping_row(pool, foreign).await?.0 == "proposed",
        "another base's mapping is untouched"
    );

    // 落地：状态、台账行、审计行一起在
    let action = token_agent_actions::decide_mapping(pool, &write(b), id, "confirmed").await?;
    anyhow::ensure!(
        mapping_row(pool, id).await? == ("confirmed".to_string(), Some(b.user)),
        "the mapping is confirmed under the token's owner"
    );
    let rows = ledger(pool, b.kb).await?;
    anyhow::ensure!(rows.len() == 1 && rows[0].id == action.id, "{rows:?}");
    let row = &rows[0];
    anyhow::ensure!(
        row.operation == "mapping.decide"
            && row.target_kind == "concept_mapping"
            && row.target_id == id
            && row.outcome == "applied"
            && row.gate_reason.is_none()
            && row.state.is_none()
            && row.owner_id == b.user
            && row.token_id == b.token
            && row.token_name == TOKEN_NAME,
        "{row:?}"
    );
    anyhow::ensure!(
        row.payload
            == json!({
                "request": { "status": "confirmed" },
                "target_label": "Revenue → warehouse.orders",
                "action": "confirmed",
            }),
        "{}",
        row.payload
    );
    let key: String =
        sqlx::query_scalar("SELECT target_key FROM token_agent_actions WHERE id = $1")
            .bind(action.id)
            .fetch_one(pool)
            .await?;
    anyhow::ensure!(key == id.to_string(), "target_key is the mapping id: {key}");
    let audited = audits(pool, b.kb, "mapping.decided").await?;
    anyhow::ensure!(audited.len() == 1, "{audited:?}");
    let (actor, target_kind, target_id, detail) = &audited[0];
    anyhow::ensure!(
        *actor == Some(b.user) && target_kind == "concept_mapping" && *target_id == Some(id),
        "{audited:?}"
    );
    anyhow::ensure!(
        *detail
            == json!({
                "status": "confirmed",
                "token_id": b.token,
                "token_name": TOKEN_NAME,
                "action_id": action.id,
            }),
        "{detail}"
    );
    anyhow::ensure!(
        finalized(pool, b).await? == 1,
        "the row counts toward the limit"
    );

    // 同一个状态再来一次：已经定了，一行不写
    let again = token_agent_actions::decide_mapping(pool, &write(b), id, "confirmed").await;
    anyhow::ensure!(
        matches!(&again, Err(AppError::CodedConflict { code, .. }) if *code == "already_decided"),
        "{again:?}"
    );
    anyhow::ensure!(ledger(pool, b.kb).await?.len() == 1);
    anyhow::ensure!(audits(pool, b.kb, "mapping.decided").await?.len() == 1);

    // 换一个状态：映射没有「定了」这一态，照样落地，再多一行
    let rejected = token_agent_actions::decide_mapping(pool, &write(b), id, "rejected").await?;
    anyhow::ensure!(mapping_row(pool, id).await?.0 == "rejected");
    let rows = ledger(pool, b.kb).await?;
    anyhow::ensure!(
        rows.len() == 2 && rows[1].id == rejected.id && rows[1].payload["action"] == "rejected",
        "{rows:?}"
    );
    anyhow::ensure!(audits(pool, b.kb, "mapping.decided").await?.len() == 2);

    // 同时到的两个相同决定：锁着行再比，正好落地一个
    let (w1, w2) = (write(b), write(b));
    let (first, second) = tokio::join!(
        token_agent_actions::decide_mapping(pool, &w1, id, "confirmed"),
        token_agent_actions::decide_mapping(pool, &w2, id, "confirmed"),
    );
    let landed = [&first, &second].iter().filter(|r| r.is_ok()).count();
    let refused = [&first, &second]
        .iter()
        .filter(|r| {
            matches!(r, Err(AppError::CodedConflict { code, .. }) if *code == "already_decided")
        })
        .count();
    anyhow::ensure!(
        landed == 1 && refused == 1,
        "exactly one of two equal decisions lands: {first:?} {second:?}"
    );
    anyhow::ensure!(ledger(pool, b.kb).await?.len() == 3);

    // Agent 一档里这一行用话说出目标与动作
    let history = token_agent_actions::agent_history(pool, b.kb, 50, 0).await?;
    let shown = json!(history);
    let shown = shown
        .as_array()
        .and_then(|rows| rows.iter().find(|r| r["id"] == json!(action.id)))
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("the decision is not in Agent History: {history:?}"))?;
    anyhow::ensure!(
        shown["source"] == "token"
            && shown["operation"] == "mapping.decide"
            && shown["outcome"] == "applied"
            && shown["target_label"] == "Revenue → warehouse.orders"
            && shown["action"] == "confirmed"
            && shown["token_name"] == TOKEN_NAME
            && shown["owner_name"] == "Token Owner",
        "{shown}"
    );
    Ok(())
}

#[tokio::test]
async fn a_token_mapping_decision_and_its_applied_row_are_one_transaction() -> anyhow::Result<()> {
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let pool = PgPool::connect(&url).await?;
    let b = base(&pool).await?;
    let other = base(&pool).await?;
    let outcome = mapping_with_its_row(&pool, &b, &other).await;
    teardown(&pool, &b).await?;
    teardown(&pool, &other).await?;
    outcome
}

// ───────────────────────── 2. 类别词 ─────────────────────────

/// 类别词「startup」的绑定行此刻的样子：状态、类、谁定的
async fn binding(
    pool: &PgPool,
    kb: Uuid,
) -> anyhow::Result<Option<(String, Option<Uuid>, Option<String>)>> {
    Ok(sqlx::query_as(
        "SELECT status, type_id, decided_by FROM type_bindings
          WHERE kb_id = $1 AND kind_word = 'startup'",
    )
    .bind(kb)
    .fetch_optional(pool)
    .await?)
}

/// 实体此刻的类与它是怎么来的
async fn typed(pool: &PgPool, entity: Uuid) -> anyhow::Result<(Option<Uuid>, String)> {
    Ok(
        sqlx::query_as("SELECT type_id, type_source FROM entities WHERE id = $1")
            .bind(entity)
            .fetch_one(pool)
            .await?,
    )
}

async fn alignment_jobs(pool: &PgPool, kb: Uuid) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "SELECT count(*) FROM jobs WHERE kind = 'align_phrases' AND payload ->> 'kb_id' = $1",
    )
    .bind(kb.to_string())
    .fetch_one(pool)
    .await?)
}

async fn kind_word_with_its_row(pool: &PgPool, b: &Base) -> anyhow::Result<()> {
    // 一个没有类的实体，抽取时叫它「Startup」；类别词还没人定
    let entity = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO entities (id, kb_id, canonical_name, specific_type)
         VALUES ($1, $2, 'Initech', 'Startup')",
    )
    .bind(entity)
    .bind(b.kb)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO type_bindings (id, kb_id, kind_word, words, status, votes, decided_at)
         VALUES ($1, $2, 'startup', ARRAY['Startup'], 'undecided',
                 '{\"first\": null, \"second\": null}'::jsonb, now())",
    )
    .bind(Uuid::now_v7())
    .bind(b.kb)
    .execute(pool)
    .await?;
    let undecided = binding(pool, b.kb).await?;
    let untyped = typed(pool, entity).await?;
    anyhow::ensure!(untyped.0.is_none(), "{untyped:?}");
    let votes = json!({ "person": "org" });

    // 写不进台账（读不到令牌的名字）：判定、投影、任务都不留下
    let failed = token_agent_actions::decide_kind_word(
        pool,
        &nameless(b),
        "Startup",
        Some(b.company_type),
        &votes,
    )
    .await;
    anyhow::ensure!(
        matches!(failed, Err(AppError::Other(_))),
        "a token without a name must fail: {failed:?}"
    );
    anyhow::ensure!(
        binding(pool, b.kb).await? == undecided,
        "the decision rolled back with the ledger row"
    );
    anyhow::ensure!(typed(pool, entity).await? == untyped, "no entity was typed");
    anyhow::ensure!(alignment_jobs(pool, b.kb).await? == 0, "no job was queued");
    anyhow::ensure!(ledger(pool, b.kb).await?.is_empty(), "no ledger row");

    // 落地：判定记成人的（令牌以主人的身份行事）、类写到实体上、任务排下、台账一行
    let action = token_agent_actions::decide_kind_word(
        pool,
        &write(b),
        "  Startup ",
        Some(b.company_type),
        &votes,
    )
    .await?
    .ok_or_else(|| anyhow::anyhow!("the decision was not written"))?;
    anyhow::ensure!(
        binding(pool, b.kb).await?
            == Some((
                "bound".to_string(),
                Some(b.company_type),
                Some("person".to_string())
            )),
        "the kind word is bound"
    );
    anyhow::ensure!(
        typed(pool, entity).await? == (Some(b.company_type), "aligned".to_string()),
        "the entity under the kind word got the class"
    );
    anyhow::ensure!(alignment_jobs(pool, b.kb).await? == 1, "one job is queued");
    let rows = ledger(pool, b.kb).await?;
    anyhow::ensure!(rows.len() == 1 && rows[0].id == action.id, "{rows:?}");
    let row = &rows[0];
    anyhow::ensure!(
        row.operation == "alignment.kind_word"
            && row.target_kind == "type_binding"
            && row.target_id == b.kb
            && row.outcome == "applied"
            && row.gate_reason.is_none()
            && row.owner_id == b.user
            && row.token_id == b.token
            && row.token_name == TOKEN_NAME,
        "{row:?}"
    );
    // 请求里存着那个词：Agent 一档的目标从它来
    anyhow::ensure!(
        row.payload
            == json!({
                "request": { "kind_word": "  Startup ", "class": "org" },
                "target_label": "Startup",
                "action": "bind",
            }),
        "{}",
        row.payload
    );
    let key: String =
        sqlx::query_scalar("SELECT target_key FROM token_agent_actions WHERE id = $1")
            .bind(action.id)
            .fetch_one(pool)
            .await?;
    anyhow::ensure!(key == "startup", "target_key is the normalized word: {key}");
    // 这个函数不写审计行：那是路由的事
    anyhow::ensure!(audits(pool, b.kb, "alignment.kind_word_decided")
        .await?
        .is_empty());
    anyhow::ensure!(
        finalized(pool, b).await? == 1,
        "the row counts toward the limit"
    );

    // 给审计 detail 补上的就是台账行的那三项
    let detail = token_agent_actions::token_audit_detail(
        json!({ "kind_word": "Startup", "class": "org" }),
        &action,
    );
    anyhow::ensure!(
        keys(&detail) == ["action_id", "class", "kind_word", "token_id", "token_name"]
            && detail["token_name"] == TOKEN_NAME
            && detail["token_id"] == json!(b.token)
            && detail["action_id"] == json!(action.id),
        "{detail}"
    );

    // 改判成「没有类对得上」：实体失去那个类，再多一行；它从不被限速挡住——哪怕这一小时
    // 里这枚令牌已经落地了几百笔
    sqlx::query(
        "INSERT INTO token_agent_actions
             (id, kb_id, operation, target_kind, target_id, owner_id, token_id, token_name,
              outcome, payload, target_key)
         SELECT gen_random_uuid(), $1, 'review.keep', 'review', gen_random_uuid(), $2, $3,
                'seeded', 'applied', '{}'::jsonb, gen_random_uuid()::text
           FROM generate_series(1, 250)",
    )
    .bind(b.kb)
    .bind(b.user)
    .bind(b.token)
    .execute(pool)
    .await?;
    let unbound =
        token_agent_actions::decide_kind_word(pool, &write(b), "startup", None, &json!({})).await?;
    let unbound = unbound.ok_or_else(|| anyhow::anyhow!("the second decision was not written"))?;
    anyhow::ensure!(
        binding(pool, b.kb).await? == Some(("none".to_string(), None, Some("person".to_string()))),
        "the kind word is unbound"
    );
    anyhow::ensure!(typed(pool, entity).await?.0.is_none(), "the class is gone");
    anyhow::ensure!(
        unbound.payload["action"] == "none"
            && unbound.payload["request"] == json!({ "kind_word": "startup", "class": null })
            && unbound.payload["target_label"] == "startup",
        "{}",
        unbound.payload
    );
    anyhow::ensure!(
        finalized(pool, b).await? == 252,
        "both rows count toward the limit"
    );

    // Agent 一档里这一行的目标就是那个词，不是空的
    let history = token_agent_actions::agent_history(pool, b.kb, 400, 0).await?;
    let shown = json!(history);
    let shown = shown
        .as_array()
        .and_then(|rows| rows.iter().find(|r| r["id"] == json!(action.id)))
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("the decision is not in Agent History"))?;
    anyhow::ensure!(
        shown["target_label"]
            .as_str()
            .is_some_and(|label| !label.trim().is_empty() && label == "Startup"),
        "the Agent History row names the kind word: {shown}"
    );
    anyhow::ensure!(
        shown["source"] == "token"
            && shown["operation"] == "alignment.kind_word"
            && shown["outcome"] == "applied"
            && shown["action"] == "bind"
            && shown["token_name"] == TOKEN_NAME
            && shown["owner_name"] == "Token Owner",
        "{shown}"
    );
    Ok(())
}

#[tokio::test]
async fn a_token_kind_word_decision_and_its_applied_row_are_one_transaction() -> anyhow::Result<()>
{
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let pool = PgPool::connect(&url).await?;
    let b = base(&pool).await?;
    let outcome = kind_word_with_its_row(&pool, &b).await;
    teardown(&pool, &b).await?;
    outcome
}
