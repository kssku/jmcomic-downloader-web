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

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult(pub SearchList<ComicInSearch>);

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchList<T> {
    pub search_query: String,
    pub total: i64,
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
    ) -> anyhow::Result<SearchResult> {
        let id_to_dir_map =
            utils::create_id_to_dir_map(app).context("创建漫画ID到下载目录映射失败")?;

        let mut docs = Vec::new();
        for comic in resp_data.content {
            let comic = ComicInSearch::from_resp_data(comic, &id_to_dir_map);
            docs.push(comic);
        }

        let result = SearchResult(SearchList {
            search_query: resp_data.search_query,
            total: resp_data.total,
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
