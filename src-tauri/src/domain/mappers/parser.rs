use crate::domain::ports::{ImportedAttachment, ImportedConversation, ImportedMessage};

use super::super::models::{Conversation, DataSource, Message, MessageRole};

pub fn parsed_conversation_to_domain(
    parsed: &ImportedConversation,
    import_path: &str,
) -> Conversation {
    let asset_count = parsed
        .messages
        .iter()
        .map(|message| message.attachments.len() as i64)
        .sum();

    Conversation {
        id: parsed.id.clone(),
        source: DataSource::ChatGpt,
        source_id: Some(parsed.id.clone()),
        title: parsed.title.clone(),
        created_at: parsed.create_time,
        updated_at: parsed.update_time,
        message_count: parsed.messages.len() as i64,
        asset_count,
        is_favorite: false,
        tags: Vec::new(),
        model: parsed.model.clone(),
        import_path: import_path.to_string(),
        summary: None,
        raw_ref: None,
    }
}

pub fn parsed_message_to_domain(
    parsed: &ImportedMessage,
    conversation_id: &str,
    sort_order: i64,
) -> Message {
    let asset_ids: Vec<String> = parsed
        .attachments
        .iter()
        .map(|attachment| attachment.pointer.clone())
        .collect();

    Message {
        id: format!("{conversation_id}::{}", parsed.id),
        conversation_id: conversation_id.to_string(),
        source: DataSource::ChatGpt,
        source_id: Some(parsed.id.clone()),
        role: MessageRole::parse(&parsed.role),
        content: Message::build_content(&parsed.content, &asset_ids),
        plain_text: parsed.content.clone(),
        created_at: parsed.create_time,
        sort_order,
        is_favorite: false,
        parent_id: None,
        asset_ids,
        raw_ref: Some(parsed.raw_json.clone()),
    }
}

pub fn parsed_attachment_asset_ids(attachments: &[ImportedAttachment]) -> Vec<String> {
    attachments
        .iter()
        .map(|attachment| attachment.pointer.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsed_conversation_maps_to_domain() {
        let parsed = ImportedConversation {
            id: "abc".to_string(),
            title: "Hello".to_string(),
            create_time: Some(1.0),
            update_time: Some(2.0),
            model: Some("gpt-4o".to_string()),
            messages: vec![ImportedMessage {
                id: "m1".to_string(),
                role: "user".to_string(),
                content: "hi".to_string(),
                create_time: None,
                raw_json: "{}".to_string(),
                attachments: vec![ImportedAttachment {
                    pointer: "ptr-1".to_string(),
                    source: "upload".to_string(),
                    prompt: None,
                    path: None,
                }],
            }],
            source_contexts: Vec::new(),
        };

        let domain = parsed_conversation_to_domain(&parsed, "/export");
        assert_eq!(domain.message_count, 1);
        assert_eq!(domain.asset_count, 1);
        assert_eq!(domain.import_path, "/export");

        let message = parsed_message_to_domain(&parsed.messages[0], "abc", 0);
        assert_eq!(message.source_id.as_deref(), Some("m1"));
        assert_eq!(message.asset_ids, vec!["ptr-1"]);
        assert_eq!(message.raw_ref.as_deref(), Some("{}"));
    }
}
