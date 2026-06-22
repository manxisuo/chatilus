use serde::{Deserialize, Serialize};

use super::DataSource;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub source: DataSource,
    pub source_id: Option<String>,
    pub title: String,
    pub created_at: Option<f64>,
    pub updated_at: Option<f64>,
    pub message_count: i64,
    pub asset_count: i64,
    pub is_favorite: bool,
    pub tags: Vec<String>,
    /// 默认模型（ChatGPT `default_model_slug`）
    pub model: Option<String>,
    /// 导入来源目录路径
    pub import_path: String,
    pub summary: Option<String>,
    pub raw_ref: Option<String>,
}
