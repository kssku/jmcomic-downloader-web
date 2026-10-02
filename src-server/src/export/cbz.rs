//! 把整本漫画的全部章节合成一个「单行本」CBZ。
//!
//! ## 与上游原版的差异
//!
//! 上游 `jmcomic-downloader` 是「每章一个 CBZ」，本模块是「整本一个 CBZ」：
//! 所有章节的图片按 `ChapterInfo.order` 顺序、全局连号命名后写进同一个 ZIP。
//! 阅读器（Komga / Kavita / Calibre）会把它当成一话完整阅读。
//!
//! ## 全局连号
//!
//! 章节内原文件名是 `001.jpg`、`002.jpg`…，跨章节会重名。若用子目录
//! （`001/001.jpg`）保留章节结构，部分阅读器不认。这里改成**全局连号**：
//!
//! ```text
//! 0001.jpg   第 1 章第 1 张
//! 0002.jpg   第 1 章第 2 张
//! ...
//! 0079.jpg   第 2 章第 1 张
//! ```
//!
//! 位数按总页数动态决定（至少 4 位），保证字典序 == 阅读序。
//!
//! ## 压缩方式
//!
//! 用 `Stored`（不压缩）。JPEG 本身已是压缩格式，再走 Deflate 只能省
//! 1%～2%，耗时却翻倍。ZIP 的 `Stored` 条目仍带 CRC32 校验，是完全合规的 ZIP。
//!
//! ## 内存
//!
//! 逐张流式写入（`io::copy`），单张峰值内存 = 一张图大小，
//! 不会把整本上千张图读进内存。

use std::{
    fs::File,
    io::{self, Write},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

use crate::types::{ChapterInfo, Comic};

/// CBZ 的图片扩展名白名单。非图片文件（`章节元数据.json` 等）一律跳过。
const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp", "bmp", "gif"];

/// 导出结果，供 API 返回给前端。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub comic_id: String,
    pub comic_title: String,
    /// 产出的 CBZ 绝对路径。
    pub cbz_path: PathBuf,
    /// 实际写入的图片总数。
    pub page_count: usize,
    /// 参与合成的章节数。导出成功时必然等于 `total_chapters`。
    pub chapter_count: usize,
    /// 漫画总章节数。
    pub total_chapters: usize,
    /// CBZ 文件大小（字节）。
    pub file_size: u64,
    /// 是否覆盖了已存在的 CBZ。
    pub overwritten: bool,
    /// 该漫画的下载根目录（`漫画下载/{漫画ID}`）。
    ///
    /// 回传它是为了让调用方在导出成功后删除原图：删除是**破坏性操作**，
    /// 由调用方决定做不做，导出函数本身只负责如实报告「图片在哪」。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comic_download_dir: Option<PathBuf>,
}

