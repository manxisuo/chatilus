use std::path::Path;

use crate::db::Database;
use crate::domain::ports::{ConversationListQuery, ConversationRepository};
use crate::models::{ConversationSummary, ExportResult};

pub fn list_conversations(
    db: &Database,
    query: Option<&str>,
    starred_only: bool,
    tag_id: Option<i64>,
    source: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<ConversationSummary>, String> {
    ConversationRepository::list(
        db,
        ConversationListQuery {
            text_query: query.map(str::to_string),
            starred_only,
            tag_id,
            source: source.map(str::to_string),
            limit,
            offset,
        },
    )
}

pub fn set_conversation_starred(
    db: &Database,
    conversation_id: &str,
    starred: bool,
) -> Result<(), String> {
    ConversationRepository::set_starred(db, conversation_id, starred)
}

pub fn set_conversation_tags(
    db: &Database,
    conversation_id: &str,
    tag_ids: &[i64],
) -> Result<Vec<String>, String> {
    ConversationRepository::set_tags(db, conversation_id, tag_ids)
}

pub fn export_conversation_markdown(
    db: &Database,
    conversation_id: &str,
    output_path: &Path,
) -> Result<ExportResult, String> {
    ConversationRepository::export_markdown(db, conversation_id, output_path)
}
