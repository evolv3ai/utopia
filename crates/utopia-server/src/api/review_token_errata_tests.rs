//! 勘误一档的令牌写入（0076）。
//!
//! 这一档的裁决路由还没开给令牌：编辑者的写令牌——写得最多的那一种——拿到的仍是 401，
//! 与开之前同一个请求、同一个回答。这条断言是从 `review_token_reads_tests` 里那个钉着
//! 一串 401 的测试挪过来的；把这条路由开给令牌的那一刀，用这一档的新行为换掉它。
//!
//! 夹具在 `review_token_write_support.rs`。没有 `UTOPIA_DATABASE_URL` 时跳过。
use axum::http::StatusCode;
use serde_json::json;
use uuid::Uuid;

use super::review_token_write_support::{post, Fx};

#[tokio::test]
async fn a_token_is_still_refused_on_the_errata_route() -> anyhow::Result<()> {
    let Some(f) = Fx::new(false).await? else {
        return Ok(());
    };
    let result = async {
        // 写得最多的那一种令牌：编辑者的写令牌，不限库
        let (_, token) = f.token(f.editor, "writer", "write", None).await?;
        let tail = format!("review/errata/{}", Uuid::now_v7());
        let (status, answer) = post(&f, &token, &tail, json!({ "action": "approve" })).await?;
        anyhow::ensure!(
            status == StatusCode::UNAUTHORIZED,
            "{tail}: {status} {answer}"
        );
        Ok(())
    }
    .await;
    f.cleanup().await?;
    result
}
