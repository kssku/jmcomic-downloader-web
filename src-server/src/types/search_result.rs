use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    path::PathBuf,
};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::{
    context::AppContext,
    responses::{ComicInSearchRespData, SearchRespData},
    utils,
};

/// jm 搜索接口固定每页返回的条数。
///
/// jm 的 `/search` 请求不传 limit，服务端约定每页 80 条；
/// 这里显式声明，供 `pages` 换算与前端分页使用。
pub const SEARCH_PAGE_SIZE: i64 = 80;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult(pub SearchList<ComicInSearch>);

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchList<T> {
    pub search_query: String,
    pub total: i64,
    /// 每页条数（jm 固定 80），用于前端分页显示，语义对齐原 pica 版。
    pub limit: i64,
    /// 当前页码（1-based）。
    pub page: i64,
    /// 总页数 = ceil(total / limit)，至少 1。
    pub pages: i64,
    pub docs: Vec<T>,
}

impl Deref for SearchResult {
    type Target = SearchList<ComicInSearch>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SearchResult {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl SearchResult {
    pub fn from_resp_data(
        app: &AppContext,
        resp_data: SearchRespData,
        page: i64,
    ) -> anyhow::Result<SearchResult> {
        let id_to_dir_map =
            utils::create_id_to_dir_map(app).context("创建漫画ID到下载目录映射失败")?;

        let mut docs = Vec::new();
        for comic in resp_data.content {
            let comic = ComicInSearch::from_resp_data(comic, &id_to_dir_map);
            docs.push(comic);
        }

        let limit = SEARCH_PAGE_SIZE;
        let total = resp_data.total;
        let pages = if total > 0 {
            (total + limit - 1) / limit
        } else {
            1
        };

        let result = SearchResult(SearchList {
            search_query: resp_data.search_query,
            total,
            limit,
            page,
            pages,
            docs,
        });

        Ok(result)
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComicInSearch {
    pub id: String,
    pub author: String,
    pub name: String,
    pub image: String,
    pub liked: bool,
    pub is_favorite: bool,
    pub update_at: i64,
    pub is_downloaded: bool,
    pub comic_download_dir: PathBuf,
}

impl ComicInSearch {
    pub fn from_resp_data(
        resp_data: ComicInSearchRespData,
        id_to_dir_map: &HashMap<String, PathBuf>,
    ) -> ComicInSearch {
        let mut comic = ComicInSearch {
            id: resp_data.id,
            author: resp_data.author,
            name: resp_data.name,
            image: resp_data.image,
            liked: resp_data.liked,
            is_favorite: resp_data.is_favorite,
            update_at: resp_data.update_at,
            is_downloaded: false,
            comic_download_dir: PathBuf::new(),
        };

        comic.update_fields(id_to_dir_map);

        comic
    }

    pub fn update_fields(&mut self, id_to_dir_map: &HashMap<String, PathBuf>) {
        if let Some(comic_download_dir) = id_to_dir_map.get(&self.id) {
            self.comic_download_dir = comic_download_dir.clone();
            self.is_downloaded = true;
        }
    }
}
