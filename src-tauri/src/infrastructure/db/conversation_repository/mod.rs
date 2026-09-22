mod export;
mod list;
mod mutate;
mod save;
mod shared;
mod timeline;

#[cfg(test)]
mod tests;

use crate::domain::models::DataSource;
use crate::domain::ports::{
    ConversationListQuery, ConversationRepository, ImportPersistCounts, ImportedConversation,
    TimelineListQuery,
};
use crate::error::AppResult;
use crate::infrastructure::db::Database;
use crate::models::{ConversationSummary, ExportResult, TimelineMonthBucket};

impl ConversationRepository for Database {
    fn list(&self, query: ConversationListQuery) -> AppResult<Vec<ConversationSummary>> {
        list::list(self, query)
    }

    fn list_timeline(
        &self,
        query: TimelineListQuery,
    ) -> AppResult<Vec<ConversationSummary>> {
        timeline::list_timeline(self, query)
    }

    fn list_timeline_months(
        &self,
        source: Option<&str>,
    ) -> AppResult<Vec<TimelineMonthBucket>> {
        timeline::list_timeline_months(self, source)
    }

    fn get_summary(&self, conversation_id: &str) -> AppResult<Option<ConversationSummary>> {
        list::get_summary(self, conversation_id)
    }

    fn set_starred(&self, conversation_id: &str, starred: bool) -> AppResult<()> {
        mutate::set_starred(self, conversation_id, starred)
    }

    fn set_tags(
        &self,
        conversation_id: &str,
        tag_ids: &[i64],
    ) -> AppResult<Vec<String>> {
        mutate::set_tags(self, conversation_id, tag_ids)
    }

    fn export_markdown(
        &self,
        conversation_id: &str,
        output_path: &std::path::Path,
    ) -> AppResult<ExportResult> {
        export::export_markdown(self, conversation_id, output_path)
    }

    fn save_many(
        &mut self,
        conversations: &[ImportedConversation],
        source_path: &str,
        data_source: DataSource,
        conversations_deduplicated: usize,
    ) -> AppResult<ImportPersistCounts> {
        save::save_many(
            self,
            conversations,
            source_path,
            data_source,
            conversations_deduplicated,
        )
    }
}
