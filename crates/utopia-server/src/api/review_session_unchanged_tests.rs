//! 会话走 Review 的那几条路由，令牌进来之前和之后一模一样（0076，SC10 / E20）。
//!
//! `list`、`summary`、`decide` 三个处理函数把提取器从 `AuthUser` 换成了 `SessionOrToken`。
//! 这个文件钉住的是换之前的回答：重复项列表、总览、分开、合并、批量的状态码与响应形状，
//! 会话分开一对写下的台账行（没有 `token_id`），以及 Viewer 分开时那个不带 code 的 403。
//! 它先于提取器的改动写成，在改动之前和之后都得过。
//!
//! 夹具 [`Fx`] 也给 `review_token_api_tests.rs` 用：两边对着同一种库比较。
//! 没有 `UTOPIA_DATABASE_URL` 时跳过。自建自拆，只碰自己建的库。
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

pub(super) struct Fx {
    pub pool: sqlx::PgPool,
    pub app: axum::Router,
    pub org: Uuid,
    pub kb: Uuid,
    /// 同一个工作区里的另一个库：编辑者在里面，Viewer 也在里面
    pub other_kb: Uuid,
    pub editor: Uuid,
    pub viewer: Uuid,
    /// 会话 JWT
    pub editor_session: String,
    pub viewer_session: String,
    pub person_type: Uuid,
    pub company_type: Uuid,
    _dir: tempfile::TempDir,
}

impl Fx {
    pub async fn new(governance: bool) -> anyhow::Result<Option<Self>> {
        let Some(url) = utopia_store::test_db::url() else {
            return Ok(None);
        };
        let pool = sqlx::PgPool::connect(&url).await?;
        utopia_store::db::migrate(&pool).await?;
        let id = Uuid::now_v7;
        let (org, ws, kb, other_kb, editor, viewer) = (id(), id(), id(), id(), id(), id());
        let (person_type, company_type) = (id(), id());
        // 插进去的只有本地生成的 UUID 和一个布尔值
        sqlx::raw_sql(&format!(
            "INSERT INTO organizations(id,name) VALUES ('{org}','review-token-test');
             INSERT INTO workspaces(id,org_id,name) VALUES ('{ws}','{org}','review-token-test');
             INSERT INTO users(id,org_id,email,display_name,password_hash) VALUES
                 ('{editor}','{org}','{editor}@review-token.test','Editor','unused'),
                 ('{viewer}','{org}','{viewer}@review-token.test','Viewer','unused');
             INSERT INTO knowledge_bases(id,workspace_id,name,governance) VALUES
                 ('{kb}','{ws}','review','{governance}'), ('{other_kb}','{ws}','other',false);
             INSERT INTO kb_members(kb_id,user_id,role) VALUES
                 ('{kb}','{editor}','editor'), ('{kb}','{viewer}','viewer'),
                 ('{other_kb}','{editor}','editor'), ('{other_kb}','{viewer}','viewer');
             INSERT INTO entity_types(id,kb_id,key,label) VALUES
                 ('{person_type}','{kb}','person','Person'),
                 ('{company_type}','{kb}','org','Organization');"
        ))
        .execute(&pool)
        .await?;
        let dir = tempfile::tempdir()?;
        let cfg = utopia_core::config::AppConfig {
            data_dir: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        };
        let search = Arc::new(utopia_search::SearchIndex::open(
            &dir.path().join("search"),
        )?);
        let state = crate::state::AppState::new(pool.clone(), &cfg, search, "test-only".into());
        let editor_session = crate::auth::issue_token(&state, editor)?;
        let viewer_session = crate::auth::issue_token(&state, viewer)?;
        let app = super::router(state, &cfg);
        Ok(Some(Self {
            pool,
            app,
            org,
            kb,
            other_kb,
            editor,
            viewer,
            editor_session,
            viewer_session,
            person_type,
            company_type,
            _dir: dir,
        }))
    }

