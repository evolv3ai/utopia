//! 令牌定对齐一档的短语签名与蕴含规则：只留提议，人答了才算，而批准之后改动是**排着**
//! （0076）；以及违规、勘误、对齐三个 MCP 工具。
//!
//! 照的是 `review_token_conflict_tests`。要钉住的行为：
//!
//! - **令牌的调用什么都不改，答的是 200**（SC1 / E1 / E9）：短语（绑到一个属性、没有属性
//!   对得上）与规则（批、驳）都回 200、`proposed`、`propose_only`——不是会话的 202，什么都
//!   还没收下；绑定行、规则行、这个库的每一条事实与实体、排着的任务数前后相等
//! - **请求体与目标照人的路由校验**（E3 / E4）：`unknown_property`、`direction_required`、
//!   `unknown_marks`、`marks_needs_state`，令牌拿到的与会话拿到的是同一个状态码、同一个
//!   code、同一句话；认不出的、别的库的是 404；哪一种都不留提议
//! - **批准之后改动是排着，不说成已经落地**（E9 / N6）：批准的回答带 `job_id` 与
//!   `queued: true`，Agent History 里那一行带 `queued: true`；判定是这个人的，审计行指着
//!   提议（E7 / N2）
//! - **短语与规则没有「定了」这一态**（E5）：行在，提议就照样落地；行没了才是 `superseded`
//! - **会话的判定一个字没变**（E9）：仍答 202、`job_id`、`status: "accepted"`，审计行的
//!   键同从前，台账一行不写
//! - **三个工具**（`review_violation`、`review_errata`、`review_alignment`）只列给编辑者的
//!   写令牌，回的就是 REST 的那个回答，什么都不改
//!
//! 短语与规则直接种进各自的表；判定排下的任务就留在队列里——这里没有 worker，没有哪个
//! 模型任务会跑。任务行不随库删除，所以每个测试收尾时自己清掉。
//!
//! 夹具在 `review_token_write_support.rs`。没有 `UTOPIA_DATABASE_URL` 时跳过。
use axum::http::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;

use super::review_token_errata_tests as errata;
use super::review_token_fact_tests::{audits_of, ledger_row, predicate_in, proposed};
use super::review_token_violation_tests as violations;
use super::review_token_write_support::{
    answer, audits, ensure_tool_refused, keys, ledger, open_proposals, post, row, snapshot,
    tool_call, tools_list, Audit, Fx,
};

const TOKEN_NAME: &str = "alignment agent";

/// 一个属性，种进这个库：回它的 id 与 key。`temporal` 是 `state` 或 `eternal`
async fn property_in(f: &Fx, kb: Uuid, temporal: &str) -> anyhow::Result<(Uuid, String)> {
    let id = Uuid::now_v7();
    let key = format!("{temporal}_{}", id.simple());
    sqlx::query(
        "INSERT INTO relation_types (id, kb_id, key, label, kind, temporal)
         VALUES ($1, $2, $3, 'works at', 'relation', $4)",
    )
    .bind(id)
    .bind(kb)
    .bind(&key)
    .bind(temporal)
    .execute(&f.pool)
    .await?;
    Ok((id, key))
}

/// 一条还没定的短语签名，直接种进 `phrase_bindings`
async fn phrase_in(f: &Fx, kb: Uuid) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO phrase_bindings (id, kb_id, phrase, status, votes, statement_count, examples)
         VALUES ($1, $2, $3, 'undecided', '{\"first\": null, \"second\": null}'::jsonb, 1,
                 ARRAY['Ada —advises→ Bo'])",
    )
    .bind(id)
    .bind(kb)
    .bind(format!("advises {}", id.simple()))
    .execute(&f.pool)
    .await?;
    Ok(id)
}

/// 一条等人批的蕴含规则，直接种进 `implication_rules`：结论落在 `property` 上
async fn rule_in(f: &Fx, kb: Uuid, property: Uuid) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO implication_rules (id, kb_id, trigger, phrase, conclude_property_id, status,
                                        votes, statement_count, examples)
         VALUES ($1, $2, 'phrase', $3, $4, 'proposed',
                 '{\"first\": \"approve\", \"second\": \"reject\"}'::jsonb, 1,
                 ARRAY['Bo —joined→ Initech'])",
    )
    .bind(id)
    .bind(kb)
    .bind(format!("joined {}", id.simple()))
    .bind(property)
    .execute(&f.pool)
    .await?;
    Ok(id)
}

/// 对齐一档的两种目标：路由的那一段、表名、操作名、目标的种类、审计的动作名
#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Phrase,
    Rule,
}

impl Kind {
    fn tail(self, id: Uuid) -> String {
        match self {
            Kind::Phrase => format!("review/alignment/phrases/{id}"),
            Kind::Rule => format!("review/alignment/rules/{id}"),
        }
    }
    fn table(self) -> &'static str {
        match self {
            Kind::Phrase => "phrase_bindings",
            Kind::Rule => "implication_rules",
        }
    }
    fn operation(self) -> &'static str {
        match self {
            Kind::Phrase => "alignment.phrase",
            Kind::Rule => "alignment.rule",
        }
    }
    fn target_kind(self) -> &'static str {
        match self {
            Kind::Phrase => "phrase_binding",
            Kind::Rule => "implication_rule",
        }
    }
    fn audit(self) -> &'static str {
        match self {
            Kind::Phrase => "alignment.phrase_decided",
            Kind::Rule => "alignment.rule_decided",
        }
    }
    async fn seed(self, f: &Fx, property: Uuid) -> anyhow::Result<Uuid> {
        match self {
            Kind::Phrase => phrase_in(f, f.kb).await,
            Kind::Rule => rule_in(f, f.kb, property).await,
        }
    }
}

