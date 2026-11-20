//! 类别词的令牌决定，以及三个直接落地的 MCP 工具（0076）。
//!
//! 类别词的路由从前就对写令牌直接落地（0014 决定 6）。这一刀**不收窄它**（E16、N8）：状态、
//! 回答、`unknown_class`、`alignment_busy`、主人不够格时那句不带 code 的 403 都同从前；
//! 多出来的只有两样——台账里一行 `applied`，审计 detail 里令牌的名字与台账行的 id。
//!
//! - **回答同从前，多一行台账**（SC4、E16）：Agent 一档那一行说得出是哪个词；会话的决定
//!   不写台账，审计行同从前
//! - **从不被限速挡住，但数进总数**（E16）：额度用完的令牌照样定得了类别词；它的行算进
//!   那 200 笔里，别的落地写入因此被拒
//! - **三个直接落地的工具**（AC6）：`review_mapping`、`review_check`、`review_kind_word` 回的
//!   是 REST 的 body，只列给编辑者的写令牌，被拒绝的调用方拿到带 code 的工具错误
//!
//! 夹具在 `review_token_write_support.rs`。没有 `UTOPIA_DATABASE_URL` 时跳过。
use axum::http::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;

use super::review_token_write_support::{
    audits, ensure_tool_refused, keys, ledger, post, seeded, tool_call, tools_list, Audit, Fx,
};

/// 种子里那个还没人定的类别词，与它在路径里的写法
const WORD: &str = "seed startup";
const TAIL: &str = "review/alignment/kind-words/seed%20startup";

/// 类别词绑定行此刻的样子：状态、类的键、谁定的
async fn binding(f: &Fx) -> anyhow::Result<(String, Option<String>, Option<String>)> {
    Ok(sqlx::query_as(
        "SELECT b.status, t.key, b.decided_by
           FROM type_bindings b LEFT JOIN entity_types t ON t.id = b.type_id
          WHERE b.kb_id = $1 AND b.kind_word = $2",
    )
    .bind(f.kb)
    .bind(WORD)
    .fetch_one(&f.pool)
    .await?)
}

/// 台账里类别词决定的行，最早的在前
async fn kind_word_rows(
    f: &Fx,
) -> anyhow::Result<Vec<utopia_store::token_agent_actions::TokenAgentAction>> {
    Ok(ledger(f)
        .await?
        .into_iter()
        .filter(|r| r.operation == "alignment.kind_word")
        .collect())
}

/// `alignment.kind_word_decided` 的审计行，最早的在前
async fn decided(f: &Fx) -> anyhow::Result<Vec<Audit>> {
    Ok(audits(f)
        .await?
        .into_iter()
        .filter(|a| a.action == "alignment.kind_word_decided")
        .collect())
}

/// 直接往台账里种 `n` 行这枚令牌在这个库里刚落地的写入：限速数的就是这些行
async fn seed_applied(f: &Fx, token_id: Uuid, n: i32) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO token_agent_actions
             (id, kb_id, operation, target_kind, target_id, owner_id, token_id, token_name,
              outcome, payload, target_key, created_at)
         SELECT gen_random_uuid(), $1, 'review.keep', 'review', gen_random_uuid(), $2, $3,
                'seeded', 'applied', '{}'::jsonb, gen_random_uuid()::text,
                now() - make_interval(mins => 5)
           FROM generate_series(1, $4)",
    )
    .bind(f.kb)
    .bind(f.editor)
    .bind(token_id)
    .bind(n)
    .execute(&f.pool)
    .await?;
    Ok(())
}

/// 这枚令牌在这个库里落地（`applied`）的行数
async fn applied_rows(f: &Fx, token_id: Uuid) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "SELECT count(*) FROM token_agent_actions
          WHERE kb_id = $1 AND token_id = $2 AND outcome = 'applied'",
    )
    .bind(f.kb)
    .bind(token_id)
    .fetch_one(&f.pool)
    .await?)
}

