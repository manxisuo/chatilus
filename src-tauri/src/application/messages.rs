use crate::db::Database;
use crate::domain::ports::MessageRepository;
use crate::models::MessageView;

pub fn get_messages(db: &Database, conversation_id: &str) -> Result<Vec<MessageView>, String> {
    MessageRepository::list_by_conversation(db, conversation_id)
}

pub fn set_message_starred(
    db: &Database,
    message_id: &str,
    starred: bool,
) -> Result<(), String> {
    MessageRepository::set_starred(db, message_id, starred)
}
