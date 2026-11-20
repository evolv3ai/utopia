//! Review 的 MCP 工具（0076 决定 2）：`review_list`、`review_item`、`review_ontology_list`、
//! `review_ontology`、`review_keep`、
//! `review_merge`、`review_revert`、`review_batch`，以及只能提议的 `review_defect`、
//! `review_fact`、`review_conflict`、`review_violation`、`review_errata`、`review_alignment`。
//!
//! 两个读工具管 Review 的十三档：`review_list` 读一档的一页（十一个列表档、总览、决策台账），
//! `review_item` 读某一档里列着的一条，违规一档的条目带上它的证明。写的有两样：
//! `review_keep` 把一对分开；`review_merge` 合并队列里的一对或手动指定的两个实体，
//! 先过服务端的闸门，被留下的不落地、只留一条等人答的提议；`review_revert` 撤回一次
//! 合并——令牌 agent 做的直接撤，人或治理 agent 做的只留提议，给 `batch_id` 就是把一批
//! 做出的合并整批撤回；`review_batch` 一次裁最多 50 对重复项，每一对走的就是单笔的那条路，
//! 服务端发一个 `batch_id`。裁定表里只能提议的操作各有一个工具，`review_defect`（对一处
//! 本体缺陷表态）是第一个，`review_fact`（确认、拒绝、闭合一条事实）与 `review_conflict`
//! （裁一处时态矛盾）照的是它，`review_violation`（裁一处公理违规）、`review_errata`（答勘误
//! agent 留给人的一笔）、`review_alignment`（定一条短语签名或一条蕴含规则）也是：调用什么
//! 都不改，只留一条等人答的提议。本体提案不在那十三档里，另有两个工具：
//! `review_ontology_list` 读还开着的本体提案（会话在本体页上看到的那一份），
//! `review_ontology` 提议拒绝或采纳其中一条。
//! **没有哪个工具能答提议。**
//!
//! **这里只做两件事：把参数递给 `review_ops`，把它的回答变成工具结果。** 授权、裁定表、
//! 台账、审计都在 `review_ops` 里面，REST 的令牌分支调的是同一批函数——所以同一枚令牌
//! 在两个入口读到的东西相等，被拒绝时拿到的 code 也相等。这里不另做一道检查：
//! 两道检查迟早有一道先变。
//!
//! **不进 `chat::tools_schema`，也不进 `tools::dispatch`。** 那两处与应用内的聊天助手
//! 共用，而助手不该拿到任何 Review 工具。这几个工具的定义与派发只在这个文件里，
//! 由 `mcp.rs` 接进 `tools/list` 与 `tools/call`。
//!
//! 标注（`readOnlyHint` 之类）只是给客户端的提示，服务端没有一条路径读它们（0076 决定 3）。

use serde_json::{json, Value};
use utopia_core::AppError;
use uuid::Uuid;

use super::review_ops::{self, ReviewCaller};
use super::review_routes::{
    CloseFactBody, ConflictBody, DecideAlignmentPhraseReq, DecideViolationReq,
};
use super::tools::{Step, ToolResult};
use crate::state::AppState;

const LIST: &str = "review_list";
const ITEM: &str = "review_item";
const KEEP: &str = "review_keep";
const MERGE: &str = "review_merge";
const REVERT: &str = "review_revert";
const BATCH: &str = "review_batch";
const DEFECT: &str = "review_defect";
const FACT: &str = "review_fact";
const CONFLICT: &str = "review_conflict";
const VIOLATION: &str = "review_violation";
const ERRATA: &str = "review_errata";
const ALIGNMENT: &str = "review_alignment";
const ONTOLOGY_LIST: &str = "review_ontology_list";
const ONTOLOGY: &str = "review_ontology";

/// 这个名字归不归这里管。归这里的，`mcp.rs` 不再拿它去问 `is_exposed`
pub(super) fn serves(name: &str) -> bool {
    matches!(
        name,
        LIST | ITEM
            | KEEP
            | MERGE
            | REVERT
            | BATCH
            | DEFECT
            | FACT
            | CONFLICT
            | VIOLATION
            | ERRATA
            | ALIGNMENT
            | ONTOLOGY_LIST
            | ONTOLOGY
    )
}

/// 写工具：只列给写得了的调用方，标注里 `readOnlyHint` 是 false
fn writes(name: &str) -> bool {
    matches!(
        name,
        KEEP | MERGE
            | REVERT
            | BATCH
            | DEFECT
            | FACT
            | CONFLICT
            | VIOLATION
            | ERRATA
            | ALIGNMENT
            | ONTOLOGY
    )
}

