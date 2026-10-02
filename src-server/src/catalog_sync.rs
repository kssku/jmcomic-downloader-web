//! 候选池自动补全：从 `jm.db` 挑 pending 本子 → 提交下载 → 回写 `done`。
//!
//! 这是把「孤儿库」接回下载器的最后一环。原设计里这条链由青龙的三个脚本
//! （`07_export_pending` / `08_submit_task` / `09_sync_downloaded`）接力完成，
//! 但青龙侧任务全禁用、从未跑过，于是 88 万本全卡在 `pending`。
//!
//! 现在下载器自己干这件事，不依赖青龙：
//!
//! ```text
//! pick_pending(N)  →  POST /api/download/by-id（内部直接调 commands）
//!                  →  等整本下完
//!                  →  mark_state(done)
//! ```
//!
//! 几个刻意的设计选择：
//!
//! - **只在整本下完后回写 `done`**。中途回写会让「下了几话」的本子被误标完成，
//!   永远不会再被补全。宁可重复挑到未完成的，也不能漏。
//! - **只回写 `done`**，不碰 `skipped` / `failed`。失败的本子留在 `pending`，
//!   下轮还会被挑到——这比标成 `failed` 然后永久沉底更符合「补全」的语义。
//! - **串行处理**。补全是个后台慢活儿，不该跟用户手动投递抢并发额度。
//!   每本下完才挑下一本。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::context::AppContext;

/// 两轮补全之间的间隔。整本漫画下载是小时级的事，不需要密集轮询。
const ROUND_INTERVAL: Duration = Duration::from_secs(600);

/// 单本等待上限。
///
/// 上游（禁漫 API）偶尔会整站故障，返回「Could not connect to mysql」这类
/// 服务端错误。这种本子会反复重试、永远下不完——若死等，一颗坏豆能卡死整轮。
/// 20 分钟足够下完绝大多数单话本，超过就放弃、留 `pending` 给下轮。
const PER_COMIC_TIMEOUT: Duration = Duration::from_secs(20 * 60);

/// 一个本子在 in-flight 里最多滞留多少轮（每轮 10 分钟，60 轮 ≈ 10 小时）。
///
/// 超过就强制释放，让它下轮重新被挑到——这是防止「标记焊死」的兜底：
/// 漫画下架会让 `recover_pending_tasks` 永久失败，标记若不释放，
/// 这本就再也不会被补全，而它在 jm.db 里将永远停在 `pending`。
const IN_FLIGHT_MAX_ROUNDS: u32 = 60;

/// 补全调度器的运行状态，供 `/api/catalog/status` 读取。
#[derive(Debug, Default)]
pub struct CatalogSyncState {
    /// 是否有一个补全轮次正在跑。
    pub running: AtomicBool,
    /// 已完成的总轮数。
    pub rounds: std::sync::atomic::AtomicU64,
    /// 已成功回写 `done` 的本数。
    pub marked_done: std::sync::atomic::AtomicU64,
    /// **已提交下载但尚未整本完成**的本子。
    ///
    /// 这是防重复投递的关键：`jm.db` 里这些本子仍是 `pending`，
    /// 若不记住它们，下一轮 `pick_pending` 会把同一批再挑一遍，
    /// 导致同一本被反复重下（上一轮实测已踩到）。
    ///
    /// 只存本轮进程内的记忆。进程重启后靠下载管理器自身的
    /// 「恢复未完成任务」机制兜底，不会漏。
    /// 值是该本子「连续滞留的轮数」。超过 `IN_FLIGHT_MAX_ROUNDS` 就强制释放，
    /// 避免因漫画下架、恢复永久失败等原因把标记焊死在这里。
    pub in_flight: parking_lot::Mutex<std::collections::HashMap<String, u32>>,
}

impl CatalogSyncState {
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn rounds(&self) -> u64 {
        self.rounds.load(Ordering::Relaxed)
    }

    pub fn marked_done(&self) -> u64 {
        self.marked_done.load(Ordering::Relaxed)
    }

    /// 判断某个本子是否已在本进程内提交过、且还没下完。
    fn is_in_flight(&self, comic_id: &str) -> bool {
        self.in_flight.lock().contains_key(comic_id)
    }

    /// 记下「已提交、待完成」。
    fn mark_in_flight(&self, comic_id: &str) {
        self.in_flight.lock().entry(comic_id.to_string()).or_insert(0);
    }

    /// 每轮开始时给所有 in-flight 本子的滞留轮数 +1。
    ///
    /// 返回本轮因超限被强制释放的本子数——它们会被下一轮重新挑到，
    /// 从而获得一次「重新提交」的机会（可能上次的提交根本没能推进）。
    fn tick_in_flight(&self) -> usize {
        let mut map = self.in_flight.lock();
        let before = map.len();
        map.retain(|_, rounds| {
            *rounds += 1;
            *rounds <= IN_FLIGHT_MAX_ROUNDS
        });
        before - map.len()
    }

