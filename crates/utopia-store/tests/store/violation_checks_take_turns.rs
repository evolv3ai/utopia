//! 同一个库的违规对账一次只跑一个（#18）。
//!
//! `run` 与改判据时的 `detection_keys` → `reconcile_ontology` 都是「检出 → 删掉这一轮
//! 没检出的 open 行」。两条并发时，一条在另一条检出与清陈之间提交的新违规会被当成
//! 陈行无声删掉——生产上两条同时设 domain/range 的 PATCH 丢掉了 450 条签名违规。
//! 这里守：持有库锁时，`run` 与改判据的对账都得等；别的库不受牵连；放锁后都跑完。
//!
//! 没有 `UTOPIA_DATABASE_URL` 时跳过而不是失败。自建自拆，绝不碰已有的库。

use sqlx::PgPool;
use std::collections::HashSet;
use std::time::Duration;
use utopia_store::reasoning;
use uuid::Uuid;

async fn blocked_by(pool: &PgPool, blocker: i32) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "WITH RECURSIVE blocked(pid) AS (
            SELECT pid FROM pg_stat_activity WHERE $1 = ANY(pg_blocking_pids(pid))
            UNION SELECT p.pid FROM pg_stat_activity p JOIN blocked b ON b.pid = ANY(pg_blocking_pids(p.pid))
         ) SELECT count(*) FROM blocked",
    )
    .bind(blocker)
    .fetch_one(pool)
    .await?)
}

async fn wait_for(pool: &PgPool, blocker: i32, count: i64) -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_secs(10), async {
        while blocked_by(pool, blocker).await? < count {
            tokio::task::yield_now().await;
        }
        anyhow::Ok(())
    })
    .await
    .map_err(|_| anyhow::anyhow!("expected {count} sessions waiting on the violation lock"))??;
    Ok(())
}

#[tokio::test]
async fn violation_checks_on_one_base_take_turns() -> anyhow::Result<()> {
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let control = PgPool::connect(&url).await?;
    let pool = PgPool::connect(&url).await?;
    let (org, ws, kb, other) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, 'violation-lock')")
        .bind(org)
        .execute(&control)
        .await?;
    sqlx::query("INSERT INTO workspaces (id, org_id, name) VALUES ($1, $2, 'violation-lock')")
        .bind(ws)
        .bind(org)
        .execute(&control)
        .await?;
    for id in [kb, other] {
        sqlx::query(
            "INSERT INTO knowledge_bases (id, workspace_id, name) VALUES ($1, $2, 'violation-lock')",
        )
        .bind(id)
        .bind(ws)
        .execute(&control)
        .await?;
    }

    let run = async {
        let mut gate = control.begin().await?;
        let blocker: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *gate)
            .await?;
        reasoning::lock_violations(&mut gate, kb).await?;

        // 1. 推理检查等着
        let p = pool.clone();
        let check = tokio::spawn(async move { reasoning::run(&p, kb).await });
        wait_for(&control, blocker, 1).await?;

        // 2. 改判据的对账连记基线都得等
        let p = pool.clone();
        let edit = tokio::spawn(async move {
            let mut tx = p.begin().await?;
            let was = reasoning::detection_keys(&mut tx, kb).await?;
            reasoning::reconcile_ontology(&mut tx, kb, &was).await?;
            tx.commit().await?;
            anyhow::Ok(())
        });
        wait_for(&control, blocker, 2).await?;

        // 3. 别的库不受牵连
        tokio::time::timeout(Duration::from_secs(10), reasoning::run(&pool, other))
            .await
            .map_err(|_| anyhow::anyhow!("a check on another base waited on this base's lock"))??;
        let mut tx = pool.begin().await?;
        let _: HashSet<_> = tokio::time::timeout(
            Duration::from_secs(10),
            reasoning::detection_keys(&mut tx, other),
        )
        .await??;
        tx.rollback().await?;

        // 4. 放锁后两条都跑完
        gate.commit().await?;
        tokio::time::timeout(Duration::from_secs(10), check).await???;
        tokio::time::timeout(Duration::from_secs(10), edit).await???;
        anyhow::Ok(())
    }
    .await;

    sqlx::query("DELETE FROM organizations WHERE id = $1")
        .bind(org)
        .execute(&control)
        .await?;
    pool.close().await;
    control.close().await;
    run
}
