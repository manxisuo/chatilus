/// Normalized conversation bundle produced by an Importer, ready for persistence.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportPackage {
    pub conversations: Vec<ImportedConversation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedConversation {
    pub id: String,
    pub title: String,
    pub create_time: Option<f64>,
    pub update_time: Option<f64>,
    pub model: Option<String>,
    pub messages: Vec<ImportedMessage>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub create_time: Option<f64>,
    pub raw_json: String,
    pub attachments: Vec<ImportedAttachment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedAttachment {
    pub pointer: String,
    pub source: String,
    pub prompt: Option<String>,
    /// Local file path resolved during import (if media index matched the pointer).
    pub path: Option<String>,
}
