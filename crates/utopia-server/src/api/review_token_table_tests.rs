//! 整张裁定表一行一行走过 REST 与 MCP（0076）。
//!
//! 前面每一档各有自己的测试模块，钉的是那一档自己的事。这里钉的是**对每一行都得成立**
//! 的那几样，所以是一张表（[`Case`]）：每一行说出它在裁定表里是哪一行（`AgentOperation`）、
//! REST 怎么调、MCP 用哪个工具和什么参数、怎么种一个目标、目标怎么算「已经定了」、
//! 哪些请求体是人的路由会拒绝的。每个测试把整张表走一遍：
//!
//! - **只能提议的调用什么都不改**（SC1 / E1）：两个入口各调一次，事实、实体、证据、排着的
//!   任务、目标自己那一行前后相等，目标上多一条开着的提议
//! - **认不出的、别的库的、已经定了的目标不留提议**（E3）：404、404、`already_decided`
//! - **请求体照人的路由校验**（E4）：令牌拿到的状态与 code 就是会话对同一个请求体拿到的
//! - **提议与被拒绝的调用不算进限速**（E5）
//! - **一次批量回答收 1 到 500 条任意操作的提议**（E10），批准的每一条都是这个人的决定（N2）
//! - **每一行的结果就是裁定表说的**（SC6 / E19 / N9）：期望值问的是编进二进制的
//!   `agent_policy`，不是这里另写的一张表；被拒绝的调用方在两个入口拿到同一个 code
//! - **每一笔都读作「主人 via 令牌名」**（SC7 / E20），令牌改名、撤销之后照旧
//! - **登录的人的决定照旧**（SC9 / E22）
//! - **没有令牌、没有工具答得了提议**（N3）；`review_batch` 的说明说清了认不出的动作拿到什么
//!
//! 夹具在 `review_token_write_support.rs`，种目标的几个函数借各档测试模块的。
//! MCP 每一次 `tools/call` 都记一行 `mcp.tool_called`（被拒绝的也记，M003 起就是这样），
//! 数审计行时不算它。没有 `UTOPIA_DATABASE_URL` 时跳过。
use std::collections::HashSet;

use axum::http::StatusCode;
use serde_json::{json, Value};
use utopia_store::execution_gate::{agent_policy, AgentOperation, AgentOutcome};
use utopia_store::token_agent_actions::TokenAgentAction;
use uuid::Uuid;

use super::review_token_conflict_tests::conflict_in;
use super::review_token_errata_tests::held_in;
use super::review_token_fact_tests::{fact_on, predicate_in, proposed, subject_in};
use super::review_token_violation_tests::derived_in;
use super::review_token_write_support::{
    audits, ensure_tool_refused, keys, ledger, open_proposals, post, row, seeded, snapshot,
    tool_call, tools_list, Audit, Fx,
};

const TOKEN_NAME: &str = "table agent";

// ───────────────────────── 表 ─────────────────────────

/// 裁定表的一行在 REST 上的一个入口。一行可以有几个入口：合并有队列里的一对与手动两种，
/// 运行有一致性检查与物化推导两种，本体提案有拒绝与采纳两种
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Case {
    Confirm,
    Reject,
    Close,
    Conflict,
    Pending,
    Governor,
    Phrase,
    Rule,
    Errata,
    OntologyReject,
    OntologyAdopt,
    Violation,
    Defect,
    Keep,
    Merge,
    ManualMerge,
    Revert,
    Batch,
    KindWord,
    Mapping,
    Consistency,
    Inference,
}

const CASES: [Case; 22] = [
    Case::Confirm,
    Case::Reject,
    Case::Close,
    Case::Conflict,
    Case::Pending,
    Case::Governor,
    Case::Phrase,
    Case::Rule,
    Case::Errata,
    Case::OntologyReject,
    Case::OntologyAdopt,
    Case::Violation,
    Case::Defect,
    Case::Keep,
    Case::Merge,
    Case::ManualMerge,
    Case::Revert,
    Case::Batch,
    Case::KindWord,
    Case::Mapping,
    Case::Consistency,
    Case::Inference,
];

/// 裁定表的每一行。**这个 `match` 不写通配分支**：`AgentOperation` 多一个变体而表里没有
/// 它的入口，这里编译不过——表漏一行不会悄悄过
fn every_row() -> Vec<AgentOperation> {
    use AgentOperation::*;
    let all = [
        ConfirmFact,
        RejectFact,
        CloseFact,
        Merge,
        Keep,
        RevertMerge,
        ResolveConflict,
        PendingNod,
        Batch,
        AnswerGovernor,
        AlignmentPhrase,
        AlignmentRule,
        AlignmentKindWord,
        Errata,
        OntologyProposal,
        Mapping,
        Violation,
        Defect,
        ConsistencyRun,
    ];
    for op in all {
        match op {
            ConfirmFact | RejectFact | CloseFact | Merge | Keep | RevertMerge | ResolveConflict
            | PendingNod | Batch | AnswerGovernor | AlignmentPhrase | AlignmentRule
            | AlignmentKindWord | Errata | OntologyProposal | Mapping | Violation | Defect
            | ConsistencyRun => {}
        }
    }
    all.to_vec()
}

/// 一个种下的目标。`id` 是路由里的那个 id（本体提案是它那一行的 id，路由按 `word` 认；
/// 撤回是那次合并；运行与类别词是库本身）；`other` 只有手动合并有（目标实体）；
/// `word` 是本体提案的 key 或类别词
#[derive(Clone, Debug)]
struct Target {
    id: Uuid,
    other: Option<Uuid>,
    word: String,
}

impl Target {
    fn of(id: Uuid) -> Self {
        Self {
            id,
            other: None,
            word: String::new(),
        }
    }

    fn keyed(id: Uuid, word: String) -> Self {
        Self {
            id,
            other: None,
            word,
        }
    }
}

/// 一个库里种目标要用的本体：一个单值状态属性（事实、矛盾、违规、勘误挂在它上面），
/// 一个关系（规则的结论、合并闸门的派生事实）
struct Seeds {
    kb: Uuid,
    state: Uuid,
    relation: Uuid,
}

impl Seeds {
    async fn new(f: &Fx, kb: Uuid) -> anyhow::Result<Self> {
        let state = predicate_in(f, kb).await?;
        let relation = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO relation_types (id, kb_id, key, label, kind, temporal)
             VALUES ($1, $2, $3, 'joined', 'relation', 'eternal')",
        )
        .bind(relation)
        .bind(kb)
        .bind(format!("joined_{}", relation.simple()))
        .execute(&f.pool)
        .await?;
        Ok(Self {
            kb,
            state,
            relation,
        })
    }
}

/// 闭合一条事实的请求体：2024 年 6 月结束
fn closing() -> Value {
    json!({ "valid_to": "2024-06-01T00:00:00Z", "valid_to_precision": "month" })
}

impl Case {
    /// 它在裁定表里是哪一行
    fn row(self) -> AgentOperation {
        use AgentOperation as Op;
        match self {
            Case::Confirm => Op::ConfirmFact,
            Case::Reject => Op::RejectFact,
            Case::Close => Op::CloseFact,
            Case::Conflict => Op::ResolveConflict,
            Case::Pending => Op::PendingNod,
            Case::Governor => Op::AnswerGovernor,
            Case::Phrase => Op::AlignmentPhrase,
            Case::Rule => Op::AlignmentRule,
            Case::Errata => Op::Errata,
            Case::OntologyReject | Case::OntologyAdopt => Op::OntologyProposal,
            Case::Violation => Op::Violation,
            Case::Defect => Op::Defect,
            Case::Keep => Op::Keep,
            Case::Merge | Case::ManualMerge => Op::Merge,
            Case::Revert => Op::RevertMerge,
            Case::Batch => Op::Batch,
            Case::KindWord => Op::AlignmentKindWord,
            Case::Mapping => Op::Mapping,
            Case::Consistency | Case::Inference => Op::ConsistencyRun,
        }
    }

    /// 裁定表说这一行只能提议：问的是 `agent_policy`
    fn proposes(self) -> bool {
        agent_policy(self.row(), None) == AgentOutcome::Proposed
    }

