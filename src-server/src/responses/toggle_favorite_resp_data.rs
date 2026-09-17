use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleFavoriteRespData {
    pub status: String,
    pub msg: String,
    #[serde(rename = "type")]
    pub toggle_type: ToggleType,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToggleType {
    #[default]
    Add,
    Remove,
}
