//! Review 里一个目标此刻还能不能裁（0076）：从**带类型的行**读，不认任何一句错误的原话。
//!
//! 令牌 agent 在 Review 其余各档只能提议。一条提议要问两次「这个目标还在等人裁吗」：
//!
//! - 提的时候：不在了（[`TargetState::Missing`]）答 not found，已经定了
//!   （[`TargetState::Decided`]）答 `already_decided`，还开着才留提议
//! - 人批准的时候、人的那段代码跑之前：不在了或定了，提议关成 `superseded`，别的不动
//!
//! M003 的三种合并靠认人的路径报的那几句话（`token_agent_actions::target_gone`）；那条路
//! 原样留着，只管那三种。这里的每一种操作都只读它自己那张表的状态列：
//!
//! - **每一次读都带着库的 id**：别的库的 id 读出来就是 `Missing`，与认不出的 id 一样
//! - **只读**：这个模块里没有一条 INSERT / UPDATE / DELETE，也不调任何会改东西的函数
//! - 有些目标没有「定了」这一态（确认一条活着的事实、短语、规则、等点头的事实）：
//!   行在就是开着，行不在就是 `Missing`
//!
//! 操作名是台账里的那一套（`fact.confirm`、`conflict.resolve`……），见 ADR 0076。
//! `request` 是那次调用的请求体原样；只有违规（`resolution`、`fact_id`）与映射
//! （`status`）的判定要读它。

use serde_json::{json, Value};
use sqlx::PgPool;
use utopia_core::{AppError, AppResult};
use uuid::Uuid;

/// 一个目标此刻的样子，对「再裁一次」这件事而言
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetState {
    /// 还等着裁：可以留提议，批准时可以照做
    Open,
    /// 行在，但已经定了（或者内容已经不是提议时的那一份）
    Decided,
    /// 这个库里没有这个目标，或者它对这种操作而言已经不成立
    Missing,
}

/// 一条事实：是不是作废了、有没有终点。这个库里没有就是 `None`
async fn fact(pool: &PgPool, kb_id: Uuid, fact_id: Uuid) -> AppResult<Option<(bool, bool)>> {
    Ok(sqlx::query_as(
        "SELECT invalidated_at IS NOT NULL, valid_to IS NOT NULL
           FROM facts WHERE id = $1 AND kb_id = $2",
    )
    .bind(fact_id)
    .bind(kb_id)
    .fetch_optional(pool)
    .await?)
}

/// 「活着、区间还开着」：没作废、没有终点。与人闭合一条事实时的那句校验同一个判据
async fn fact_live_and_open(pool: &PgPool, kb_id: Uuid, fact_id: Uuid) -> AppResult<bool> {
    Ok(matches!(
        fact(pool, kb_id, fact_id).await?,
        Some((false, false))
    ))
}

/// 一张表里一行的状态列。表名与列名都是这个模块里写死的字面量，不来自调用方
async fn status_of(
    pool: &PgPool,
    table: &'static str,
    kb_id: Uuid,
    id: Uuid,
) -> AppResult<Option<String>> {
    Ok(sqlx::query_scalar(&format!(
        "SELECT status FROM {table} WHERE id = $1 AND kb_id = $2"
    ))
    .bind(id)
    .bind(kb_id)
    .fetch_optional(pool)
    .await?)
}

/// 行在不在
async fn exists(pool: &PgPool, table: &'static str, kb_id: Uuid, id: Uuid) -> AppResult<bool> {
    Ok(sqlx::query_scalar(&format!(
        "SELECT EXISTS (SELECT 1 FROM {table} WHERE id = $1 AND kb_id = $2)"
    ))
    .bind(id)
    .bind(kb_id)
    .fetch_one(pool)
    .await?)
}

/// 状态列等于 `open` 就是开着，行在而不等就是定了，行不在就是没有
fn by_status(status: Option<String>, open: &str) -> TargetState {
    match status {
        None => TargetState::Missing,
        Some(s) if s == open => TargetState::Open,
        Some(_) => TargetState::Decided,
    }
}