/// 把整本漫画合成一个单行本 CBZ。
///
/// **必须全部章节都已下载**，否则拒绝导出并报出还差几章。
/// 这是刻意的选择：单行本的意义是「一次读完」，缺章的单行本会让人
/// 以为漫画本身不完整。部分导出对阅读体验是负价值。
///
/// 产出路径：`{export_dir}/{漫画ID}.cbz`。
///
/// 用漫画 ID 而非漫画名做文件名：漫画名可能含空格、emoji、超长标题，
/// 也可能被外部工具改名导致「找不到已导出文件」；ID 稳定且唯一。
pub fn export_comic_cbz(comic: &Comic, export_dir: &Path) -> anyhow::Result<ExportResult> {
    // ── 1. 逐章判定「已下载」，分三类 ────────────────────────────
    //
    // 只看目录是否存在是不够的：下载中断会留下「目录 + 章节元数据.json
    // 但零张图片」的空壳残骸，那是**未完成**，不是已下载。把它当成已下载
    // 会让完整性校验误判通过，然后在收集图片阶段才炸出「没有找到任何图片」
    // ——错误信息指向了错误的原因。
    //
    // 所以这里按「目录里的图片数」判定：
    //   - 有图片        → 已下载，参与合成
    //   - 无目录        → 未下载
    //   - 有目录无图片  → 数据不完整（空壳残骸），单独报出来
    let total = comic.chapter_infos.len();
    let mut downloaded: Vec<&ChapterInfo> = Vec::new();
    let mut empty_shells: Vec<&ChapterInfo> = Vec::new();

    for chapter in &comic.chapter_infos {
        let Some(dir) = chapter.chapter_download_dir.as_ref() else {
            continue;
        };
        if !dir.is_dir() {
            continue;
        }
        let has_images = collect_images(dir)
            .map(|imgs| !imgs.is_empty())
            .unwrap_or(false);
        if has_images {
            downloaded.push(chapter);
        } else {
            empty_shells.push(chapter);
        }
    }

    // 空壳残骸比「未下载」更值得提醒：目录和元数据都在，只有图片没了，
    // 用户看到目录会以为下过了。优先报这个，并给出可操作的建议。
    if !empty_shells.is_empty() {
        let titles: Vec<String> = empty_shells
            .iter()
            .map(|c| format!("第{}话", c.order))
            .collect();
        return Err(anyhow!(
            "漫画`{}`有 {} 章目录存在但没有任何图片（{}），数据不完整，拒绝导出单行本；请重新下载这些章节",
            comic.name,
            empty_shells.len(),
            titles.join("、")
        ));
    }

    if downloaded.is_empty() {
        return Err(anyhow!(
            "漫画`{}`没有任何已下载的章节，无法导出",
            comic.name
        ));
    }

    // ── 1.1 完整性校验：缺章直接拒绝 ────────────────────────────
    let missing = total.saturating_sub(downloaded.len());
    if missing > 0 {
        return Err(anyhow!(
            "漫画`{}`还有 {} 章未下载（已下载 {}/{}），拒绝导出单行本",
            comic.name,
            missing,
            downloaded.len(),
            total
        ));
    }

    downloaded.sort_by_key(|c| c.order);

    // ── 2. 逐章收集图片路径（保持章内顺序）──────────────────────
    // 用 (章节序号, 章内序号, 路径) 保证全局有序。
    let mut images: Vec<PathBuf> = Vec::new();
    for chapter in &downloaded {
        let dir = chapter
            .chapter_download_dir
            .as_ref()
            .context("`chapter_download_dir`字段为`None`")?;
        let mut chapter_images = collect_images(dir).with_context(|| {
            format!("收集章节`{}`的图片失败", chapter.chapter_title)
        })?;
        // `collect_images` 已按文件名排序，这里直接追加即可保证全局顺序
        images.append(&mut chapter_images);
    }

    if images.is_empty() {
        return Err(anyhow!(
            "漫画`{}`的已下载章节里没有找到任何图片",
            comic.name
        ));
    }

    // ── 3. 准备输出路径 ────────────────────────────────────────
    std::fs::create_dir_all(export_dir)
        .with_context(|| format!("创建导出目录`{}`失败", export_dir.display()))?;

    // 文件名用漫画 ID：稳定、唯一、不含特殊字符，外部工具改名也不影响重导覆盖
    let cbz_name = format!("{}.cbz", comic.id);
    let cbz_path = export_dir.join(cbz_name);
    let overwritten = cbz_path.exists();

    // ── 4. 写 ZIP：先 ComicInfo.xml，再全局连号的图片 ───────────
    let total_pages = images.len();
    // 至少 4 位；超过 9999 页时自动加宽
    let width = total_pages.to_string().len().max(4);

    let file = File::create(&cbz_path)
        .with_context(|| format!("创建文件`{}`失败", cbz_path.display()))?;
    let mut zip = ZipWriter::new(file);
    // Stored：JPEG 再压缩收益极低，省掉 Deflate 的 CPU
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    // 4.1 ComicInfo.xml —— CBZ 标准的核心，必须在根目录
    let comic_info_xml = build_comic_info_xml(comic, total_pages);
    zip.start_file("ComicInfo.xml", opts)
        .context("在 CBZ 中创建`ComicInfo.xml`失败")?;
    zip.write_all(comic_info_xml.as_bytes())
        .context("写入`ComicInfo.xml`失败")?;

    // 4.2 图片：全局连号，流式写入
    for (idx, image_path) in images.iter().enumerate() {
        let entry_name = format!("{:0width$}.{}", idx + 1, "jpg", width = width);
        zip.start_file(&entry_name, opts)
            .with_context(|| format!("在 CBZ 中创建条目`{entry_name}`失败"))?;

        let mut src = File::open(image_path)
            .with_context(|| format!("打开`{}`失败", image_path.display()))?;
        io::copy(&mut src, &mut zip)
            .with_context(|| format!("将`{}`写入 CBZ 失败", image_path.display()))?;
    }

    zip.finish().context("关闭 CBZ 文件失败")?;

    let file_size = std::fs::metadata(&cbz_path)
        .with_context(|| format!("读取`{}`元信息失败", cbz_path.display()))?
        .len();

    Ok(ExportResult {
        comic_id: comic.id.clone(),
        comic_title: comic.name.clone(),
        cbz_path,
        page_count: total_pages,
        chapter_count: downloaded.len(),
        total_chapters: comic.chapter_infos.len(),
        file_size,
        overwritten,
        comic_download_dir: comic.comic_download_dir.clone(),
    })
}