/// 定一条短语或规则：人的那条路由，令牌与会话都从这里进
async fn decide(
    f: &Fx,
    bearer: &str,
    kind: Kind,
    id: Uuid,
    body: Value,
) -> anyhow::Result<(StatusCode, Value)> {
    post(f, bearer, &kind.tail(id), body).await
}

/// 令牌经 REST 提一条，必须成、必须是 200：回提议的 id
async fn propose(f: &Fx, token: &str, kind: Kind, id: Uuid, body: Value) -> anyhow::Result<Uuid> {
    let (status, answer) = decide(f, token, kind, id, body).await?;
    anyhow::ensure!(status == StatusCode::OK, "a proposal: {status} {answer}");
    proposed(&answer)
}

/// 四种判定：(目标的种类, 请求体, 台账里的动作)。`key` 是一个状态属性的 key
fn decisions(key: &str) -> [(Kind, Value, &'static str); 4] {
    [
        (
            Kind::Phrase,
            json!({ "property": key, "direction": "forward", "marks": "start" }),
            "bind",
        ),
        (Kind::Phrase, json!({}), "none"),
        (Kind::Rule, json!({ "approve": true }), "approve"),
        (Kind::Rule, json!({ "approve": false }), "reject"),
    ]
}

/// 绑定行或规则行里人的判定，不带时刻
fn decision_of(row: &Value) -> Value {
    json!([
        row["status"],
        row["relation_type_id"],
        row["direction"],
        row["marks"],
        row["decided_by"],
        row["votes"]
    ])
}

/// 一个任务：种类与状态。没有这一行就是测试失败
async fn job(f: &Fx, id: &Value) -> anyhow::Result<(String, String)> {
    let id = id
        .as_i64()
        .ok_or_else(|| anyhow::anyhow!("a job id is a number: {id}"))?;
    Ok(
        sqlx::query_as("SELECT kind, status FROM jobs WHERE id = $1 AND payload ->> 'kb_id' = $2")
            .bind(id)
            .bind(f.kb.to_string())
            .fetch_one(&f.pool)
            .await?,
    )
}

/// 这个库排着的任务数
async fn queued_jobs(f: &Fx) -> anyhow::Result<i64> {
    Ok(snapshot(f).await?["queued_jobs"].as_i64().unwrap_or(-1))
}

/// 某个目标上的判定审计行
fn decided_audits(rows: &[Audit], kind: Kind, target: Uuid) -> Vec<&Audit> {
    rows.iter()
        .filter(|a| a.action == kind.audit() && a.target_id == Some(target))
        .collect()
}

/// Agent History 里的一行（`GET /review?queue=agent`）
async fn history_row(f: &Fx, proposal: Uuid) -> anyhow::Result<Value> {
    let (status, history) = f
        .call(
            &f.editor_session,
            "GET",
            &format!("/kbs/{}/review?queue=agent&limit=200", f.kb),
            None,
        )
        .await?;
    anyhow::ensure!(status == StatusCode::OK, "{status} {history}");
    history["items"]
        .as_array()
        .and_then(|rows| rows.iter().find(|r| r["id"] == json!(proposal)))
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("the proposal is not in Agent History: {history}"))
}

/// 收尾：判定排下的任务不随库删除，先清掉这两个库的，再交给夹具
async fn finish(f: Fx, result: anyhow::Result<()>) -> anyhow::Result<()> {
    let cleared = sqlx::query("DELETE FROM jobs WHERE payload ->> 'kb_id' = ANY($1)")
        .bind(vec![f.kb.to_string(), f.other_kb.to_string()])
        .execute(&f.pool)
        .await;
    f.cleanup().await?;
    cleared?;
    result
}