    /// 台账里这一次调用写下的那一行的操作名（成批的一批写的是每一对的那一行）
    fn operation(self) -> &'static str {
        match self {
            Case::Confirm => "fact.confirm",
            Case::Reject => "fact.reject",
            Case::Close => "fact.close",
            Case::Conflict => "conflict.resolve",
            Case::Pending => "pending.nod",
            Case::Governor => "governor.answer",
            Case::Phrase => "alignment.phrase",
            Case::Rule => "alignment.rule",
            Case::Errata => "errata.decide",
            Case::OntologyReject | Case::OntologyAdopt => "ontology.proposal",
            Case::Violation => "violation.decide",
            Case::Defect => "defect.decide",
            Case::Keep => "review.keep",
            Case::Merge | Case::Batch => "review.merge",
            Case::ManualMerge => "merge.manual",
            Case::Revert => "merge.revert",
            Case::KindWord => "alignment.kind_word",
            Case::Mapping => "mapping.decide",
            Case::Consistency => "consistency.check",
            Case::Inference => "inference.run",
        }
    }

    /// 这一刀的台账目标种类；M003 的四行不在这里比
    fn target_kind(self) -> Option<&'static str> {
        Some(match self {
            Case::Confirm | Case::Reject | Case::Close => "fact",
            Case::Conflict => "conflict",
            Case::Pending => "pending_fact",
            Case::Governor => "agent_decision",
            Case::Phrase => "phrase_binding",
            Case::Rule => "implication_rule",
            Case::Errata => "errata_action",
            Case::OntologyReject | Case::OntologyAdopt => "ontology_proposal",
            Case::Violation => "axiom_violation",
            Case::Defect => "ontology_defect",
            Case::KindWord => "type_binding",
            Case::Mapping => "concept_mapping",
            Case::Consistency | Case::Inference => "knowledge_base",
            Case::Keep | Case::Merge | Case::ManualMerge | Case::Revert | Case::Batch => {
                return None
            }
        })
    }

    /// 目标自己那一行在哪张表：只能提议的行比它前后相等
    fn table(self) -> Option<&'static str> {
        Some(match self {
            Case::Confirm | Case::Reject | Case::Close => "facts",
            Case::Conflict => "fact_conflicts",
            Case::Pending => "pending_facts",
            Case::Governor => "agent_decisions",
            Case::Phrase => "phrase_bindings",
            Case::Rule => "implication_rules",
            Case::Errata => "errata_actions",
            Case::OntologyReject | Case::OntologyAdopt => "ontology_proposals",
            Case::Violation => "axiom_violations",
            Case::Defect => "ontology_defects",
            _ => return None,
        })
    }

    /// 这一行有没有一个按 id 认的目标（类别词按词 upsert，两种运行的目标是库本身）
    fn has_target(self) -> bool {
        !matches!(self, Case::KindWord | Case::Consistency | Case::Inference)
    }

    /// 令牌的 REST 调用：`/kbs/{kb}/` 后面那一段与请求体
    fn rest(self, t: &Target) -> (String, Value) {
        let id = t.id;
        match self {
            Case::Confirm => (format!("facts/{id}/confirm"), json!({})),
            Case::Reject => (format!("facts/{id}/reject"), json!({})),
            Case::Close => (format!("facts/{id}/close"), closing()),
            Case::Conflict => (format!("conflicts/{id}"), json!({ "action": "keep" })),
            Case::Pending => (
                format!("review/pending/{id}/propose"),
                json!({ "action": "confirm" }),
            ),
            Case::Governor => (
                format!("review/agent/{id}/propose"),
                json!({ "action": "merge" }),
            ),
            Case::Phrase => (format!("review/alignment/phrases/{id}"), json!({})),
            Case::Rule => (
                format!("review/alignment/rules/{id}"),
                json!({ "approve": true }),
            ),
            Case::Errata => (format!("review/errata/{id}"), json!({ "approve": false })),
            Case::OntologyReject => (
                "ontology/proposals".into(),
                json!({ "section": "entity_types", "key": t.word, "status": "rejected" }),
            ),
            Case::OntologyAdopt => (
                "ontology/proposals/adopt".into(),
                json!({ "section": "entity_types", "key": t.word }),
            ),
            Case::Violation => (
                format!("review/violations/{id}"),
                json!({ "resolution": "accepted" }),
            ),
            Case::Defect => (
                format!("review/defects/{id}"),
                json!({ "resolution": "fixed" }),
            ),
            Case::Keep => (format!("review/{id}"), json!({ "action": "keep" })),
            Case::Merge => (format!("review/{id}"), json!({ "action": "merge" })),
            Case::ManualMerge => (
                "entities/merge".into(),
                json!({ "source": id, "target": t.other }),
            ),
            Case::Revert => (format!("merges/{id}/revert"), json!({})),
            Case::Batch => (
                "review/batch".into(),
                json!({ "ids": [id], "action": "merge" }),
            ),
            Case::KindWord => (
                format!("review/alignment/kind-words/{}", t.word),
                json!({ "class": "person" }),
            ),
            Case::Mapping => (
                format!("review/mappings/{id}"),
                json!({ "status": "confirmed" }),
            ),
            Case::Consistency => ("consistency/check".into(), json!({})),
            Case::Inference => ("inference/run".into(), json!({})),
        }
    }

    /// 登录的人自己的那条路由：等点头的事实与治理的一笔是人的点头与回答路由，
    /// 别的就是令牌调的同一条
    fn person(self, t: &Target) -> String {
        match self {
            Case::Pending => format!("review/pending/{}", t.id),
            Case::Governor => format!("review/agent/{}", t.id),
            _ => self.rest(t).0,
        }
    }

    /// MCP 工具与参数
    fn tool(self, t: &Target) -> (&'static str, Value) {
        let id = t.id;
        match self {
            Case::Confirm => ("review_fact", json!({ "id": id, "action": "confirm" })),
            Case::Reject => ("review_fact", json!({ "id": id, "action": "reject" })),
            Case::Close => (
                "review_fact",
                json!({ "id": id, "action": "close", "valid_to": "2024-06-01T00:00:00Z",
                        "valid_to_precision": "month" }),
            ),
            Case::Conflict => ("review_conflict", json!({ "id": id, "action": "keep" })),
            Case::Pending => (
                "review_pending_fact",
                json!({ "id": id, "action": "confirm" }),
            ),
            Case::Governor => ("review_governor", json!({ "id": id, "action": "merge" })),
            Case::Phrase => ("review_alignment", json!({ "kind": "phrase", "id": id })),
            Case::Rule => (
                "review_alignment",
                json!({ "kind": "rule", "id": id, "approve": true }),
            ),
            Case::Errata => ("review_errata", json!({ "id": id, "approve": false })),
            Case::OntologyReject => (
                "review_ontology",
                json!({ "section": "entity_types", "key": t.word, "action": "reject" }),
            ),
            Case::OntologyAdopt => (
                "review_ontology",
                json!({ "section": "entity_types", "key": t.word, "action": "adopt" }),
            ),
            Case::Violation => (
                "review_violation",
                json!({ "id": id, "resolution": "accepted" }),
            ),
            Case::Defect => ("review_defect", json!({ "id": id, "resolution": "fixed" })),
            Case::Keep => ("review_keep", json!({ "id": id })),
            Case::Merge => ("review_merge", json!({ "id": id })),
            Case::ManualMerge => ("review_merge", json!({ "source": id, "target": t.other })),
            Case::Revert => ("review_revert", json!({ "merge_id": id })),
            Case::Batch => ("review_batch", json!({ "ids": [id], "action": "merge" })),
            Case::KindWord => (
                "review_kind_word",
                json!({ "kind_word": t.word, "class": "person" }),
            ),
            Case::Mapping => ("review_mapping", json!({ "id": id, "status": "confirmed" })),
            Case::Consistency => ("review_check", json!({ "run": "consistency" })),
            Case::Inference => ("review_check", json!({ "run": "inference" })),
        }
    }

    /// 人的路由会拒绝的请求体：(REST 的请求体, MCP 的参数)。MCP 表达不出来的（本体提案
    /// 的拒绝，工具替调用方写上 `rejected`）没有参数
    fn invalid(self, t: &Target) -> Vec<(Value, Option<Value>)> {
        let id = t.id;
        let key = &t.word;
        match self {
            Case::Close => vec![
                (
                    json!({ "valid_to": "2024-06-01T00:00:00Z", "valid_to_precision": "fortnight" }),
                    Some(
                        json!({ "id": id, "action": "close", "valid_to": "2024-06-01T00:00:00Z",
                                 "valid_to_precision": "fortnight" }),
                    ),
                ),
                (
                    json!({ "valid_to": "2023-01-01T00:00:00Z" }),
                    Some(json!({ "id": id, "action": "close",
                                 "valid_to": "2023-01-01T00:00:00Z" })),
                ),
            ],
            Case::Conflict => vec![
                (
                    json!({ "action": "keep", "close_at_precision": "fortnight" }),
                    Some(json!({ "id": id, "action": "keep", "close_at_precision": "fortnight" })),
                ),
                (
                    json!({ "action": "dismiss" }),
                    Some(json!({ "id": id, "action": "dismiss" })),
                ),
                (
                    json!({ "action": "retime_new" }),
                    Some(json!({ "id": id, "action": "retime_new" })),
                ),
            ],
            Case::Pending => vec![(
                json!({ "action": "maybe" }),
                Some(json!({ "id": id, "action": "maybe" })),
            )],
            Case::Governor => vec![(
                json!({ "action": "revert" }),
                Some(json!({ "id": id, "action": "revert" })),
            )],
            Case::Phrase => vec![(
                json!({ "property": "no_such_property" }),
                Some(json!({ "kind": "phrase", "id": id, "property": "no_such_property" })),
            )],
            Case::OntologyReject => vec![
                (
                    json!({ "section": "entity_types", "key": key, "status": "adopted" }),
                    None,
                ),
                (
                    json!({ "section": "entity_types", "key": key, "status": "bogus" }),
                    None,
                ),
            ],
            Case::OntologyAdopt => vec![(
                json!({ "section": "entity_types", "key": key, "parents": ["no_such_class"] }),
                Some(
                    json!({ "section": "entity_types", "key": key, "action": "adopt",
                             "parents": ["no_such_class"] }),
                ),
            )],
            Case::Violation => vec![
                (
                    json!({ "resolution": "ignore" }),
                    Some(json!({ "id": id, "resolution": "ignore" })),
                ),
                (
                    json!({ "resolution": "fact_closed" }),
                    Some(json!({ "id": id, "resolution": "fact_closed" })),
                ),
            ],
            Case::Defect => vec![(
                json!({ "resolution": "dismiss" }),
                Some(json!({ "id": id, "resolution": "dismiss" })),
            )],
            Case::Mapping => vec![(
                json!({ "status": "maybe" }),
                Some(json!({ "id": id, "status": "maybe" })),
            )],
            Case::KindWord => vec![(
                json!({ "class": "no_such_class" }),
                Some(json!({ "kind_word": t.word, "class": "no_such_class" })),
            )],
            _ => Vec::new(),
        }
    }

    /// 种一个对这一行开着的目标。`token` 只有撤回用：令牌撤得动的是令牌做的合并，所以
    /// 先用它合并一对（别的库里没有它的路由，那边用 store 合并）
    async fn seed(self, f: &Fx, s: &Seeds, token: &str) -> anyhow::Result<Target> {
        let kb = s.kb;
        let id = match self {
            Case::Confirm | Case::Reject | Case::Close => {
                let subject = subject_in(f, kb).await?;
                fact_on(f, kb, subject, s.state, "open", Some("2024-01-01")).await?
            }
            Case::Conflict => conflict_in(f, kb, s.state, Some("2024-03-01")).await?.2,
            Case::Pending => pending_in(f, kb).await?,
            Case::Governor => {
                let (review, _, _) = pair_in(f, kb).await?;
                decision_in(f, kb, review).await?
            }
            Case::Phrase => phrase_in(f, kb).await?,
            Case::Rule => rule_in(f, kb, s.relation).await?,
            Case::Errata => held_in(f, kb, s.state, true).await?.0,
            Case::OntologyReject | Case::OntologyAdopt => {
                // 类的键是小写 snake_case、最长 40：取 uuid 尾部那几位随机的
                let key = format!("table_class_{}", &Uuid::now_v7().simple().to_string()[20..]);
                let id = ontology_in(f, kb, &key).await?;
                return Ok(Target::keyed(id, key));
            }
            Case::Violation => derived_in(f, kb, s.state).await?,
            Case::Defect => defect_in(f, kb).await?,
            Case::Keep | Case::Merge | Case::Batch => pair_in(f, kb).await?.0,
            Case::ManualMerge => {
                let (_, source, target) = pair_in(f, kb).await?;
                return Ok(Target {
                    id: source,
                    other: Some(target),
                    word: String::new(),
                });
            }
            Case::Revert => {
                let (review, left, right) = pair_in(f, kb).await?;
                if kb == f.kb {
                    let (status, answer) = post(
                        f,
                        token,
                        &format!("review/{review}"),
                        json!({ "action": "merge" }),
                    )
                    .await?;
                    anyhow::ensure!(
                        status == StatusCode::OK && answer["outcome"] == "applied",
                        "a token merge to revert: {status} {answer}"
                    );
                    uuid_at(&answer, "merge_id")?
                } else {
                    utopia_store::resolution::merge_entities(
                        &f.pool,
                        kb,
                        right,
                        left,
                        None,
                        "governance",
                    )
                    .await?
                }
            }
            Case::KindWord => {
                return Ok(Target::keyed(
                    kb,
                    format!("tableword{}", Uuid::now_v7().simple()),
                ))
            }
            Case::Mapping => mapping_in(f, kb).await?,
            Case::Consistency => kb,
            Case::Inference => {
                sqlx::query(
                    "UPDATE knowledge_bases SET materialize_inferences = TRUE WHERE id = $1",
                )
                .bind(kb)
                .execute(&f.pool)
                .await?;
                kb
            }
        };
        Ok(Target::of(id))
    }

    /// 一个已经定了的目标；这一行没有「定了」这一态（确认、短语、规则、等点头的事实、
    /// 类别词、两种运行）是 None
    async fn decided(self, f: &Fx, s: &Seeds, token: &str) -> anyhow::Result<Option<Target>> {
        let sql = match self {
            Case::Reject | Case::Close => "UPDATE facts SET invalidated_at = now() WHERE id = $1",
            Case::Conflict => "UPDATE fact_conflicts SET status = 'resolved' WHERE id = $1",
            Case::Governor => {
                "UPDATE agent_decisions SET status = 'accepted', decided_at = now() WHERE id = $1"
            }
            Case::Errata => "UPDATE errata_actions SET status = 'rejected' WHERE id = $1",
            Case::OntologyReject | Case::OntologyAdopt => {
                "UPDATE ontology_proposals SET status = 'rejected' WHERE id = $1"
            }
            Case::Violation => "UPDATE axiom_violations SET status = 'resolved' WHERE id = $1",
            Case::Defect => "UPDATE ontology_defects SET status = 'resolved' WHERE id = $1",
            Case::Keep | Case::Merge | Case::Batch => {
                "UPDATE resolution_reviews SET status = 'kept', decided_at = now() WHERE id = $1"
            }
            Case::Mapping => "UPDATE concept_mappings SET status = 'confirmed' WHERE id = $1",
            Case::ManualMerge => "UPDATE entities SET merged_into = $2 WHERE id = $1",
            Case::Revert => {
                // 撤过一次的合并：令牌自己撤，第二次就是「已经定了」
                let t = self.seed(f, s, token).await?;
                let (status, answer) = post(f, token, &self.rest(&t).0, json!({})).await?;
                anyhow::ensure!(
                    status == StatusCode::OK && answer["outcome"] == "applied",
                    "a token revert: {status} {answer}"
                );
                return Ok(Some(t));
            }
            _ => return Ok(None),
        };
        let t = self.seed(f, s, token).await?;
        let query = sqlx::query(sql).bind(t.id);
        // 只有手动合并的那一句有第二个参数（源并进了谁）
        let query = match (self, t.other) {
            (Case::ManualMerge, Some(other)) => query.bind(other),
            _ => query,
        };
        query.execute(&f.pool).await?;
        Ok(Some(t))
    }

    /// 这个库里没有的目标
    fn unknown(self) -> Target {
        match self {
            Case::OntologyReject | Case::OntologyAdopt => Target::keyed(
                Uuid::now_v7(),
                format!("nosuchclass{}", Uuid::now_v7().simple()),
            ),
            Case::ManualMerge => Target {
                id: Uuid::now_v7(),
                other: Some(Uuid::now_v7()),
                word: String::new(),
            },
            _ => Target::of(Uuid::now_v7()),
        }
    }
}

