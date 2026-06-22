use serde::{Deserialize, Serialize};

use super::DataSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
    Unknown,
}

impl MessageRole {
    pub fn parse(value: &str) -> Self {
        match value {
            "user" => Self::User,
            "assistant" => Self::Assistant,
            "system" => Self::System,
            "tool" => Self::Tool,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::System => "system",
            Self::Tool => "tool",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageContent {
    Text { text: String },
    ImageRef { asset_id: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub source: DataSource,
    pub source_id: Option<String>,
    pub role: MessageRole,
    pub content: Vec<MessageContent>,
    pub plain_text: String,
    pub created_at: Option<f64>,
    pub sort_order: i64,
    pub is_favorite: bool,
    pub parent_id: Option<String>,
    pub asset_ids: Vec<String>,
    pub raw_ref: Option<String>,
}

impl Message {
    pub fn build_content(plain_text: &str, asset_ids: &[String]) -> Vec<MessageContent> {
        let mut blocks = Vec::new();
        if !plain_text.is_empty() {
            blocks.push(MessageContent::Text {
                text: plain_text.to_string(),
            });
        }
        for asset_id in asset_ids {
            blocks.push(MessageContent::ImageRef {
                asset_id: asset_id.clone(),
            });
        }
        blocks
    }

    /// 从 DB 复合 ID（`conversation_id::message_id`）提取外部平台消息 ID。
    pub fn extract_source_id(stored_id: &str) -> Option<String> {
        stored_id
            .rsplit_once("::")
            .map(|(_, source_id)| source_id.to_string())
            .filter(|id| !id.is_empty())
    }
}