#[tokio::test]
async fn a_token_phrase_and_rule_decision_wait_as_proposals_answered_200() -> anyhow::Result<()> {
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
        let (state_property, key) = property_in(&f, f.kb, "state").await?;
        let (_, eternal_key) = property_in(&f, f.kb, "eternal").await?;
        let (foreign_property, _) = property_in(&f, f.other_kb, "state").await?;
        let phrase = phrase_in(&f, f.kb).await?;
        let rule = rule_in(&f, f.kb, state_property).await?;
        let foreign_phrase = phrase_in(&f, f.other_kb).await?;
        let foreign_rule = rule_in(&f, f.other_kb, foreign_property).await?;
        let bind = json!({ "property": key, "direction": "forward" });

        let before = snapshot(&f).await?;
        let phrase_row = row(&f, "phrase_bindings", phrase).await?;
        let rule_row = row(&f, "implication_rules", rule).await?;
        let rows_before = ledger(&f).await?.len();
        let audits_before = audits(&f).await?.len();

        // ── 没过检查的调用：哪一种都一行不写、一个任务不排 ──
        // 规则的请求体解不出来：提取器先答
        for body in [json!({}), json!({ "approve": "yes" })] {
            let (status, answered) = decide(&f, &token, Kind::Rule, rule, body.clone()).await?;
            anyhow::ensure!(
                status == StatusCode::UNPROCESSABLE_ENTITY,
                "{body}: {status} {answered}"
            );
        }
        // 写的检查，次序是 scope、库、角色；排在「目标在不在」和请求体的校验前面
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
            for (kind, on, body) in [
                (Kind::Phrase, phrase, bind.clone()),
                (Kind::Phrase, Uuid::now_v7(), json!({ "property": "nope" })),
                (Kind::Rule, rule, json!({ "approve": true })),
                (Kind::Rule, Uuid::now_v7(), json!({ "approve": true })),
            ] {
                let (status, answered) = decide(&f, bearer, kind, on, body).await?;
                anyhow::ensure!(
                    status == StatusCode::FORBIDDEN && answered["code"] == code,
                    "{who}, {kind:?}: {status} {answered}"
                );
            }
        }
        // 认不出的、别的库的短语与规则：与会话拿到的是同一个 404，排在请求体的校验前面
        for (kind, on, body) in [
            (Kind::Phrase, Uuid::now_v7(), json!({ "property": "nope" })),
            (Kind::Phrase, foreign_phrase, bind.clone()),
            (Kind::Rule, Uuid::now_v7(), json!({ "approve": true })),
            (Kind::Rule, foreign_rule, json!({ "approve": false })),
        ] {
            let by_token = decide(&f, &token, kind, on, body.clone()).await?;
            let by_person = decide(&f, &f.editor_session, kind, on, body).await?;
            anyhow::ensure!(
                by_token.0 == StatusCode::NOT_FOUND && by_person.0 == StatusCode::NOT_FOUND,
                "an unknown {kind:?}: {by_token:?} {by_person:?}"
            );
        }
        // 短语的请求体：令牌拿到的就是会话拿到的那一个回答，逐字相等。会话的这几次同样
        // 什么都不改
        for (what, body, code) in [
            (
                "a property the ontology does not have",
                json!({ "property": "nope", "direction": "forward" }),
                "unknown_property",
            ),
            (
                "a property without a direction",
                json!({ "property": key }),
                "direction_required",
            ),
            (
                "a property with an unknown direction",
                json!({ "property": key, "direction": "sideways" }),
                "direction_required",
            ),
            (
                "marks other than start, end or none",
                json!({ "property": key, "direction": "forward", "marks": "middle" }),
                "unknown_marks",
            ),
            (
                "marks under a property that is not a state",
                json!({ "property": eternal_key, "direction": "forward", "marks": "start" }),
                "marks_needs_state",
            ),
            (
                "marks without a property",
                json!({ "marks": "start" }),
                "marks_needs_state",
            ),
        ] {
            let by_token = decide(&f, &token, Kind::Phrase, phrase, body.clone()).await?;
            let by_person = decide(&f, &f.editor_session, Kind::Phrase, phrase, body).await?;
            anyhow::ensure!(
                by_token.0 == StatusCode::UNPROCESSABLE_ENTITY && by_token.1["code"] == code,
                "{what}: {by_token:?}"
            );
            anyhow::ensure!(
                by_person == by_token,
                "{what}: the token gets the person's refusal\n{by_token:?}\n{by_person:?}"
            );
        }
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before
                && audits(&f).await?.len() == audits_before
                && snapshot(&f).await? == before
                && row(&f, "phrase_bindings", phrase).await? == phrase_row
                && row(&f, "implication_rules", rule).await? == rule_row,
            "a refusal wrote something"
        );

        // ── 四种判定：各提一条，答的是 200，什么都没动、一个任务没排 ──
        let mut landed = Vec::new();
        for (kind, body, action) in decisions(&key) {
            let target = kind.seed(&f, state_property).await?;
            let before = snapshot(&f).await?;
            let target_row = row(&f, kind.table(), target).await?;

            let (status, answered) = decide(&f, &token, kind, target, body.clone()).await?;
            anyhow::ensure!(
                status == StatusCode::OK,
                "a token's {kind:?} decision is answered 200, not 202: {status} {answered}"
            );
            // 回答正好是提议的那五个键：没有 `job_id`，也没有 `status: "accepted"`
            let proposal = proposed(&answered)?;
            anyhow::ensure!(
                answered["outcome"] == "proposed"
                    && answered["reason"] == "propose_only"
                    && answered.get("job_id").is_none()
                    && answered.get("status").is_none(),
                "{answered}"
            );
            anyhow::ensure!(
                snapshot(&f).await? == before,
                "a {kind:?} proposal changed a fact, an entity, evidence or queued a job"
            );
            anyhow::ensure!(
                row(&f, kind.table(), target).await? == target_row
                    && decided_audits(&audits(&f).await?, kind, target).is_empty(),
                "a {kind:?} proposal changed its row or wrote a decision's audit row"
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
                    && p.operation == kind.operation()
                    && p.target_kind == kind.target_kind()
                    && p.outcome == "proposed"
                    && p.gate_reason.as_deref() == Some("propose_only")
                    && p.owner_id == f.editor
                    && p.token_id == token_id
                    && p.token_name == TOKEN_NAME,
                "{p:?}"
            );
            anyhow::ensure!(
                keys(&p.payload) == ["action", "request", "target_label"],
                "{}",
                p.payload
            );
            anyhow::ensure!(
                p.payload["request"] == body
                    && p.payload["action"] == action
                    && p.payload["target_label"]
                        .as_str()
                        .is_some_and(|said| !said.is_empty()),
                "{}",
                p.payload
            );
            // 开着的提议在 Agent History 里不读作排着
            let listed = history_row(&f, proposal).await?;
            anyhow::ensure!(
                listed["operation"] == kind.operation()
                    && listed["outcome"] == "proposed"
                    && listed["action"] == action
                    && listed.get("queued").is_none()
                    && listed.get("state").is_none(),
                "{listed}"
            );
            // 重复：同一条提议加 `already_proposed`，仍是 200，换一种判定也是同一个目标
            let other = match kind {
                Kind::Phrase => json!({ "property": key, "direction": "reverse" }),
                Kind::Rule => json!({ "approve": action != "approve" }),
            };
            let (status, again) = decide(&f, &token, kind, target, other).await?;
            anyhow::ensure!(
                status == StatusCode::OK
                    && again["code"] == "already_proposed"
                    && again["action_id"] == json!(proposal),
                "a repeat: {status} {again}"
            );
            landed.push((kind, target, proposal, target_row));
        }
        anyhow::ensure!(ledger(&f).await?.len() == rows_before + 4);

        // ── 拒绝：什么都不改，一个任务不排，那一行不读作排着 ──
        let before = snapshot(&f).await?;
        for (kind, target, proposal, target_row) in landed {
            let (status, answered) = answer(&f, &f.editor_session, proposal, "decline").await?;
            anyhow::ensure!(
                status == StatusCode::OK && answered == json!({ "ok": true, "state": "declined" }),
                "{kind:?}: {status} {answered}"
            );
            anyhow::ensure!(
                snapshot(&f).await? == before && row(&f, kind.table(), target).await? == target_row,
                "a declined {kind:?} proposal changed something"
            );
            let listed = history_row(&f, proposal).await?;
            anyhow::ensure!(
                listed["state"] == "declined" && listed.get("queued").is_none(),
                "{listed}"
            );
            anyhow::ensure!(decided_audits(&audits(&f).await?, kind, target).is_empty());
        }
        Ok(())
    }
    .await;
    finish(f, result).await
}