/// 工具定义，OpenAI 的形状——`chat::check_call` 认的就是这个形状，参数守卫与别的工具
/// 共用一份。
///
/// 翻页参数（`limit`、`offset`、`page`、`per`）**都不写上下界**：越界的值不是错误，照会话
/// 路由的规矩夹（`review_ops::list` 夹到 1 到 200，`review_ops::history` 夹到 1 到 100）。
/// 写了 `minimum` 的话，守规矩的客户端会在本地就把 0 拦下。
///
/// `review_item` 的 `id` **不写 `format: uuid`**：对齐一档的类别词没有 uuid，认的是它的词。
/// `review_keep` 的 `id` 照旧是 uuid——它只分重复项。
///
/// `review_merge` **没有必填参数**：要么给 `id`（队列里的一对），要么给 `source` 与
/// `target`（两个实体）。两种给法是哪一种、给全了没有，由 [`call`] 看——`required`
/// 表达不了「二选一」
///
/// `review_revert` 的说明**不许把撤回说成完全复原**（0076）：它只把图放回去，合并还在时
/// 推出的派生、记下的违规、导出的东西、答出去的话都不收回；没有类型的存活方留着拿到的
/// 类型，存活的事实留着抄给它的陈述来源。这三句写在说明里，对面的 agent 读得到。
/// 它同样**没有必填参数**：`merge_id`（一次合并）与 `batch_id`（一批）正好给一个，
/// 给了几个由 [`call`] 看
///
/// `review_batch` 的 `ids` **不写 `minItems` / `maxItems`**：0 条与 51 条是操作自己的拒绝
/// （`batch_size`，与 REST 同一个 code），不是客户端在本地拦下的参数错误
fn definitions() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": LIST,
                "description": "Read one section of this knowledge base's Review tab; all \
                    thirteen sections are served. The eleven queue sections are \"pending\" \
                    (Awaiting your nod), \"duplicates\", \"conflicts\", \"unconfirmed\", \
                    \"lowconf\" (Low confidence), \"violations\" (Axioms), \"defects\" \
                    (Ontology), \"alignment\", \"errata\", \"agent\" (Agent History) and \
                    \"merges\": each returns one page as {counts, queue, items, total}, paged \
                    with limit and offset; total is the number of items the section lists over \
                    all pages, so the last item is at offset total - 1 (counts holds the tab's \
                    badges, which for some sections is a different number). \"decisions\" \
                    returns one page of the decision history as {events, total}, paged with \
                    page and per. \"overview\" returns the Review summary (what is waiting, \
                    what was decided, health, the governor's recent rows) and takes no paging \
                    argument. A paging argument the section does not take is ignored. Any \
                    other section name is refused with code unknown_queue. Reading is all \
                    these sections offer: nothing listed here is decided by reading it.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "section": {
                            "type": "string",
                            "description": "One of \"overview\", \"pending\", \"duplicates\", \
                                \"conflicts\", \"unconfirmed\", \"lowconf\", \"violations\", \
                                \"defects\", \"alignment\", \"errata\", \"agent\", \
                                \"decisions\", \"merges\"."
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Queue sections only: page size (default 10). A \
                                value below 1 is served as 1 and a value above 200 as 200."
                        },
                        "offset": {
                            "type": "integer",
                            "description": "Queue sections only: zero-based start of the page \
                                (default 0). An offset at or past total returns no items."
                        },
                        "page": {
                            "type": "integer",
                            "description": "\"decisions\" only: zero-based page number \
                                (default 0)."
                        },
                        "per": {
                            "type": "integer",
                            "description": "\"decisions\" only: events per page (default 20). \
                                A value below 1 is served as 1 and a value above 100 as 100."
                        }
                    },
                    "required": ["section"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": ITEM,
                "description": "Read one item that review_list lists, by its section and id. \
                    Returns that item exactly as review_list returns it: for \"duplicates\" \
                    the pair's score, reason, stage, both entities with their type, \
                    disambiguator, fact count and top facts, and the governor's open proposal; \
                    for \"decisions\" the event. For \"violations\" (Axioms) the item also \
                    carries proof: the proof steps when its kind is derived_contradiction, \
                    null for any other kind. section is one of the eleven queue sections \
                    (\"pending\", \"duplicates\", \"conflicts\", \"unconfirmed\", \"lowconf\", \
                    \"violations\", \"defects\", \"alignment\", \"errata\", \"agent\", \
                    \"merges\") or \"decisions\". An id that is not listed in that section of \
                    this knowledge base (unknown, of another knowledge base, or no longer \
                    waiting there) is refused with code not_found, as is any id with section \
                    \"overview\", which lists no items. Any other section name is refused \
                    with code unknown_queue.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "section": {
                            "type": "string",
                            "description": "Section the item is listed in. Default \
                                \"duplicates\"."
                        },
                        "id": {
                            "type": "string",
                            "description": "The item's id from review_list (a uuid; for \
                                \"decisions\" the event's id, for \"agent\" the row's id). An \
                                \"alignment\" item of kind kind_word has no id: pass its \
                                kind_word exactly as listed, same letter case and spacing."
                        }
                    },
                    "required": ["id"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": KEEP,
                "description": "Keep one pending duplicate pair apart: decide that its two \
                    entities are NOT the same thing. Applied at once and recorded under the \
                    token's owner and the token's name. Changes no entity and no fact; the \
                    pair leaves the Duplicates section. Read the pair with review_item first. \
                    A pair that was already decided is refused with code already_decided. A \
                    token finalizes at most 200 writes (keeps, merges, reverts) per knowledge \
                    base in a rolling hour; past that the call is refused with code \
                    agent_rate_limited and nothing is changed.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "id": {
                            "type": "string",
                            "format": "uuid",
                            "description": "Review id (uuid) of a pair from review_list."
                        },
                        "rationale": {
                            "type": "string",
                            "description": "Optional: one sentence on what tells the two apart."
                        }
                    },
                    "required": ["id"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": MERGE,
                "description": "Merge two entities that are the same thing. Pass either id (the \
                    review id of a pending duplicate pair from review_list; the side with more \
                    facts survives, as when a person merges the pair in the Review tab) or \
                    source and target (two entity ids; source is merged into target). The \
                    server's merge gate is asked first. If it passes, the merge is applied at \
                    once and recorded under the token's owner and the token's name, and the \
                    result is {ok, outcome: \"applied\", action_id, merge_id}. If the gate \
                    holds the merge (a source that treats names as identifiers, a one-value \
                    relation the merge would contradict, derived facts resting on one side, an \
                    entity named in an answer, a pair a person kept apart, or a pair whose \
                    merge was reverted), nothing is changed: the call waits as a proposal that \
                    only a signed-in person can answer in the Review tab, and the result is \
                    {ok, outcome: \"held\" or \"proposed\", action_id, reason, explanation}. \
                    Repeating a held call returns the same proposal with code \
                    already_proposed. No tool answers a proposal. A pair that was already \
                    decided, or an entity already merged into another, is refused with code \
                    already_decided; merging an entity into itself with code self_merge; an \
                    unknown id with code not_found. After people have reverted two of this \
                    token's merges on this knowledge base within seven days, its merges are \
                    not applied for the rest of that window: each waits as a proposal with \
                    reason breaker. A token finalizes at most 200 writes per knowledge base in \
                    a rolling hour; past that the call is refused with code agent_rate_limited \
                    and nothing is changed.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "id": {
                            "type": "string",
                            "format": "uuid",
                            "description": "Review id (uuid) of a pending duplicate pair from \
                                review_list. Leave out when source and target are given."
                        },
                        "source": {
                            "type": "string",
                            "format": "uuid",
                            "description": "Entity id (uuid) to merge away. Given together \
                                with target, instead of id."
                        },
                        "target": {
                            "type": "string",
                            "format": "uuid",
                            "description": "Entity id (uuid) that survives. Given together \
                                with source, instead of id."
                        },
                        "rationale": {
                            "type": "string",
                            "description": "Optional: one sentence on what makes the two the \
                                same thing."
                        }
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": REVERT,
                "description": "Revert one merge, named by its merge_id (from the result of \
                    review_merge, or from review_list with section \"merges\"), or every merge \
                    one review_batch call made, named by its batch_id. Pass exactly one of \
                    merge_id and batch_id. With batch_id each merge of the batch that is not \
                    yet reverted goes through the same single revert, best effort; the result \
                    is {batch_id, reverted, outcomes: [{merge_id, outcome, code, error}]}, a \
                    merge that was already reverted is that item's already_decided, pairs the \
                    batch kept apart are left as they are, and a batch_id no action of this \
                    knowledge base carries is refused with code not_found. A merge that a \
                    token agent made on this knowledge base, through this token or another, is \
                    reverted at once and recorded under the token's owner and the token's \
                    name; the result is {ok, outcome: \"applied\", action_id, merge_id}. A \
                    merge that a person or the governor made is not reverted by a token: \
                    nothing is changed, the call waits as a proposal that only a signed-in \
                    person can answer in the Review tab, and the result is {ok, outcome: \
                    \"proposed\", action_id, reason, explanation} with reason merge_by_person, \
                    merge_by_governor or breaker. Repeating such a call returns the same \
                    proposal with code already_proposed. No tool answers a proposal. A revert \
                    restores graph state only and does not recall derivations, violations, \
                    exports or answers drawn while the merge was live. Two differences from \
                    the state before the merge remain: a target that had no type before the \
                    merge keeps the type it took, and a surviving fact keeps the statement \
                    links the merge copied onto it. A merge that was already reverted is \
                    refused with code already_decided; a merge id that is unknown, or of \
                    another knowledge base, with code not_found. Reason breaker means people \
                    reverted two of this token's merges on this knowledge base within seven \
                    days, so for the rest of that window its reverts only propose. A token \
                    finalizes at most 200 writes per knowledge base in a rolling hour; past \
                    that a revert is refused with code agent_rate_limited (with batch_id, as \
                    that item's code) and nothing is changed.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "merge_id": {
                            "type": "string",
                            "format": "uuid",
                            "description": "Merge id (uuid): merge_id in the result of \
                                review_merge, or the id of an item of the \"merges\" section. \
                                Leave out when batch_id is given."
                        },
                        "batch_id": {
                            "type": "string",
                            "format": "uuid",
                            "description": "Batch id (uuid): batch_id in the result of \
                                review_batch. Reverts every merge that batch made. Given \
                                instead of merge_id."
                        },
                        "rationale": {
                            "type": "string",
                            "description": "Optional: one sentence on why the merge was wrong."
                        }
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": BATCH,
                "description": "Decide 1 to 50 pending duplicate pairs in one call, all with \
                    the same action: \"merge\" or \"keep\". Each pair goes through the same \
                    path as review_merge or review_keep, one by one in the order given, so a \
                    batch grants nothing a single call would not: the server's merge gate is \
                    asked for every pair, and a pair it holds changes nothing and waits as a \
                    proposal that only a signed-in person can answer. The server issues one \
                    batch_id, recorded with every action of the batch under the token's owner \
                    and the token's name. The result is {batch_id, decided, outcomes: [{id, \
                    outcome, action_id, merge_id, reason, code, error}]}, one outcome per id in \
                    the order given; outcome is \"applied\", \"held\" or \"proposed\", or null \
                    with code and error set when that pair was refused (not_found for an \
                    unknown id, already_decided for a pair that was already decided, \
                    agent_rate_limited for each pair past the limit of 200 finalized writes \
                    per knowledge base in a rolling hour); decided counts the applied pairs. One refused pair does not stop the others. An id \
                    given twice gets, the second time, the answer its state then gives. An \
                    empty list or more than 50 ids is refused as a whole with code batch_size, \
                    and so is an action other than merge or keep; nothing is changed. To undo \
                    the merges of a batch, call review_revert with its batch_id.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "ids": {
                            "type": "array",
                            "items": { "type": "string", "format": "uuid" },
                            "description": "Review ids (uuids) of 1 to 50 pending duplicate \
                                pairs from review_list, in the order to decide them."
                        },
                        "action": {
                            "type": "string",
                            "description": "\"merge\" or \"keep\", applied to every pair."
                        },
                        "rationale": {
                            "type": "string",
                            "description": "Optional: one sentence that holds for the whole \
                                batch."
                        }
                    },
                    "required": ["ids", "action"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": DEFECT,
                "description": "Propose that a person resolve one open ontology defect, an item \
                    of the \"defects\" (Ontology) section of review_list, as \"fixed\" (the \
                    ontology was corrected) or \"accepted\" (it was looked at and is left as it \
                    is). The call changes nothing: the defect stays open and the call waits as \
                    a proposal, recorded under the token's owner and the token's name, that a \
                    signed-in editor answers in the Review tab; only when that person approves \
                    it is the defect resolved, as that person's decision. The result is {ok, \
                    outcome: \"proposed\", action_id, reason: \"propose_only\", explanation}. \
                    Repeating the call while its proposal is open returns the same proposal \
                    with code already_proposed. A resolution other than fixed or accepted is \
                    refused with code bad_resolution; a defect that is already resolved with \
                    code already_decided; an id that is unknown, or of another knowledge base, \
                    with code not_found. Proposals do not count toward the limit on finalized \
                    writes. No tool answers a proposal.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "id": {
                            "type": "string",
                            "format": "uuid",
                            "description": "Defect id (uuid) of an item of the \"defects\" \
                                section from review_list."
                        },
                        "resolution": {
                            "type": "string",
                            "description": "\"fixed\" or \"accepted\"."
                        }
                    },
                    "required": ["id", "resolution"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": FACT,
                "description": "Propose that a person confirm, reject or close one fact, an \
                    item of the \"unconfirmed\" or \"lowconf\" (Low confidence) section of \
                    review_list: \"confirm\" (the fact is right; its confidence becomes 1), \
                    \"reject\" (the fact is wrong and is retracted) or \"close\" (the fact was \
                    true and ended at valid_to). The call changes nothing: the fact stays as \
                    it is and the call waits as a proposal, recorded under the token's owner \
                    and the token's name, that a signed-in editor answers in the Review tab; \
                    only when that person approves it is the fact confirmed, rejected or \
                    closed, as that person's decision. The result is {ok, outcome: \
                    \"proposed\", action_id, reason: \"propose_only\", explanation}. \
                    Repeating the call while its proposal is open returns the same proposal \
                    with code already_proposed; confirm, reject and close are separate \
                    proposals, so more than one may be open on one fact. A fact id that is \
                    unknown, of another knowledge base, or (for confirm) retracted is refused \
                    with code not_found, as is closing a fact that already has an end; \
                    rejecting or closing a retracted fact with code already_decided; a \
                    valid_to_precision that is not year, month, day, hour, minute or second \
                    with code bad_precision; a valid_to before the fact's start with code \
                    interval_inverted; an action other than the three with code \
                    unknown_action. Proposals do not count toward the limit on finalized \
                    writes. No tool answers a proposal.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "id": {
                            "type": "string",
                            "format": "uuid",
                            "description": "Fact id (uuid) of an item of the \"unconfirmed\" \
                                or \"lowconf\" section from review_list."
                        },
                        "action": {
                            "type": "string",
                            "description": "\"confirm\", \"reject\" or \"close\"."
                        },
                        "valid_to": {
                            "type": "string",
                            "description": "\"close\" only, and required for it: when the \
                                fact ended, as an RFC 3339 date-time such as \
                                2024-06-01T00:00:00Z."
                        },
                        "valid_to_precision": {
                            "type": "string",
                            "description": "\"close\" only: how precise valid_to is, one of \
                                year, month, day, hour, minute, second (default day)."
                        }
                    },
                    "required": ["id", "action"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": CONFLICT,
                "description": "Propose that a person resolve one open temporal conflict, an \
                    item of the \"conflicts\" section of review_list, in which an old and a \
                    new fact cannot both hold: \"close\" (end the old fact at close_at, or at \
                    the new fact's start when close_at is left out), \"retime_new\" (move the \
                    new fact's start to new_from), \"keep\" (keep both facts as they are) or \
                    \"reject_new\" (retract the new fact). The call changes nothing: the \
                    conflict stays open, both facts stay as they are, and the call waits as a \
                    proposal, recorded under the token's owner and the token's name, that a \
                    signed-in editor answers in the Review tab; only when that person approves \
                    it is the conflict resolved, as that person's decision. The result is {ok, \
                    outcome: \"proposed\", action_id, reason: \"propose_only\", explanation}. \
                    Repeating the call while its proposal is open returns the same proposal \
                    with code already_proposed. A conflict that is already resolved, or one of \
                    whose facts was retracted, is refused with code already_decided; an id \
                    that is unknown, or of another knowledge base, with code not_found; a \
                    precision that is not year, month, day, hour, minute or second with code \
                    bad_precision; \"close\" without close_at when the new fact has no start \
                    with code close_at_required; \"retime_new\" without new_from with code \
                    new_from_required, and on a fact that is not a state with code \
                    not_a_state; a date that would put an interval's end before its start with \
                    code interval_inverted; an unknown action without a code. If the conflict \
                    comes to name other facts before the proposal is answered, approving the \
                    proposal changes nothing and closes it as superseded. Proposals do not \
                    count toward the limit on finalized writes. No tool answers a proposal.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "id": {
                            "type": "string",
                            "format": "uuid",
                            "description": "Conflict id (uuid) of an item of the \
                                \"conflicts\" section from review_list."
                        },
                        "action": {
                            "type": "string",
                            "description": "\"close\", \"retime_new\", \"keep\" or \
                                \"reject_new\"."
                        },
                        "close_at": {
                            "type": "string",
                            "description": "\"close\" only: when the old fact ended, as an \
                                RFC 3339 date-time. Required when the new fact has no start."
                        },
                        "close_at_precision": {
                            "type": "string",
                            "description": "How precise close_at is, one of year, month, day, \
                                hour, minute, second (default day)."
                        },
                        "new_from": {
                            "type": "string",
                            "description": "\"retime_new\" only, and required for it: the \
                                new fact's start, as an RFC 3339 date-time."
                        },
                        "new_from_precision": {
                            "type": "string",
                            "description": "How precise new_from is, one of year, month, day, \
                                hour, minute, second (default day)."
                        }
                    },
                    "required": ["id", "action"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": VIOLATION,
                "description": "Propose that a person decide one open axiom violation, an item \
                    of the \"violations\" (Axioms) section of review_list, in one of four ways: \
                    \"fact_retracted\" (a fact is wrong and is retracted; fact_id names it), \
                    \"fact_closed\" (for a derived_contradiction only: the assertion was true \
                    and ended at close_at), \"axiom_relaxed\" (the ontology's axiom is wrong, \
                    not the facts) or \"accepted\" (the facts may stand together). The call \
                    changes nothing: the violation stays open, every fact stays as it is, and \
                    the call waits as a proposal, recorded under the token's owner and the \
                    token's name, that a signed-in editor answers in the Review tab; only when \
                    that person approves it is the violation decided, as that person's \
                    decision. The result is {ok, outcome: \"proposed\", action_id, reason: \
                    \"propose_only\", explanation}. Repeating the call while its proposal is \
                    open returns the same proposal with code already_proposed. A resolution \
                    other than the four is refused with code bad_resolution, as is fact_closed \
                    on a violation that is not a derived_contradiction; fact_retracted on a \
                    violation of more than one fact without a fact_id it lists with code \
                    fact_required; fact_closed without close_at with code close_at_required, \
                    with a close_at_precision that is not year, month, day, hour, minute or \
                    second with code bad_precision, and with a close_at before the \
                    assertion's start with code interval_inverted. A violation that is already \
                    decided, or whose named fact was retracted or closed meanwhile, is refused \
                    with code already_decided; an id that is unknown, or of another knowledge \
                    base, with code not_found. If the violation is decided, or that fact is \
                    retracted, before the proposal is answered, approving the proposal changes \
                    nothing and closes it as superseded. Proposals do not count toward the \
                    limit on finalized writes. No tool answers a proposal.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "id": {
                            "type": "string",
                            "format": "uuid",
                            "description": "Violation id (uuid) of an item of the \
                                \"violations\" section from review_list."
                        },
                        "resolution": {
                            "type": "string",
                            "description": "\"fact_retracted\", \"fact_closed\", \
                                \"axiom_relaxed\" or \"accepted\"."
                        },
                        "fact_id": {
                            "type": "string",
                            "format": "uuid",
                            "description": "\"fact_retracted\" only: the fact to retract, one \
                                of the facts the violation lists. May be left out when the \
                                violation involves a single fact."
                        },
                        "close_at": {
                            "type": "string",
                            "description": "\"fact_closed\" only, and required for it: when \
                                the assertion ended, as an RFC 3339 date-time such as \
                                2024-06-01T00:00:00Z."
                        },
                        "close_at_precision": {
                            "type": "string",
                            "description": "\"fact_closed\" only: how precise close_at is, one \
                                of year, month, day, hour, minute, second (default day)."
                        }
                    },
                    "required": ["id", "resolution"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": ERRATA,
                "description": "Propose that a person answer one held errata action, an item \
                    of the \"errata\" section of review_list: a retraction, revision or \
                    addition the errata agent wanted and the server's gate held for a person. \
                    approve true proposes that the action be carried out; approve false \
                    proposes that it be rejected and only recorded. The call changes nothing: \
                    the action stays held, every fact stays as it is, and the call waits as a \
                    proposal, recorded under the token's owner and the token's name, that a \
                    signed-in editor answers in the Review tab; only when that person approves \
                    the proposal is the action carried out or rejected, as that person's \
                    answer. The result is {ok, outcome: \"proposed\", action_id, reason: \
                    \"propose_only\", explanation}. Repeating the call while its proposal is \
                    open returns the same proposal with code already_proposed. An action that \
                    was already answered is refused with code already_decided; an id that is \
                    unknown, or of another knowledge base, with code not_found. If the action \
                    is answered before the proposal is, approving the proposal changes nothing \
                    and closes it as superseded. Proposals do not count toward the limit on \
                    finalized writes. No tool answers a proposal.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "id": {
                            "type": "string",
                            "format": "uuid",
                            "description": "Action id (uuid) of an item of the \"errata\" \
                                section from review_list."
                        },
                        "approve": {
                            "type": "boolean",
                            "description": "true to propose carrying the held action out, \
                                false to propose rejecting it."
                        }
                    },
                    "required": ["id", "approve"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": ALIGNMENT,
                "description": "Propose that a person decide one item of the \"alignment\" \
                    section of review_list: a phrase signature (kind \"phrase\") or an \
                    implication rule (kind \"rule\"). For a phrase, property is the key of the \
                    ontology property the phrase is bound to, with direction \"forward\" or \
                    \"reverse\" and, under a state property, marks (\"start\", \"end\" or \
                    \"none\": what a statement's single date marks); leave property out to \
                    propose that no property fits. For a rule, approve true proposes approving \
                    it and approve false rejecting it. The call changes nothing: the phrase or \
                    rule stays as it is, no job is queued, and the call waits as a proposal, \
                    recorded under the token's owner and the token's name, that a signed-in \
                    editor answers in the Review tab. When that person approves the proposal, \
                    the decision is recorded as that person's and the change to the typed \
                    graph is queued as a job, not applied at once: an approval queues the \
                    change, and its row in Agent History reads queued. The result is {ok, \
                    outcome: \"proposed\", action_id, reason: \"propose_only\", explanation}. \
                    Repeating the call while its proposal is open returns the same proposal \
                    with code already_proposed. A phrase or rule has no decided state, so a \
                    proposal is taken, and applied on approval, while the item exists; an id \
                    that is unknown, or of another knowledge base, is refused with code \
                    not_found. For a phrase, a property key the ontology does not have is \
                    refused with code unknown_property; a property without a direction of \
                    forward or reverse with code direction_required; marks other than start, \
                    end or none with code unknown_marks; marks without a state property with \
                    code marks_needs_state. Kind words are not decided with this tool. \
                    Proposals do not count toward the limit on finalized writes. No tool \
                    answers a proposal.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "kind": {
                            "type": "string",
                            "description": "\"phrase\" or \"rule\": the kind of the item, as \
                                review_list gives it."
                        },
                        "id": {
                            "type": "string",
                            "format": "uuid",
                            "description": "Id (uuid) of a phrase or rule item of the \
                                \"alignment\" section from review_list."
                        },
                        "property": {
                            "type": "string",
                            "description": "\"phrase\" only: key of the property to bind the \
                                phrase to. Leave out to propose that no property fits."
                        },
                        "direction": {
                            "type": "string",
                            "description": "\"phrase\" only, required with property: \
                                \"forward\" or \"reverse\"."
                        },
                        "marks": {
                            "type": "string",
                            "description": "\"phrase\" only, under a state property: \
                                \"start\", \"end\" or \"none\"."
                        },
                        "approve": {
                            "type": "boolean",
                            "description": "\"rule\" only, and required for it: true to \
                                propose approving the rule, false to propose rejecting it."
                        }
                    },
                    "required": ["kind", "id"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": ONTOLOGY_LIST,
                "description": "Read the open ontology proposals of this knowledge base: the \
                    classes, relations and attributes the ontology agent proposes to add, and \
                    the existing elements it proposes to reuse, that no person has adopted or \
                    rejected yet. These are not a section of review_list; a signed-in person \
                    sees them on the Ontology page. Returns {entity_types, relation_types, \
                    attribute_types, map_to}, one list per section, as a signed-in person gets \
                    them: each item has its key and the proposed fields, proposed_by, serves \
                    (the competency questions it serves) and signatures (the shapes it would \
                    bind), and carries agent_proposals only while a token's proposal on it is \
                    open. An item is named by the section it is listed under and its key. \
                    Takes no argument. Reading is all this tool offers: nothing listed here is \
                    decided by reading it.",
                "parameters": {
                    "type": "object",
                    "properties": {}
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": ONTOLOGY,
                "description": "Propose that a person reject or adopt one open ontology \
                    proposal, an item of review_ontology_list named by its section and key: \
                    \"reject\" (the ontology should not get this element; reason says why) or \
                    \"adopt\" (the element is added to the ontology, or for a map_to item the \
                    existing element is reused; label, description, datatype, domains, ranges \
                    and parents replace what the ontology proposal says, each only when \
                    given). The call changes nothing: the ontology proposal stays open, the \
                    ontology stays as it is, no job is queued, and the call waits as a \
                    proposal, recorded under the token's owner and the token's name, that a \
                    signed-in editor answers in the Review tab; only when that person approves \
                    it is the ontology proposal rejected or adopted, as that person's \
                    decision. A reason given with \"reject\" stays on the token's proposal and \
                    is not recorded as the person's reason. The result is {ok, outcome: \
                    \"proposed\", action_id, reason: \"propose_only\", explanation}. One \
                    proposal is open per ontology proposal, whichever action it asks for: \
                    repeating the call, or calling with the other action, while it is open \
                    returns the same proposal with code already_proposed. A section and key \
                    that review_ontology_list does not list, in this knowledge base, is \
                    refused with code not_found; an ontology proposal that was already adopted \
                    or rejected with code already_decided; an action other than reject or \
                    adopt with code unknown_action; \"adopt\" naming a class the ontology does \
                    not have, in parents, domains or ranges, with code unknown_class. If the \
                    ontology proposal is decided, or the ontology agent rewrites it, before \
                    the proposal is answered, approving the proposal changes nothing and \
                    closes it as superseded. Proposals do not count toward the limit on \
                    finalized writes. No tool answers a proposal.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "section": {
                            "type": "string",
                            "description": "Section the item is listed under in \
                                review_ontology_list: \"entity_types\", \"relation_types\", \
                                \"attribute_types\" or \"map_to\"."
                        },
                        "key": {
                            "type": "string",
                            "description": "The item's key, exactly as review_ontology_list \
                                gives it."
                        },
                        "action": {
                            "type": "string",
                            "description": "\"reject\" or \"adopt\"."
                        },
                        "reason": {
                            "type": "string",
                            "description": "\"reject\" only, optional: one sentence on why \
                                the ontology should not get this element."
                        },
                        "label": {
                            "type": "string",
                            "description": "\"adopt\" only, optional: the label to adopt the \
                                element with, in place of the proposed one."
                        },
                        "description": {
                            "type": "string",
                            "description": "\"adopt\" only, optional: the description to \
                                adopt the element with, in place of the proposed one."
                        },
                        "datatype": {
                            "type": "string",
                            "description": "\"adopt\" of an attribute only, optional: \
                                \"text\", \"number\", \"date\" or \"bool\"."
                        },
                        "domains": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "\"adopt\" of a relation or attribute only, \
                                optional: keys of the classes it applies to, in place of the \
                                proposed ones."
                        },
                        "ranges": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "\"adopt\" of a relation only, optional: keys of \
                                the classes its object may be, in place of the proposed ones."
                        },
                        "parents": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "\"adopt\" of a class only, optional: keys of its \
                                parent classes, in place of the proposed ones."
                        }
                    },
                    "required": ["section", "key", "action"]
                }
            }
        }
    ])
}

