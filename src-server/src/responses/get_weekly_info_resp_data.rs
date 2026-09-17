use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GetWeeklyInfoRespData {
    pub categories: Vec<CategoryInWeeklyInfo>,
    #[serde(rename = "type")]
    pub type_field: Vec<WeeklyType>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategoryInWeeklyInfo {
    pub id: String,
    pub title: String,
    pub time: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeeklyType {
    pub id: String,
    pub title: String,
}