/// 这个目标对这种操作而言此刻的样子。
///
/// `fingerprint` 是提议时存下的那一份（[`fingerprint`] 的返回值）；提的时候传 `None`。
/// 给了的话，矛盾的两条事实 id、本体提案存下的 payload 与它不一样，就是 `Decided`——
/// id 没变，内容已经不是 agent 当时看到的那一份。
///
/// 认不出的操作名是调用方的错，报错而不是猜一个状态
pub async fn target_state(
    pool: &PgPool,
    kb_id: Uuid,
    operation: &str,
    target_id: Uuid,
    request: &Value,
    fingerprint: Option<&Value>,
) -> AppResult<TargetState> {
    Ok(match operation {
        // 确认：没有「定了」这一态。活着的事实可以再确认一次；没了或作废了就是没有
        "fact.confirm" => match fact(pool, kb_id, target_id).await? {
            Some((false, _)) => TargetState::Open,
            _ => TargetState::Missing,
        },
        "fact.reject" => match fact(pool, kb_id, target_id).await? {
            None => TargetState::Missing,
            Some((true, _)) => TargetState::Decided,
            Some((false, _)) => TargetState::Open,
        },
        // 闭合：作废了是定了；活着但已经有终点，人的路由答 not found，这里也是没有
        "fact.close" => match fact(pool, kb_id, target_id).await? {
            None => TargetState::Missing,
            Some((true, _)) => TargetState::Decided,
            Some((false, true)) => TargetState::Missing,
            Some((false, false)) => TargetState::Open,
        },
        "conflict.resolve" => {
            let row: Option<(String, Uuid, Uuid, bool, bool)> = sqlx::query_as(
                "SELECT c.status, c.old_fact_id, c.new_fact_id,
                        o.invalidated_at IS NULL, n.invalidated_at IS NULL
                   FROM fact_conflicts c
                   JOIN facts o ON o.id = c.old_fact_id
                   JOIN facts n ON n.id = c.new_fact_id
                  WHERE c.id = $1 AND c.kb_id = $2",
            )
            .bind(target_id)
            .bind(kb_id)
            .fetch_optional(pool)
            .await?;
            match row {
                None => TargetState::Missing,
                Some((status, old, new, old_live, new_live)) => {
                    let drifted = fingerprint.is_some_and(|f| *f != conflict_fingerprint(old, new));
                    if status == "open" && old_live && new_live && !drifted {
                        TargetState::Open
                    } else {
                        TargetState::Decided
                    }
                }
            }
        }
        "violation.decide" => {
            let row: Option<(String, Uuid, Uuid, Vec<Uuid>)> = sqlx::query_as(
                "SELECT status, left_fact, right_fact, path FROM axiom_violations
                  WHERE id = $1 AND kb_id = $2",
            )
            .bind(target_id)
            .bind(kb_id)
            .fetch_optional(pool)
            .await?;
            match row {
                None => TargetState::Missing,
                Some((status, _, _, _)) if status != "open" => TargetState::Decided,
                Some((_, left, right, path)) => {
                    // 请求指着的那条事实：撤（`fact_retracted`）是请求点名的那条，单事实的
                    // 违规可以不点名；闭合（`fact_closed`）闭的是违规的左事实，与人的路由
                    // 同一个选法。点不出名字的请求不在这里拦——那是请求体的校验
                    let named = match request.get("resolution").and_then(Value::as_str) {
                        Some("fact_retracted") => {
                            let asked = request
                                .get("fact_id")
                                .and_then(Value::as_str)
                                .and_then(|s| Uuid::parse_str(s).ok());
                            crate::reasoning::pick_retraction(left, right, &path, asked)
                        }
                        Some("fact_closed") => Some(left),
                        _ => None,
                    };
                    match named {
                        Some(fact_id) if !fact_live_and_open(pool, kb_id, fact_id).await? => {
                            TargetState::Decided
                        }
                        _ => TargetState::Open,
                    }
                }
            }
        }
        "defect.decide" => by_status(
            status_of(pool, "ontology_defects", kb_id, target_id).await?,
            "open",
        ),
        "errata.decide" => by_status(
            status_of(pool, "errata_actions", kb_id, target_id).await?,
            "held",
        ),
        "ontology.proposal" => {
            let row: Option<(String, String, Value)> = sqlx::query_as(
                "SELECT status, proposed_by, payload FROM ontology_proposals
                  WHERE id = $1 AND kb_id = $2",
            )
            .bind(target_id)
            .bind(kb_id)
            .fetch_optional(pool)
            .await?;
            match row {
                None => TargetState::Missing,
                Some((status, _, _)) if status != "open" => TargetState::Decided,
                // 开着、但不是本体代理提的（一次性建议留下的）：采纳的那条路只认代理提的，
                // 对它答 not found，这里也是没有
                Some((_, by, _)) if by != "agent" => TargetState::Missing,
                Some((_, _, payload)) if fingerprint.is_some_and(|f| *f != payload) => {
                    TargetState::Decided
                }
                Some(_) => TargetState::Open,
            }
        }
        // 没有「定了」这一态的三种：行在就照做（E8），行不在就是没有
        "alignment.phrase" => presence(exists(pool, "phrase_bindings", kb_id, target_id).await?),
        "alignment.rule" => presence(exists(pool, "implication_rules", kb_id, target_id).await?),
        "pending.nod" => presence(exists(pool, "pending_facts", kb_id, target_id).await?),
        "governor.answer" => {
            // 建议还开着（proposed）要它指着的那一对也还在等；自动落地的（applied）可以撤、
            // 可以推翻。别的状态都是有人答过了
            let row: Option<(String, Option<String>)> = sqlx::query_as(
                "SELECT d.status, rr.status
                   FROM agent_decisions d
                   LEFT JOIN resolution_reviews rr
                          ON rr.id = d.target_id AND rr.kb_id = d.kb_id
                  WHERE d.id = $1 AND d.kb_id = $2",
            )
            .bind(target_id)
            .bind(kb_id)
            .fetch_optional(pool)
            .await?;
            match row {
                None => TargetState::Missing,
                Some((status, review)) => match (status.as_str(), review.as_deref()) {
                    ("proposed", Some("pending")) | ("applied", _) => TargetState::Open,
                    _ => TargetState::Decided,
                },
            }
        }
        // 映射直接落地：已经是请求要的那个状态就是定了（E13），别的都还能定
        "mapping.decide" => match status_of(pool, "concept_mappings", kb_id, target_id).await? {
            None => TargetState::Missing,
            Some(status) => {
                if request.get("status").and_then(Value::as_str) == Some(status.as_str()) {
                    TargetState::Decided
                } else {
                    TargetState::Open
                }
            }
        },
        // 目标就是库本身的三种（类别词、一致性检查、推理）：没有提议、没有「定了」
        "alignment.kind_word" | "consistency.check" | "inference.run" => {
            let known: bool =
                sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM knowledge_bases WHERE id = $1)")
                    .bind(kb_id)
                    .fetch_one(pool)
                    .await?;
            presence(known && target_id == kb_id)
        }
        other => {
            return Err(AppError::Other(anyhow::anyhow!(
                "no target state is defined for the operation {other}"
            )))
        }
    })
}