    /// 本子已了结（下完并回写 done，或本轮放弃），从 in-flight 里摘掉。
    fn clear_in_flight(&self, comic_id: &str) {
        self.in_flight.lock().remove(comic_id);
    }

    /// 当前 in-flight 的本数（供状态接口展示）。
    pub fn in_flight_count(&self) -> usize {
        self.in_flight.lock().len()
    }
}

/// 手动触发一次补全（`POST /api/catalog/trigger`）。
///
/// 已有轮次在跑时直接返回 `None`，避免并发重复挑同一批本子。
pub async fn run_once(app: AppContext) -> Option<CatalogRoundResult> {
    let state = app.catalog_sync_state();
    if state
        .running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        tracing::info!(err_title = "补全轮次已在运行，跳过本次触发");
        return None;
    }

    let result = do_round(&app).await;
    state.running.store(false, Ordering::SeqCst);
    state.rounds.fetch_add(1, Ordering::Relaxed);

    Some(result)
}

/// 一轮补全的结果，直接作为 API 响应体。
#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogRoundResult {
    /// 本轮挑出的本子数。
    pub picked: usize,
    /// 成功提交下载的本子数。
    pub submitted: usize,
    /// 整本下完并回写 `done` 的本子数。
    pub marked_done: usize,
    /// 中途出错的本子（留 `pending`，下轮重挑）。
    pub errors: Vec<String>,
}

async fn do_round(app: &AppContext) -> CatalogRoundResult {
    let mut out = CatalogRoundResult::default();

    // 开关关了就空转。放在这里而不是调度层，是为了手动触发也遵守开关。
    let (enabled, batch) = {
        let config = app.config_read();
        (config.catalog_auto_fill, config.catalog_fill_batch)
    };
    if !enabled {
        tracing::info!(err_title = "候选池自动补全已关闭，本轮空转");
        return out;
    }

    let Some(catalog) = app.catalog() else {
        tracing::warn!(err_title = "候选池未接入，补全轮次空转");
        return out;
    };

    let sync_state = app.catalog_sync_state();

    // 每轮给 in-flight 计数 +1，超限的强制释放。必须在挑选之前做，
    // 否则被释放的本子要等到下一轮才有机会被挑到。
    let released = sync_state.tick_in_flight();
    if released > 0 {
        tracing::warn!(
            err_title = "强制释放滞留过久的 in-flight 本子",
            released = released,
            max_rounds = IN_FLIGHT_MAX_ROUNDS
        );
    }

    // 挑本子。这一步很快（有索引），不必 spawn_blocking。
    let picked = match catalog.pick_pending(batch) {
        Ok(items) => items,
        Err(err) => {
            tracing::warn!(err_title = "挑选待下载本子失败", message = %err);
            out.errors.push(format!("挑选失败: {err}"));
            return out;
        }
    };

    // 过滤掉「已提交但还没下完」的。它们在 jm.db 里仍是 pending，
    // 不去重就会每轮重复投递同一批。
    let before = picked.len();
    let picked: Vec<_> = picked
        .into_iter()
        .filter(|e| !sync_state.is_in_flight(&e.comic_id))
        .collect();
    let skipped_in_flight = before - picked.len();
    if skipped_in_flight > 0 {
        tracing::info!(
            err_title = "跳过已提交未完成的本子",
            skipped = skipped_in_flight,
            in_flight = sync_state.in_flight_count()
        );
    }

    out.picked = picked.len();
    if picked.is_empty() {
        tracing::info!(err_title = "候选池本轮无新本子可挑（可能都在下载中）");
        return out;
    }

    tracing::info!(err_title = "候选池补全开始", count = picked.len());

    // 挑出来就立刻记 in-flight，避免同轮内重复。
    for entry in &picked {
        sync_state.mark_in_flight(&entry.comic_id);
    }

    // ── 阶段一：并发提交 ───────────────────────────────
    //
    // 早先的实现是「提交一本 → 等它下完 → 再提交下一本」，于是 10 本要串行
    // 排队，整轮耗时是各本之和。实测第一轮就卡在第 4 本上十几分钟，第二轮
    // 迟迟无法开始。
    //
    // 现在先把整批都丢进下载队列，让下载管理器自己的并发度去调度；
    // 之后再逐个等待完成。整轮耗时降到「最慢那本」，而不是累加。
    let mut to_wait: Vec<(String, String)> = Vec::new();
    for entry in &picked {
        match submit_one(app, &entry.comic_id).await {
            Ok(()) => {
                to_wait.push((entry.comic_id.clone(), entry.title.clone()));
            }
            Err(err) => {
                // 提交失败：摘掉标记，让下轮有机会重试。
                sync_state.clear_in_flight(&entry.comic_id);
                tracing::warn!(
                    err_title = "补全单本提交失败",
                    comic_id = %entry.comic_id,
                    message = %err
                );
                out.errors.push(format!("{}: {err}", entry.comic_id));
            }
        }
    }

    tracing::info!(
        err_title = "候选池本批已全部提交，开始等待完成",
        submitted = to_wait.len(),
        failed = out.errors.len()
    );

    // ── 阶段二：逐个等待并回写 ─────────────────────────
    for (comic_id, title) in to_wait {
        match wait_one(app, &comic_id).await {
            Ok(true) => {
                out.submitted += 1;
                // 整本下完，回写 done。
                match catalog.mark_state(&comic_id, "done") {
                    Ok(true) => {
                        out.marked_done += 1;
                        sync_state.marked_done.fetch_add(1, Ordering::Relaxed);
                        // 了结了，摘掉 in-flight 标记。
                        sync_state.clear_in_flight(&comic_id);
                        tracing::info!(
                            err_title = "补全完成并回写 done",
                            comic_id = %comic_id,
                            title = %title
                        );
                    }
                    Ok(false) => {
                        tracing::warn!(
                            err_title = "回写 done 未命中任何行",
                            comic_id = %comic_id
                        );
                    }
                    Err(err) => {
                        tracing::warn!(
                            err_title = "回写 done 失败",
                            comic_id = %comic_id,
                            message = %err
                        );
                        out.errors.push(format!("{comic_id} 回写失败: {err}"));
                    }
                }
            }
            Ok(false) => {
                // 提交了但没下完。**保留 in-flight 标记**，下轮不再重复投递；
                // 等它真正下完，由下载管理器恢复/完成流程兜底。
                tracing::info!(
                    err_title = "本子未完成，保留 in-flight 待下轮复查",
                    comic_id = %comic_id
                );
            }
            Err(err) => {
                // 等待过程出错：摘掉标记，让下轮重新提交。
                sync_state.clear_in_flight(&comic_id);
                tracing::warn!(
                    err_title = "补全单本等待失败",
                    comic_id = %comic_id,
                    message = %err
                );
                out.errors.push(format!("{comic_id}: {err}"));
            }
        }
    }

    tracing::info!(
        err_title = "候选池补全轮次结束",
        picked = out.picked,
        submitted = out.submitted,
        marked_done = out.marked_done,
        errors = out.errors.len(),
        in_flight = sync_state.in_flight_count()
    );

    out
}

