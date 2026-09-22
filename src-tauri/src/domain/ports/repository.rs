use crate::error::AppResult;
use crate::domain::models::ImageKindFilter;
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
    pub image_kind: ImageKindFilter,
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
    fn list(&self, query: ConversationListQuery) -> AppResult<Vec<ConversationSummary>>;
    fn list_timeline(
        &self,
        query: TimelineListQuery,
    ) -> AppResult<Vec<ConversationSummary>>;
    fn list_timeline_months(
        &self,
        source: Option<&str>,
    ) -> AppResult<Vec<crate::models::TimelineMonthBucket>>;
    fn get_summary(&self, conversation_id: &str) -> AppResult<Option<ConversationSummary>>;
    fn set_starred(&self, conversation_id: &str, starred: bool) -> AppResult<()>;
    fn set_tags(
        &self,
        conversation_id: &str,
        tag_ids: &[i64],
    ) -> AppResult<Vec<String>>;
    fn export_markdown(
        &self,
        conversation_id: &str,
        output_path: &std::path::Path,
    ) -> AppResult<ExportResult>;
    fn save_many(
        &mut self,
        conversations: &[ImportedConversation],
        source_path: &str,
        data_source: DataSource,
        conversations_deduplicated: usize,
    ) -> AppResult<ImportPersistCounts>;
}

pub trait MessageRepository {
    fn list_by_conversation(&self, conversation_id: &str) -> AppResult<Vec<MessageView>>;
    fn list_starred(
        &self,
        source: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<crate::models::SearchHit>>;
    fn set_starred(&self, message_id: &str, starred: bool) -> AppResult<()>;
}

pub trait AssetRepository {
    fn list(&self, query: AssetListQuery) -> AppResult<Vec<Asset>>;
    fn count(
        &self,
        image_kind: ImageKindFilter,
        conversation_source: Option<&str>,
    ) -> AppResult<i64>;
    fn count_by_source(&self) -> AppResult<(i64, i64, i64)>;
    fn image_counts_by_conversation_source(&self) -> AppResult<Vec<crate::models::SourceCount>>;
    fn count_filtered(
        &self,
        image_kind: ImageKindFilter,
        conversation_source: Option<&str>,
        month: Option<&str>,
        conversation_id: Option<&str>,
    ) -> AppResult<i64>;
    fn image_counts_by_message_month(
        &self,
        conversation_source: Option<&str>,
    ) -> AppResult<std::collections::HashMap<String, i64>>;
}
