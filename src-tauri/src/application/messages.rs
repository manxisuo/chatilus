use crate::error::AppResult;
use crate::infrastructure::db::Database;
use crate::domain::ports::MessageRepository;
use crate::models::{MessageView, SearchHit};

pub fn get_messages(db: &Database, conversation_id: &str) -> AppResult<Vec<MessageView>> {
    MessageRepository::list_by_conversation(db, conversation_id)
}

pub fn list_starred_messages(
    db: &Database,
    source: Option<&str>,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<SearchHit>> {
    MessageRepository::list_starred(db, source, limit, offset)
}

pub fn set_message_starred(
    db: &Database,
    message_id: &str,
    starred: bool,
) -> AppResult<()> {
    MessageRepository::set_starred(db, message_id, starred)
}