// ───────────────────────── 种目标 ─────────────────────────

/// 一个新实体。`f.kb` 里的带 Person 类（合并要两边同类），别的库里的没有类
async fn entity_in(f: &Fx, kb: Uuid, name: &str) -> anyhow::Result<Uuid> {
    if kb == f.kb {
        return f.entity(f.person_type, name).await;
    }
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO entities (id, kb_id, canonical_name) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(kb)
        .bind(name)
        .execute(&f.pool)
        .await?;
    Ok(id)
}

/// 一对等人的重复项，名字每次都不一样。回 (审核行, 左, 右)
async fn pair_in(f: &Fx, kb: Uuid) -> anyhow::Result<(Uuid, Uuid, Uuid)> {
    let name = format!("Table {}", Uuid::now_v7().simple());
    let left = entity_in(f, kb, &format!("{name} Left")).await?;
    let right = entity_in(f, kb, &format!("{name} Right")).await?;
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO resolution_reviews (id, kb_id, left_id, right_id, score, reason, stage)
         VALUES ($1, $2, $3, $4, 0.5, 'namesake', 'human')",
    )
    .bind(id)
    .bind(kb)
    .bind(left)
    .bind(right)
    .execute(&f.pool)
    .await?;
    Ok((id, left, right))
}

/// 治理 agent 的一条开着的建议：合并这一对
async fn decision_in(f: &Fx, kb: Uuid, review: Uuid) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO agent_decisions (id, kb_id, run_id, target_kind, target_id, action,
                                      confidence, reason, status)
         VALUES ($1, $2, $3, 'review', $4, 'merge', 0.8, 'test: same person', 'proposed')",
    )
    .bind(id)
    .bind(kb)
    .bind(Uuid::now_v7())
    .bind(review)
    .execute(&f.pool)
    .await?;
    Ok(id)
}

/// 一条等点头的事实：照种子里那条开放陈述（`advises`）抄一行，主语是一个新实体，
/// 记下它的人是 `f.viewer`
async fn pending_in(f: &Fx, kb: Uuid) -> anyhow::Result<Uuid> {
    let subject = entity_in(f, kb, &format!("Nod {}", Uuid::now_v7().simple())).await?;
    let id = Uuid::now_v7();
    let inserted = sqlx::query(
        "INSERT INTO pending_facts (id, kb_id, subject_id, object_id, proposed_predicate,
                                    confidence, chunk_id, phrase, proposed_by)
         SELECT $1, $2, $3, p.object_id, p.proposed_predicate, p.confidence, p.chunk_id,
                p.phrase, $4
           FROM pending_facts p
          WHERE p.kb_id = $5 AND p.phrase = 'advises' AND p.proposed_by IS NULL
          ORDER BY p.created_at, p.id LIMIT 1",
    )
    .bind(id)
    .bind(kb)
    .bind(subject)
    .bind(f.viewer)
    .bind(f.kb)
    .execute(&f.pool)
    .await?
    .rows_affected();
    anyhow::ensure!(
        inserted == 1,
        "the seeded `advises` pending fact is missing"
    );
    Ok(id)
}

/// 一条还没定的短语签名
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

/// 一条等人批的蕴含规则，结论落在 `property` 上
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

/// 本体代理提的一个新类（没有父类）
async fn ontology_in(f: &Fx, kb: Uuid, key: &str) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO ontology_proposals (id, kb_id, section, key, payload, proposed_by)
         VALUES ($1, $2, 'entity_types', $3, $4, 'agent')",
    )
    .bind(id)
    .bind(kb)
    .bind(key)
    .bind(
        json!({ "key": key, "label": format!("The {key}"), "description": format!("A {key}."),
                  "parents": [] }),
    )
    .execute(&f.pool)
    .await?;
    Ok(id)
}

/// 一处开着的本体缺陷（指着的本体元素是编出来的 id：缺陷的两列没有外键）
async fn defect_in(f: &Fx, kb: Uuid) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO ontology_defects (id, kb_id, kind, subject, detail)
         VALUES ($1, $2, 'symmetric_and_asymmetric', $3, '{}'::jsonb)",
    )
    .bind(id)
    .bind(kb)
    .bind(Uuid::now_v7())
    .execute(&f.pool)
    .await?;
    Ok(id)
}

/// 一条等人表态的语义层映射
async fn mapping_in(f: &Fx, kb: Uuid) -> anyhow::Result<Uuid> {
    let concept = entity_in(f, kb, &format!("Concept {}", Uuid::now_v7().simple())).await?;
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO concept_mappings (id, kb_id, concept_id, source, table_name)
         VALUES ($1, $2, $3, 'warehouse', $4)",
    )
    .bind(id)
    .bind(kb)
    .bind(concept)
    .bind(format!("table_{}", id.simple()))
    .execute(&f.pool)
    .await?;
    Ok(id)
}

/// 合并闸门会留下的一对：一侧有一条派生事实压着（`derived`）。回审核行
async fn held_pair(f: &Fx, s: &Seeds) -> anyhow::Result<Uuid> {
    let (review, left, _) = pair_in(f, f.kb).await?;
    let object = f.entity(f.company_type, "Held City").await?;
    let rule = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO rules (id, kb_id, predicate_id, kind) VALUES ($1, $2, $3, 'transitive')",
    )
    .bind(rule)
    .bind(f.kb)
    .bind(s.relation)
    .execute(&f.pool)
    .await?;
    sqlx::query(
        "INSERT INTO derived_facts (id, kb_id, subject_id, predicate_id, object_id, rule_id)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(Uuid::now_v7())
    .bind(f.kb)
    .bind(left)
    .bind(s.relation)
    .bind(object)
    .bind(rule)
    .execute(&f.pool)
    .await?;
    Ok(review)
}

/// 往台账里直接种 `n` 行这枚令牌五分钟前落地的写入：限速数的就是这些行
async fn seed_applied(f: &Fx, token_id: Uuid, n: i32) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO token_agent_actions
             (id, kb_id, operation, target_kind, target_id, owner_id, token_id, token_name,
              outcome, payload, target_key, created_at)
         SELECT gen_random_uuid(), $1, 'review.keep', 'review', gen_random_uuid(), $2, $3,
                'seeded', 'applied', '{}'::jsonb, gen_random_uuid()::text,
                now() - interval '5 minutes'
           FROM generate_series(1, $4) g",
    )
    .bind(f.kb)
    .bind(f.editor)
    .bind(token_id)
    .bind(n)
    .execute(&f.pool)
    .await?;
    Ok(())
}

// ───────────────────────── 调用与判断 ─────────────────────────

