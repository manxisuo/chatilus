use crate::domain::models::Asset;
use crate::domain::models::DataSource;
use crate::domain::ports::ImportedConversation;
use crate::models::{ConversationSummary, ExportResult, MessageView};

#[derive(Debug, Clone, Default)]
pub struct ConversationListQuery {
    pub text_query: Option<String>,
    pub starred_only: bool,
    pub tag_id: Option<i64>,
    pub source: Option<String>,
    pub has_images: bool,
    pub has_code: bool,
    pub has_attachments: bool,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Default)]
pub struct TimelineListQuery {
    pub source: Option<String>,
    pub month: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone)]
pub struct AssetListQuery {
    pub limit: i64,
    pub offset: i64,
    pub include_uploads: bool,
    pub conversation_source: Option<String>,
    pub month: Option<String>,
    pub conversation_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportPersistCounts {
    pub new_conversations: usize,
    pub updated_conversations: usize,
    pub messages: usize,
    pub conversations_deduplicated: usize,
}

pub trait ConversationRepository {
    fn list(&self, query: ConversationListQuery) -> Result<Vec<ConversationSummary>, String>;
    fn list_timeline(
        &self,
        query: TimelineListQuery,
    ) -> Result<Vec<ConversationSummary>, String>;
    fn list_timeline_months(
        &self,
        source: Option<&str>,
    ) -> Result<Vec<crate::models::TimelineMonthBucket>, String>;
    fn get_summary(&self, conversation_id: &str) -> Result<Option<ConversationSummary>, String>;
    fn set_starred(&self, conversation_id: &str, starred: bool) -> Result<(), String>;
    fn set_tags(
        &self,
        conversation_id: &str,
        tag_ids: &[i64],
    ) -> Result<Vec<String>, String>;
    fn export_markdown(
        &self,
        conversation_id: &str,
        output_path: &std::path::Path,
    ) -> Result<ExportResult, String>;
    fn save_many(
        &mut self,
        conversations: &[ImportedConversation],
        source_path: &str,
        data_source: DataSource,
        conversations_deduplicated: usize,
    ) -> Result<ImportPersistCounts, String>;
}

pub trait MessageRepository {
    fn list_by_conversation(&self, conversation_id: &str) -> Result<Vec<MessageView>, String>;
    fn list_starred(
        &self,
        source: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<crate::models::SearchHit>, String>;
    fn set_starred(&self, message_id: &str, starred: bool) -> Result<(), String>;
}

pub trait AssetRepository {
    fn list(&self, query: AssetListQuery) -> Result<Vec<Asset>, String>;
    fn count(
        &self,
        include_uploads: bool,
        conversation_source: Option<&str>,
    ) -> Result<i64, String>;
    fn count_by_source(&self) -> Result<(i64, i64, i64), String>;
    fn image_counts_by_conversation_source(&self) -> Result<Vec<crate::models::SourceCount>, String>;
    fn count_filtered(
        &self,
        include_uploads: bool,
        conversation_source: Option<&str>,
        month: Option<&str>,
        conversation_id: Option<&str>,
    ) -> Result<i64, String>;
    fn image_counts_by_message_month(
        &self,
        conversation_source: Option<&str>,
    ) -> Result<std::collections::HashMap<String, i64>, String>;
}