/// Agent 一档里 id 是这一个的那一行（会话读）
async fn history_row(f: &Fx, id: Uuid) -> anyhow::Result<Value> {
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
        .and_then(|items| items.iter().find(|i| i["id"] == json!(id)))
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("{id} is not in Agent History: {history}"))
}

#[tokio::test]
async fn a_token_kind_word_decision_keeps_its_answers_and_gains_one_applied_row(
) -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (token_id, token) = f.token(f.editor, "kind agent", "write", None).await?;
        anyhow::ensure!(binding(&f).await?.0 == "undecided");

        // 从前就有的三种拒绝，一个字不差：只读令牌、够不着这个库的令牌带 code；主人不够格
        // 是那句**不带 code** 的 403
        let (_, reader) = f.token(f.editor, "reader", "read", None).await?;
        let (_, elsewhere) = f
            .token(f.editor, "elsewhere", "write", Some(&[f.other_kb]))
            .await?;
        let (_, below) = f.token(f.viewer, "viewer writer", "write", None).await?;
        let (status, body) = post(&f, &reader, TAIL, json!({ "class": "org" })).await?;
        anyhow::ensure!(
            status == StatusCode::FORBIDDEN && body["code"] == "token_read_only",
            "{status} {body}"
        );
        let (status, body) = post(&f, &elsewhere, TAIL, json!({ "class": "org" })).await?;
        anyhow::ensure!(
            status == StatusCode::FORBIDDEN && body["code"] == "token_kb_out_of_scope",
            "{status} {body}"
        );
        let below_token = post(&f, &below, TAIL, json!({ "class": "org" })).await?;
        let below_session = post(&f, &f.viewer_session, TAIL, json!({ "class": "org" })).await?;
        anyhow::ensure!(
            below_token.0 == StatusCode::FORBIDDEN
                && below_token.1.get("code").is_none()
                && below_token == below_session,
            "a below-editor owner gets the plain 403: {below_token:?} {below_session:?}"
        );
        anyhow::ensure!(binding(&f).await?.0 == "undecided");
        anyhow::ensure!(kind_word_rows(&f).await?.is_empty() && decided(&f).await?.is_empty());

        // 认不出的类：与会话同一个状态、同一个 body，一行不写
        let by_session = post(&f, &f.editor_session, TAIL, json!({ "class": "nope" })).await?;
        let by_token = post(&f, &token, TAIL, json!({ "class": "nope" })).await?;
        anyhow::ensure!(
            by_token == by_session && by_token.1["code"] == "unknown_class",
            "{by_token:?} {by_session:?}"
        );
        anyhow::ensure!(kind_word_rows(&f).await?.is_empty());

        // 另一个操作正锁着这个类别词：`alignment_busy`，一行不写
        let mut gate = f.pool.begin().await?;
        sqlx::query("SELECT id FROM type_bindings WHERE kb_id = $1 AND kind_word = $2 FOR UPDATE")
            .bind(f.kb)
            .bind(WORD)
            .fetch_one(&mut *gate)
            .await?;
        let busy = post(&f, &token, TAIL, json!({ "class": "org" })).await;
        gate.rollback().await?;
        let (status, body) = busy?;
        anyhow::ensure!(
            status == StatusCode::CONFLICT && body["code"] == "alignment_busy",
            "{status} {body}"
        );
        anyhow::ensure!(binding(&f).await?.0 == "undecided");
        anyhow::ensure!(
            kind_word_rows(&f).await?.is_empty() && decided(&f).await?.is_empty(),
            "a refused decision writes no ledger row and no audit row"
        );

        // 落地：状态与回答同从前
        let (status, body) = post(&f, &token, TAIL, json!({ "class": "org" })).await?;
        anyhow::ensure!(
            status == StatusCode::OK && body == json!({ "ok": true }),
            "{status} {body}"
        );
        anyhow::ensure!(
            binding(&f).await?
                == (
                    "bound".to_string(),
                    Some("org".to_string()),
                    Some("person".to_string())
                ),
            "the kind word is bound as a person's decision"
        );

        // 多出来的第一样：台账里正好一行 applied
        let rows = kind_word_rows(&f).await?;
        anyhow::ensure!(rows.len() == 1, "{rows:?}");
        let applied = &rows[0];
        anyhow::ensure!(
            applied.outcome == "applied"
                && applied.target_kind == "type_binding"
                && applied.target_id == f.kb
                && applied.gate_reason.is_none()
                && applied.owner_id == f.editor
                && applied.token_id == token_id
                && applied.token_name == "kind agent",
            "{applied:?}"
        );
        anyhow::ensure!(
            applied.payload
                == json!({
                    "request": { "kind_word": WORD, "class": "org" },
                    "target_label": WORD,
                    "action": "bind",
                }),
            "{}",
            applied.payload
        );

        // 多出来的第二样：审计 detail 里令牌的名字与台账行的 id；别的键同从前
        let audited = decided(&f).await?;
        anyhow::ensure!(audited.len() == 1, "{audited:?}");
        anyhow::ensure!(
            audited[0].actor == Some(f.editor)
                && audited[0].target_kind == "type_binding"
                && audited[0].target_id.is_none()
                && audited[0].detail
                    == json!({
                        "kind_word": WORD,
                        "class": "org",
                        "token_id": token_id,
                        "token_name": "kind agent",
                        "action_id": applied.id,
                    }),
            "{audited:?}"
        );

        // Agent 一档：这一行的目标就是那个词，不是空的，读得出「主人经哪一枚令牌」
        let shown = history_row(&f, applied.id).await?;
        anyhow::ensure!(
            shown["target_label"]
                .as_str()
                .is_some_and(|label| !label.trim().is_empty() && label == WORD),
            "the Agent History row names the kind word: {shown}"
        );
        anyhow::ensure!(
            shown["source"] == "token"
                && shown["operation"] == "alignment.kind_word"
                && shown["outcome"] == "applied"
                && shown["action"] == "bind"
                && shown["owner_name"] == "Editor"
                && shown["token_name"] == "kind agent",
            "{shown}"
        );

        // 会话的决定同从前：同一个回答，不写台账，审计 detail 里只有词与类
        let (status, body) = post(&f, &f.editor_session, TAIL, json!({ "class": null })).await?;
        anyhow::ensure!(
            status == StatusCode::OK && body == json!({ "ok": true }),
            "{status} {body}"
        );
        anyhow::ensure!(binding(&f).await?.0 == "none");
        anyhow::ensure!(
            kind_word_rows(&f).await?.len() == 1,
            "a session kind-word decision writes no ledger row"
        );
        let audited = decided(&f).await?;
        anyhow::ensure!(
            audited.len() == 2
                && audited[1].actor == Some(f.editor)
                && audited[1].detail == json!({ "kind_word": WORD, "class": null }),
            "{audited:?}"
        );

        // 令牌再定一次「没有类对得上」：类别词没有「定了」这一态，照样落地，再多一行
        let (status, body) = post(&f, &token, TAIL, json!({})).await?;
        anyhow::ensure!(
            status == StatusCode::OK && body == json!({ "ok": true }),
            "{status} {body}"
        );
        let rows = kind_word_rows(&f).await?;
        anyhow::ensure!(
            rows.len() == 2
                && rows[1].payload["action"] == "none"
                && rows[1].payload["target_label"] == WORD,
            "{rows:?}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn a_kind_word_decision_is_never_refused_by_the_limit_and_counts_toward_it(
) -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (token_id, token) = f.token(f.editor, "kind agent", "write", None).await?;
        let mapping: Uuid = sqlx::query_scalar("SELECT id FROM concept_mappings WHERE kb_id = $1")
            .bind(f.kb)
            .fetch_one(&f.pool)
            .await?;
        let mapping_tail = format!("review/mappings/{mapping}");

        // 这一小时里已经落地 199 笔：还剩一笔的额度。类别词的决定落地，数成第 200 笔
        seed_applied(&f, token_id, 199).await?;
        let (status, body) = post(&f, &token, TAIL, json!({ "class": "org" })).await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {body}");
        anyhow::ensure!(
            applied_rows(&f, token_id).await? == 200,
            "the kind-word row counts toward the hourly total"
        );

        // 所以别的落地写入现在被拒：没有类别词那一行，这一次映射决定本该是第 200 笔
        let (status, body) =
            post(&f, &token, &mapping_tail, json!({ "status": "confirmed" })).await?;
        anyhow::ensure!(
            status == StatusCode::TOO_MANY_REQUESTS && body["code"] == "agent_rate_limited",
            "{status} {body}"
        );
        anyhow::ensure!(applied_rows(&f, token_id).await? == 200);

        // 类别词自己从不被限速挡住：额度用完了，REST 与工具照样落地，各多一行
        let (status, body) = post(&f, &token, TAIL, json!({ "class": null })).await?;
        anyhow::ensure!(
            status == StatusCode::OK && body == json!({ "ok": true }),
            "past the limit: {status} {body}"
        );
        let by_tool = tool_call(
            &f,
            &token,
            "review_kind_word",
            json!({ "kind_word": WORD, "class": "org" }),
        )
        .await?;
        anyhow::ensure!(
            by_tool["isError"] == false && by_tool["structuredContent"] == json!({ "ok": true }),
            "past the limit: {by_tool}"
        );
        anyhow::ensure!(applied_rows(&f, token_id).await? == 202);
        let rows = kind_word_rows(&f).await?;
        anyhow::ensure!(
            rows.len() == 3 && rows.iter().all(|r| r.outcome == "applied"),
            "{rows:?}"
        );
        anyhow::ensure!(decided(&f).await?.len() == 3, "one audit row per decision");

        // 映射仍被拒：限速没有因为类别词放行而松开
        let by_tool = tool_call(
            &f,
            &token,
            "review_mapping",
            json!({ "id": mapping, "status": "confirmed" }),
        )
        .await?;
        ensure_tool_refused(&by_tool, "agent_rate_limited")?;
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}