/// 收集目录下的图片文件，按文件名排序（`001.jpg` < `002.jpg` < `010.jpg`）。
///
/// 只取当前目录的**直接子文件**，不递归 —— 章节目录里不应有子目录，
/// 递归反而可能把封面之类不该进正文的图带进来。
pub(crate) fn collect_images(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut images: Vec<PathBuf> = Vec::new();

    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("读取目录`{}`失败", dir.display()))?;

    for entry in entries {
        let entry = entry.with_context(|| format!("遍历目录`{}`失败", dir.display()))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        let Some(ext) = ext else { continue };
        if IMAGE_EXTS.contains(&ext.as_str()) {
            images.push(path);
        }
    }

    // 文件名排序：下载时已按 001/002 命名，字典序即阅读序
    images.sort();
    Ok(images)
}

/// XML 文本转义。`ComicInfo` 里全是用户数据（标题/标签/简介），必须转义。
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// 生成 `ComicInfo.xml`（Anansi Project 规范，Kavita / Komga / Calibre 通用）。
///
/// 手写 XML 而不引 `yaserde`：字段少，且能精确控制转义和可选字段的省略，
/// 避免为一个 XML 序列化库新增依赖。
fn build_comic_info_xml(comic: &Comic, page_count: usize) -> String {
    let mut xml = String::with_capacity(1024);
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<ComicInfo xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" ");
    xml.push_str("xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\">\n");

    // 固定字段
    xml.push_str("  <Manga>Yes</Manga>\n");
    xml.push_str(&format!(
        "  <Series>{}</Series>\n",
        xml_escape(&comic.name)
    ));
    xml.push_str("  <Publisher>禁漫天堂</Publisher>\n");

    // 作者（jm 的 author 是数组）
    if !comic.author.is_empty() {
        xml.push_str(&format!(
            "  <Writer>{}</Writer>\n",
            xml_escape(&comic.author.join(", "))
        ));
    }

    // 标签
    if !comic.tags.is_empty() {
        xml.push_str(&format!(
            "  <Tags>{}</Tags>\n",
            xml_escape(&comic.tags.join(", "))
        ));
    }

    // 简介
    if !comic.description.is_empty() {
        xml.push_str(&format!(
            "  <Summary>{}</Summary>\n",
            xml_escape(&comic.description)
        ));
    }

    // 单行本：Title 用漫画名，Number/Volume 留空（整本没有单一序号）
    xml.push_str(&format!("  <Title>{}</Title>\n", xml_escape(&comic.name)));

    // 章节总数：0 = Ongoing，非零 = 完结/连载中
    xml.push_str(&format!(
        "  <Count>{}</Count>\n",
        comic.chapter_infos.len()
    ));

    // 总页数
    xml.push_str(&format!("  <PageCount>{page_count}</PageCount>\n"));

    xml.push_str("</ComicInfo>\n");
    xml
}

/// 按漫画 ID 查找已导出的 CBZ 文件。
///
/// 文件名就是 `{漫画ID}.cbz`，直接拼路径即可，无需遍历导出目录。
pub fn find_exported_cbz(export_dir: &Path, comic_id: &str) -> Option<PathBuf> {
    let cbz_name = format!("{comic_id}.cbz");
    let path = export_dir.join(cbz_name);
    path.is_file().then_some(path)
}