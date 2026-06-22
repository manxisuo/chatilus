use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub create_time: Option<f64>,
    pub update_time: Option<f64>,
    pub model: Option<String>,
    pub message_count: i64,
    pub is_starred: bool,
    pub source_path: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttachmentView {
    pub file_key: String,
    pub path: String,
    #[serde(default = "default_image_source")]
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

fn default_image_source() -> String {
    "unknown".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageView {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub create_time: Option<f64>,
    pub sort_order: i64,
    pub is_starred: bool,
    pub attachments: Vec<AttachmentView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub message_id: String,
    pub conversation_id: String,
    pub conversation_title: String,
    pub role: String,
    pub snippet: String,
    pub create_time: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagView {
    pub id: i64,
    pub name: String,
    pub conversation_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub conversations_imported: usize,
    pub messages_imported: usize,
    pub files_processed: usize,
    pub source_path: String,
    pub media_files_indexed: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageGalleryItem {
    pub path: String,
    pub file_key: String,
    pub conversation_id: String,
    pub conversation_title: String,
    pub message_id: String,
    pub role: String,
    pub create_time: Option<f64>,
    #[serde(default = "default_image_source")]
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStats {
    pub conversation_count: i64,
    pub message_count: i64,
    pub image_count: i64,
    pub generated_image_count: i64,
    pub upload_image_count: i64,
    pub starred_conversation_count: i64,
    pub starred_message_count: i64,
    pub tag_count: i64,
    pub db_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub path: String,
    pub message_count: i64,
}