#[tokio::test]
async fn approving_an_alignment_proposal_answers_the_job_id_and_its_row_reads_queued(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let (_, token) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let (state_property, key) = property_in(&f, f.kb, "state").await?;

        // ── 四种判定：这个人批准提议，落下的与他自己判的一模一样，而且是排着 ──
        for (kind, body, action) in decisions(&key) {
            let by_person = kind.seed(&f, state_property).await?;
            let by_proposal = kind.seed(&f, state_property).await?;
            let proposal = propose(&f, &token, kind, by_proposal, body.clone()).await?;

            // 对照：这个人自己判
            let (status, own) =
                decide(&f, &f.editor_session, kind, by_person, body.clone()).await?;
            anyhow::ensure!(status == StatusCode::ACCEPTED, "{kind:?}: {status} {own}");
            let own_job = job(&f, &own["job_id"]).await?;
            let mine_row = row(&f, kind.table(), by_person).await?;

            // 批准：回答带上排下的任务，说的是排着（E9 / N6），正好这四个键
            let jobs_before = queued_jobs(&f).await?;
            let (status, answered) = answer(&f, &f.editor_session, proposal, "approve").await?;
            anyhow::ensure!(
                status == StatusCode::OK,
                "{kind:?} {action}: {status} {answered}"
            );
            anyhow::ensure!(
                keys(&answered) == ["job_id", "ok", "queued", "state"]
                    && answered["ok"] == true
                    && answered["state"] == "approved"
                    && answered["queued"] == true
                    && answered["job_id"].is_i64(),
                "an approval answers the queued job: {answered}"
            );
            anyhow::ensure!(
                answered["job_id"] != own["job_id"],
                "the approval queued its own job: {answered} {own}"
            );
            // 那个任务真的在队列里、还没跑，种类与这个人自己判时排下的相同
            let queued = job(&f, &answered["job_id"]).await?;
            anyhow::ensure!(
                queued.1 == "queued" && queued == own_job,
                "{queued:?} / {own_job:?}"
            );
            anyhow::ensure!(
                queued_jobs(&f).await? == jobs_before + 1,
                "an approval queues exactly one job"
            );
            // 判定同这个人自己判的一模一样
            let theirs_row = row(&f, kind.table(), by_proposal).await?;
            anyhow::ensure!(
                decision_of(&theirs_row) == decision_of(&mine_row)
                    && theirs_row["decided_by"] == "person",
                "{theirs_row}\n{mine_row}"
            );
            // 台账与 Agent History：批准了，而且读作排着
            let closed = ledger_row(&f, proposal).await?;
            anyhow::ensure!(
                closed.state.as_deref() == Some("approved")
                    && closed.state_by == Some(f.editor)
                    && closed.outcome == "proposed",
                "{closed:?}"
            );
            let listed = history_row(&f, proposal).await?;
            anyhow::ensure!(
                listed["state"] == "approved"
                    && listed["queued"] == true
                    && listed["operation"] == kind.operation()
                    && listed["action"] == action,
                "the Agent History row reads queued: {listed}"
            );
            // 审计：actor 都是这个人，同一个动作名；批准的那一行多指着提议，记的是它自己
            // 排下的任务，不带令牌
            let rows = audits(&f).await?;
            let own_audit = decided_audits(&rows, kind, by_person);
            let approved = decided_audits(&rows, kind, by_proposal);
            anyhow::ensure!(
                own_audit.len() == 1 && approved.len() == 1,
                "{own_audit:?} {approved:?}"
            );
            let (own_audit, approved) = (own_audit[0], approved[0]);
            anyhow::ensure!(
                own_audit.actor == Some(f.editor)
                    && own_audit.detail.get("agent_proposal").is_none()
                    && own_audit.detail["job_id"] == own["job_id"],
                "{own_audit:?}"
            );
            let mut expected = keys(&own_audit.detail);
            expected.push("agent_proposal".into());
            expected.sort();
            anyhow::ensure!(
                approved.actor == Some(f.editor)
                    && approved.target_kind == kind.target_kind()
                    && keys(&approved.detail) == expected
                    && approved.detail["agent_proposal"] == json!(proposal)
                    && approved.detail["job_id"] == answered["job_id"]
                    && approved.detail.get("token_id").is_none(),
                "the approval is the person's act and names its proposal: {approved:?}"
            );
            for field in keys(&own_audit.detail) {
                // 短语的原文与任务的 id 各是各的；其余的键逐字相等
                if field == "job_id" || field == "phrase" {
                    continue;
                }
                anyhow::ensure!(
                    approved.detail[&field] == own_audit.detail[&field],
                    "{field}: {approved:?} / {own_audit:?}"
                );
            }
        }
        anyhow::ensure!(audits_of(&audits(&f).await?, "agent.proposal.superseded").is_empty());

        // ── 批量：一条结果同样带上 `job_id` 与 `queued` ──
        let in_bulk = [
            (Kind::Phrase, json!({})),
            (Kind::Rule, json!({ "approve": true })),
        ];
        let mut ids = Vec::new();
        for (kind, body) in &in_bulk {
            let target = kind.seed(&f, state_property).await?;
            ids.push(propose(&f, &token, *kind, target, body.clone()).await?);
        }
        let jobs_before = queued_jobs(&f).await?;
        let (status, body) = post(
            &f,
            &f.editor_session,
            "review/agent-proposals",
            json!({ "ids": ids, "action": "approve" }),
        )
        .await?;
        anyhow::ensure!(
            status == StatusCode::OK && body["answered"] == 2,
            "{status} {body}"
        );
        for (i, id) in ids.iter().enumerate() {
            let outcome = &body["outcomes"][i];
            anyhow::ensure!(
                keys(outcome) == ["code", "error", "id", "job_id", "merge_id", "queued", "state"]
                    && outcome["id"] == json!(id)
                    && outcome["state"] == "approved"
                    && outcome["queued"] == true
                    && outcome["code"].is_null(),
                "{outcome}"
            );
            anyhow::ensure!(job(&f, &outcome["job_id"]).await?.1 == "queued");
            anyhow::ensure!(history_row(&f, *id).await?["queued"] == true);
        }
        anyhow::ensure!(queued_jobs(&f).await? == jobs_before + 2);

        // ── 短语与规则没有「定了」这一态：行在，提议就照样落地 ──
        // 等着的时候这个人已经判过同一行；批准提议是他后来的又一次判定，盖过前一次
        for (kind, proposed_body, meanwhile, status_after) in [
            (
                Kind::Phrase,
                json!({ "property": key, "direction": "reverse" }),
                json!({}),
                "bound",
            ),
            (
                Kind::Rule,
                json!({ "approve": false }),
                json!({ "approve": true }),
                "rejected",
            ),
        ] {
            let target = kind.seed(&f, state_property).await?;
            let proposal = propose(&f, &token, kind, target, proposed_body).await?;
            let (status, body) = decide(&f, &f.editor_session, kind, target, meanwhile).await?;
            anyhow::ensure!(status == StatusCode::ACCEPTED, "{status} {body}");
            let (status, answered) = answer(&f, &f.editor_session, proposal, "approve").await?;
            anyhow::ensure!(
                status == StatusCode::OK
                    && answered["state"] == "approved"
                    && answered["queued"] == true
                    && job(&f, &answered["job_id"]).await?.1 == "queued",
                "a {kind:?} proposal applies while its row exists: {status} {answered}"
            );
            let decided = row(&f, kind.table(), target).await?;
            anyhow::ensure!(decided["status"] == status_after, "{decided}");
            anyhow::ensure!(decided_audits(&audits(&f).await?, kind, target).len() == 2);
        }
        // 行没了：批准什么都不改、一个任务不排，答 superseded，那一行不读作排着
        for (kind, body) in [
            (
                Kind::Phrase,
                json!({ "property": key, "direction": "forward" }),
            ),
            (Kind::Rule, json!({ "approve": true })),
        ] {
            let target = kind.seed(&f, state_property).await?;
            let proposal = propose(&f, &token, kind, target, body).await?;
            sqlx::query(&format!("DELETE FROM {} WHERE id = $1", kind.table()))
                .bind(target)
                .execute(&f.pool)
                .await?;
            let before = snapshot(&f).await?;
            let (status, answered) = answer(&f, &f.editor_session, proposal, "approve").await?;
            anyhow::ensure!(
                status == StatusCode::OK
                    && answered == json!({ "ok": true, "state": "superseded" }),
                "a {kind:?} proposal whose row is gone: {status} {answered}"
            );
            anyhow::ensure!(
                snapshot(&f).await? == before,
                "a superseded {kind:?} proposal changed something or queued a job"
            );
            let listed = history_row(&f, proposal).await?;
            anyhow::ensure!(
                listed["state"] == "superseded" && listed.get("queued").is_none(),
                "{listed}"
            );
            anyhow::ensure!(decided_audits(&audits(&f).await?, kind, target).is_empty());
        }
        Ok(())
    }
    .await;
    finish(f, result).await
}