/// 提示，不是约束：能不能做由 `review_ops` 说了算
fn annotations(name: &str) -> Value {
    match name {
        KEEP => json!({ "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false }),
        // 合并改事实与实体：落地的那一次不是只添不改
        MERGE => json!({ "readOnlyHint": false, "destructiveHint": true, "idempotentHint": false }),
        // 撤回同样改事实与实体；第二次是 `already_decided`，不是同一个结果再来一遍
        REVERT => {
            json!({ "readOnlyHint": false, "destructiveHint": true, "idempotentHint": false })
        }
        // 一批里落地的合并同样改事实与实体
        BATCH => {
            json!({ "readOnlyHint": false, "destructiveHint": true, "idempotentHint": false })
        }
        // 只能提议的工具：这一次调用什么都不改，只留一条提议——不是只读（台账多一行），
        // 也不破坏什么
        DEFECT | FACT | CONFLICT | VIOLATION | ERRATA | ALIGNMENT | ONTOLOGY => {
            json!({ "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false })
        }
        _ => json!({ "readOnlyHint": true }),
    }
}

/// `tools/list` 里的那几项。读工具（两个读 Review 的、一个读本体提案的）对每一个进得来的调用方都列；写工具（[`writes`]）只在
/// `can_write`（令牌 scope 是 write 且主人至少是 Editor）时列——列出一个注定被拒绝的
/// 工具，等于让对面的 agent 反复去试
pub(super) fn listed(can_write: bool) -> Vec<Value> {
    definitions()
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|tool| {
            let f = tool.get("function")?;
            let name = f.get("name")?.as_str()?;
            if writes(name) && !can_write {
                return None;
            }
            Some(json!({
                "name": name,
                "description": f.get("description").and_then(|d| d.as_str()).unwrap_or(""),
                "inputSchema": f.get("parameters").cloned().unwrap_or(json!({
                    "type": "object", "properties": {}
                })),
                "annotations": annotations(name),
            }))
        })
        .collect()
}

/// 参数没说清：这次调用没有执行。与别的工具一样是纯文字的工具错误，不带 code——
/// code 留给操作本身的拒绝
fn bad_argument(name: &str, key: &str, expected: &str) -> ToolResult {
    ToolResult::new(
        format!(
            "{name} needs `{key}` to be {expected}, and it was not, so the call was not run. \
            Call it again with `{key}` corrected, or leave it out."
        ),
        Step::new("tool", name, format!("invalid {key}"))
            .invalid(key)
            .json(),
    )
    .error()
}

/// 可选的整数：没给或 null 是「没给」，给了别的类型是参数错误——不猜
fn optional_integer(name: &str, args: &Value, key: &str) -> Result<Option<i64>, ToolResult> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_i64()
            .map(Some)
            .ok_or_else(|| bad_argument(name, key, "an integer")),
    }
}

/// 可选的 uuid：没给或 null 是「没给」，给了不是 uuid 的东西是参数错误
fn optional_uuid(name: &str, args: &Value, key: &str) -> Result<Option<Uuid>, ToolResult> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_str()
            .and_then(|s| s.trim().parse::<Uuid>().ok())
            .map(Some)
            .ok_or_else(|| bad_argument(name, key, "a uuid returned by another review tool")),
    }
}

