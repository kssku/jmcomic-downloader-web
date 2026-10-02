//! 导出成功后的原图清理。
//!
//! ## 为什么单独成模块
//!
//! 删原图是**整个流程里唯一不可逆的一步**。把它和导出逻辑分开，是为了让
//! 「什么情况下允许删」这件事有唯一的、可被测试覆盖的入口，而不是散落在
//! 下载流程的各个角落。
//!
//! ## 什么时候允许删
//!
//! 调用方必须先确认 CBZ 已经**成功落盘且大小合理**，本模块才该被调用。
//! 本模块内部仍会做一次自检（见 [`remove_comic_download_dir`]），因为
//! 「调用方以为导出成功了」和「导出真的成功了」不是同一件事——中途失败
//! 留下的半截 CBZ 也可能存在，那是不能拿来换原图的。

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};

/// 删除一整本漫画的下载目录（`漫画下载/{漫画ID}`）。
///
/// ## 安全护栏
///
/// 这是不可逆操作，所以在真正 `remove_dir_all` 之前逐条校验：
///
/// 1. **CBZ 必须真实存在**且大小与导出报告一致——防止「导出中途失败、
///    留下半截文件」被当成成功，导致原图被删而 CBZ 是坏的。
/// 2. **目录必须非空且确实像一本漫画的目录**——至少含有一个子目录。
///    空的或只有文件的目录说明传错了路径，宁可不删。
/// 3. **目录必须是绝对路径**——相对路径的含义依赖进程 cwd，不可控。
///
/// 任一条不满足就返回 `Err`，**不删任何东西**。调用方记录错误即可，
/// 原图保留，用户可手动处理。
///
/// ## 为什么不做「保留元数据/封面」的折中
///
/// 这是刻意的选择：CBZ 才是最终读物，下载目录只是中间产物。留一半
/// （元数据+封面）会让控制台显示「已下载」但实际没有图片可读，反而
/// 制造出「看起来下过、点进去打不开」的困惑。要么完整保留、要么完整
/// 删掉，没有中间态。
pub fn remove_comic_download_dir(
    comic_download_dir: &Path,
    cbz_path: &Path,
    expected_cbz_size: u64,
) -> anyhow::Result<()> {
    // ── 护栏 1：CBZ 必须真实存在，大小与报告一致 ────────────────
    let meta = std::fs::metadata(cbz_path)
        .with_context(|| format!("读取 CBZ `{}` 元信息失败，为安全起见不删除原图", cbz_path.display()))?;
    if !meta.is_file() {
        return Err(anyhow!(
            "CBZ 路径`{}`不是文件，为安全起见不删除原图",
            cbz_path.display()
        ));
    }
    let actual_size = meta.len();
    if actual_size != expected_cbz_size {
        return Err(anyhow!(
            "CBZ 大小与导出报告不一致（实际 {} 字节，报告 {} 字节），\
             疑似导出未完成，为安全起见不删除原图",
            actual_size,
            expected_cbz_size
        ));
    }
    if actual_size == 0 {
        return Err(anyhow!(
            "CBZ `{}` 为空文件，为安全起见不删除原图",
            cbz_path.display()
        ));
    }

    // ── 护栏 2：必须绝对路径 ──────────────────────────────────
    if !comic_download_dir.is_absolute() {
        return Err(anyhow!(
            "下载目录`{}`不是绝对路径，为安全起见不删除",
            comic_download_dir.display()
        ));
    }

    // ── 护栏 3：目录必须存在，且至少含一个子目录 ────────────────
    if !comic_download_dir.is_dir() {
        return Err(anyhow!(
            "下载目录`{}`不存在或不是目录，无法删除",
            comic_download_dir.display()
        ));
    }
    let mut has_subdir = false;
    let entries = std::fs::read_dir(comic_download_dir)
        .with_context(|| format!("读取下载目录`{}`失败", comic_download_dir.display()))?;
    for entry in entries {
        let entry = entry.with_context(|| {
            format!("遍历下载目录`{}`失败", comic_download_dir.display())
        })?;
        if entry.path().is_dir() {
            has_subdir = true;
            break;
        }
    }
    if !has_subdir {
        return Err(anyhow!(
            "下载目录`{}`没有任何章节子目录，不像一本已下载的漫画，\
             为安全起见不删除",
            comic_download_dir.display()
        ));
    }

    // ── 护栏全过，执行删除 ────────────────────────────────────
    std::fs::remove_dir_all(comic_download_dir).with_context(|| {
        format!(
            "删除下载目录`{}`失败（CBZ 已生成，原图保留）",
            comic_download_dir.display()
        )
    })?;

    Ok(())
}

