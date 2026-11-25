//! 成批清空类型的路由（#9）：`/entities/clear-type` 不被 `/entities/{entity_id}` 吞掉，
//! 缺省是试跑，正式跑才写。没有 `UTOPIA_DATABASE_URL` 时跳过。
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn a_batch_clear_is_a_dry_run_unless_asked() -> anyhow::Result<()> {
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let pool = sqlx::PgPool::connect(&url).await?;
    utopia_store::db::migrate(&pool).await?;
    let [org, ws, kb, editor, concept, a, b] = std::array::from_fn(|_| Uuid::now_v7());
    // Only locally generated UUIDs are interpolated into fixture SQL.
    sqlx::raw_sql(&format!(
        "INSERT INTO organizations(id,name) VALUES ('{org}','clear-type-route');
         INSERT INTO workspaces(id,org_id,name) VALUES ('{ws}','{org}','clear-type-route');
         INSERT INTO users(id,org_id,email,display_name,password_hash)
              VALUES ('{editor}','{org}','{editor}@clear.test','editor','unused');
         INSERT INTO knowledge_bases(id,workspace_id,name) VALUES ('{kb}','{ws}','clear');
         INSERT INTO kb_members(kb_id,user_id,role) VALUES ('{kb}','{editor}','editor');
         INSERT INTO entity_types(id,kb_id,key,label) VALUES ('{concept}','{kb}','concept','Concept');
         INSERT INTO entities(id,kb_id,canonical_name,type_id,type_source)
              VALUES ('{a}','{kb}','Alpha','{concept}','extracted'),
                     ('{b}','{kb}','Beta','{concept}','extracted');"
    ))
    .execute(&pool)
    .await?;
    let result = async {
        let dir = tempfile::tempdir()?;
        let config = utopia_core::config::AppConfig {
            data_dir: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        };
        let search = Arc::new(utopia_search::SearchIndex::open(&dir.path().join("search"))?);
        let state = crate::state::AppState::new(pool.clone(), &config, search, "test-only".into());
        let token = crate::auth::issue_token(&state, editor)?;
        let app = crate::api::router(state, &config);
        let post = |body: Value| {
            let app = app.clone();
            let token = token.clone();
            async move {
                let response = app
                    .oneshot(
                        Request::builder()
                            .method("POST")
                            .uri(format!("/api/v1/kbs/{kb}/entities/clear-type"))
                            .header("content-type", "application/json")
                            .header("authorization", format!("Bearer {token}"))
                            .body(Body::from(body.to_string()))?,
                    )
                    .await?;
                let status = response.status();
                let bytes = to_bytes(response.into_body(), usize::MAX).await?;
                anyhow::Ok((status, serde_json::from_slice::<Value>(&bytes)?))
            }
        };
        let typed = || async {
            let n: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM entities WHERE kb_id = $1 AND type_id IS NOT NULL",
            )
            .bind(kb)
            .fetch_one(&pool)
            .await?;
            anyhow::Ok(n)
        };

        let (status, body) = post(json!({"entity_ids": [a, b], "expect_type_id": concept})).await?;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(
            (&body["dry_run"], &body["would_clear"], &body["cleared"]),
            (&json!(true), &json!(2), &json!(0))
        );
        assert_eq!(body["entities"][0]["type_key"], json!("concept"));
        assert_eq!(typed().await?, 2, "no dry_run field means a dry run");

        let (status, body) = post(
            json!({"entity_ids": [a, b, Uuid::now_v7()], "expect_type_id": concept, "dry_run": false}),
        )
        .await?;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(typed().await?, 2, "a refused batch changes nothing");

        let (status, body) =
            post(json!({"entity_ids": [a, b], "expect_type_id": concept, "dry_run": false})).await?;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["cleared"], json!(2));
        assert_eq!(typed().await?, 0);
        anyhow::Ok(())
    }
    .await;
    sqlx::query("DELETE FROM knowledge_bases WHERE id = $1")
        .bind(kb)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM organizations WHERE id = $1")
        .bind(org)
        .execute(&pool)
        .await?;
    result
}
