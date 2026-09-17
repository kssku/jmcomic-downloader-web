use serde::{Deserialize, Serialize};

mod chapter_info;
mod comic;
mod downloaded_format;
mod get_favorite_result;
mod get_favorite_sort;
mod log_level;
mod search_result;
mod search_sort;

pub use chapter_info::*;
pub use comic::*;
pub use downloaded_format::*;
pub use get_favorite_result::*;
pub use get_favorite_sort::*;
pub use log_level::*;
pub use search_result::*;
pub use search_sort::*;
/// jm 分类（搜索/周榜用）。
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: Option<String>,
    pub title: Option<String>,
}

/// jm 子分类。
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategorySub {
    pub id: Option<String>,
    pub title: Option<String>,
}