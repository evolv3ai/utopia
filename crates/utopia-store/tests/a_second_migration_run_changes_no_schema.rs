//! 在已经迁移好的库上再跑一遍迁移：成功，而且什么结构都不改。
//!
//! 每次验证用自己的一个库（迁移是编进二进制的，换一套迁移就得换一个库），同一个库在一次
//! 验证里会被迁移器碰不止一次：先 `cargo run -p utopia-store --example migrate`，后面零星
//! 几个测试又自己调 `db::migrate`。第二遍要是会改结构，前后两段测试看到的就不是同一个库。
//!
//! 这个测试自成一个二进制（为什么见 `store/main.rs`）：它读的是**全库**的结构——表、列、
//! 索引、约束——和 `_sqlx_migrations` 的每一行，不属于任何一个 kb。和别的测试同时跑的话，
//! 那几个 `CREATE INDEX CONCURRENTLY` 的测试会在两次快照之间改掉它正在比的东西。
//!
//! 它假定库已经迁移过（`test_db` 的约定），所以这里调的 `db::migrate` 就是「第二遍」。

use sqlx::PgPool;

/// 全库结构的一份快照，一行一个对象，按文本排好序。
///
/// 直接读 `pg_catalog` 而不是 `information_schema`：后者看不到 `vector(1536)` 这样的类型
/// 修饰，也看不到索引是否有效。系统自带的 schema 不算。
async fn schema(pool: &PgPool) -> anyhow::Result<Vec<String>> {
    let lines = sqlx::query_scalar::<_, String>(
        r#"
        SELECT format('table|%s.%s|kind=%s|persistence=%s',
                      n.nspname, c.relname, c.relkind, c.relpersistence)
          FROM pg_class c
          JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE c.relkind IN ('r', 'p', 'v', 'm')
           AND n.nspname <> 'information_schema' AND n.nspname !~ '^pg_'
        UNION ALL
        SELECT format('column|%s.%s.%s|pos=%s|type=%s|notnull=%s|default=%s|identity=%s|generated=%s',
                      n.nspname, c.relname, a.attname, a.attnum,
                      format_type(a.atttypid, a.atttypmod), a.attnotnull,
                      coalesce(pg_get_expr(d.adbin, d.adrelid), ''),
                      a.attidentity, a.attgenerated)
          FROM pg_attribute a
          JOIN pg_class c ON c.oid = a.attrelid
          JOIN pg_namespace n ON n.oid = c.relnamespace
          LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum
         WHERE a.attnum > 0 AND NOT a.attisdropped
           AND c.relkind IN ('r', 'p', 'v', 'm')
           AND n.nspname <> 'information_schema' AND n.nspname !~ '^pg_'
        UNION ALL
        SELECT format('index|%s.%s|on=%s|valid=%s|ready=%s|def=%s',
                      n.nspname, c.relname, t.relname, i.indisvalid, i.indisready,
                      pg_get_indexdef(i.indexrelid))
          FROM pg_index i
          JOIN pg_class c ON c.oid = i.indexrelid
          JOIN pg_class t ON t.oid = i.indrelid
          JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE n.nspname <> 'information_schema' AND n.nspname !~ '^pg_'
        UNION ALL
        SELECT format('constraint|%s.%s|on=%s|type=%s|validated=%s|def=%s',
                      n.nspname, k.conname,
                      CASE WHEN k.conrelid = 0 THEN '' ELSE k.conrelid::regclass::text END,
                      k.contype, k.convalidated, pg_get_constraintdef(k.oid))
          FROM pg_constraint k
          JOIN pg_namespace n ON n.oid = k.connamespace
         WHERE n.nspname <> 'information_schema' AND n.nspname !~ '^pg_'
         ORDER BY 1
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(lines)
}

/// `_sqlx_migrations` 的每一行，整行转成文本：版本、说明、安装时间、校验和、耗时都在里面，
/// 第二遍哪怕只是重写了某一行的时间也看得出来。
async fn migration_rows(pool: &PgPool) -> anyhow::Result<Vec<String>> {
    let rows = sqlx::query_scalar::<_, String>(
        "SELECT m::text FROM _sqlx_migrations m ORDER BY m.version",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 两份快照不等时只报差出来的行——整份结构有几千行，全打出来找不到是哪一行。
fn difference(before: &[String], after: &[String]) -> String {
    let gone: Vec<&String> = before.iter().filter(|l| !after.contains(l)).collect();
    let new: Vec<&String> = after.iter().filter(|l| !before.contains(l)).collect();
    format!("only before: {gone:#?}\nonly after: {new:#?}")
}

#[tokio::test]
async fn a_second_migration_run_changes_no_schema() -> anyhow::Result<()> {
    let Some(url) = utopia_store::test_db::url() else {
        return Ok(());
    };
    let pool = PgPool::connect(&url).await?;

    let schema_before = schema(&pool).await?;
    let rows_before = migration_rows(&pool).await?;
    // 快照要是空的，下面的「相等」就什么也没证明：四类对象各至少有一行，迁移表也不是空的
    for kind in ["table|", "column|", "index|", "constraint|"] {
        assert!(
            schema_before.iter().any(|l| l.starts_with(kind)),
            "the schema snapshot holds no `{kind}` line: the database is not migrated"
        );
    }
    assert!(
        !rows_before.is_empty(),
        "_sqlx_migrations is empty: the database is not migrated"
    );

    utopia_store::db::migrate(&pool).await?;

    let schema_after = schema(&pool).await?;
    let rows_after = migration_rows(&pool).await?;
    pool.close().await;

    assert!(
        schema_before == schema_after,
        "a second migration run changed the schema\n{}",
        difference(&schema_before, &schema_after)
    );
    assert!(
        rows_before == rows_after,
        "a second migration run changed _sqlx_migrations\n{}",
        difference(&rows_before, &rows_after)
    );
    Ok(())
}