#[tokio::test]
async fn the_mcp_finalize_tools_answer_the_rest_bodies_and_refuse_with_the_rest_codes(
) -> anyhow::Result<()> {
    let Some(f) = seeded().await? else {
        return Ok(());
    };
    let result = async {
        let (_, viewer_read) = f.token(f.viewer, "reader", "read", None).await?;
        let (_, editor_read) = f.token(f.editor, "editor reader", "read", None).await?;
        let (_, viewer_write) = f.token(f.viewer, "viewer writer", "write", None).await?;
        let (token_id, token) = f.token(f.editor, "finalizer", "write", None).await?;
        let mapping: Uuid = sqlx::query_scalar("SELECT id FROM concept_mappings WHERE kb_id = $1")
            .bind(f.kb)
            .fetch_one(&f.pool)
            .await?;
        sqlx::query("UPDATE knowledge_bases SET materialize_inferences = true WHERE id = $1")
            .bind(f.kb)
            .execute(&f.pool)
            .await?;
        let finalize = ["review_check", "review_kind_word", "review_mapping"];

        // 列给谁：三个直接落地的工具只列给编辑者的写令牌；读映射的工具人人都列
        let names = |tools: &[Value]| -> Vec<String> {
            let mut names: Vec<String> = tools
                .iter()
                .filter_map(|t| t["name"].as_str())
                .filter(|n| finalize.contains(n) || *n == "review_mapping_list")
                .map(str::to_string)
                .collect();
            names.sort();
            names
        };
        for (who, bearer) in [
            ("a viewer's read token", &viewer_read),
            ("an editor's read token", &editor_read),
            ("a viewer's write token", &viewer_write),
        ] {
            let tools = tools_list(&f, bearer).await?;
            anyhow::ensure!(
                names(&tools) == ["review_mapping_list"],
                "{who}: {:?}",
                names(&tools)
            );
        }
        let tools = tools_list(&f, &token).await?;
        anyhow::ensure!(
            names(&tools)
                == [
                    "review_check",
                    "review_kind_word",
                    "review_mapping",
                    "review_mapping_list"
                ],
            "{:?}",
            names(&tools)
        );
        let find = |name: &str| {
            tools
                .iter()
                .find(|t| t["name"] == name)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("{name} is not listed"))
        };
        // 标注：直接落地的三个会改东西；读的那个只读
        for name in finalize {
            let tool = find(name)?;
            anyhow::ensure!(
                tool["annotations"]
                    == json!({
                        "readOnlyHint": false, "destructiveHint": true, "idempotentHint": false,
                    }),
                "{tool}"
            );
            // 说明里写着它当场落地、记在主人与令牌名下，而不是留一条提议
            let said = tool["description"].as_str().unwrap_or_default();
            anyhow::ensure!(
                said.contains("This call finalizes")
                    && said.contains("the token's owner and the token's name")
                    && !said.starts_with("Propose that a person"),
                "{name}: {said}"
            );
        }
        let list = find("review_mapping_list")?;
        anyhow::ensure!(
            list["annotations"] == json!({ "readOnlyHint": true }),
            "{list}"
        );
        // 参数就是路由的 id 与请求体的字段
        anyhow::ensure!(
            keys(&list["inputSchema"]["properties"]) == ["limit", "offset", "q", "status"]
                && list["inputSchema"].get("required").is_none(),
            "{list}"
        );
        let schema = find("review_mapping")?["inputSchema"].clone();
        anyhow::ensure!(
            keys(&schema["properties"]) == ["id", "status"]
                && schema["required"] == json!(["id", "status"])
                && schema["properties"]["id"]["format"] == "uuid",
            "{schema}"
        );
        let schema = find("review_check")?["inputSchema"].clone();
        anyhow::ensure!(
            keys(&schema["properties"]) == ["run"] && schema["required"] == json!(["run"]),
            "{schema}"
        );
        let schema = find("review_kind_word")?["inputSchema"].clone();
        anyhow::ensure!(
            keys(&schema["properties"]) == ["class", "kind_word"]
                && schema["required"] == json!(["kind_word"]),
            "{schema}"
        );
        // 名字的规矩没破：列出来的名字里没有这四个词
        for tool in &tools {
            let name = tool["name"].as_str().unwrap_or_default();
            for word in ["proposal", "approve", "decline", "answer"] {
                anyhow::ensure!(!name.contains(word), "{name} contains {word}");
            }
        }

        // 没被列出来的调用方照样调得到工具，拿到的是带 REST 那个 code 的工具错误，不是
        // 「没有这个工具」；什么都不改、一行不写
        let rows_before = ledger(&f).await?.len();
        let calls = [
            (
                "review_mapping",
                json!({ "id": mapping, "status": "confirmed" }),
            ),
            ("review_check", json!({ "run": "consistency" })),
            ("review_check", json!({ "run": "inference" })),
            (
                "review_kind_word",
                json!({ "kind_word": WORD, "class": "org" }),
            ),
        ];
        for (bearer, code) in [
            (&viewer_read, "token_read_only"),
            (&editor_read, "token_read_only"),
            (&viewer_write, "token_owner_below_editor"),
        ] {
            for (name, args) in &calls {
                let refused = tool_call(&f, bearer, name, args.clone()).await?;
                ensure_tool_refused(&refused, code)?;
            }
        }
        anyhow::ensure!(
            ledger(&f).await?.len() == rows_before,
            "a refused caller writes no ledger row"
        );
        let status: String =
            sqlx::query_scalar("SELECT status FROM concept_mappings WHERE id = $1")
                .bind(mapping)
                .fetch_one(&f.pool)
                .await?;
        anyhow::ensure!(
            status == "proposed" && binding(&f).await?.0 == "undecided",
            "a refused caller changes nothing"
        );
        anyhow::ensure!(
            !audits(&f).await?.iter().any(|a| matches!(
                a.action.as_str(),
                "mapping.decided"
                    | "consistency.checked"
                    | "inference.materialized"
                    | "alignment.kind_word_decided"
            )),
            "a refused caller writes no decision audit row"
        );

        // 编辑者的写令牌：每个工具回的是 REST 的那个 body
        // 映射：工具定成 confirmed，REST 再定回 rejected——两个回答相等
        let by_tool = tool_call(&f, &token, "review_mapping", calls[0].1.clone()).await?;
        let (status, by_rest) = post(
            &f,
            &token,
            &format!("review/mappings/{mapping}"),
            json!({ "status": "rejected" }),
        )
        .await?;
        anyhow::ensure!(
            status == StatusCode::OK
                && by_tool["isError"] == false
                && by_tool["structuredContent"] == by_rest,
            "review_mapping: {by_tool} {by_rest}"
        );
        // 两种运行：先各跑一遍让库稳定下来，之后 REST 的报告与工具的报告相等
        for (run, tail) in [
            ("consistency", "consistency/check"),
            ("inference", "inference/run"),
        ] {
            let (status, first) = post(&f, &token, tail, json!({})).await?;
            anyhow::ensure!(status == StatusCode::OK, "{tail}: {status} {first}");
            let (status, by_rest) = post(&f, &token, tail, json!({})).await?;
            anyhow::ensure!(status == StatusCode::OK, "{tail}: {status} {by_rest}");
            let by_tool = tool_call(&f, &token, "review_check", json!({ "run": run })).await?;
            anyhow::ensure!(
                by_tool["isError"] == false && by_tool["structuredContent"] == by_rest,
                "review_check {run}: {by_tool} {by_rest}"
            );
        }
        // 类别词：工具绑上，REST 解开——两个回答相等；认不出的类是 REST 的那个 code
        let by_tool = tool_call(&f, &token, "review_kind_word", calls[3].1.clone()).await?;
        anyhow::ensure!(binding(&f).await?.0 == "bound");
        let (status, by_rest) = post(&f, &token, TAIL, json!({})).await?;
        anyhow::ensure!(
            status == StatusCode::OK
                && by_tool["isError"] == false
                && by_tool["structuredContent"] == by_rest,
            "review_kind_word: {by_tool} {by_rest}"
        );
        let refused = tool_call(
            &f,
            &token,
            "review_kind_word",
            json!({ "kind_word": WORD, "class": "nope" }),
        )
        .await?;
        ensure_tool_refused(&refused, "unknown_class")?;

        // 每一次落地都在台账里（Agent 一档读它）与审计里（Activity 读它），都带着主人与
        // 令牌的名字：映射 2 次、检查 3 次、推导 3 次、类别词 2 次
        let rows = ledger(&f).await?;
        let mine: Vec<_> = rows.iter().filter(|r| r.token_id == token_id).collect();
        let count = |operation: &str| mine.iter().filter(|r| r.operation == operation).count();
        anyhow::ensure!(
            mine.len() == 10
                && count("mapping.decide") == 2
                && count("consistency.check") == 3
                && count("inference.run") == 3
                && count("alignment.kind_word") == 2
                && mine
                    .iter()
                    .all(|r| r.outcome == "applied" && r.token_name == "finalizer"),
            "{mine:?}"
        );
        let all = audits(&f).await?;
        for row in &mine {
            let shown = history_row(&f, row.id).await?;
            anyhow::ensure!(
                shown["source"] == "token"
                    && shown["owner_name"] == "Editor"
                    && shown["token_name"] == "finalizer"
                    && shown["outcome"] == "applied"
                    && shown["target_label"]
                        .as_str()
                        .is_some_and(|label| !label.trim().is_empty()),
                "{shown}"
            );
            let activity: Vec<&Audit> = all
                .iter()
                .filter(|a| a.detail["action_id"] == json!(row.id))
                .collect();
            anyhow::ensure!(
                activity.len() == 1
                    && activity[0].actor == Some(f.editor)
                    && activity[0].detail["token_name"] == "finalizer"
                    && activity[0].detail["token_id"] == json!(token_id),
                "one Activity row names the owner and the token for {}: {activity:?}",
                row.operation
            );
        }
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}