fn presence(there: bool) -> TargetState {
    if there {
        TargetState::Open
    } else {
        TargetState::Missing
    }
}

fn conflict_fingerprint(old_fact: Uuid, new_fact: Uuid) -> Value {
    json!({ "old_fact_id": old_fact, "new_fact_id": new_fact })
}

/// 提议时要存下、批准时要比对的那一份内容：矛盾是它的两条事实 id，本体提案是存下的
/// payload。别的操作没有（`None`），目标不在这个库里也是 `None`
pub async fn fingerprint(
    pool: &PgPool,
    kb_id: Uuid,
    operation: &str,
    target_id: Uuid,
) -> AppResult<Option<Value>> {
    Ok(match operation {
        "conflict.resolve" => {
            let row: Option<(Uuid, Uuid)> = sqlx::query_as(
                "SELECT old_fact_id, new_fact_id FROM fact_conflicts WHERE id = $1 AND kb_id = $2",
            )
            .bind(target_id)
            .bind(kb_id)
            .fetch_optional(pool)
            .await?;
            row.map(|(old, new)| conflict_fingerprint(old, new))
        }
        "ontology.proposal" => {
            sqlx::query_scalar(
                "SELECT payload FROM ontology_proposals WHERE id = $1 AND kb_id = $2",
            )
            .bind(target_id)
            .bind(kb_id)
            .fetch_optional(pool)
            .await?
        }
        _ => None,
    })
}