/// 一组调用方：编辑者的写令牌、编辑者的只读令牌、Viewer 的写令牌、够不着这个库的写令牌
struct Callers {
    token_id: Uuid,
    write: String,
    read: String,
    viewer: String,
    elsewhere: String,
}

impl Callers {
    async fn new(f: &Fx) -> anyhow::Result<Self> {
        let (token_id, write) = f.token(f.editor, TOKEN_NAME, "write", None).await?;
        let (_, read) = f.token(f.editor, "table reader", "read", None).await?;
        let (_, viewer) = f.token(f.viewer, "table viewer", "write", None).await?;
        let elsewhere = [f.other_kb];
        let (_, elsewhere) = f
            .token(f.editor, "table elsewhere", "write", Some(&elsewhere))
            .await?;
        Ok(Self {
            token_id,
            write,
            read,
            viewer,
            elsewhere,
        })
    }
}

fn uuid_at(v: &Value, key: &str) -> anyhow::Result<Uuid> {
    Ok(v[key]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("no {key} in {v}"))?
        .parse()?)
}

/// 令牌经 REST 调这一行
async fn call_rest(
    f: &Fx,
    bearer: &str,
    case: Case,
    t: &Target,
) -> anyhow::Result<(StatusCode, Value)> {
    let (tail, body) = case.rest(t);
    post(f, bearer, &tail, body).await
}

/// 令牌经 MCP 调这一行：回工具结果
async fn call_tool(f: &Fx, bearer: &str, case: Case, t: &Target) -> anyhow::Result<Value> {
    let (name, args) = case.tool(t);
    tool_call(f, bearer, name, args).await
}

/// 一个工具结果里成功的那一份
fn structured(case: Case, result: &Value) -> anyhow::Result<Value> {
    anyhow::ensure!(result["isError"] != true, "{case:?} over MCP: {result}");
    Ok(result["structuredContent"].clone())
}

/// 成批调用里那一对的结果：`(outcome, code)`
fn batch_item(body: &Value) -> (Value, Value) {
    let item = &body["outcomes"][0];
    (item["outcome"].clone(), item["code"].clone())
}

/// 台账里这一组 id 之外的行，最早的在前
async fn rows_since(f: &Fx, seen: &HashSet<Uuid>) -> anyhow::Result<Vec<TokenAgentAction>> {
    Ok(ledger(f)
        .await?
        .into_iter()
        .filter(|r| !seen.contains(&r.id))
        .collect())
}

async fn ledger_ids(f: &Fx) -> anyhow::Result<HashSet<Uuid>> {
    Ok(ledger(f).await?.into_iter().map(|r| r.id).collect())
}

/// 这个库的审计行，不算 MCP 每次调用都记的那一行
async fn audits_without_mcp(f: &Fx) -> anyhow::Result<Vec<Audit>> {
    Ok(audits(f)
        .await?
        .into_iter()
        .filter(|a| a.action != "mcp.tool_called")
        .collect())
}

/// 一个 REST 拒绝：状态，以及 code（`None` = 响应里没有 `code` 键）
fn ensure_rest_refused(
    what: &str,
    status: StatusCode,
    body: &Value,
    expected: StatusCode,
    code: Option<&str>,
) -> anyhow::Result<()> {
    anyhow::ensure!(status == expected, "{what}: {status} {body}");
    match code {
        Some(code) => anyhow::ensure!(body["code"] == code, "{what}: {status} {body}"),
        None => anyhow::ensure!(body.get("code").is_none(), "{what}: {status} {body}"),
    }
    Ok(())
}

/// 一次 JSON-RPC 调用的 HTTP 状态与整个响应体（越过 `tool_call` 的「必须是 200」）
async fn rpc(
    f: &Fx,
    bearer: &str,
    method: &str,
    params: Value,
) -> anyhow::Result<(StatusCode, Value)> {
    post(
        f,
        bearer,
        "mcp",
        json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }),
    )
    .await
}

/// 答一条提议的两条路由（只收会话）
async fn answer_one(
    f: &Fx,
    bearer: &str,
    id: Uuid,
    action: &str,
) -> anyhow::Result<(StatusCode, Value)> {
    post(
        f,
        bearer,
        &format!("review/agent-proposals/{id}"),
        json!({ "action": action }),
    )
    .await
}

async fn answer_many(
    f: &Fx,
    bearer: &str,
    ids: &[Uuid],
    action: &str,
) -> anyhow::Result<(StatusCode, Value)> {
    post(
        f,
        bearer,
        "review/agent-proposals",
        json!({ "ids": ids, "action": action }),
    )
    .await
}

/// 令牌经 REST 提一条，必须成：回提议的 id
async fn propose_rest(f: &Fx, token: &str, case: Case, t: &Target) -> anyhow::Result<Uuid> {
    let (status, body) = call_rest(f, token, case, t).await?;
    anyhow::ensure!(status == StatusCode::OK, "{case:?}: {status} {body}");
    proposed(&body).map_err(|e| anyhow::anyhow!("{case:?}: {e}"))
}

/// 收尾：判定、采纳、点头排下的任务不随库删除，先清掉这两个库的，再交给夹具
async fn finish(f: Fx, result: anyhow::Result<()>) -> anyhow::Result<()> {
    let cleared = sqlx::query("DELETE FROM jobs WHERE payload ->> 'kb_id' = ANY($1)")
        .bind(vec![f.kb.to_string(), f.other_kb.to_string()])
        .execute(&f.pool)
        .await;
    f.cleanup().await?;
    cleared?;
    result
}

// ───────────────────────── E1：只能提议的调用什么都不改 ─────────────────────────

#[tokio::test]
async fn a_propose_only_call_changes_no_row_and_queues_no_job_for_every_operation(
) -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let c = Callers::new(&f).await?;
        let s = Seeds::new(&f, f.kb).await?;
        // 先把每一行的两个目标都种好，再拍快照：种目标本身会添实体与事实
        let mut planned = Vec::new();
        for case in CASES.into_iter().filter(|c| c.proposes()) {
            let over_rest = case.seed(&f, &s, &c.write).await?;
            let over_mcp = case.seed(&f, &s, &c.write).await?;
            planned.push((case, over_rest, over_mcp));
        }
        // 十二行只能提议（本体提案有拒绝与采纳两个入口）
        let rows: HashSet<_> = planned.iter().map(|(case, ..)| case.row()).collect();
        anyhow::ensure!(
            rows.len() == 12 && planned.len() == 13,
            "twelve propose-only rows: {rows:?}"
        );
        let before = snapshot(&f).await?;
        for (case, over_rest, over_mcp) in &planned {
            let table = case
                .table()
                .ok_or_else(|| anyhow::anyhow!("{case:?} has no table"))?;
            for (surface, target) in [("REST", over_rest), ("MCP", over_mcp)] {
                let row_before = row(&f, table, target.id).await?;
                anyhow::ensure!(!row_before.is_null(), "{case:?}: no target row");
                let body = if surface == "REST" {
                    let (status, body) = call_rest(&f, &c.write, *case, target).await?;
                    anyhow::ensure!(status == StatusCode::OK, "{case:?}: {status} {body}");
                    body
                } else {
                    structured(*case, &call_tool(&f, &c.write, *case, target).await?)?
                };
                let id = proposed(&body).map_err(|e| anyhow::anyhow!("{case:?} {surface}: {e}"))?;
                anyhow::ensure!(
                    snapshot(&f).await? == before,
                    "{case:?} over {surface} changed a fact, an entity, evidence or the queued jobs"
                );
                anyhow::ensure!(
                    row(&f, table, target.id).await? == row_before,
                    "{case:?} over {surface} changed its target row"
                );
                let open: Vec<_> = open_proposals(&f)
                    .await?
                    .into_iter()
                    .filter(|p| p.target_id == target.id)
                    .collect();
                anyhow::ensure!(
                    open.len() == 1,
                    "{case:?} over {surface}: one open proposal: {open:?}"
                );
                let p = &open[0];
                anyhow::ensure!(
                    p.id == id
                        && p.operation == case.operation()
                        && Some(p.target_kind.as_str()) == case.target_kind()
                        && p.outcome == AgentOutcome::Proposed.as_str()
                        && p.gate_reason.as_deref() == Some("propose_only")
                        && p.owner_id == f.editor
                        && p.token_id == c.token_id
                        && p.token_name == TOKEN_NAME,
                    "{case:?} over {surface}: {p:?}"
                );
            }
        }
        Ok(())
    }
    .await;
    finish(f, result).await
}

// ───────────────────────── E3：没有的、别的库的、已经定了的目标 ─────────────────────────

#[tokio::test]
async fn an_unknown_foreign_or_decided_target_lands_no_proposal_for_every_operation(
) -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let c = Callers::new(&f).await?;
        let s = Seeds::new(&f, f.kb).await?;
        let elsewhere = Seeds::new(&f, f.other_kb).await?;
        for case in CASES.into_iter().filter(|c| c.has_target()) {
            let mut targets = vec![
                (
                    "unknown",
                    case.unknown(),
                    StatusCode::NOT_FOUND,
                    "not_found",
                ),
                (
                    "foreign",
                    case.seed(&f, &elsewhere, &c.write).await?,
                    StatusCode::NOT_FOUND,
                    "not_found",
                ),
            ];
            if let Some(t) = case.decided(&f, &s, &c.write).await? {
                targets.push(("decided", t, StatusCode::CONFLICT, "already_decided"));
            }
            for (what, t, expected, code) in targets {
                let seen = ledger_ids(&f).await?;
                let audits_before = audits_without_mcp(&f).await?.len();
                let (status, body) = call_rest(&f, &c.write, case, &t).await?;
                let result = call_tool(&f, &c.write, case, &t).await?;
                if case == Case::Batch {
                    // 成批的一批整个是 200：那一对的结果带着 code
                    anyhow::ensure!(status == StatusCode::OK, "{case:?} {what}: {status} {body}");
                    let (outcome, said) = batch_item(&body);
                    anyhow::ensure!(outcome.is_null() && said == code, "{case:?} {what}: {body}");
                    let (outcome, said) = batch_item(&structured(case, &result)?);
                    anyhow::ensure!(
                        outcome.is_null() && said == code,
                        "{case:?} {what} over MCP: {result}"
                    );
                } else {
                    let rest_code = (expected != StatusCode::NOT_FOUND).then_some(code);
                    ensure_rest_refused(
                        &format!("{case:?} {what}"),
                        status,
                        &body,
                        expected,
                        rest_code,
                    )?;
                    ensure_tool_refused(&result, code)
                        .map_err(|e| anyhow::anyhow!("{case:?} {what} over MCP: {e}"))?;
                }
                anyhow::ensure!(
                    rows_since(&f, &seen).await?.is_empty(),
                    "{case:?} {what} wrote a ledger row"
                );
                anyhow::ensure!(
                    audits_without_mcp(&f).await?.len() == audits_before,
                    "{case:?} {what} wrote an audit row"
                );
            }
        }
        Ok(())
    }
    .await;
    finish(f, result).await
}