    pub async fn entity(&self, type_id: Uuid, name: &str) -> anyhow::Result<Uuid> {
        let id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO entities (id, kb_id, type_id, canonical_name) VALUES ($1, $2, $3, $4)",
        )
        .bind(id)
        .bind(self.kb)
        .bind(type_id)
        .bind(name)
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    /// 一对等人的重复项，直接种进 `resolution_reviews`。分数取 0.5：REAL 里是精确的
    pub async fn pair_of(&self, left: Uuid, right: Uuid) -> anyhow::Result<Uuid> {
        let id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO resolution_reviews (id, kb_id, left_id, right_id, score, reason, stage)
             VALUES ($1, $2, $3, $4, 0.5, 'namesake', 'human')",
        )
        .bind(id)
        .bind(self.kb)
        .bind(left)
        .bind(right)
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    /// 两个同类（Person）的新实体配成一对
    pub async fn pair(&self, left: &str, right: &str) -> anyhow::Result<Uuid> {
        let l = self.entity(self.person_type, left).await?;
        let r = self.entity(self.person_type, right).await?;
        self.pair_of(l, r).await
    }

    /// 两边类型不同的一对（`types=conflict` 那一档）
    pub async fn conflicting_pair(&self, left: &str, right: &str) -> anyhow::Result<Uuid> {
        let l = self.entity(self.person_type, left).await?;
        let r = self.entity(self.company_type, right).await?;
        self.pair_of(l, r).await
    }

    /// 发一枚个人令牌，回它的 id 与明文
    pub async fn token(
        &self,
        user: Uuid,
        name: &str,
        scope: &str,
        kbs: Option<&[Uuid]>,
    ) -> anyhow::Result<(Uuid, String)> {
        let (view, plain) =
            utopia_store::tokens::issue(&self.pool, user, name, scope, kbs, None).await?;
        Ok((view.id, plain))
    }

    /// `path` 接在 `/api/v1` 后面。`bearer` 是会话 JWT 或令牌明文
    pub async fn call(
        &self,
        bearer: &str,
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> anyhow::Result<(StatusCode, Value)> {
        let request = Request::builder()
            .method(method)
            .uri(format!("/api/v1{path}"))
            .header("Authorization", format!("Bearer {bearer}"));
        let request = match body {
            Some(b) => request
                .header("Content-Type", "application/json")
                .body(Body::from(b.to_string()))?,
            None => request.body(Body::empty())?,
        };
        let response = self.app.clone().oneshot(request).await?;
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(Value::Null)
        };
        Ok((status, value))
    }

