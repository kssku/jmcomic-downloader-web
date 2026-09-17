use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FavoriteSort {
    TimeNewest,
    TimeOldest,
}

impl FavoriteSort {
    pub fn as_str(&self) -> &'static str {
        match self {
            FavoriteSort::TimeNewest => "dd",
            FavoriteSort::TimeOldest => "da",
        }
    }
}