// ───────────────────────── E4：请求体照人的路由校验 ─────────────────────────

#[tokio::test]
async fn an_invalid_body_gets_the_persons_code_and_lands_no_proposal() -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let c = Callers::new(&f).await?;
        let s = Seeds::new(&f, f.kb).await?;
        let mut checked = 0;
        for case in CASES {
            let t = case.seed(&f, &s, &c.write).await?;
            for (body, args) in case.invalid(&t) {
                let what = format!("{case:?} {body}");
                let seen = ledger_ids(&f).await?;
                let before = snapshot(&f).await?;
                let (tail, _) = case.rest(&t);
                let (status, said) = post(&f, &c.write, &tail, body.clone()).await?;
                let person = case.person(&t);
                let (session_status, session_said) =
                    post(&f, &f.editor_session, &person, body.clone()).await?;
                anyhow::ensure!(
                    status.is_client_error() && status != StatusCode::FORBIDDEN,
                    "{what}: the token was not refused by the body: {status} {said}"
                );
                anyhow::ensure!(
                    (status, said.get("code")) == (session_status, session_said.get("code")),
                    "{what}: token {status} {said}, session {session_status} {session_said}"
                );
                if let Some(args) = args {
                    let (name, _) = case.tool(&t);
                    let result = tool_call(&f, &c.write, name, args).await?;
                    match said["code"].as_str() {
                        Some(code) => ensure_tool_refused(&result, code)
                            .map_err(|e| anyhow::anyhow!("{what} over MCP: {e}"))?,
                        None => anyhow::ensure!(
                            result["isError"] == true
                                && result["structuredContent"].get("code").is_none(),
                            "{what} over MCP: {result}"
                        ),
                    }
                }
                anyhow::ensure!(
                    rows_since(&f, &seen).await?.is_empty(),
                    "{what} wrote a ledger row"
                );
                anyhow::ensure!(snapshot(&f).await? == before, "{what} changed the graph");
                checked += 1;
            }
            // 请求体被拒绝之后，同一个目标照样提得了：拒绝什么都没留下
            if case.proposes() && !case.invalid(&t).is_empty() {
                propose_rest(&f, &c.write, case, &t).await?;
            }
        }
        anyhow::ensure!(checked == 16, "{checked} invalid bodies were checked");
        // 映射的 `bad_status` 还在
        let mapping = Case::Mapping.seed(&f, &s, &c.write).await?;
        let (status, said) = post(
            &f,
            &c.write,
            &Case::Mapping.rest(&mapping).0,
            json!({ "status": "maybe" }),
        )
        .await?;
        ensure_rest_refused(
            "a mapping status",
            status,
            &said,
            StatusCode::UNPROCESSABLE_ENTITY,
            Some("bad_status"),
        )
    }
    .await;
    finish(f, result).await
}

// ───────────────────────── E5：提议与被拒绝的调用不算进限速 ─────────────────────────

#[tokio::test]
async fn proposals_and_refused_calls_never_count_toward_the_limit() -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let c = Callers::new(&f).await?;
        let s = Seeds::new(&f, f.kb).await?;
        // 落地过 199 笔：再落一笔就到 200
        seed_applied(&f, c.token_id, 199).await?;
        let defects: Vec<Uuid> = sqlx::query_scalar(
            "INSERT INTO ontology_defects (id, kb_id, kind, subject, detail)
             SELECT gen_random_uuid(), $1, 'symmetric_and_asymmetric', gen_random_uuid(),
                    '{}'::jsonb
               FROM generate_series(1, 200) g
             RETURNING id",
        )
        .bind(f.kb)
        .fetch_all(&f.pool)
        .await?;
        // 200 条提议
        for defect in &defects {
            propose_rest(&f, &c.write, Case::Defect, &Target::of(*defect)).await?;
        }
        // 一些被拒绝的调用：请求体不对、目标没有、已经提过
        for defect in defects.iter().take(5) {
            let (status, body) = post(
                &f,
                &c.write,
                &format!("review/defects/{defect}"),
                json!({ "resolution": "dismiss" }),
            )
            .await?;
            anyhow::ensure!(
                status == StatusCode::UNPROCESSABLE_ENTITY,
                "{status} {body}"
            );
            let (status, body) =
                call_rest(&f, &c.write, Case::Defect, &Case::Defect.unknown()).await?;
            anyhow::ensure!(status == StatusCode::NOT_FOUND, "{status} {body}");
            let (status, body) =
                call_rest(&f, &c.write, Case::Defect, &Target::of(*defect)).await?;
            anyhow::ensure!(
                status == StatusCode::OK && body["code"] == "already_proposed",
                "{status} {body}"
            );
        }
        // 200 条提议与那些拒绝之后，映射的决定照样落地：第 200 笔
        let mapping = Case::Mapping.seed(&f, &s, &c.write).await?;
        let seen = ledger_ids(&f).await?;
        let (status, body) = call_rest(&f, &c.write, Case::Mapping, &mapping).await?;
        anyhow::ensure!(status == StatusCode::OK, "the 200th write: {status} {body}");
        let written = rows_since(&f, &seen).await?;
        anyhow::ensure!(
            written.len() == 1
                && written[0].operation == "mapping.decide"
                && written[0].outcome == "applied",
            "{written:?}"
        );
        // 已经落地 200 笔：每一行只能提议的操作仍是 `proposed`，从不是 `agent_rate_limited`
        for case in CASES.into_iter().filter(|c| c.proposes()) {
            let t = case.seed(&f, &s, &c.write).await?;
            propose_rest(&f, &c.write, case, &t).await?;
            let t = case.seed(&f, &s, &c.write).await?;
            let body = structured(case, &call_tool(&f, &c.write, case, &t).await?)?;
            proposed(&body).map_err(|e| anyhow::anyhow!("{case:?} over MCP: {e}"))?;
        }
        // 而下一笔要落地的写入被限速：什么都不改，一行不写
        let next = Case::Mapping.seed(&f, &s, &c.write).await?;
        let seen = ledger_ids(&f).await?;
        let (status, body) = call_rest(&f, &c.write, Case::Mapping, &next).await?;
        ensure_rest_refused(
            "the 201st write",
            status,
            &body,
            StatusCode::TOO_MANY_REQUESTS,
            Some("agent_rate_limited"),
        )?;
        anyhow::ensure!(rows_since(&f, &seen).await?.is_empty());
        let status_now: String =
            sqlx::query_scalar("SELECT status FROM concept_mappings WHERE id = $1")
                .bind(next.id)
                .fetch_one(&f.pool)
                .await?;
        anyhow::ensure!(
            status_now == "proposed",
            "a limited call changed the mapping"
        );
        Ok(())
    }
    .await;
    finish(f, result).await
}

// ───────────────────────── E10 / N2：一次答多条，批准的是这个人的决定 ─────────────────────────

#[tokio::test]
async fn one_bulk_answer_takes_1_to_500_proposals_of_any_mix_of_operations() -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let c = Callers::new(&f).await?;
        let s = Seeds::new(&f, f.kb).await?;
        // 每一行只能提议的操作一条
        let mut ids = Vec::new();
        let mut kinds = Vec::new();
        for case in CASES.into_iter().filter(|c| c.proposes()) {
            let t = case.seed(&f, &s, &c.write).await?;
            ids.push(propose_rest(&f, &c.write, case, &t).await?);
            kinds.push(case.operation());
        }
        // 再加 M003 的三种：闸门留下的队列合并、人分开过的一对的手动合并、人做的合并的撤回
        let held = held_pair(&f, &s).await?;
        let (status, body) = post(
            &f,
            &c.write,
            &format!("review/{held}"),
            json!({ "action": "merge" }),
        )
        .await?;
        anyhow::ensure!(
            status == StatusCode::OK && body["outcome"] == "held",
            "a held merge: {status} {body}"
        );
        ids.push(uuid_at(&body, "action_id")?);
        kinds.push("review.merge");
        let (kept, left, right) = pair_in(&f, f.kb).await?;
        let (status, body) = post(
            &f,
            &f.editor_session,
            &format!("review/{kept}"),
            json!({ "action": "keep" }),
        )
        .await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let (status, body) = post(
            &f,
            &c.write,
            "entities/merge",
            json!({ "source": left, "target": right }),
        )
        .await?;
        anyhow::ensure!(
            status == StatusCode::OK && body["outcome"] == "held",
            "a held manual merge: {status} {body}"
        );
        ids.push(uuid_at(&body, "action_id")?);
        kinds.push("merge.manual");
        let (merged, left, right) = pair_in(&f, f.kb).await?;
        let (status, body) = post(
            &f,
            &f.editor_session,
            &format!("review/{merged}"),
            json!({ "action": "merge" }),
        )
        .await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let merge: Uuid = sqlx::query_scalar(
            "SELECT id FROM entity_merges
              WHERE kb_id = $1 AND source_id = ANY($2) AND reverted_at IS NULL",
        )
        .bind(f.kb)
        .bind(vec![left, right])
        .fetch_one(&f.pool)
        .await?;
        let (status, body) =
            post(&f, &c.write, &format!("merges/{merge}/revert"), json!({})).await?;
        anyhow::ensure!(
            status == StatusCode::OK && body["outcome"] == "proposed",
            "a proposed revert: {status} {body}"
        );
        ids.push(uuid_at(&body, "action_id")?);
        kinds.push("merge.revert");

        // 空的一批与 501 条：整个拒绝，什么都没答、什么都没改
        let states = |rows: Vec<TokenAgentAction>| -> Vec<(Uuid, Option<String>)> {
            rows.into_iter().map(|r| (r.id, r.state)).collect()
        };
        let ledger_before = states(ledger(&f).await?);
        let graph_before = snapshot(&f).await?;
        let mut too_many: Vec<Uuid> = ids.clone();
        too_many.extend((too_many.len()..501).map(|_| Uuid::now_v7()));
        for (what, batch) in [("empty", Vec::new()), ("501 ids", too_many)] {
            let (status, body) = answer_many(&f, &f.editor_session, &batch, "approve").await?;
            ensure_rest_refused(
                what,
                status,
                &body,
                StatusCode::UNPROCESSABLE_ENTITY,
                Some("batch_size"),
            )?;
        }
        anyhow::ensure!(
            states(ledger(&f).await?) == ledger_before,
            "a refused bulk answer closed a proposal"
        );
        anyhow::ensure!(
            snapshot(&f).await? == graph_before,
            "a refused bulk answer changed the graph"
        );

        // 一次批准全部：一条一个结果，按请求的次序
        let audits_before: HashSet<_> = audits(&f)
            .await?
            .into_iter()
            .map(|a| (a.action, a.target_id, a.detail.to_string()))
            .collect();
        let (status, body) = answer_many(&f, &f.editor_session, &ids, "approve").await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let outcomes = body["outcomes"].as_array().cloned().unwrap_or_default();
        anyhow::ensure!(
            body["answered"] == ids.len() && outcomes.len() == ids.len(),
            "{body}"
        );
        for ((outcome, id), operation) in outcomes.iter().zip(&ids).zip(&kinds) {
            anyhow::ensure!(
                outcome["id"] == json!(id) && outcome["state"] == "approved",
                "{operation}: {outcome}"
            );
            match *operation {
                "alignment.phrase" | "alignment.rule" => anyhow::ensure!(
                    outcome["job_id"].is_i64() && outcome["queued"] == true,
                    "{operation}: {outcome}"
                ),
                "pending.nod" => {
                    anyhow::ensure!(outcome["fact_id"].is_string(), "{operation}: {outcome}")
                }
                "review.merge" | "merge.manual" | "merge.revert" => {
                    anyhow::ensure!(outcome["merge_id"].is_string(), "{operation}: {outcome}")
                }
                _ => {}
            }
        }
        // N2：每一条批准都记成这个人的——actor 是这个人，detail 指着那条提议，不带 token_id
        let fresh: Vec<Audit> = audits(&f)
            .await?
            .into_iter()
            .filter(|a| {
                !audits_before.contains(&(a.action.clone(), a.target_id, a.detail.to_string()))
            })
            .collect();
        for (id, operation) in ids.iter().zip(&kinds) {
            let named: Vec<_> = fresh
                .iter()
                .filter(|a| a.detail["agent_proposal"] == json!(id))
                .collect();
            anyhow::ensure!(
                !named.is_empty(),
                "{operation}: no audit row names {id}: {fresh:?}"
            );
            for a in named {
                anyhow::ensure!(
                    a.actor == Some(f.editor)
                        && a.detail.get("token_id").is_none()
                        && a.detail.get("token_name").is_none(),
                    "{operation}: {a:?}"
                );
            }
        }
        anyhow::ensure!(
            open_proposals(&f).await?.is_empty(),
            "a proposal is still open"
        );
        Ok(())
    }
    .await;
    finish(f, result).await
}

