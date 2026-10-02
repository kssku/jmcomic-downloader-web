//! 下载完成后的自动导出，以及「导出后删原图」的共用实现。
//!
//! ## 触发时机
//!
//! 每次「一个章节下载完成」后，由 `DownloadTask::process()` 退出时调用
//! [`try_auto_export_comic`]。它按 `comic_id` 查该漫画的**全部**任务：
//!
//! - 只要还有一章不是 `Completed`（含 `Failed`、`Pending`、`Downloading`），
//!   就静默返回——整本没下完，谈不上导出。
//! - 全部 `Completed` 时，才走「导出 → 删图」。
//!
//! 这个检查是**幂等**的：最后完成的那一章会触发它；若因某种原因重复触发，
//! 第二次会发现 CBZ 已存在且下载目录已被删除，于是安全地什么都不做。
//!
//! ## 为什么走本地元数据而不是联网
//!
//! 用 [`crate::types::Comic::from_metadata`] 从 `元数据.json` 重建 `Comic`，
//! 不调 `utils::get_comic`。两个原因：
//!
//! 1. **离线可用**：下载刚完成时网络状况可能不佳（限流、代理抖动），
//!    联网取详情会让自动导出无故失败，而本地数据已经足够。
//! 2. **数据一致**：`from_metadata` 内部的 `update_chapter_infos_fields()`
//!    以「目录里真有图片」为已下载的唯一证据——与下载流程同源，
//!    不会出现「DB 说下完了、磁盘其实没有」的错判。
//!
//! 手动导出（`POST /api/export/comic/:comic_id`）现在也走同一条本地路径，
//! 原因相同：网络断了不该连已下载的漫画都导不出来。

use anyhow::{anyhow, Context};

use crate::{
    context::AppContext,
    export::{export_comic_cbz, remove_comic_download_dir, ExportResult},
    store::{repo::TaskRepo, types::DbTaskState},
    types::Comic,
};

/// 一条漫画的下载目录是否整本下完。
///
/// 「整本下完」的定义刻意严格：该 `comic_id` 下的**每一条**任务都必须是
/// `Completed`。任何一章是 `Failed`/`Pending`/`Downloading`/`Paused`/`Cancelled`
/// 都算没下完——单行本的意义是「一次读完」，缺章导出是负价值。
fn is_comic_fully_downloaded(app: &AppContext, comic_id: &str) -> anyhow::Result<bool> {
    // `limit = i64::MAX` 取全量：一本漫画的章节数是几十量级，不存在分页问题。
    let tasks = TaskRepo::list(app.store(), None, Some(comic_id), None, i64::MAX, 0)
        .context("查询该漫画的下载任务失败")?;

    if tasks.is_empty() {
        // 没有任何任务记录：可能是任务已被清理（`purge_terminal`），
        // 也可能这本漫画根本没被投递过。两种情况都不该触发自动导出。
        return Ok(false);
    }

    Ok(tasks.iter().all(|t| t.state == DbTaskState::Completed))
}

/// 找到该漫画的 `元数据.json` 路径。
///
/// 目录名由 `dir_fmt` 的 `{comic_id}` 决定，历史版本可能用过别的格式，
/// 所以不硬拼路径，而是**在下载根目录下按漫画 ID 找**。
fn find_comic_metadata_path(app: &AppContext, comic_id: &str) -> Option<std::path::PathBuf> {
    let download_dir = app.download_dir();
    let candidate = download_dir.join(comic_id).join("元数据.json");
    if candidate.is_file() {
        return Some(candidate);
    }

    // 兜底：目录名不是漫画 ID（老数据/自定义 dir_fmt）。遍历一层子目录，
    // 读每个 `元数据.json` 的 `id` 字段比对。这一层遍历很浅（一本漫画一个
    // 目录），代价可以忽略。
    let entries = std::fs::read_dir(&download_dir).ok()?;
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let metadata_path = entry.path().join("元数据.json");
        if !metadata_path.is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&metadata_path) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        if json.get("id").and_then(|v| v.as_str()) == Some(comic_id) {
            return Some(metadata_path);
        }
    }

    None
}

