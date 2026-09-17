use std::{collections::HashMap, path::PathBuf};

use anyhow::Context;
use walkdir::WalkDir;

use crate::{
    context::AppContext,
    extensions::{AppContextExt, WalkDirEntryExt},
    types::Comic,
};

/// 计算 MD5 并返回十六进制字符串（jm 签名/数据解密用）。
pub fn md5_hex(data: &str) -> String {
    format!("{:x}", md5::compute(data))
}

pub fn filename_filter(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\\' | '/' | '\n' => ' ',
            ':' => '：',
            '*' => '⭐',
            '?' => '？',
            '"' => '\'',
            '<' => '《',
            '>' => '》',
            '|' => '丨',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .trim_end_matches('.')
        .trim()
        .to_string()
}

pub async fn get_comic(app: &AppContext, comic_id: &str) -> anyhow::Result<Comic> {
    let aid: i64 = comic_id.parse().context("comic_id 无法解析为 i64")?;
    let jm_client = app.get_jm_client();
    let comic_resp_data = jm_client
        .get_comic(aid)
        .await
        .context("获取漫画详情失败")?;
    let comic = Comic::from_comic_resp_data(app, comic_resp_data)?;
    Ok(comic)
}

pub fn create_id_to_dir_map(app: &AppContext) -> anyhow::Result<HashMap<String, PathBuf>> {
    let mut id_to_dir_map: HashMap<String, PathBuf> = HashMap::new();
    let download_dir = app.get_config().read().download_dir.clone();
    if !download_dir.exists() {
        return Ok(id_to_dir_map);
    }

    for entry in WalkDir::new(&download_dir)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !entry.is_comic_metadata() {
            continue;
        }

        let metadata_str =
            std::fs::read_to_string(path).context(format!("读取`{}`失败", path.display()))?;
        let comic_json: serde_json::Value = serde_json::from_str(&metadata_str)
            .context(format!("将`{}`反序列化为serde_json::Value失败", path.display()))?;
        let id = comic_json
            .get("id")
            .and_then(|id| id.as_str())
            .context(format!("`{path:?}`没有`id`字段"))?
            .to_string();

        let parent = path
            .parent()
            .context(format!("`{}`没有父目录", path.display()))?;

        id_to_dir_map.entry(id).or_insert(parent.to_path_buf());
    }

    Ok(id_to_dir_map)
}