// ───────────────────────── SC6 / E19 / N9：结果就是裁定表说的 ─────────────────────────

#[tokio::test]
async fn every_operation_of_the_ruled_table_gives_its_outcome_or_a_stable_refusal(
) -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let c = Callers::new(&f).await?;
        let s = Seeds::new(&f, f.kb).await?;
        // 表里有裁定表的每一行
        let covered: HashSet<_> = CASES.iter().map(|case| case.row()).collect();
        for op in every_row() {
            anyhow::ensure!(covered.contains(&op), "the table has no entry for {op:?}");
        }

        // 编辑者的写令牌：每一个入口在两个面上的结果，都是 `agent_policy` 给这一行的那个
        for case in CASES {
            let expected = agent_policy(case.row(), None).as_str();
            for surface in ["REST", "MCP"] {
                let t = case.seed(&f, &s, &c.write).await?;
                let seen = ledger_ids(&f).await?;
                let graph = snapshot(&f).await?;
                let body = if surface == "REST" {
                    let (status, body) = call_rest(&f, &c.write, case, &t).await?;
                    anyhow::ensure!(status == StatusCode::OK, "{case:?}: {status} {body}");
                    body
                } else {
                    structured(case, &call_tool(&f, &c.write, case, &t).await?)?
                };
                let written = rows_since(&f, &seen).await?;
                anyhow::ensure!(
                    written.len() == 1,
                    "{case:?} over {surface}: one ledger row: {written:?} {body}"
                );
                let w = &written[0];
                anyhow::ensure!(
                    w.operation == case.operation()
                        && w.outcome == expected
                        && w.owner_id == f.editor
                        && w.token_id == c.token_id
                        && w.token_name == TOKEN_NAME,
                    "{case:?} over {surface}: expected {expected}: {w:?}"
                );
                if let Some(kind) = case.target_kind() {
                    anyhow::ensure!(w.target_kind == kind, "{case:?}: {w:?}");
                }
                // 回答里说出来的结果与台账一致（成批的一批说在那一对的结果里）
                let said = match case {
                    Case::Batch => batch_item(&body).0,
                    _ => body.get("outcome").cloned().unwrap_or(Value::Null),
                };
                anyhow::ensure!(
                    said.is_null() || said == expected,
                    "{case:?} over {surface}: {body}"
                );
                // 只能提议的那一行没有哪个入口能让它落地（N9）
                if case.proposes() {
                    anyhow::ensure!(
                        snapshot(&f).await? == graph,
                        "{case:?} over {surface} changed the graph"
                    );
                }
            }
        }

        // 写不了的调用方：每一个入口在两个面上都拿到稳定的 code，一行不写（E19）
        for case in CASES {
            // 种撤回的目标时有一次令牌的合并：在量之前
            let t = case.seed(&f, &s, &c.write).await?;
            let seen_here = ledger_ids(&f).await?;
            let audits_here = audits_without_mcp(&f).await?.len();
            for (who, bearer, code) in [
                ("a read token", &c.read, Some("token_read_only")),
                (
                    "a token of another base",
                    &c.elsewhere,
                    Some("token_kb_out_of_scope"),
                ),
                (
                    "a write token of a viewer",
                    &c.viewer,
                    Some("token_owner_below_editor"),
                ),
            ] {
                let (status, body) = call_rest(&f, bearer, case, &t).await?;
                // 类别词的 REST 路由留着它从前那句不带 code 的 403（E16）
                let code = if case == Case::KindWord && who == "a write token of a viewer" {
                    None
                } else {
                    code
                };
                ensure_rest_refused(
                    &format!("{case:?}, {who}"),
                    status,
                    &body,
                    StatusCode::FORBIDDEN,
                    code,
                )?;
                if who == "a token of another base" {
                    // 够不着这个库的令牌连 MCP 端点都进不来
                    let (name, args) = case.tool(&t);
                    let (status, body) = rpc(
                        &f,
                        bearer,
                        "tools/call",
                        json!({ "name": name, "arguments": args }),
                    )
                    .await?;
                    anyhow::ensure!(
                        status == StatusCode::FORBIDDEN,
                        "{case:?}, {who} over MCP: {status} {body}"
                    );
                } else {
                    let result = call_tool(&f, bearer, case, &t).await?;
                    let code = match who {
                        "a read token" => "token_read_only",
                        _ => "token_owner_below_editor",
                    };
                    ensure_tool_refused(&result, code)
                        .map_err(|e| anyhow::anyhow!("{case:?}, {who} over MCP: {e}"))?;
                }
            }
            // 拒绝的调用一行不写：台账、审计（MCP 每次调用那一行不算）都没多
            anyhow::ensure!(
                rows_since(&f, &seen_here).await?.is_empty(),
                "{case:?}: a refused call wrote a ledger row"
            );
            anyhow::ensure!(
                audits_without_mcp(&f).await?.len() == audits_here,
                "{case:?}: a refused call wrote an audit row"
            );
        }

        // `tools/list`：写工具只列给编辑者的写令牌；读的工具谁都看得到
        let write_tools: HashSet<&str> = CASES
            .iter()
            .map(|case| case.tool(&Target::of(f.kb)).0)
            .collect();
        let listed = |tools: Vec<Value>| -> HashSet<String> {
            tools
                .iter()
                .filter_map(|t| t["name"].as_str().map(str::to_string))
                .collect()
        };
        let editor = listed(tools_list(&f, &c.write).await?);
        for name in &write_tools {
            anyhow::ensure!(editor.contains(*name), "{name} is not listed for an editor");
        }
        for (who, bearer) in [("a read token", &c.read), ("a viewer", &c.viewer)] {
            let tools = listed(tools_list(&f, bearer).await?);
            for name in &write_tools {
                anyhow::ensure!(!tools.contains(*name), "{name} is listed for {who}");
            }
            for name in [
                "review_list",
                "review_item",
                "review_ontology_list",
                "review_mapping_list",
            ] {
                anyhow::ensure!(tools.contains(name), "{name} is not listed for {who}");
            }
        }
        Ok(())
    }
    .await;
    finish(f, result).await
}

// ───────────────────────── SC7 / E20：主人 via 令牌名 ─────────────────────────