    /// 审核行此刻的样子：状态、谁裁的、经哪枚令牌
    pub async fn review_row(
        &self,
        id: Uuid,
    ) -> anyhow::Result<(String, Option<Uuid>, Option<Uuid>)> {
        Ok(sqlx::query_as(
            "SELECT status, decided_by, decided_token FROM resolution_reviews WHERE id = $1",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?)
    }

    /// 这一对上某个动作的审计行：actor 与 detail
    pub async fn audits(
        &self,
        action: &str,
        review: Uuid,
    ) -> anyhow::Result<Vec<(Option<Uuid>, Value)>> {
        Ok(sqlx::query_as(
            "SELECT actor_id, detail FROM audit_events
              WHERE action = $1 AND target_kind = 'review' AND target_id = $2",
        )
        .bind(action)
        .bind(review)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn cleanup(self) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM jobs WHERE kind = 'govern' AND payload->>'kb_id' = $1")
            .bind(self.kb.to_string())
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM knowledge_bases WHERE id = ANY($1)")
            .bind([self.kb, self.other_kb])
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM organizations WHERE id = $1")
            .bind(self.org)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

/// 对象的键，排好序：比的是响应的形状，不是每个值
fn keys(v: &Value) -> Vec<String> {
    let mut k: Vec<String> = v
        .as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    k.sort();
    k
}

#[tokio::test]
async fn a_session_editor_gets_todays_duplicates_overview_keep_merge_and_batch_responses(
) -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        let kb = f.kb;
        let keep = f.pair("Zhang Wei", "Zhang Wei").await?;
        let merge = f.pair("Acme Ltd", "Acme Limited").await?;
        let batch_a = f.pair("Li Na", "Li Na").await?;
        let batch_b = f.pair("Wang Fang", "Wang Fang").await?;
        let refused = f.conflicting_pair("Mercury", "Mercury").await?;

        // 重复项列表：计数、档名、当前这一页
        let (status, body) = f
            .call(
                &f.editor_session,
                "GET",
                &format!("/kbs/{kb}/review?queue=duplicates"),
                None,
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "list: {status} {body}");
        anyhow::ensure!(
            keys(&body) == ["counts", "items", "queue"],
            "list body: {body}"
        );
        anyhow::ensure!(body["queue"] == "duplicates", "{body}");
        anyhow::ensure!(body["counts"]["duplicates"] == 5, "{body}");
        anyhow::ensure!(body["counts"]["duplicates_same_type"] == 4, "{body}");
        anyhow::ensure!(body["counts"]["duplicates_type_conflict"] == 1, "{body}");
        let items = body["items"].as_array().cloned().unwrap_or_default();
        // 最新的在前
        let ids: Vec<String> = items
            .iter()
            .map(|i| i["id"].as_str().unwrap_or_default().to_string())
            .collect();
        let expected: Vec<String> = [refused, batch_b, batch_a, merge, keep]
            .iter()
            .map(Uuid::to_string)
            .collect();
        anyhow::ensure!(ids == expected, "list order: {ids:?}");
        let first = &items[4];
        anyhow::ensure!(
            first["left"]["name"] == "Zhang Wei" && first["right"]["name"] == "Zhang Wei",
            "{first}"
        );
        anyhow::ensure!(
            first["score"] == 0.5 && first["stage"] == "human",
            "{first}"
        );
        // 缺省就是 duplicates
        let (status, default) = f
            .call(&f.editor_session, "GET", &format!("/kbs/{kb}/review"), None)
            .await?;
        anyhow::ensure!(status == StatusCode::OK && default == body, "{default}");
        // 分档与翻页
        let (status, page) = f
            .call(
                &f.editor_session,
                "GET",
                &format!("/kbs/{kb}/review?queue=duplicates&types=conflict&limit=1&offset=0"),
                None,
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "{status} {page}");
        anyhow::ensure!(
            page["items"].as_array().map(Vec::len) == Some(1)
                && page["items"][0]["id"] == refused.to_string(),
            "{page}"
        );
        // 别的档会话照旧拿得到；认不出的档名是 422 unknown_queue
        for queue in ["pending", "agent", "merges"] {
            let (status, other) = f
                .call(
                    &f.editor_session,
                    "GET",
                    &format!("/kbs/{kb}/review?queue={queue}"),
                    None,
                )
                .await?;
            anyhow::ensure!(
                status == StatusCode::OK && other["queue"] == queue && other["items"].is_array(),
                "{queue}: {status} {other}"
            );
        }
        let (status, unknown) = f
            .call(
                &f.editor_session,
                "GET",
                &format!("/kbs/{kb}/review?queue=nope"),
                None,
            )
            .await?;
        anyhow::ensure!(
            status == StatusCode::UNPROCESSABLE_ENTITY && unknown["code"] == "unknown_queue",
            "{status} {unknown}"
        );

        // 总览
        let (status, summary) = f
            .call(
                &f.editor_session,
                "GET",
                &format!("/kbs/{kb}/review/summary"),
                None,
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "summary: {status} {summary}");
        anyhow::ensure!(
            keys(&summary) == ["agent", "decided", "health", "waiting"],
            "summary body: {summary}"
        );
        // Viewer 也读得到这两样
        for path in [
            format!("/kbs/{kb}/review?queue=duplicates"),
            format!("/kbs/{kb}/review/summary"),
        ] {
            let (status, _) = f.call(&f.viewer_session, "GET", &path, None).await?;
            anyhow::ensure!(status == StatusCode::OK, "viewer {path}: {status}");
        }

        // Viewer 分开一对：不带 code 的 403，什么都没动
        let (status, body) = f
            .call(
                &f.viewer_session,
                "POST",
                &format!("/kbs/{kb}/review/{refused}"),
                Some(json!({ "action": "keep" })),
            )
            .await?;
        anyhow::ensure!(status == StatusCode::FORBIDDEN, "viewer keep: {status}");
        anyhow::ensure!(keys(&body) == ["error"], "viewer keep body: {body}");
        anyhow::ensure!(f.review_row(refused).await?.0 == "pending");

        // 分开
        let (status, body) = f
            .call(
                &f.editor_session,
                "POST",
                &format!("/kbs/{kb}/review/{keep}"),
                Some(json!({ "action": "keep", "rationale": "different people" })),
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "keep: {status} {body}");
        anyhow::ensure!(body == json!({ "ok": true }), "keep body: {body}");
        anyhow::ensure!(
            f.review_row(keep).await? == ("kept".to_string(), Some(f.editor), None),
            "a session keep is decided by the person and carries no token"
        );
        let audits = f.audits("review.keep", keep).await?;
        anyhow::ensure!(audits.len() == 1, "one audit row: {audits:?}");
        anyhow::ensure!(audits[0].0 == Some(f.editor), "{audits:?}");
        anyhow::ensure!(
            audits[0].1
                == json!({ "left": "Zhang Wei", "right": "Zhang Wei", "score": 0.5,
                           "why": "different people" }),
            "a session keep's audit detail has no token_id: {}",
            audits[0].1
        );
        // 再裁一次：这一对已经不在 pending 里，404
        let (status, _) = f
            .call(
                &f.editor_session,
                "POST",
                &format!("/kbs/{kb}/review/{keep}"),
                Some(json!({ "action": "keep" })),
            )
            .await?;
        anyhow::ensure!(status == StatusCode::NOT_FOUND, "repeat keep: {status}");

        // 合并：不过闸门，直接落地
        let (status, body) = f
            .call(
                &f.editor_session,
                "POST",
                &format!("/kbs/{kb}/review/{merge}"),
                Some(json!({ "action": "merge" })),
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "merge: {status} {body}");
        anyhow::ensure!(body == json!({ "ok": true }), "merge body: {body}");
        anyhow::ensure!(f.review_row(merge).await?.0 == "merged");
        let merges: i64 = sqlx::query_scalar("SELECT count(*) FROM entity_merges WHERE kb_id = $1")
            .bind(kb)
            .fetch_one(&f.pool)
            .await?;
        anyhow::ensure!(merges == 1, "one merge row, got {merges}");
        anyhow::ensure!(f.audits("review.merge", merge).await?.len() == 1);

        // 批量
        let (status, body) = f
            .call(
                &f.editor_session,
                "POST",
                &format!("/kbs/{kb}/review/batch"),
                Some(json!({ "ids": [batch_a, batch_b], "action": "keep" })),
            )
            .await?;
        anyhow::ensure!(status == StatusCode::OK, "batch: {status} {body}");
        anyhow::ensure!(
            body == json!({ "decided": 2, "outcomes": [
                { "id": batch_a, "error": null }, { "id": batch_b, "error": null } ] }),
            "batch body: {body}"
        );
        for id in [batch_a, batch_b] {
            anyhow::ensure!(f.review_row(id).await? == ("kept".to_string(), Some(f.editor), None));
            let audits = f.audits("review.keep", id).await?;
            anyhow::ensure!(
                audits.len() == 1 && audits[0].1.get("token_id").is_none(),
                "{audits:?}"
            );
        }

        // 人的裁决不进令牌 agent 的台账
        let ledger = utopia_store::token_agent_actions::count(&f.pool, kb).await?;
        anyhow::ensure!(ledger == 0, "no ledger row for a session, got {ledger}");
        // 只剩 Viewer 没动成的那一对
        let (_, body) = f
            .call(
                &f.editor_session,
                "GET",
                &format!("/kbs/{kb}/review?queue=duplicates"),
                None,
            )
            .await?;
        anyhow::ensure!(
            body["counts"]["duplicates"] == 1 && body["items"][0]["id"] == refused.to_string(),
            "{body}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}