/// 可选的时刻（RFC 3339）：没给或 null 是「没给」，给了解不出来的东西是参数错误。
/// 解法与 REST 的请求体同一个（`chrono` 的反序列化），两个入口认的是同一种写法
fn optional_instant(
    name: &str,
    args: &Value,
    key: &str,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, ToolResult> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => serde_json::from_value(v.clone())
            .map(Some)
            .map_err(|_| bad_argument(name, key, "an RFC 3339 date-time")),
    }
}

fn optional_string<'a>(
    name: &str,
    args: &'a Value,
    key: &str,
) -> Result<Option<&'a str>, ToolResult> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(_) => Err(bad_argument(name, key, "a string")),
    }
}

/// 成功：`structuredContent` 就是令牌 API 回的那个 JSON；文字那一份是同一个 JSON 的
/// 序列化，给不读结构化结果的客户端
fn success(name: &str, content: Value) -> ToolResult {
    ToolResult::new(content.to_string(), Step::new("tool", name, "ok").json()).structured(content)
}

/// 操作的拒绝变成工具错误，`structuredContent` 是 `{"code", "message"}`。
///
/// **code 与 message 取自 REST 的错误响应本身**（`ApiErr` 渲染出来的那个 body），
/// 而不是在这里再写一张对照表：两张表迟早有一张先变，而「两个入口同一个 code」
/// 正是这一层要保证的。404 在 REST 那边没有 code，这里给它 `not_found`；别的没有
/// code 的错误不带 `code` 键。内部错误的细节同样只留在日志里。
/// 限速的 429（`AppError::RateLimited`）走的也是这一条：REST 的 body 里是
/// `agent_rate_limited`，工具错误里就是 `agent_rate_limited`——四个写工具都一样，
/// 成批的两个工具里它是那一条结果的 code
async fn refusal(name: &str, error: AppError) -> ToolResult {
    // 渲染在 `review_ops::refused`：成批调用里每一条的 code 读的也是它
    let (code, message) = review_ops::refused(error).await;
    let mut content = json!({ "message": message });
    if let Some(code) = &code {
        content["code"] = json!(code);
    }
    let detail = code.unwrap_or_else(|| "failed".to_string());
    ToolResult::new(
        message,
        Step::new("tool", name, detail).status("failed").json(),
    )
    .structured(content)
    .error()
}

