//! jm 漫画元数据总库（`jm.db`）的只读接入 + 下载状态回写。
//!
//! 背景：`/databases/jm.db` 是青龙侧爬虫建的**候选池**——88 万本 jm 漫画的
//! 元数据索引，`download_state.state` 标记每本的下载状态。原设计里这条链是：
//!
//! ```text
//! 青龙 jm_07_export_pending → pending/jm_pending_*.txt
//! 青龙 jm_08_submit_task    → POST /api/download/by-id
//! 青龙 jm_09_sync_downloaded → 回写 jm.db done
//! ```
//!
//! 但青龙侧全部禁用、从未跑过（88 万本全 `pending`）。本模块让下载器
//! **自己**完成「读候选池 → 挑未下载 → 提交 → 回写」这条链，不再依赖青龙。
//!
//! 设计约束：
//!
//! - **独立连接，不混进 `Store`**。`Store` 管的是 `jmcomic_server.db`（本
//!   下载器自己的任务状态），schema、生命周期、写权限都不同。`jm.db` 是
//!   别人的库，我们只读 + 只回写 `state` 一列。
//! - **打开失败不阻塞启动**。`jm.db` 没挂进来 / 损坏 / 无权限时，下载器
//!   的既有功能（API 投递、导出 CBZ）必须照常可用。所以 `AppContext`
//!   里它是 `Option<JmCatalog>`。
//! - **回写只碰 `state` 与 `updated_at`**。绝不改标题、标签、`lrr_id`——
//!   那些是爬虫的地盘，改了会污染它的索引。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;
use parking_lot::Mutex;
use rusqlite::Connection;

/// jm 候选池的只读句柄。廉价可克隆（内部是 `Arc`）。
#[derive(Clone)]
pub struct JmCatalog {
    conn: Arc<Mutex<Connection>>,
    path: Arc<PathBuf>,
}

/// 候选池里的一本漫画（补全任务要用的最小字段）。
#[derive(Debug, Clone)]
pub struct CatalogEntry {
    pub comic_id: String,
    pub title: String,
}

impl JmCatalog {
    /// 以读写方式打开 `jm.db`。
    ///
    /// 回写 `done` 需要写权限，但**不跑迁移、不改 schema**——这不是我们的库。
    pub fn open(db_path: &Path) -> anyhow::Result<Self> {
        if !db_path.exists() {
            anyhow::bail!("候选池数据库 `{}` 不存在", db_path.display());
        }

        let conn = Connection::open(db_path)
            .with_context(|| format!("打开候选池数据库 `{}` 失败", db_path.display()))?;

        // 只设置连接级 pragma。不要改 journal_mode——那是库级持久设置，
        // 动了会改变爬虫那边的运行特性。
        conn.execute_batch(
            r#"
            PRAGMA busy_timeout = 10000;
            "#,
        )
        .context("设置候选池连接 pragma 失败")?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            path: Arc::new(db_path.to_path_buf()),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 借出连接。调用方持锁期间不要做网络 IO。
    fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> anyhow::Result<T>) -> anyhow::Result<T> {
        let conn = self.conn.lock();
        f(&conn)
    }