/// 提交一本去下载（不等）。
///
/// 直接复用 `commands::download_by_id`，不走 HTTP——省一次环回，
/// 也让补全跟用户手动投递共享同一套业务校验（去重、冲突守卫）。
async fn submit_one(app: &AppContext, comic_id: &str) -> anyhow::Result<()> {
    use crate::api::commands;

    let result = commands::download_by_id(app, comic_id.to_string(), None)
        .await
        .map_err(|err| anyhow::anyhow!("提交下载失败: {} — {}", err.err_title, err.err_message))?;

    // 冲突说明这个 comic_id 的章节已被别的本子占用，跳过别硬来。
    if !result.conflicted_chapters.is_empty() {
        anyhow::bail!(
            "与已有任务冲突的章节: {}",
            result.conflicted_chapters.join(", ")
        );
    }

    Ok(())
}

/// 等一本整本下完，最多等 `PER_COMIC_TIMEOUT`。
///
/// 等待是轮询任务表，不是死等某个章节——多话本要等所有话都完成。
async fn wait_one(app: &AppContext, comic_id: &str) -> anyhow::Result<bool> {
    let deadline = tokio::time::Instant::now() + PER_COMIC_TIMEOUT;
    loop {
        if crate::export::check_comic_fully_downloaded(app, comic_id)? {
            return Ok(true);
        }
        if tokio::time::Instant::now() > deadline {
            tracing::warn!(
                err_title = "等待整本下载超时，留待下轮",
                comic_id = %comic_id,
                timeout_secs = PER_COMIC_TIMEOUT.as_secs()
            );
            return Ok(false);
        }
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}

/// 启动后台补全调度任务。立即跑第一轮，之后每 `ROUND_INTERVAL` 一轮。
pub fn spawn_scheduler(app: AppContext) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        // 启动后先歇一会儿，别跟开机时的初始化抢资源。
        tokio::time::sleep(Duration::from_secs(60)).await;

        loop {
            let _ = run_once(app.clone()).await;
            tokio::time::sleep(ROUND_INTERVAL).await;
        }
    })
}

/// 供 `AppContext` 持有的共享状态句柄。
pub type SharedCatalogSyncState = Arc<CatalogSyncState>;