#[tokio::test]
async fn every_action_reads_owner_via_token_name_in_agent_history_and_activity(
) -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let c = Callers::new(&f).await?;
        let s = Seeds::new(&f, f.kb).await?;
        // 每一行只能提议的操作一条提议
        let mut proposals = Vec::new();
        for case in CASES.into_iter().filter(|c| c.proposes()) {
            let t = case.seed(&f, &s, &c.write).await?;
            proposals.push((case, t.clone(), propose_rest(&f, &c.write, case, &t).await?));
        }
        let find = |case: Case| {
            proposals
                .iter()
                .find(|(c, ..)| *c == case)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("no {case:?} proposal"))
        };
        // 一条批准、一条拒绝、一条过时（人先把违规裁了）
        let (_, _, approved) = find(Case::Defect)?;
        let (_, _, declined) = find(Case::Errata)?;
        let (_, violation, superseded) = find(Case::Violation)?;
        let (status, body) = answer_one(&f, &f.editor_session, approved, "approve").await?;
        anyhow::ensure!(body["state"] == "approved", "{status} {body}");
        let (status, body) = answer_one(&f, &f.editor_session, declined, "decline").await?;
        anyhow::ensure!(body["state"] == "declined", "{status} {body}");
        let (status, body) = post(
            &f,
            &f.editor_session,
            &format!("review/violations/{}", violation.id),
            json!({ "resolution": "accepted" }),
        )
        .await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let (status, body) = answer_one(&f, &f.editor_session, superseded, "approve").await?;
        anyhow::ensure!(body["state"] == "superseded", "{status} {body}");
        // 直接落地的四种：映射、两种运行、类别词
        let mut finalized = Vec::new();
        for case in [
            Case::Mapping,
            Case::Consistency,
            Case::Inference,
            Case::KindWord,
        ] {
            let t = case.seed(&f, &s, &c.write).await?;
            let seen = ledger_ids(&f).await?;
            let (status, body) = call_rest(&f, &c.write, case, &t).await?;
            anyhow::ensure!(status == StatusCode::OK, "{case:?}: {status} {body}");
            let written = rows_since(&f, &seen).await?;
            anyhow::ensure!(written.len() == 1, "{case:?}: {written:?}");
            finalized.push((case, written[0].id));
        }

        // 令牌改名、撤销
        let renamed = sqlx::query("UPDATE personal_tokens SET name = 'renamed' WHERE id = $1")
            .bind(c.token_id)
            .execute(&f.pool)
            .await?
            .rows_affected();
        anyhow::ensure!(renamed == 1, "the token was not renamed");
        utopia_store::tokens::revoke(&f.pool, f.editor, c.token_id).await?;

        // Agent History：每一行都是主人加存下的令牌名
        let (status, history) = f
            .call(
                &f.editor_session,
                "GET",
                &format!("/kbs/{}/review?queue=agent&limit=200", f.kb),
                None,
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {history}");
        let rows = history["items"].as_array().cloned().unwrap_or_default();
        let history_row = |id: Uuid| {
            rows.iter()
                .find(|r| r["source"] == "token" && r["id"] == json!(id))
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("{id} is not in Agent History"))
        };
        let mut every: Vec<(String, Uuid)> = proposals
            .iter()
            .map(|(case, _, id)| (format!("{case:?}"), *id))
            .collect();
        every.extend(
            finalized
                .iter()
                .map(|(case, id)| (format!("{case:?}"), *id)),
        );
        for (what, id) in &every {
            let r = history_row(*id)?;
            anyhow::ensure!(
                r["owner_name"] == "Editor" && r["token_name"] == TOKEN_NAME,
                "{what}: {r}"
            );
        }
        for (id, state) in [
            (approved, "approved"),
            (declined, "declined"),
            (superseded, "superseded"),
        ] {
            let r = history_row(id)?;
            anyhow::ensure!(r["state"] == state, "{state}: {r}");
        }

        // Activity（库 Admin 读）：令牌的每一笔是主人 via 存下的令牌名
        sqlx::query("UPDATE kb_members SET role = 'admin' WHERE kb_id = $1 AND user_id = $2")
            .bind(f.kb)
            .bind(f.editor)
            .execute(&f.pool)
            .await?;
        let (status, activity) = f
            .call(
                &f.editor_session,
                "GET",
                &format!("/kbs/{}/audit?limit=200", f.kb),
                None,
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "Activity: {status} {activity}");
        let events = activity["events"].as_array().cloned().unwrap_or_default();
        for (what, id) in &every {
            let event = events
                .iter()
                .find(|e| e["detail"]["action_id"] == json!(id))
                .ok_or_else(|| anyhow::anyhow!("{what}: no Activity row for {id}"))?;
            anyhow::ensure!(
                event["actor_name"] == "Editor"
                    && event["token_name"] == TOKEN_NAME
                    && event["token_id"] == json!(c.token_id.to_string()),
                "{what}: {event}"
            );
        }
        // 人的回答：拒绝、过时、批准那三行都是这个人的（N2），不带令牌；读的时候从提议的
        // 台账行取出提议它的令牌（存下的名字，改名、撤销之后照旧）与主人，写在
        // `proposed_by_token` / `proposed_by_owner` 上
        let mut answers = Vec::new();
        for (id, action) in [
            (declined, "agent.proposal.declined"),
            (superseded, "agent.proposal.superseded"),
        ] {
            let event = events
                .iter()
                .find(|e| e["action"] == action && e["target_id"] == json!(id))
                .ok_or_else(|| anyhow::anyhow!("no {action} row"))?;
            // 存下的 detail 原样：拒绝与过时那两行本来就记着令牌名
            anyhow::ensure!(
                event["detail"]["proposed_by_token"] == TOKEN_NAME,
                "{action}: {event}"
            );
            answers.push((action, event));
        }
        let event = events
            .iter()
            .find(|e| {
                e["detail"]["agent_proposal"] == json!(approved) && e["action"] == "defect.decided"
            })
            .ok_or_else(|| anyhow::anyhow!("no defect.decided row for the approval"))?;
        answers.push(("defect.decided", event));
        for (action, event) in answers {
            anyhow::ensure!(
                event["actor_id"] == json!(f.editor)
                    && event["actor_name"] == "Editor"
                    && event["proposed_by_token"] == TOKEN_NAME
                    && event["proposed_by_owner"] == "Editor",
                "{action}: {event}"
            );
            for key in ["token_id", "token_name", "declared_client", "batch_id"] {
                anyhow::ensure!(event.get(key).is_none(), "{action}: {key} in {event}");
            }
        }
        Ok(())
    }
    .await;
    finish(f, result).await
}

/// 这个库的 Activity（库 Admin 读），一页读完
async fn activity(f: &Fx) -> anyhow::Result<Vec<Value>> {
    let path = format!("/kbs/{}/audit?limit=200", f.kb);
    let (status, body) = f.call(&f.editor_session, "GET", &path, None).await?;
    anyhow::ensure!(status == StatusCode::OK, "Activity: {status} {body}");
    anyhow::ensure!(
        body["total"].as_i64().unwrap_or(i64::MAX) <= 200,
        "Activity has more than one page: {}",
        body["total"]
    );
    Ok(body["events"].as_array().cloned().unwrap_or_default())
}

/// Activity 里人答一条令牌提议的一行：actor 是这个人，不带令牌的键，提议它的令牌与主人
/// 由读的时候从台账行取出
fn ensure_names_the_proposer(f: &Fx, what: &str, event: &Value) -> anyhow::Result<()> {
    anyhow::ensure!(
        event["actor_id"] == json!(f.editor)
            && event["actor_name"] == "Editor"
            && event["proposed_by_token"] == TOKEN_NAME
            && event["proposed_by_owner"] == "Editor",
        "{what}: {event}"
    );
    for key in ["token_id", "token_name", "declared_client", "batch_id"] {
        anyhow::ensure!(event.get(key).is_none(), "{what}: {key} in {event}");
    }
    Ok(())
}

#[tokio::test]
async fn activity_names_the_proposing_token_on_every_approval_decline_and_supersession(
) -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let c = Callers::new(&f).await?;
        let s = Seeds::new(&f, f.kb).await?;
        // 每一行只能提议的操作各两条提议：一条批准、一条拒绝
        let mut answered: Vec<(String, Uuid, Uuid)> = Vec::new();
        for case in CASES.into_iter().filter(|c| c.proposes()) {
            let t = case.seed(&f, &s, &c.write).await?;
            let approve = propose_rest(&f, &c.write, case, &t).await?;
            let t = case.seed(&f, &s, &c.write).await?;
            let decline = propose_rest(&f, &c.write, case, &t).await?;
            answered.push((format!("{case:?}"), approve, decline));
        }
        anyhow::ensure!(
            answered.len() == 13,
            "{} propose-only entries",
            answered.len()
        );
        // M003 的合并：闸门留下的两对（派生规则一个关系只能有一条：每一对各种一套本体）
        let mut gated = Vec::new();
        for _ in 0..2 {
            let held = held_pair(&f, &Seeds::new(&f, f.kb).await?).await?;
            let (status, body) = post(
                &f,
                &c.write,
                &format!("review/{held}"),
                json!({ "action": "merge" }),
            )
            .await?;
            anyhow::ensure!(
                status == StatusCode::OK && body["outcome"] == "held",
                "a held merge: {status} {body}"
            );
            gated.push(uuid_at(&body, "action_id")?);
        }
        answered.push(("gated merge".into(), gated[0], gated[1]));
        for (what, approve, decline) in &answered {
            let (status, body) = answer_one(&f, &f.editor_session, *approve, "approve").await?;
            anyhow::ensure!(body["state"] == "approved", "{what}: {status} {body}");
            let (status, body) = answer_one(&f, &f.editor_session, *decline, "decline").await?;
            anyhow::ensure!(body["state"] == "declined", "{what}: {status} {body}");
        }
        // 一条过时：人先把缺陷裁了，再批准那条提议
        let defect = Case::Defect.seed(&f, &s, &c.write).await?;
        let stale = propose_rest(&f, &c.write, Case::Defect, &defect).await?;
        let (status, body) = post(
            &f,
            &f.editor_session,
            &format!("review/defects/{}", defect.id),
            json!({ "resolution": "fixed" }),
        )
        .await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        let (status, body) = answer_one(&f, &f.editor_session, stale, "approve").await?;
        anyhow::ensure!(body["state"] == "superseded", "{status} {body}");

        // 这个人自己做的同一种决定（没有提议）：记下这之前的审计行
        sqlx::query("UPDATE kb_members SET role = 'admin' WHERE kb_id = $1 AND user_id = $2")
            .bind(f.kb)
            .bind(f.editor)
            .execute(&f.pool)
            .await?;
        let before_own: HashSet<String> = activity(&f)
            .await?
            .iter()
            .filter_map(|e| e["id"].as_str().map(str::to_string))
            .collect();
        let mut own = 0;
        for case in CASES.into_iter().filter(|c| c.proposes()) {
            let t = case.seed(&f, &s, &c.write).await?;
            let (status, answer) =
                post(&f, &f.editor_session, &case.person(&t), case.rest(&t).1).await?;
            anyhow::ensure!(
                status.is_success(),
                "{case:?} by a person: {status} {answer}"
            );
            own += 1;
        }
        let (pair, ..) = pair_in(&f, f.kb).await?;
        let (status, answer) = post(
            &f,
            &f.editor_session,
            &format!("review/{pair}"),
            json!({ "action": "merge" }),
        )
        .await?;
        anyhow::ensure!(
            status == StatusCode::OK,
            "a person's merge: {status} {answer}"
        );
        anyhow::ensure!(own == 13, "{own} own decisions");

        // 令牌改名、撤销：Activity 写的是提议那一刻存下的名字
        sqlx::query("UPDATE personal_tokens SET name = 'renamed' WHERE id = $1")
            .bind(c.token_id)
            .execute(&f.pool)
            .await?;
        utopia_store::tokens::revoke(&f.pool, f.editor, c.token_id).await?;

        let events = activity(&f).await?;
        for (what, approve, decline) in &answered {
            // 批准：这个人自己的决定行（一笔批准可以写几行，每一行都指着提议）
            let approvals: Vec<&Value> = events
                .iter()
                .filter(|e| e["detail"]["agent_proposal"] == json!(approve))
                .filter(|e| e["action"] != "agent.proposal.declined")
                .collect();
            anyhow::ensure!(!approvals.is_empty(), "{what}: no row for the approval");
            for event in approvals {
                ensure_names_the_proposer(&f, &format!("{what} approved"), event)?;
            }
            let declined = events
                .iter()
                .find(|e| {
                    e["action"] == "agent.proposal.declined" && e["target_id"] == json!(decline)
                })
                .ok_or_else(|| anyhow::anyhow!("{what}: no decline row"))?;
            ensure_names_the_proposer(&f, &format!("{what} declined"), declined)?;
        }
        let superseded = events
            .iter()
            .find(|e| e["action"] == "agent.proposal.superseded" && e["target_id"] == json!(stale))
            .ok_or_else(|| anyhow::anyhow!("no supersession row"))?;
        ensure_names_the_proposer(&f, "superseded", superseded)?;

        // 人自己的决定：不带提议人的键
        let fresh: Vec<&Value> = events
            .iter()
            .filter(|e| e["id"].as_str().is_some_and(|id| !before_own.contains(id)))
            .collect();
        // 人自己拒绝本体提案不留审计行（照旧），别的决定都留：至少有几行
        anyhow::ensure!(!fresh.is_empty(), "no row of own decisions");
        for event in fresh {
            for key in [
                "proposed_by_token",
                "proposed_by_owner",
                "token_id",
                "token_name",
            ] {
                anyhow::ensure!(event.get(key).is_none(), "{key} on a person's own: {event}");
            }
        }
        // 令牌自己的行仍是令牌的：不挂提议人
        for event in events.iter().filter(|e| e.get("token_id").is_some()) {
            for key in ["proposed_by_token", "proposed_by_owner"] {
                anyhow::ensure!(event.get(key).is_none(), "{key} on a token row: {event}");
            }
        }

        // 决策台账（共用同一个结构）：哪一行都没有这两个键
        let (status, history) = f
            .call(
                &f.editor_session,
                "GET",
                &format!("/kbs/{}/review/history?per=100", f.kb),
                None,
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "history: {status} {history}");
        let rows = history["events"].as_array().cloned().unwrap_or_default();
        anyhow::ensure!(!rows.is_empty(), "{history}");
        anyhow::ensure!(
            rows.iter()
                .any(|e| e["detail"].get("agent_proposal").is_some()),
            "no approval in Decisions: {history}"
        );
        for event in &rows {
            for key in ["proposed_by_token", "proposed_by_owner"] {
                anyhow::ensure!(event.get(key).is_none(), "{key} in Decisions: {event}");
            }
        }
        Ok(())
    }
    .await;
    finish(f, result).await
}