#[tokio::test]
async fn a_session_alignment_decision_still_answers_202_with_its_job_id() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let (state_property, key) = property_in(&f, f.kb, "state").await?;
        for (kind, body, _) in decisions(&key) {
            let target = kind.seed(&f, state_property).await?;
            let target_row = row(&f, kind.table(), target).await?;
            let jobs_before = queued_jobs(&f).await?;
            // Viewer 的会话照旧是 403，什么都不排
            let (status, refused) =
                decide(&f, &f.viewer_session, kind, target, body.clone()).await?;
            anyhow::ensure!(
                status == StatusCode::FORBIDDEN
                    && queued_jobs(&f).await? == jobs_before
                    && row(&f, kind.table(), target).await? == target_row,
                "a viewer: {status} {refused}"
            );
            // 编辑者的会话：202，正好这三个键
            let (status, answered) =
                decide(&f, &f.editor_session, kind, target, body.clone()).await?;
            anyhow::ensure!(
                status == StatusCode::ACCEPTED,
                "a session {kind:?} decision is still answered 202: {status} {answered}"
            );
            anyhow::ensure!(
                keys(&answered) == ["job_id", "ok", "status"]
                    && answered["ok"] == true
                    && answered["status"] == "accepted"
                    && answered["job_id"].is_i64(),
                "{answered}"
            );
            anyhow::ensure!(
                job(&f, &answered["job_id"]).await?.1 == "queued"
                    && queued_jobs(&f).await? == jobs_before + 1,
                "the session decision queued its job"
            );
            let decided = row(&f, kind.table(), target).await?;
            anyhow::ensure!(
                decided["decided_by"] == "person" && decided["status"] != target_row["status"],
                "{decided}"
            );
            // 审计行：同从前的动作名、同从前的键，记着排下的任务，不指着哪条提议
            let rows = audits(&f).await?;
            let own = decided_audits(&rows, kind, target);
            anyhow::ensure!(own.len() == 1, "{own:?}");
            let expected: &[&str] = match kind {
                Kind::Phrase => &["direction", "job_id", "marks", "phrase", "property"],
                Kind::Rule => &["approve", "job_id", "phrase", "reading", "trigger"],
            };
            anyhow::ensure!(
                own[0].actor == Some(f.editor)
                    && own[0].target_kind == kind.target_kind()
                    && keys(&own[0].detail) == expected
                    && own[0].detail["job_id"] == answered["job_id"],
                "a session {kind:?} decision records what it always did: {:?}",
                own[0]
            );
            match kind {
                Kind::Phrase => anyhow::ensure!(
                    own[0].detail["property"] == body["property"]
                        && own[0].detail["direction"] == body["direction"]
                        && own[0].detail["marks"] == body["marks"],
                    "{:?}",
                    own[0]
                ),
                Kind::Rule => anyhow::ensure!(
                    own[0].detail["approve"] == body["approve"]
                        && own[0].detail["trigger"] == "phrase"
                        && own[0].detail["reading"].is_null(),
                    "{:?}",
                    own[0]
                ),
            }
        }
        // 会话的判定不进台账
        anyhow::ensure!(
            ledger(&f).await?.is_empty(),
            "a session decision writes no ledger row"
        );
        Ok(())
    }
    .await;
    finish(f, result).await
}