/// 本体提案在人的路由里按 section 与 key 认；台账要一个 uuid，就是 `ontology_proposals`
/// 那一行的 id。哪个状态的行都找得到——定没定过由 [`target_state`] 说
pub async fn ontology_proposal_id(
    pool: &PgPool,
    kb_id: Uuid,
    section: &str,
    key: &str,
) -> AppResult<Option<Uuid>> {
    Ok(sqlx::query_scalar(
        "SELECT id FROM ontology_proposals WHERE kb_id = $1 AND section = $2 AND key = $3",
    )
    .bind(kb_id)
    .bind(section)
    .bind(key)
    .fetch_optional(pool)
    .await?)
}

/// 一条事实读成一句话：主语、谓词、宾语。与审计快照同一个显示口径（宾语是字面值时
/// 优先取 summary）
async fn fact_words(pool: &PgPool, kb_id: Uuid, fact_id: Uuid) -> AppResult<Option<String>> {
    let row: Option<(String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT s.canonical_name, COALESCE(r.label, fact_surface_predicate(f.id)),
                COALESCE(o.canonical_name, f.object_value ->> 'summary', f.object_value #>> '{}')
           FROM facts f
           JOIN entities s ON s.id = f.subject_id
           LEFT JOIN relation_types r ON r.id = f.predicate_id
           LEFT JOIN entities o ON o.id = f.object_id
          WHERE f.id = $1 AND f.kb_id = $2",
    )
    .bind(fact_id)
    .bind(kb_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(s, p, o)| sentence(&[Some(s), p, o])))
}