    /// 统计各状态的数量，用于状态接口展示。
    pub fn state_counts(&self) -> anyhow::Result<Vec<(String, i64)>> {
        self.with_conn(|conn| {
            let mut stmt = conn
                .prepare("SELECT state, COUNT(*) FROM download_state GROUP BY state ORDER BY COUNT(*) DESC")
                .context("准备状态统计查询失败")?;
            let rows = stmt
                .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
                .context("执行状态统计查询失败")?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row.context("读取状态统计行失败")?);
            }
            Ok(out)
        })
    }

    /// 挑 N 本「待下载」的漫画。
    ///
    /// 筛选条件（每一道都是为了提高下载成功率）：
    ///
    /// 1. `state = 'pending'` —— 还没下过的
    /// 2. `comics.title != ''` —— 有标题，说明详情爬过了
    /// 3. `comics.cover_status = 1` —— 有封面，是「完整条目」的强信号
    /// 4. `ORDER BY CAST(comic_id AS INTEGER) DESC` —— **最新优先**。
    ///    升序会从头撞上 ID=10 这类上古条目；降序优先下近期条目。
    pub fn pick_pending(&self, limit: usize) -> anyhow::Result<Vec<CatalogEntry>> {
        self.with_conn(|conn| {
            let mut stmt = conn
                .prepare(
                    r#"
                    SELECT c.comic_id, c.title
                    FROM comics c
                    JOIN download_state d ON d.comic_id = c.comic_id
                    WHERE d.state = 'pending'
                      AND c.title IS NOT NULL AND c.title != ''
                      AND c.cover_status = 1
                    ORDER BY CAST(c.comic_id AS INTEGER) DESC
                    LIMIT ?1
                    "#,
                )
                .context("准备待下载查询失败")?;

            let rows = stmt
                .query_map([limit as i64], |row| {
                    Ok(CatalogEntry {
                        comic_id: row.get(0)?,
                        title: row.get(1)?,
                    })
                })
                .context("执行待下载查询失败")?;

            let mut out = Vec::new();
            for row in rows {
                out.push(row.context("读取待下载行失败")?);
            }
            Ok(out)
        })
    }

    /// 回写下载状态。
    ///
    /// **只更新 `state` 与 `updated_at`**，绝不碰别的列。
    /// `updated_at` 用 SQLite 自己算，避免调用方传错格式。
    pub fn mark_state(&self, comic_id: &str, state: &str) -> anyhow::Result<bool> {
        self.with_conn(|conn| {
            let changed = conn
                .execute(
                    "UPDATE download_state\n                        SET state = ?1, updated_at = CAST(strftime('%s','now') AS INTEGER)\n                      WHERE comic_id = ?2",
                    rusqlite::params![state, comic_id],
                )
                .with_context(|| format!("回写漫画 `{comic_id}` 状态 `{state}` 失败"))?;
            Ok(changed > 0)
        })
    }

    /// 查单本当前状态。用于回写前的核对与幂等判断。
    pub fn get_state(&self, comic_id: &str) -> anyhow::Result<Option<String>> {
        self.with_conn(|conn| {
            let state = conn
                .query_row(
                    "SELECT state FROM download_state WHERE comic_id = ?1",
                    [comic_id],
                    |row| row.get::<_, String>(0),
                )
                .ok();
            Ok(state)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 建一个内存候选池，schema 与真实 `jm.db` 对齐（只含用到的列）。
    fn fixture() -> JmCatalog {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE comics (
                comic_id TEXT PRIMARY KEY,
                title TEXT,
                cover_status INTEGER DEFAULT 0
            );
            CREATE TABLE download_state (
                comic_id TEXT PRIMARY KEY,
                state TEXT NOT NULL DEFAULT 'pending',
                updated_at INTEGER DEFAULT 0
            );
            "#,
        )
        .unwrap();
        JmCatalog {
            conn: Arc::new(Mutex::new(conn)),
            path: Arc::new(PathBuf::from(":memory:")),
        }
    }

    fn insert(cat: &JmCatalog, id: &str, title: &str, cover: i64) {
        cat.with_conn(|conn| {
            conn.execute(
                "INSERT INTO comics (comic_id, title, cover_status) VALUES (?1, ?2, ?3)",
                rusqlite::params![id, title, cover],
            )?;
            conn.execute(
                "INSERT INTO download_state (comic_id, state) VALUES (?1, 'pending')",
                [id],
            )?;
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn pick_pending_skips_titleless_and_coverless() {
        let cat = fixture();
        insert(&cat, "100", "有标题有封面", 1);
        insert(&cat, "200", "", 1);
        insert(&cat, "300", "有标题无封面", 0);

        let picked = cat.pick_pending(10).unwrap();
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].comic_id, "100");
    }

    #[test]
    fn pick_pending_prefers_largest_id() {
        let cat = fixture();
        insert(&cat, "10", "上古条目", 1);
        insert(&cat, "1473775", "新条目", 1);
        insert(&cat, "900", "中间条目", 1);

        let picked = cat.pick_pending(2).unwrap();
        assert_eq!(picked.len(), 2);
        assert_eq!(picked[0].comic_id, "1473775");
        assert_eq!(picked[1].comic_id, "900");
    }

    #[test]
    fn pick_pending_skips_already_done() {
        let cat = fixture();
        insert(&cat, "100", "已完成", 1);
        cat.mark_state("100", "done").unwrap();
        insert(&cat, "200", "待下载", 1);

        let picked = cat.pick_pending(10).unwrap();
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].comic_id, "200");
    }

    #[test]
    fn mark_state_only_touches_state_and_updated_at() {
        let cat = fixture();
        insert(&cat, "100", "原标题", 1);
        assert!(cat.mark_state("100", "done").unwrap());

        assert_eq!(cat.get_state("100").unwrap().as_deref(), Some("done"));
        let title: String = cat
            .with_conn(|conn| Ok(conn.query_row("SELECT title FROM comics WHERE comic_id='100'", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(title, "原标题");
    }

    #[test]
    fn mark_state_returns_false_for_unknown_id() {
        let cat = fixture();
        assert!(!cat.mark_state("999", "done").unwrap());
    }

    #[test]
    fn state_counts_groups_by_state() {
        let cat = fixture();
        insert(&cat, "100", "a", 1);
        insert(&cat, "200", "b", 1);
        cat.mark_state("200", "done").unwrap();

        let counts = cat.state_counts().unwrap();
        let map: std::collections::HashMap<_, _> = counts.into_iter().collect();
        assert_eq!(map.get("pending"), Some(&1));
        assert_eq!(map.get("done"), Some(&1));
    }
}