/// 从本地元数据导出 CBZ，并按 `auto_export_cbz` 开关决定是否删除原图目录。
///
/// 这是**手动导出与自动导出共用的执行体**。它不做「整本是否下完」的判断——
/// 那是调用方的事：
///
/// - 自动导出（[`try_auto_export_comic`]）要求整本全部 `Completed`；
/// - 手动导出由用户显式触发，用户点「导出」就意味着他知道自己在导什么。
///
/// 返回 `Ok(None)` 表示本地没有可用的元数据（无从导出）；`Ok(Some(result))`
/// 表示 CBZ 已生成，`result.cbz_path` 可直接交给调用方。
///
/// ## 失败一律不删图
///
/// 任何一步出错（重建 `Comic`、导出、CBZ 校验）都**只记日志、原图保留**。
/// 不该因为导出失败就把用户的数据删掉——那正是「不可逆操作 + 无人值守」
/// 最危险的组合。
pub fn export_local_comic(
    app: &AppContext,
    comic_id: &str,
) -> anyhow::Result<Option<ExportResult>> {
    // ── 前置 1：找到本地元数据 ─────────────────────────────────
    let Some(metadata_path) = find_comic_metadata_path(app, comic_id) else {
        tracing::warn!(comic_id, "找不到`元数据.json`，无法从本地导出");
        return Ok(None);
    };

    // ── 前置 2：原图目录是否还在 ───────────────────────────────
    // 可能已经导出并删过图，此时再触发是正常的重复调用，直接跳过。
    let comic_download_dir = metadata_path
        .parent()
        .ok_or_else(|| anyhow!("`{}`没有父目录", metadata_path.display()))?
        .to_path_buf();
    if !comic_download_dir.is_dir() {
        tracing::trace!(
            comic_id,
            dir = %comic_download_dir.display(),
            "下载目录已不存在（可能已导出并清理），跳过"
        );
        return Ok(None);
    }

    // ── 重建 Comic（本地，不联网）──────────────────────────────
    let comic = Comic::from_metadata(&metadata_path).with_context(|| {
        format!(
            "从元数据`{}`重建 Comic 失败",
            metadata_path.display()
        )
    })?;

    // ── 导出 ─────────────────────────────────────────────────
    let export_dir = app.config().read().export_dir.clone();
    let result = export_comic_cbz(&comic, &export_dir).with_context(|| {
        format!("漫画`{}`导出 CBZ 失败", comic.name)
    })?;

    tracing::info!(
        comic_id,
        title = %comic.name,
        cbz = %result.cbz_path.display(),
        pages = result.page_count,
        chapters = result.chapter_count,
        size = result.file_size,
        overwritten = result.overwritten,
        "从本地元数据导出 CBZ 成功"
    );

    // ── 删图（受开关控制，护栏在 cleanup 内部）─────────────────
    if !app.config().read().auto_export_cbz {
        tracing::debug!(comic_id, "`autoExportCbz` 已关闭，导出后保留原图");
        return Ok(Some(result));
    }

    if let Err(err) = remove_comic_download_dir(
        &comic_download_dir,
        &result.cbz_path,
        result.file_size,
    ) {
        tracing::error!(
            comic_id,
            dir = %comic_download_dir.display(),
            err = %err,
            "导出后删除原图目录失败，CBZ 已生成、原图保留"
        );
        // 导出成功但删图失败不算整体失败：CBZ 已经可用，原图多留一份无害。
        return Ok(Some(result));
    }

    tracing::info!(
        comic_id,
        dir = %comic_download_dir.display(),
        "导出后已删除原图目录"
    );
    Ok(Some(result))
}

/// 下载完成后尝试自动导出该漫画，并在成功后删除原图目录。
///
/// 返回 `Ok(true)` 表示本次真的导出并删图了；`Ok(false)` 表示没到时机
/// （整本没下完 / 自动导出关闭 / 已有 CBZ 且原图已删），属于正常路径。
///
/// ## 为什么删图不可逆也没做二次确认
///
/// 删图的前置条件已经足够强：整本全部 `Completed` **且** CBZ 真实落盘
/// **且** 大小与报告一致。三条同时满足时，原图的信息价值已经 100% 转移到
/// CBZ 里。这里再加一层确认只会让「下完即用」的默认体验变成需要人盯着。
pub async fn try_auto_export_comic(app: &AppContext, comic_id: &str) -> anyhow::Result<bool> {
    // ── 前置 1：开关 ──────────────────────────────────────────
    if !app.config().read().auto_export_cbz {
        tracing::trace!(comic_id, "自动导出 CBZ 已关闭，跳过");
        return Ok(false);
    }

    // ── 前置 2：整本是否下完 ───────────────────────────────────
    if !is_comic_fully_downloaded(app, comic_id)? {
        tracing::trace!(comic_id, "该漫画尚未整本下载完成，暂不自动导出");
        return Ok(false);
    }

    // ── 执行（共用体内部已含「元数据不存在」「目录已删」的兜底）──
    match export_local_comic(app, comic_id) {
        Ok(Some(_)) => Ok(true),
        Ok(None) => Ok(false),
        Err(err) => {
            tracing::error!(
                comic_id,
                err = %err,
                "自动导出失败，保留原图"
            );
            Ok(false)
        }
    }
}

/// 供测试与调试：只判断整本是否下完，不产生副作用。
pub fn check_comic_fully_downloaded(
    app: &AppContext,
    comic_id: &str,
) -> anyhow::Result<bool> {
    is_comic_fully_downloaded(app, comic_id)
}