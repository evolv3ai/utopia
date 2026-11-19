//! 对齐队列（#725 的「Alignment」一档）：对齐器两票不一致、拿不定的签名与类别词，等人来定。
//!
//! 两种条目走同一档：一条短语签名（短语 × 两端的类，几条例句，两票各选了什么）要人定
//! 属性和方向或「没有」；一个类别词（词、写法、例名、两票）要人定类或「没有」。人定了写回
//! `phrase_bindings` / `type_bindings`，`decided_by = 'person'`，代理此后不再改它；短语定了
//! 类型化图谱按新绑定重算，类别词定了它名下的实体换类、短语签名跟着变。
//!
//! 卡片由账本拼：这里只读两张绑定表和类别词签名，不调模型。

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use utopia_core::AppResult;
use uuid::Uuid;

/// 队列里的一条：短语签名或类别词。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AlignmentItem {
    Phrase {
        id: Uuid,
        phrase: String,
        subject_class: Option<String>,
        object_class: Option<String>,
        object_is_value: bool,
        statement_count: i32,
        /// 判定时记下的例句（「主语 —短语→ 宾语」）
        examples: Vec<String>,
        /// 两票各选了什么：`{"first": {...} | null, "second": {...} | null}`
        votes: Option<serde_json::Value>,
        decided_at: DateTime<Utc>,
        /// 绑上了、却没说一刻标哪一端的签名（#966）：人绑的，或对齐器问过、两票没说出同一个
        /// 值的。现在绑到的属性与方向，界面按它预选、只等人补上 marks。undecided 的条目两个
        /// 都是 None
        bound_to: Option<String>,
        direction: Option<String>,
    },
    KindWord {
        kind_word: String,
        words: Vec<String>,
        examples: Vec<String>,
        phrases: Vec<String>,
        entity_count: i64,
        votes: Option<serde_json::Value>,
        decided_at: DateTime<Utc>,
    },
    /// 对齐器提的一条蕴含规则（0044 决定 3 第五片）：这种形状蕴含哪条属性、宾语怎么读
    Rule {
        id: Uuid,
        trigger: String,
        phrase: String,
        subject_class: Option<String>,
        object_class: Option<String>,
        object_is_value: bool,
        property: String,
        property_label: String,
        reading: Option<String>,
        statement_count: i32,
        examples: Vec<String>,
        votes: Option<serde_json::Value>,
        decided_at: DateTime<Utc>,
    },
}

#[derive(sqlx::FromRow)]
struct RuleRow {
    id: Uuid,
    trigger: String,
    phrase: String,
    subject_class: Option<String>,
    object_class: Option<String>,
    object_is_value: bool,
    property: String,
    property_label: String,
    reading: Option<String>,
    statement_count: i32,
    examples: Vec<String>,
    votes: Option<serde_json::Value>,
    decided_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct PhraseRow {
    id: Uuid,
    phrase: String,
    subject_class: Option<String>,
    object_class: Option<String>,
    object_is_value: bool,
    statement_count: i32,
    examples: Vec<String>,
    votes: Option<serde_json::Value>,
    decided_at: DateTime<Utc>,
    bound_to: Option<String>,
    direction: Option<String>,
}

/// 等人定的短语签名：两票不一致的，加上绑到状态属性、却没说一刻标哪一端的（#966）——人绑的
/// （代理不改人的判定），和对齐器问过、两票没说出同一个值的（只问一次，0053 修订）。不列
/// 出来，它们名下只说了一刻的陈述就一直不算。这一列之前代理绑上、还没问过的不列：对齐器
/// 下一轮先问
pub(crate) const WAITING_PHRASE: &str = "(b.status = 'undecided'
       OR (b.status = 'bound' AND b.marks IS NULL
           AND (b.decided_by = 'person' OR b.marks_asked_at IS NOT NULL)
           AND EXISTS (SELECT 1 FROM relation_types rt
                        WHERE rt.id = b.relation_type_id AND rt.temporal = 'state')))";

#[derive(sqlx::FromRow)]
struct KindWordRow {
    kind_word: String,
    words: Vec<String>,
    votes: Option<serde_json::Value>,
    decided_at: DateTime<Utc>,
}

