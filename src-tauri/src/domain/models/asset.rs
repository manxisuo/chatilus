use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::DataSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AssetType {
    Image,
    File,
    Code,
    Prompt,
    Link,
}

impl AssetType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::File => "file",
            Self::Code => "code",
            Self::Prompt => "prompt",
            Self::Link => "link",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub id: String,
    pub source: DataSource,
    pub conversation_id: Option<String>,
    pub message_id: Option<String>,
    pub asset_type: AssetType,
    pub title: Option<String>,
    pub mime_type: Option<String>,
    pub uri: Option<String>,
    pub local_path: Option<String>,
    pub content: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub created_at: Option<f64>,
    pub is_favorite: bool,
    pub tags: Vec<String>,
}