// ───────────────────────── SC9 / E22：登录的人的决定照旧 ─────────────────────────

/// 一个会话调用在 `d5d2356` 时的回答：状态与响应体的键
fn before(case: Case) -> Option<(StatusCode, Vec<&'static str>)> {
    let ok = (StatusCode::OK, vec!["ok"]);
    Some(match case {
        Case::Confirm
        | Case::Reject
        | Case::Close
        | Case::Conflict
        | Case::Governor
        | Case::Errata
        | Case::OntologyReject
        | Case::Violation
        | Case::Defect
        | Case::KindWord
        | Case::Mapping => ok,
        Case::Phrase | Case::Rule => (StatusCode::ACCEPTED, vec!["job_id", "ok", "status"]),
        Case::OntologyAdopt => (StatusCode::OK, vec!["id", "skipped"]),
        Case::Pending => (
            StatusCode::OK,
            vec!["conflicts", "created", "fact_id", "ok"],
        ),
        Case::Consistency => (
            StatusCode::OK,
            vec![
                "classes",
                "cleared",
                "cycles_capped",
                "defects_found",
                "defects_new",
                "edges",
                "found",
                "inserted",
                "predicates_with_axioms",
            ],
        ),
        Case::Inference => (
            StatusCode::OK,
            vec![
                "capped",
                "derived",
                "edges",
                "inserted",
                "invalidated",
                "rules",
            ],
        ),
        // M003 的四种由它们自己的会话测试钉住
        Case::Keep | Case::Merge | Case::ManualMerge | Case::Revert | Case::Batch => return None,
    })
}

#[tokio::test]
async fn a_signed_in_persons_decisions_answer_as_before_in_every_opened_section(
) -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let c = Callers::new(&f).await?;
        let s = Seeds::new(&f, f.kb).await?;
        let seen = ledger_ids(&f).await?;
        let audits_before = audits(&f).await?.len();
        let mut opened = 0;
        for case in CASES {
            let Some((expected, expected_keys)) = before(case) else {
                continue;
            };
            let t = case.seed(&f, &s, &c.write).await?;
            let (_, body) = case.rest(&t);
            let tail = case.person(&t);
            let (status, answer) = post(&f, &f.editor_session, &tail, body).await?;
            anyhow::ensure!(
                status == expected && keys(&answer) == expected_keys,
                "{case:?}: {status} {answer}"
            );
            opened += 1;
        }
        anyhow::ensure!(opened == 17, "{opened} opened routes");
        // 会话一行台账都不写；它写的审计行没有一行带令牌
        anyhow::ensure!(
            rows_since(&f, &seen).await?.is_empty(),
            "a session decision wrote a ledger row"
        );
        let written: Vec<_> = audits(&f).await?.into_iter().skip(audits_before).collect();
        anyhow::ensure!(!written.is_empty(), "no session decision was audited");
        for a in &written {
            anyhow::ensure!(
                a.actor == Some(f.editor)
                    && a.detail.get("token_id").is_none()
                    && a.detail.get("agent_proposal").is_none(),
                "{a:?}"
            );
        }
        Ok(())
    }
    .await;
    finish(f, result).await
}

// ───────────────────────── N3：没有令牌、没有工具答得了提议 ─────────────────────────

#[tokio::test]
async fn no_token_and_no_tool_answers_a_proposal() -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let c = Callers::new(&f).await?;
        let s = Seeds::new(&f, f.kb).await?;
        let (_, other_writer) = f.token(f.editor, "another agent", "write", None).await?;
        let defect = Case::Defect.seed(&f, &s, &c.write).await?;
        let proposal = propose_rest(&f, &c.write, Case::Defect, &defect).await?;
        let row_before = row(&f, "ontology_defects", defect.id).await?;
        // 提它的令牌、别的写令牌、只读的、Viewer 的、别的库的：两条回答路由都是 401
        for (who, bearer) in [
            ("the proposing token", &c.write),
            ("another write token", &other_writer),
            ("a read token", &c.read),
            ("a viewer's write token", &c.viewer),
            ("a token of another base", &c.elsewhere),
        ] {
            for action in ["approve", "decline"] {
                let (status, body) = answer_one(&f, bearer, proposal, action).await?;
                anyhow::ensure!(
                    status == StatusCode::UNAUTHORIZED,
                    "{who}, {action}: {status} {body}"
                );
                let (status, body) = answer_many(&f, bearer, &[proposal], action).await?;
                anyhow::ensure!(
                    status == StatusCode::UNAUTHORIZED,
                    "{who}, bulk {action}: {status} {body}"
                );
            }
        }
        // 列出来的工具没有一个名字是答提议的；那几个名字是不认识的工具
        let tools = tools_list(&f, &c.write).await?;
        for tool in &tools {
            let name = tool["name"].as_str().unwrap_or_default();
            for word in ["proposal", "approve", "decline", "answer"] {
                anyhow::ensure!(
                    !name.contains(word),
                    "a listed tool answers proposals: {name}"
                );
            }
        }
        for name in [
            "review_confirm",
            "review_decide",
            "review_pending",
            "review_approve",
        ] {
            let (status, body) = rpc(
                &f,
                &c.write,
                "tools/call",
                json!({ "name": name, "arguments": { "id": proposal, "action": "approve" } }),
            )
            .await?;
            anyhow::ensure!(
                status == StatusCode::OK && body["error"]["code"] == -32601,
                "{name}: {status} {body}"
            );
        }
        // 提议还开着，缺陷一个字节没动
        anyhow::ensure!(
            open_proposals(&f).await?.iter().any(|p| p.id == proposal),
            "the proposal was answered"
        );
        anyhow::ensure!(row(&f, "ontology_defects", defect.id).await? == row_before);
        Ok(())
    }
    .await;
    finish(f, result).await
}

// ───────────────────────── `review_batch` 的说明 ─────────────────────────

#[tokio::test]
async fn the_review_batch_description_says_what_an_unknown_action_gets() -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let c = Callers::new(&f).await?;
        let tools = tools_list(&f, &c.write).await?;
        let batch = tools
            .iter()
            .find(|t| t["name"] == "review_batch")
            .ok_or_else(|| anyhow::anyhow!("review_batch is not listed"))?;
        let description = batch["description"].as_str().unwrap_or_default();
        // 说的就是它做的：认不出的动作整批被拒，不带 code
        anyhow::ensure!(
            description
                .contains("an action other than merge or keep is refused as a whole with no code"),
            "{description}"
        );
        anyhow::ensure!(
            !description.contains("and so is an action other than merge or keep"),
            "{description}"
        );
        for said in ["batch_id", "batch_size", "review_revert", "50"] {
            anyhow::ensure!(description.contains(said), "{said}: {description}");
        }
        // 行为没变：REST 是不带 code 的 422，工具错误里同样没有 code
        let (first, ..) = pair_in(&f, f.kb).await?;
        let (second, ..) = pair_in(&f, f.kb).await?;
        let seen = ledger_ids(&f).await?;
        for action in ["reject", "revert"] {
            let (status, body) = post(
                &f,
                &c.write,
                "review/batch",
                json!({ "ids": [first, second], "action": action }),
            )
            .await?;
            ensure_rest_refused(
                action,
                status,
                &body,
                StatusCode::UNPROCESSABLE_ENTITY,
                None,
            )?;
            let result = tool_call(
                &f,
                &c.write,
                "review_batch",
                json!({ "ids": [first, second], "action": action }),
            )
            .await?;
            anyhow::ensure!(
                result["isError"] == true
                    && result["structuredContent"].get("code").is_none()
                    && result["structuredContent"]["message"] == body["error"],
                "{action}: {result} {body}"
            );
        }
        anyhow::ensure!(
            rows_since(&f, &seen).await?.is_empty(),
            "a refused batch wrote a row"
        );
        Ok(())
    }
    .await;
    finish(f, result).await
}