/// 执行一次调用。`name` 必须是 [`serves`] 认的名字。
///
/// **写工具没被列出来的调用方照样走到 `review_ops` 的那个函数**：拒绝它的是那里的
/// 检查，回来的是带 code 的工具错误（`token_read_only`、`token_owner_below_editor`），
/// 与 REST 同一个 code——对面的 agent 由此知道是换一枚令牌，还是换令牌也没用
pub(super) async fn call(
    state: &AppState,
    caller: &ReviewCaller,
    name: &str,
    args: &Value,
) -> ToolResult {
    let args = match super::chat::check_call(&definitions(), name, &args.to_string()) {
        Ok(args) => args,
        Err((text, step)) => return ToolResult::new(text, step).error(),
    };
    let outcome = match name {
        LIST => {
            // `section` 必填且是字符串，上面的守卫已经看过。这里只按档名挑操作，不判它
            // 认不认得：认不出的名字落到 `review_ops::list`，由它拒绝（`unknown_queue`）
            let section = args["section"].as_str().unwrap_or_default().trim();
            // 翻页参数只读这一档用的那一组，先看类型再读库。这一档不用的参数不读也不查：
            // 总览从来不看 `limit`，决策台账同理不看它，列表档不看 `page` / `per`
            match section {
                "overview" => match review_ops::summary(state, caller).await {
                    Ok(summary) => serde_json::to_value(summary)
                        .map_err(|e| AppError::Other(anyhow::Error::new(e))),
                    Err(e) => Err(e),
                },
                // 决策台账有它自己的 `page` / `per`，回它自己的 `{events, total}`
                "decisions" => {
                    let page = match optional_integer(name, &args, "page") {
                        Ok(v) => v,
                        Err(refused) => return refused,
                    };
                    let per = match optional_integer(name, &args, "per") {
                        Ok(v) => v,
                        Err(refused) => return refused,
                    };
                    review_ops::history(state, caller, page, per).await
                }
                _ => {
                    let limit = match optional_integer(name, &args, "limit") {
                        Ok(v) => v,
                        Err(refused) => return refused,
                    };
                    let offset = match optional_integer(name, &args, "offset") {
                        Ok(v) => v,
                        Err(refused) => return refused,
                    };
                    review_ops::list(state, caller, section, None, limit, offset).await
                }
            }
        }
        ITEM => {
            // 不给 `section` 就是重复项，与只有这一档时的调用同一个意思
            let section = match optional_string(name, &args, "section") {
                Ok(v) => v.map_or("duplicates", str::trim),
                Err(refused) => return refused,
            };
            // `id` 必填且是字符串，守卫看过；**原样递下去**——类别词按存着的样子比，
            // 是不是 uuid、在不在这一档里，都由 `review_ops::item` 说
            let id = args["id"].as_str().unwrap_or_default();
            review_ops::item(state, caller, section, id).await
        }
        KEEP => {
            // `format: uuid` 由守卫挡过；这里再解析一次，解析不了同样不执行
            let Some(id) = args["id"]
                .as_str()
                .and_then(|s| s.trim().parse::<Uuid>().ok())
            else {
                return bad_argument(name, "id", "a uuid returned by review_list");
            };
            let rationale = match optional_string(name, &args, "rationale") {
                Ok(v) => v,
                Err(refused) => return refused,
            };
            review_ops::keep(state, caller, id, rationale, None).await
        }
        MERGE => {
            let (id, source, target) = match (
                optional_uuid(name, &args, "id"),
                optional_uuid(name, &args, "source"),
                optional_uuid(name, &args, "target"),
            ) {
                (Ok(id), Ok(source), Ok(target)) => (id, source, target),
                (Err(refused), _, _) | (_, Err(refused), _) | (_, _, Err(refused)) => {
                    return refused
                }
            };
            let rationale = match optional_string(name, &args, "rationale") {
                Ok(v) => v,
                Err(refused) => return refused,
            };
            // 两种给法各走各的操作，与 REST 的两条路由一一对应；闸门在操作里面。
            // 给法说不清（两种都给、只给一半、都没给）就不执行，不猜是哪一种
            match (id, source, target) {
                (Some(id), None, None) => {
                    review_ops::merge_pair(state, caller, id, rationale, None).await
                }
                (None, Some(source), Some(target)) => {
                    review_ops::merge_entities(state, caller, source, target, rationale).await
                }
                _ => {
                    return ToolResult::new(
                        format!(
                            "{name} needs either `id` (a pending duplicate pair) or both \
                            `source` and `target` (two entity ids), and not both forms, so \
                            the call was not run. Call it again with one of the two."
                        ),
                        Step::new("tool", name, "invalid arguments")
                            .invalid("id")
                            .json(),
                    )
                    .error()
                }
            }
        }
        REVERT => {
            // 两个 id 都不是必填，守卫不看它们的格式：这里解析，解析不了就不执行
            let (merge_id, batch_id) = match (
                optional_uuid(name, &args, "merge_id"),
                optional_uuid(name, &args, "batch_id"),
            ) {
                (Ok(merge_id), Ok(batch_id)) => (merge_id, batch_id),
                (Err(refused), _) | (_, Err(refused)) => return refused,
            };
            let rationale = match optional_string(name, &args, "rationale") {
                Ok(v) => v,
                Err(refused) => return refused,
            };
            // 两种给法各走各的操作，与 REST 的两条路由一一对应：一次合并的撤回（直接撤还是
            // 只留提议，由它里面的规则说），一批的整批撤回。两个都给、都没给就不执行，不猜
            match (merge_id, batch_id) {
                (Some(merge_id), None) => {
                    review_ops::revert_merge(state, caller, merge_id, rationale, None).await
                }
                (None, Some(batch_id)) => {
                    review_ops::revert_batch(state, caller, batch_id, rationale).await
                }
                _ => {
                    return ToolResult::new(
                        format!(
                            "{name} needs exactly one of `merge_id` (one merge) and `batch_id` \
                            (every merge of one batch), so the call was not run. Call it again \
                            with one of the two."
                        ),
                        Step::new("tool", name, "invalid arguments")
                            .invalid("merge_id")
                            .json(),
                    )
                    .error()
                }
            }
        }
        BATCH => {
            // `ids` 与 `action` 必填，守卫看过在不在；`ids` 里每一个都得是 uuid——有一个
            // 不是，整批不执行（REST 上同样是整个请求被拒）。条数不在这里看：0 条与 51 条
            // 由 `review_ops::batch` 拒绝，带的是 REST 的 code
            let ids: Option<Vec<Uuid>> = args["ids"].as_array().and_then(|ids| {
                ids.iter()
                    .map(|id| id.as_str().and_then(|s| s.trim().parse::<Uuid>().ok()))
                    .collect()
            });
            let Some(ids) = ids else {
                return bad_argument(name, "ids", "a list of uuids returned by review_list");
            };
            let Some(action) = args["action"].as_str() else {
                return bad_argument(name, "action", "\"merge\" or \"keep\"");
            };
            let rationale = match optional_string(name, &args, "rationale") {
                Ok(v) => v,
                Err(refused) => return refused,
            };
            review_ops::batch(state, caller, &ids, action.trim(), rationale).await
        }
        DEFECT => {
            // 参数就是那条路由的 id 与请求体的字段；`resolution` 认不认得由操作说
            // （`bad_resolution`，与 REST 同一个 code），这里原样递下去
            let Some(id) = args["id"]
                .as_str()
                .and_then(|s| s.trim().parse::<Uuid>().ok())
            else {
                return bad_argument(name, "id", "a uuid returned by review_list");
            };
            let Some(resolution) = args["resolution"].as_str() else {
                return bad_argument(name, "resolution", "\"fixed\" or \"accepted\"");
            };
            review_ops::propose_defect(state, caller, id, resolution).await
        }
        FACT => {
            // 参数是路由的 id、走哪一条路由（`action`）、闭合那条路由的请求体字段。
            // `action` 认不认得、精度认不认得由操作说（`unknown_action`、`bad_precision`）
            let Some(id) = args["id"]
                .as_str()
                .and_then(|s| s.trim().parse::<Uuid>().ok())
            else {
                return bad_argument(name, "id", "a uuid returned by review_list");
            };
            let Some(action) = args["action"].as_str().map(str::trim) else {
                return bad_argument(name, "action", "\"confirm\", \"reject\" or \"close\"");
            };
            if action != "close" {
                // 确认与拒绝的路由没有请求体：闭合的两个参数给了也不读
                review_ops::propose_fact(state, caller, id, action, None).await
            } else {
                let valid_to = match optional_instant(name, &args, "valid_to") {
                    Ok(Some(at)) => at,
                    // REST 上没有 `valid_to` 的请求体同样到不了操作那里
                    Ok(None) => {
                        return bad_argument(
                            name,
                            "valid_to",
                            "an RFC 3339 date-time when action is \"close\"",
                        )
                    }
                    Err(refused) => return refused,
                };
                let valid_to_precision = match optional_string(name, &args, "valid_to_precision") {
                    Ok(v) => v.map(str::to_string),
                    Err(refused) => return refused,
                };
                let body = CloseFactBody {
                    valid_to,
                    valid_to_precision,
                };
                review_ops::propose_fact(state, caller, id, action, Some(&body)).await
            }
        }
        CONFLICT => {
            // 参数是路由的 id 与 `ConflictBody` 的字段；动作、精度、日期够不够都由操作说
            let Some(id) = args["id"]
                .as_str()
                .and_then(|s| s.trim().parse::<Uuid>().ok())
            else {
                return bad_argument(name, "id", "a uuid returned by review_list");
            };
            let Some(action) = args["action"].as_str() else {
                return bad_argument(
                    name,
                    "action",
                    "\"close\", \"retime_new\", \"keep\" or \"reject_new\"",
                );
            };
            let (close_at, new_from) = match (
                optional_instant(name, &args, "close_at"),
                optional_instant(name, &args, "new_from"),
            ) {
                (Ok(close_at), Ok(new_from)) => (close_at, new_from),
                (Err(refused), _) | (_, Err(refused)) => return refused,
            };
            let (close_at_precision, new_from_precision) = match (
                optional_string(name, &args, "close_at_precision"),
                optional_string(name, &args, "new_from_precision"),
            ) {
                (Ok(close), Ok(new)) => (close.map(str::to_string), new.map(str::to_string)),
                (Err(refused), _) | (_, Err(refused)) => return refused,
            };
            let body = ConflictBody {
                action: action.trim().to_string(),
                close_at,
                close_at_precision,
                new_from,
                new_from_precision,
            };
            review_ops::propose_conflict(state, caller, id, &body).await
        }
        VIOLATION => {
            // 参数是路由的 id 与 `DecideViolationReq` 的字段；出路认不认得、日期够不够、
            // 撤哪一条说没说清都由操作说（`bad_resolution`、`close_at_required`、
            // `fact_required`，与 REST 同一个 code）
            let Some(id) = args["id"]
                .as_str()
                .and_then(|s| s.trim().parse::<Uuid>().ok())
            else {
                return bad_argument(name, "id", "a uuid returned by review_list");
            };
            let Some(resolution) = args["resolution"].as_str() else {
                return bad_argument(
                    name,
                    "resolution",
                    "\"fact_retracted\", \"fact_closed\", \"axiom_relaxed\" or \"accepted\"",
                );
            };
            let close_at = match optional_instant(name, &args, "close_at") {
                Ok(v) => v,
                Err(refused) => return refused,
            };
            let close_at_precision = match optional_string(name, &args, "close_at_precision") {
                Ok(v) => v.map(str::to_string),
                Err(refused) => return refused,
            };
            let fact_id = match optional_uuid(name, &args, "fact_id") {
                Ok(v) => v,
                Err(refused) => return refused,
            };
            let body = DecideViolationReq {
                resolution: resolution.trim().to_string(),
                close_at,
                close_at_precision,
                fact_id,
            };
            review_ops::propose_violation(state, caller, id, &body).await
        }
        ERRATA => {
            // 参数是路由的 id 与请求体的那一个布尔值。REST 上不是布尔值的请求体到不了操作
            // 那里，这里同样不执行
            let Some(id) = args["id"]
                .as_str()
                .and_then(|s| s.trim().parse::<Uuid>().ok())
            else {
                return bad_argument(name, "id", "a uuid returned by review_list");
            };
            let Some(approve) = args["approve"].as_bool() else {
                return bad_argument(name, "approve", "true or false");
            };
            review_ops::propose_errata(state, caller, id, approve).await
        }
        ALIGNMENT => {
            // `kind` 说走哪一条路由（短语或规则），其余参数是那条路由的 id 与请求体的字段。
            // 属性认不认得、方向与 marks 对不对由操作说（`unknown_property` 等，与 REST
            // 同一个 code）。类别词不走这里：它的决定直接落地，是另一个工具的事
            let Some(id) = args["id"]
                .as_str()
                .and_then(|s| s.trim().parse::<Uuid>().ok())
            else {
                return bad_argument(name, "id", "a uuid returned by review_list");
            };
            match args["kind"].as_str().map(str::trim) {
                Some("phrase") => {
                    let (property, direction, marks) = match (
                        optional_string(name, &args, "property"),
                        optional_string(name, &args, "direction"),
                        optional_string(name, &args, "marks"),
                    ) {
                        (Ok(property), Ok(direction), Ok(marks)) => (property, direction, marks),
                        (Err(refused), _, _) | (_, Err(refused), _) | (_, _, Err(refused)) => {
                            return refused
                        }
                    };
                    // 规则那条路由的 `approve` 给了也不读
                    let body = DecideAlignmentPhraseReq {
                        property: property.map(str::to_string),
                        direction: direction.map(str::to_string),
                        marks: marks.map(str::to_string),
                    };
                    review_ops::propose_alignment_phrase(state, caller, id, &body).await
                }
                Some("rule") => {
                    // REST 上没有 `approve` 的请求体同样到不了操作那里
                    let Some(approve) = args["approve"].as_bool() else {
                        return bad_argument(
                            name,
                            "approve",
                            "true or false when kind is \"rule\"",
                        );
                    };
                    review_ops::propose_alignment_rule(state, caller, id, approve).await
                }
                _ => return bad_argument(name, "kind", "\"phrase\" or \"rule\""),
            }
        }
        // 没有参数：读的就是会话在本体页上看到的那一份
        ONTOLOGY_LIST => review_ops::ontology_proposals(state, caller).await,
        ONTOLOGY => {
            // `action` 说走哪一条路由（拒绝或采纳），其余参数是那条路由的请求体的字段；
            // 递下去的请求体与 REST 上那条路由收到的同一个样子。动作认不认得、提案在不在、
            // 类认不认得都由操作说（`unknown_action`、not found、`unknown_class`）
            let (Some(section), Some(key), Some(action)) = (
                args["section"].as_str(),
                args["key"].as_str(),
                args["action"].as_str().map(str::trim),
            ) else {
                return bad_argument(name, "section", "a string, as are `key` and `action`");
            };
            let mut request = json!({ "section": section, "key": key });
            if action == "adopt" {
                // 采纳的那条路由：人改过的几格，给了才带。拒绝的理由给了也不读
                for field in ["label", "description", "datatype"] {
                    match optional_string(name, &args, field) {
                        Ok(Some(text)) => request[field] = json!(text),
                        Ok(None) => {}
                        Err(refused) => return refused,
                    }
                }
                for field in ["domains", "ranges", "parents"] {
                    match args.get(field) {
                        None | Some(Value::Null) => {}
                        Some(Value::Array(keys)) if keys.iter().all(Value::is_string) => {
                            request[field] = json!(keys);
                        }
                        Some(_) => return bad_argument(name, field, "a list of class keys"),
                    }
                }
            } else {
                // 拒绝的那条路由：`status` 只有一个值，工具替调用方写上；理由给了才带。
                // 认不出的动作也走到这里，由操作拒绝，什么都不读
                request["status"] = json!("rejected");
                match optional_string(name, &args, "reason") {
                    Ok(Some(reason)) => request["reason"] = json!(reason),
                    Ok(None) => {}
                    Err(refused) => return refused,
                }
            }
            review_ops::propose_ontology(state, caller, section, key, action, request).await
        }
        // `serves` 之外的名字到不了这里；到了也只说没有这个工具，不去碰库
        _ => {
            return ToolResult::new(
                format!("Unknown tool: {name}"),
                Step::new("tool", name, "unknown").status("failed").json(),
            )
            .error()
        }
    };
    match outcome {
        Ok(content) => success(name, content),
        Err(error) => refusal(name, error).await,
    }
}