/// 从 `Comic` 推导出它的下载根目录。
///
/// 优先用 `comic_download_dir`（下载流程中会填好）；若为 `None`，
/// 用 `download_dir/{漫画ID}` 兜底——目录名就是漫画 ID 由 `dir_fmt`
/// 的 `{comic_id}` 决定。
pub fn resolve_comic_download_dir(
    comic_download_dir: Option<&PathBuf>,
    download_dir: &Path,
    comic_id: &str,
) -> PathBuf {
    match comic_download_dir {
        Some(dir) => dir.clone(),
        None => download_dir.join(comic_id),
    }
}
#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// 在临时目录里造一份「看起来像一本已下载漫画」的结构：
    /// `{root}/{comic_id}/{1,2}/xxx.jpg` + `元数据.json`。
    fn make_comic_dir(root: &Path, comic_id: &str) -> PathBuf {
        let comic_dir = root.join(comic_id);
        for chapter in ["1", "2"] {
            let chapter_dir = comic_dir.join(chapter);
            fs::create_dir_all(&chapter_dir).unwrap();
            fs::write(chapter_dir.join("001.jpg"), b"fake-jpg").unwrap();
        }
        fs::write(comic_dir.join("元数据.json"), format!("{{\"id\":\"{comic_id}\"}}")).unwrap();
        comic_dir
    }

    /// 造一个指定大小的假 CBZ。
    fn make_cbz(path: &Path, size: usize) -> u64 {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, vec![0u8; size]).unwrap();
        size as u64
    }

    /// 每张测试用独立的临时根目录，避免互相干扰。
    fn temp_root(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("cleanup-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    // ── 正路径：护栏全过 → 目录被删 ─────────────────────────────

    #[test]
    fn removes_dir_when_all_guards_pass() {
        let root = temp_root("ok");
        let comic_dir = make_comic_dir(&root, "1461316");
        let cbz = root.join("导出/1461316.cbz");
        let size = make_cbz(&cbz, 1024);

        remove_comic_download_dir(&comic_dir, &cbz, size).unwrap();

        assert!(!comic_dir.exists(), "护栏全过时应当删掉整个漫画目录");
        assert!(cbz.exists(), "CBZ 不能被误删");
        let _ = fs::remove_dir_all(&root);
    }

    // ── 护栏 1：CBZ 缺失 / 大小不符 / 为空 → 拒绝 ───────────────

    #[test]
    fn rejects_when_cbz_missing() {
        let root = temp_root("no-cbz");
        let comic_dir = make_comic_dir(&root, "1461316");
        let cbz = root.join("导出/1461316.cbz"); // 故意不创建

        let err = remove_comic_download_dir(&comic_dir, &cbz, 1024).unwrap_err();

        assert!(err.to_string().contains("元信息失败"), "错误应指明 CBZ 读取失败");
        assert!(comic_dir.exists(), "CBZ 不存在时必须保留原图");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_when_cbz_size_mismatch() {
        let root = temp_root("size-mismatch");
        let comic_dir = make_comic_dir(&root, "1461316");
        let cbz = root.join("导出/1461316.cbz");
        make_cbz(&cbz, 1024);

        // 报告 2048，实际 1024 —— 模拟「导出中途失败留下半截 CBZ」。
        let err = remove_comic_download_dir(&comic_dir, &cbz, 2048).unwrap_err();

        assert!(
            err.to_string().contains("大小与导出报告不一致"),
            "错误应指明大小不一致，实际错误：{err}"
        );
        assert!(comic_dir.exists(), "CBZ 大小不符时必须保留原图");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_when_cbz_empty() {
        let root = temp_root("empty-cbz");
        let comic_dir = make_comic_dir(&root, "1461316");
        let cbz = root.join("导出/1461316.cbz");
        fs::create_dir_all(cbz.parent().unwrap()).unwrap();
        fs::write(&cbz, b"").unwrap();

        let err = remove_comic_download_dir(&comic_dir, &cbz, 0).unwrap_err();

        assert!(err.to_string().contains("为空文件"), "错误应指明 CBZ 为空");
        assert!(comic_dir.exists(), "空 CBZ 时必须保留原图");
        let _ = fs::remove_dir_all(&root);
    }

    // ── 护栏 2：相对路径 → 拒绝 ────────────────────────────────

    #[test]
    fn rejects_relative_path() {
        let root = temp_root("relative");
        let cbz = root.join("导出/1461316.cbz");
        let size = make_cbz(&cbz, 1024);

        let err =
            remove_comic_download_dir(Path::new("漫画下载/1461316"), &cbz, size).unwrap_err();

        assert!(
            err.to_string().contains("不是绝对路径"),
            "错误应指明路径非绝对，实际错误：{err}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    // ── 护栏 3：目录不存在 / 无子目录 → 拒绝 ────────────────────

    #[test]
    fn rejects_when_dir_missing() {
        let root = temp_root("dir-missing");
        let comic_dir = root.join("1461316"); // 故意不创建
        let cbz = root.join("导出/1461316.cbz");
        let size = make_cbz(&cbz, 1024);

        let err = remove_comic_download_dir(&comic_dir, &cbz, size).unwrap_err();

        assert!(
            err.to_string().contains("不存在或不是目录"),
            "错误应指明目录不存在，实际错误：{err}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_dir_without_subdir() {
        let root = temp_root("no-subdir");
        let comic_dir = root.join("1461316");
        fs::create_dir_all(&comic_dir).unwrap();
        // 只有元数据文件，没有任何章节目录 —— 不像一本已下载的漫画。
        fs::write(comic_dir.join("元数据.json"), b"{}").unwrap();
        let cbz = root.join("导出/1461316.cbz");
        let size = make_cbz(&cbz, 1024);

        let err = remove_comic_download_dir(&comic_dir, &cbz, size).unwrap_err();

        assert!(
            err.to_string().contains("没有任何章节子目录"),
            "错误应指明缺少章节子目录，实际错误：{err}"
        );
        assert!(comic_dir.exists(), "无章节子目录时必须保留目录");
        let _ = fs::remove_dir_all(&root);
    }

    // ── resolve_comic_download_dir ─────────────────────────────

    #[test]
    fn resolves_from_comic_download_dir_when_present() {
        let given = PathBuf::from("/data/漫画下载/1461316");
        let resolved = resolve_comic_download_dir(
            Some(&given),
            Path::new("/data/漫画下载"),
            "9999999",
        );
        assert_eq!(resolved, given, "有 comic_download_dir 时应直接用它");
    }

    #[test]
    fn falls_back_to_download_dir_join_comic_id() {
        let resolved = resolve_comic_download_dir(
            None,
            Path::new("/data/漫画下载"),
            "1461316",
        );
        assert_eq!(resolved, PathBuf::from("/data/漫画下载/1461316"));
    }
}