/// 有值的几段用空格连起来
fn sentence(parts: &[Option<String>]) -> String {
    parts
        .iter()
        .flatten()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// 用话说出一个目标，提议那一刻存进 payload 的 `target_label`：Agent History 的行与卡片
/// 上的标记读它，目标后来改了、没了，这句话还是当时的样子。目标不在这个库里：`NotFound`。
///
/// 类别词的目标是库本身，词在路由的路径里而不在请求体里：调用方把它放在 `request` 的
/// `kind_word` 键下，这里原样取；没给就只说「kind word」
pub async fn target_label(
    pool: &PgPool,
    kb_id: Uuid,
    operation: &str,
    target_id: Uuid,
    request: &Value,
) -> AppResult<String> {
    let label = match operation {
        "fact.confirm" | "fact.reject" | "fact.close" => fact_words(pool, kb_id, target_id).await?,
        "conflict.resolve" => {
            let row: Option<(Uuid, Uuid)> = sqlx::query_as(
                "SELECT old_fact_id, new_fact_id FROM fact_conflicts WHERE id = $1 AND kb_id = $2",
            )
            .bind(target_id)
            .bind(kb_id)
            .fetch_optional(pool)
            .await?;
            match row {
                None => None,
                Some((old, new)) => {
                    let old = fact_words(pool, kb_id, old).await?.unwrap_or_default();
                    let new = fact_words(pool, kb_id, new).await?.unwrap_or_default();
                    Some(format!("{old} / {new}"))
                }
            }
        }
        "violation.decide" => {
            let row: Option<(String, Uuid, Uuid)> = sqlx::query_as(
                "SELECT kind, left_fact, right_fact FROM axiom_violations
                  WHERE id = $1 AND kb_id = $2",
            )
            .bind(target_id)
            .bind(kb_id)
            .fetch_optional(pool)
            .await?;
            match row {
                None => None,
                Some((kind, left, right)) => {
                    let mut said = format!(
                        "{kind}: {}",
                        fact_words(pool, kb_id, left).await?.unwrap_or_default()
                    );
                    if right != left {
                        if let Some(right) = fact_words(pool, kb_id, right).await? {
                            said.push_str(" / ");
                            said.push_str(&right);
                        }
                    }
                    Some(said)
                }
            }
        }
        "defect.decide" => {
            // 缺陷指着的是本体里的元素：关系类型或实体类型，哪张表里有就取哪个的名字
            let row: Option<(String, Option<String>, Option<String>)> = sqlx::query_as(
                "SELECT d.kind,
                        COALESCE((SELECT label FROM relation_types
                                   WHERE id = d.subject AND kb_id = d.kb_id),
                                 (SELECT label FROM entity_types
                                   WHERE id = d.subject AND kb_id = d.kb_id)),
                        COALESCE((SELECT label FROM relation_types
                                   WHERE id = d.other AND kb_id = d.kb_id),
                                 (SELECT label FROM entity_types
                                   WHERE id = d.other AND kb_id = d.kb_id))
                   FROM ontology_defects d WHERE d.id = $1 AND d.kb_id = $2",
            )
            .bind(target_id)
            .bind(kb_id)
            .fetch_optional(pool)
            .await?;
            row.map(|(kind, subject, other)| {
                let named: Vec<String> = [subject, other].into_iter().flatten().collect();
                if named.is_empty() {
                    kind
                } else {
                    format!("{kind}: {}", named.join(" / "))
                }
            })
        }
        "errata.decide" => {
            let row: Option<(String, Option<Uuid>, Option<String>, String)> = sqlx::query_as(
                "SELECT action, fact_id, quote, reason FROM errata_actions
                  WHERE id = $1 AND kb_id = $2",
            )
            .bind(target_id)
            .bind(kb_id)
            .fetch_optional(pool)
            .await?;
            match row {
                None => None,
                Some((action, fact_id, quote, reason)) => {
                    let about = match fact_id {
                        Some(fact_id) => fact_words(pool, kb_id, fact_id).await?,
                        None => None,
                    };
                    let about = about
                        .or(quote)
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or(reason);
                    Some(sentence(&[Some(format!("{action}:")), Some(about)]))
                }
            }
        }
        "alignment.phrase" => {
            let row: Option<(String, Option<String>, Option<String>, bool)> = sqlx::query_as(
                "SELECT b.phrase, st.label, ot.label, b.object_is_value
                   FROM phrase_bindings b
                   LEFT JOIN entity_types st ON st.id = b.subject_type_id
                   LEFT JOIN entity_types ot ON ot.id = b.object_type_id
                  WHERE b.id = $1 AND b.kb_id = $2",
            )
            .bind(target_id)
            .bind(kb_id)
            .fetch_optional(pool)
            .await?;
            row.map(|(phrase, subject, object, is_value)| {
                let object = if is_value {
                    Some("value".to_string())
                } else {
                    object
                };
                match (subject, object) {
                    (None, None) => phrase,
                    (s, o) => format!(
                        "{phrase} ({} → {})",
                        s.as_deref().unwrap_or("any"),
                        o.as_deref().unwrap_or("any")
                    ),
                }
            })
        }
        "alignment.rule" => {
            let row: Option<(String, Option<String>)> = sqlx::query_as(
                "SELECT r.phrase, p.label
                   FROM implication_rules r
                   LEFT JOIN relation_types p ON p.id = r.conclude_property_id
                  WHERE r.id = $1 AND r.kb_id = $2",
            )
            .bind(target_id)
            .bind(kb_id)
            .fetch_optional(pool)
            .await?;
            row.map(|(phrase, concludes)| match concludes {
                Some(concludes) => format!("{phrase} ⇒ {concludes}"),
                None => phrase,
            })
        }
        "pending.nod" => {
            // 那句话，加上是谁记下的（N5：agent 替 agent 记下的话点头，人要看得出来）
            #[allow(clippy::type_complexity)]
            let row: Option<(String, Option<String>, Option<String>, Option<String>)> =
                sqlx::query_as(
                    "SELECT s.canonical_name, COALESCE(r.label, p.proposed_predicate, p.phrase),
                        COALESCE(o.canonical_name, p.object_value ->> 'summary',
                                 p.object_value #>> '{}'),
                        u.display_name
                   FROM pending_facts p
                   JOIN entities s ON s.id = p.subject_id
                   LEFT JOIN relation_types r ON r.id = p.predicate_id
                   LEFT JOIN entities o ON o.id = p.object_id
                   LEFT JOIN users u ON u.id = p.proposed_by
                  WHERE p.id = $1 AND p.kb_id = $2",
                )
                .bind(target_id)
                .bind(kb_id)
                .fetch_optional(pool)
                .await?;
            row.map(|(s, p, o, by)| {
                let said = sentence(&[Some(s), p, o]);
                match by {
                    Some(by) => format!("{said} (remembered by {by})"),
                    None => said,
                }
            })
        }
        "governor.answer" => {
            let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
                "SELECT a.canonical_name, b.canonical_name
                   FROM agent_decisions d
                   LEFT JOIN resolution_reviews rr ON rr.id = d.target_id AND rr.kb_id = d.kb_id
                   LEFT JOIN entities a ON a.id = rr.left_id
                   LEFT JOIN entities b ON b.id = rr.right_id
                  WHERE d.id = $1 AND d.kb_id = $2",
            )
            .bind(target_id)
            .bind(kb_id)
            .fetch_optional(pool)
            .await?;
            row.map(|(left, right)| {
                let pair: Vec<String> = [left, right].into_iter().flatten().collect();
                if pair.is_empty() {
                    "a governor decision".to_string()
                } else {
                    pair.join(" / ")
                }
            })
        }
        "mapping.decide" => {
            let row: Option<(String, String, Option<String>)> = sqlx::query_as(
                "SELECT c.canonical_name, m.source, m.table_name
                   FROM concept_mappings m
                   JOIN entities c ON c.id = m.concept_id
                  WHERE m.id = $1 AND m.kb_id = $2",
            )
            .bind(target_id)
            .bind(kb_id)
            .fetch_optional(pool)
            .await?;
            row.map(|(concept, source, table)| match table {
                Some(table) if !table.trim().is_empty() => {
                    format!("{concept} → {source}.{table}")
                }
                _ => format!("{concept} → {source}"),
            })
        }
        "ontology.proposal" => {
            let row: Option<(String, String)> = sqlx::query_as(
                "SELECT section, key FROM ontology_proposals WHERE id = $1 AND kb_id = $2",
            )
            .bind(target_id)
            .bind(kb_id)
            .fetch_optional(pool)
            .await?;
            row.map(|(section, key)| format!("{section}: {key}"))
        }
        "alignment.kind_word" if target_id == kb_id => Some(
            request
                .get("kind_word")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|w| !w.is_empty())
                .unwrap_or("kind word")
                .to_string(),
        ),
        "consistency.check" | "inference.run" if target_id == kb_id => {
            sqlx::query_scalar("SELECT name FROM knowledge_bases WHERE id = $1")
                .bind(kb_id)
                .fetch_optional(pool)
                .await?
        }
        "alignment.kind_word" | "consistency.check" | "inference.run" => None,
        other => {
            return Err(AppError::Other(anyhow::anyhow!(
                "no target label is defined for the operation {other}"
            )))
        }
    };
    label.ok_or(AppError::NotFound)
}