#[tokio::test]
async fn the_mcp_violation_errata_and_alignment_tools_propose_and_change_nothing(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let (token_id, editor_write) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let (_, editor_read) = f.token(f.editor, "reader", "read", None).await?;
        let (_, viewer_read) = f.token(f.viewer, "viewer reader", "read", None).await?;
        let (_, viewer_write) = f.token(f.viewer, "viewer writer", "write", None).await?;
        let (state_property, key) = property_in(&f, f.kb, "state").await?;
        let status_of = predicate_in(&f, f.kb).await?;
        let manages = violations::asymmetric_in(&f, f.kb).await?;
        let real = violations::asymmetries(&f, manages, 2).await?;
        let (violation, resolved) = (real[0], real[1]);
        let (status, body) = violations::decide(
            &f,
            &f.editor_session,
            resolved,
            json!({ "resolution": "accepted" }),
        )
        .await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let derived = violations::derived_in(&f, f.kb, status_of).await?;
        let violation_fact =
            violations::fact_of(&row(&f, "axiom_violations", violation).await?, "left_fact")?;
        let (held, _) = errata::held_in(&f, f.kb, status_of, true).await?;
        let (answered, _) = errata::held_in(&f, f.kb, status_of, true).await?;
        let (status, body) =
            errata::decide(&f, &f.editor_session, answered, json!({ "approve": false })).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let phrase = phrase_in(&f, f.kb).await?;
        let rule = rule_in(&f, f.kb, state_property).await?;

        let before = snapshot(&f).await?;
        let targets = [
            ("axiom_violations", violation),
            ("axiom_violations", derived),
            ("errata_actions", held),
            ("phrase_bindings", phrase),
            ("implication_rules", rule),
        ];
        let mut target_rows = Vec::new();
        for (table, id) in targets {
            target_rows.push(row(&f, table, id).await?);
        }
        let rows_before = ledger(&f).await?.len();
        // 人的裁决写下的审计行：工具的调用一行都不该多（每次工具调用自己的
        // `mcp.tool_called`、提议落进台账的那一行不算）
        let decisions_in = |rows: &[Audit]| {
            rows.iter()
                .filter(|a| {
                    [
                        "violation.decided",
                        "errata.decided",
                        "alignment.phrase_decided",
                        "alignment.rule_decided",
                        "fact.reject",
                        "fact.close",
                    ]
                    .contains(&a.action.as_str())
                })
                .count()
        };
        let audits_before = decisions_in(&audits(&f).await?);
        const TOOLS: [&str; 3] = ["review_violation", "review_errata", "review_alignment"];

        // ── 三个工具只列给编辑者的写令牌 ──
        let mut described = Vec::new();
        for (who, token, listed) in [
            ("write token of an editor", &editor_write, true),
            ("read token of an editor", &editor_read, false),
            ("read token of a viewer", &viewer_read, false),
            ("write token of a viewer", &viewer_write, false),
        ] {
            let tools = tools_list(&f, token).await?;
            for name in TOOLS {
                let tool = tools.iter().find(|t| t["name"] == name).cloned();
                anyhow::ensure!(
                    tool.is_some() == listed,
                    "{who}: {name} listed = {}",
                    tool.is_some()
                );
                if let Some(tool) = tool {
                    described.push(tool);
                }
            }
            anyhow::ensure!(
                tools.iter().any(|t| t["name"] == "review_list"),
                "{who}: the read tools stay listed"
            );
            // 没有哪个工具能答提议，名字里也没有
            for tool in &tools {
                let name = tool["name"].as_str().unwrap_or_default();
                anyhow::ensure!(
                    !["proposal", "approve", "decline", "answer"]
                        .iter()
                        .any(|word| name.contains(word)),
                    "no tool answers a proposal: {name}"
                );
            }
        }
        anyhow::ensure!(described.len() == 3, "{described:?}");
        for tool in &described {
            anyhow::ensure!(
                tool["annotations"]
                    == json!({ "readOnlyHint": false, "destructiveHint": false,
                               "idempotentHint": false }),
                "{tool}"
            );
            anyhow::ensure!(
                tool["inputSchema"]["properties"]["id"]["format"] == "uuid",
                "{tool}"
            );
            // 说明：是提议、什么都不改、谁来答、回什么、重复时怎样；没有答提议的工具
            let description = tool["description"].as_str().unwrap_or_default();
            anyhow::ensure!(
                description.starts_with("Propose that a person")
                    && description.ends_with("No tool answers a proposal."),
                "{description}"
            );
            for said in [
                "changes nothing",
                "a signed-in editor answers in the Review tab",
                "outcome: \"proposed\"",
                "action_id",
                "propose_only",
                "already_proposed",
            ] {
                anyhow::ensure!(description.contains(said), "{said}: {description}");
            }
        }
        // 参数就是路由的 id 与请求体的字段；对齐的多一个 `kind`，说走哪一条路由
        for (tool, properties, required) in [
            (
                &described[0],
                vec![
                    "close_at",
                    "close_at_precision",
                    "fact_id",
                    "id",
                    "resolution",
                ],
                json!(["id", "resolution"]),
            ),
            (
                &described[1],
                vec!["approve", "id"],
                json!(["id", "approve"]),
            ),
            (
                &described[2],
                vec!["approve", "direction", "id", "kind", "marks", "property"],
                json!(["kind", "id"]),
            ),
        ] {
            let schema = &tool["inputSchema"];
            anyhow::ensure!(
                keys(&schema["properties"]) == properties && schema["required"] == required,
                "{tool}"
            );
        }
        anyhow::ensure!(
            described[1]["inputSchema"]["properties"]["approve"]["type"] == "boolean"
                && described[2]["inputSchema"]["properties"]["approve"]["type"] == "boolean"
                && described[0]["inputSchema"]["properties"]["fact_id"]["format"] == "uuid",
            "{described:?}"
        );
        // 对齐的工具说了：批准之后改动是排着，不是已经落地（N6）
        let alignment = described[2]["description"].as_str().unwrap_or_default();
        for said in [
            "no job is queued",
            "queued as a job, not applied at once",
            "an approval queues the change",
            "reads queued",
        ] {
            anyhow::ensure!(alignment.contains(said), "{said}: {alignment}");
        }

        // 每个工具的一次合规调用，以及同一件事的 REST 调用
        let calls = [
            (
                "review_violation",
                json!({ "id": violation, "resolution": "accepted" }),
                format!("review/violations/{violation}"),
                json!({ "resolution": "accepted" }),
            ),
            (
                "review_errata",
                json!({ "id": held, "approve": true }),
                format!("review/errata/{held}"),
                json!({ "approve": true }),
            ),
            (
                "review_alignment",
                json!({ "kind": "phrase", "id": phrase, "property": key,
                        "direction": "forward" }),
                format!("review/alignment/phrases/{phrase}"),
                json!({ "property": key, "direction": "forward" }),
            ),
            (
                "review_alignment",
                json!({ "kind": "rule", "id": rule, "approve": true }),
                format!("review/alignment/rules/{rule}"),
                json!({ "approve": true }),
            ),
        ];

        // ── 写不了的调用方调得到这三个名字，回来的是带 REST code 的工具错误 ──
        for (token, code) in [
            (&editor_read, "token_read_only"),
            (&viewer_read, "token_read_only"),
            (&viewer_write, "token_owner_below_editor"),
        ] {
            for (name, args, tail, body) in &calls {
                let refused = tool_call(&f, token, name, args.clone()).await?;
                ensure_tool_refused(&refused, code)
                    .map_err(|e| anyhow::anyhow!("{name} {args}: {e}"))?;
                let (status, over_rest) = post(&f, token, tail, body.clone()).await?;
                anyhow::ensure!(
                    status == StatusCode::FORBIDDEN
                        && over_rest["code"] == code
                        && refused["structuredContent"]["message"] == over_rest["error"],
                    "{name}: {status} {over_rest} {refused}"
                );
            }
        }
        // 操作自己的拒绝：与 REST 同一个 code
        let unknown = Uuid::now_v7();
        for (name, args, code) in [
            (
                "review_violation",
                json!({ "id": unknown, "resolution": "accepted" }),
                "not_found",
            ),
            (
                "review_violation",
                json!({ "id": resolved, "resolution": "accepted" }),
                "already_decided",
            ),
            (
                "review_violation",
                json!({ "id": violation, "resolution": "dismissed" }),
                "bad_resolution",
            ),
            (
                "review_violation",
                json!({ "id": violation, "resolution": "fact_retracted" }),
                "fact_required",
            ),
            (
                "review_violation",
                json!({ "id": violation, "resolution": "fact_retracted", "fact_id": unknown }),
                "fact_required",
            ),
            (
                "review_violation",
                json!({ "id": violation, "resolution": "fact_closed",
                        "close_at": "2024-08-13T00:00:00Z" }),
                "bad_resolution",
            ),
            (
                "review_violation",
                json!({ "id": derived, "resolution": "fact_closed" }),
                "close_at_required",
            ),
            (
                "review_violation",
                json!({ "id": derived, "resolution": "fact_closed",
                        "close_at": "2024-08-13T00:00:00Z", "close_at_precision": "week" }),
                "bad_precision",
            ),
            (
                "review_violation",
                json!({ "id": derived, "resolution": "fact_closed",
                        "close_at": "2023-06-01T00:00:00Z" }),
                "interval_inverted",
            ),
            (
                "review_errata",
                json!({ "id": unknown, "approve": true }),
                "not_found",
            ),
            (
                "review_errata",
                json!({ "id": answered, "approve": true }),
                "already_decided",
            ),
            (
                "review_alignment",
                json!({ "kind": "phrase", "id": unknown }),
                "not_found",
            ),
            (
                "review_alignment",
                json!({ "kind": "rule", "id": unknown, "approve": true }),
                "not_found",
            ),
            // 短语的 id 不是规则的 id：`kind` 说的那条路由上没有它
            (
                "review_alignment",
                json!({ "kind": "rule", "id": phrase, "approve": true }),
                "not_found",
            ),
            (
                "review_alignment",
                json!({ "kind": "phrase", "id": phrase, "property": "nope",
                        "direction": "forward" }),
                "unknown_property",
            ),
            (
                "review_alignment",
                json!({ "kind": "phrase", "id": phrase, "property": key }),
                "direction_required",
            ),
            (
                "review_alignment",
                json!({ "kind": "phrase", "id": phrase, "property": key,
                        "direction": "forward", "marks": "middle" }),
                "unknown_marks",
            ),
            (
                "review_alignment",
                json!({ "kind": "phrase", "id": phrase, "marks": "start" }),
                "marks_needs_state",
            ),
        ] {
            let refused = tool_call(&f, &editor_write, name, args.clone()).await?;
            ensure_tool_refused(&refused, code)
                .map_err(|e| anyhow::anyhow!("{name} {args}: {e}"))?;
        }
        // 参数没给全、给得不对：没有执行，不带 code
        for (name, args) in [
            ("review_violation", json!({ "id": violation })),
            (
                "review_violation",
                json!({ "id": "the first one", "resolution": "accepted" }),
            ),
            (
                "review_violation",
                json!({ "id": derived, "resolution": "fact_closed", "close_at": "soon" }),
            ),
            (
                "review_violation",
                json!({ "id": violation, "resolution": "fact_retracted", "fact_id": "left" }),
            ),
            ("review_errata", json!({ "id": held })),
            ("review_errata", json!({ "id": held, "approve": "yes" })),
            ("review_alignment", json!({ "id": phrase })),
            (
                "review_alignment",
                json!({ "kind": "kind_word", "id": phrase }),
            ),
            ("review_alignment", json!({ "kind": "rule", "id": rule })),
            (
                "review_alignment",
                json!({ "kind": "rule", "id": rule, "approve": "yes" }),
            ),
        ] {
            let refused = tool_call(&f, &editor_write, name, args.clone()).await?;
            anyhow::ensure!(
                refused["isError"] == true && refused["structuredContent"].get("code").is_none(),
                "{name} {args}: {refused}"
            );
        }
        let unchanged = |what: &'static str| {
            let (f, before, target_rows) = (&f, &before, &target_rows);
            async move {
                anyhow::ensure!(
                    &snapshot(f).await? == before,
                    "{what} changed a fact, an entity, evidence or the queued jobs"
                );
                for ((table, id), was) in targets.iter().zip(target_rows) {
                    anyhow::ensure!(
                        &row(f, table, *id).await? == was,
                        "{what} changed a row of {table}"
                    );
                }
                anyhow::ensure!(
                    decisions_in(&audits(f).await?) == audits_before,
                    "{what} wrote a decision's audit row"
                );
                Ok(())
            }
        };
        unchanged("a refused tool call").await?;
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before,
            "a refused tool call wrote a row"
        );

        // ── 提议：`structuredContent` 就是 REST 的那个回答；什么都没动 ──
        let expected = [
            ("violation.decide", "axiom_violation", violation, "accepted"),
            ("errata.decide", "errata_action", held, "approve"),
            ("alignment.phrase", "phrase_binding", phrase, "bind"),
            ("alignment.rule", "implication_rule", rule, "approve"),
        ];
        for ((name, args, tail, body), (operation, target_kind, target, action)) in
            calls.iter().zip(expected)
        {
            let result = tool_call(&f, &editor_write, name, args.clone()).await?;
            anyhow::ensure!(result["isError"] == false, "{name}: {result}");
            let proposal = proposed(&result["structuredContent"])?;
            unchanged("a tool's proposal").await?;
            let stored = ledger_row(&f, proposal).await?;
            anyhow::ensure!(
                stored.operation == operation
                    && stored.target_kind == target_kind
                    && stored.target_id == target
                    && stored.token_id == token_id
                    && stored.outcome == "proposed"
                    && stored.state.is_none()
                    && &stored.payload["request"] == body
                    && stored.payload["action"] == action,
                "{stored:?}"
            );
            // 重复：两个入口拿回的是同一个回答，逐字相等
            let (status, over_rest) = post(&f, &editor_write, tail, body.clone()).await?;
            anyhow::ensure!(
                status == StatusCode::OK
                    && over_rest["code"] == "already_proposed"
                    && over_rest["action_id"] == json!(proposal),
                "{name}: {status} {over_rest}"
            );
            let again = tool_call(&f, &editor_write, name, args.clone()).await?;
            anyhow::ensure!(
                again["isError"] == false && again["structuredContent"] == over_rest,
                "the tool answers what the token API answers\n{again}\n{over_rest}"
            );
        }
        // 撤事实、闭合断言的参数原样进了请求体
        let args = json!({ "id": derived, "resolution": "fact_closed",
                           "close_at": "2024-08-13T00:00:00Z", "close_at_precision": "month" });
        let result = tool_call(&f, &editor_write, "review_violation", args).await?;
        anyhow::ensure!(result["isError"] == false, "{result}");
        let stored = ledger_row(&f, proposed(&result["structuredContent"])?).await?;
        anyhow::ensure!(
            stored.payload["request"]
                == json!({ "resolution": "fact_closed", "close_at": "2024-08-13T00:00:00Z",
                           "close_at_precision": "month" })
                && stored.payload["action"] == "fact_closed",
            "{}",
            stored.payload
        );
        // 说了撤哪一条的撤回过得了操作的检查；这处违规已经有一条开着的提议，回的是那一条
        let result = tool_call(
            &f,
            &editor_write,
            "review_violation",
            json!({ "id": violation, "resolution": "fact_retracted", "fact_id": violation_fact }),
        )
        .await?;
        anyhow::ensure!(
            result["isError"] == false && result["structuredContent"]["code"] == "already_proposed",
            "{result}"
        );
        unchanged("a tool's proposal").await?;
        anyhow::ensure!(ledger(&f).await?.len() == rows_before + 5);
        anyhow::ensure!(
            open_proposals(&f).await?.len() == 5,
            "every proposal a tool landed is still open"
        );
        Ok(())
    }
    .await;
    finish(f, result).await
}