/// 等人定的条目，先来的在前，两种混排。
pub async fn list(
    pool: &PgPool,
    kb_id: Uuid,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<AlignmentItem>> {
    let phrases: Vec<PhraseRow> = sqlx::query_as(&format!(
        "SELECT b.id, b.phrase, st.key AS subject_class, ot.key AS object_class,
                b.object_is_value, b.statement_count, b.examples, b.votes, b.decided_at,
                CASE WHEN b.status = 'bound' THEN p.key END AS bound_to,
                CASE WHEN b.status = 'bound' THEN b.direction END AS direction
           FROM phrase_bindings b
      LEFT JOIN entity_types st ON st.id = b.subject_type_id
      LEFT JOIN entity_types ot ON ot.id = b.object_type_id
      LEFT JOIN relation_types p ON p.id = b.relation_type_id
          WHERE b.kb_id = $1 AND {WAITING_PHRASE}"
    ))
    .bind(kb_id)
    .fetch_all(pool)
    .await?;
    let rules: Vec<RuleRow> = sqlx::query_as(
        "SELECT r.id, r.trigger, r.phrase, st.key AS subject_class, ot.key AS object_class,
                r.object_is_value, p.key AS property, p.label AS property_label, r.reading,
                r.statement_count, r.examples, r.votes, r.decided_at
           FROM implication_rules r
           JOIN relation_types p ON p.id = r.conclude_property_id
      LEFT JOIN entity_types st ON st.id = r.subject_type_id
      LEFT JOIN entity_types ot ON ot.id = r.object_type_id
          WHERE r.kb_id = $1 AND r.status = 'proposed'",
    )
    .bind(kb_id)
    .fetch_all(pool)
    .await?;
    let words: Vec<KindWordRow> = sqlx::query_as(
        "SELECT kind_word, words, votes, decided_at
           FROM type_bindings WHERE kb_id = $1 AND status = 'undecided'",
    )
    .bind(kb_id)
    .fetch_all(pool)
    .await?;
    // 类别词的例名和短语来自签名（活着的实体数出来的），按词对上
    let sigs = if words.is_empty() {
        Vec::new()
    } else {
        crate::type_bindings::signatures(pool, kb_id).await?
    };
    // 排序的钥匙：先 `decided_at`，再条目自己的标识（0076）——短语与规则是 uuid `id`，
    // 类别词是 `kind_word`。只按时间排的话，一轮对齐写下的行时间相同，翻页时每一页的
    // 先后可以不同：有的条目出现两次，有的一次也不出现。末尾的种类序号只在一个类别词
    // 恰好写成某个 uuid 的样子时才起作用，保证这是一个全序
    type OrderKey = (DateTime<Utc>, String, u8);
    let mut items: Vec<(OrderKey, AlignmentItem)> = phrases
        .into_iter()
        .map(|r| {
            (
                (r.decided_at, r.id.to_string(), 0),
                AlignmentItem::Phrase {
                    id: r.id,
                    phrase: r.phrase,
                    subject_class: r.subject_class,
                    object_class: r.object_class,
                    object_is_value: r.object_is_value,
                    statement_count: r.statement_count,
                    examples: r.examples,
                    votes: r.votes,
                    decided_at: r.decided_at,
                    bound_to: r.bound_to,
                    direction: r.direction,
                },
            )
        })
        .collect();
    for w in words {
        let sig = sigs.iter().find(|s| s.kind_word == w.kind_word);
        items.push((
            (w.decided_at, w.kind_word.clone(), 1),
            AlignmentItem::KindWord {
                kind_word: w.kind_word,
                words: w.words,
                examples: sig.map(|s| s.examples.clone()).unwrap_or_default(),
                phrases: sig.map(|s| s.phrases.clone()).unwrap_or_default(),
                entity_count: sig.map(|s| s.count).unwrap_or(0),
                votes: w.votes,
                decided_at: w.decided_at,
            },
        ));
    }
    for r in rules {
        items.push((
            (r.decided_at, r.id.to_string(), 2),
            AlignmentItem::Rule {
                id: r.id,
                trigger: r.trigger,
                phrase: r.phrase,
                subject_class: r.subject_class,
                object_class: r.object_class,
                object_is_value: r.object_is_value,
                property: r.property,
                property_label: r.property_label,
                reading: r.reading,
                statement_count: r.statement_count,
                examples: r.examples,
                votes: r.votes,
                decided_at: r.decided_at,
            },
        ));
    }
    items.sort_by(|(a, _), (b, _)| a.cmp(b));
    Ok(items
        .into_iter()
        .map(|(_, item)| item)
        .skip(usize::try_from(offset).unwrap_or(0))
        .take(usize::try_from(limit).unwrap_or(usize::MAX))
        .collect())
}

/// [`list`] 翻到底一共多少条（0076：列表响应里的 `total`）：短语签名、类别词，**加上**
/// 提议中的蕴含规则。`counts.alignment`（徽标）不数规则，按它翻页最后几条规则翻不到。
/// 三段各用列表自己的 FROM / JOIN / WHERE——规则内连接结论属性，照样连
pub async fn total(pool: &PgPool, kb_id: Uuid) -> AppResult<i64> {
    Ok(sqlx::query_scalar(&format!(
        "SELECT (SELECT count(*) FROM phrase_bindings b WHERE b.kb_id = $1 AND {WAITING_PHRASE})
              + (SELECT count(*) FROM implication_rules r
                   JOIN relation_types p ON p.id = r.conclude_property_id
                  WHERE r.kb_id = $1 AND r.status = 'proposed')
              + (SELECT count(*) FROM type_bindings WHERE kb_id = $1 AND status = 'undecided')"
    ))
    .bind(kb_id)
    .fetch_one(pool)
    .await?)
}

/// 等人定的条目数，以及最老的一条从什么时候起在等。
pub async fn waiting(pool: &PgPool, kb_id: Uuid) -> AppResult<(i64, Option<DateTime<Utc>>)> {
    Ok(sqlx::query_as(&format!(
        "SELECT count(*), min(decided_at) FROM (
            SELECT b.decided_at FROM phrase_bindings b WHERE b.kb_id = $1 AND {WAITING_PHRASE}
            UNION ALL
            SELECT decided_at FROM type_bindings WHERE kb_id = $1 AND status = 'undecided'
            UNION ALL
            SELECT decided_at FROM implication_rules WHERE kb_id = $1 AND status = 'proposed'
         ) q"
    ))
    .bind(kb_id)
    .fetch_one(pool)
    .await?)
}
