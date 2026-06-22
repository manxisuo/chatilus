use crate::models::{AttachmentView, MessageView};

use super::super::models::{DataSource, Message, MessageRole};

pub fn message_from_db_fields(
    id: String,
    conversation_id: String,
    role: &str,
    plain_text: String,
    created_at: Option<f64>,
    sort_order: i64,
    is_starred: bool,
    attachments: &[AttachmentView],
    raw_ref: Option<String>,
) -> Message {
    let asset_ids: Vec<String> = attachments
        .iter()
        .map(|attachment| attachment.file_key.clone())
        .collect();

    Message {
        id: id.clone(),
        conversation_id,
        source: DataSource::ChatGpt,
        source_id: Message::extract_source_id(&id),
        role: MessageRole::parse(role),
        content: Message::build_content(&plain_text, &asset_ids),
        plain_text,
        created_at,
        sort_order,
        is_favorite: is_starred,
        parent_id: None,
        asset_ids,
        raw_ref,
    }
}

pub fn view_to_message(view: &MessageView) -> Message {
    let asset_ids: Vec<String> = view
        .attachments
        .iter()
        .map(|attachment| attachment.file_key.clone())
        .collect();

    Message {
        id: view.id.clone(),
        conversation_id: view.conversation_id.clone(),
        source: DataSource::ChatGpt,
        source_id: Message::extract_source_id(&view.id),
        role: MessageRole::parse(&view.role),
        content: Message::build_content(&view.content, &asset_ids),
        plain_text: view.content.clone(),
        created_at: view.create_time,
        sort_order: view.sort_order,
        is_favorite: view.is_starred,
        parent_id: None,
        asset_ids,
        raw_ref: None,
    }
}

pub fn message_to_view(message: Message, attachments: Vec<AttachmentView>) -> MessageView {
    MessageView {
        id: message.id,
        conversation_id: message.conversation_id,
        role: message.role.as_str().to_string(),
        content: message.plain_text,
        create_time: message.created_at,
        sort_order: message.sort_order,
        is_starred: message.is_favorite,
        attachments,
    }
}

impl From<MessageView> for Message {
    fn from(view: MessageView) -> Self {
        view_to_message(&view)
    }
}

impl From<&MessageView> for Message {
    fn from(view: &MessageView) -> Self {
        view_to_message(view)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::super::models::MessageContent;

    #[test]
    fn message_view_to_domain_builds_content_blocks() {
        let view = MessageView {
            id: "conv::msg-1".to_string(),
            conversation_id: "conv".to_string(),
            role: "assistant".to_string(),
            content: "你好".to_string(),
            create_time: Some(100.0),
            sort_order: 0,
            is_starred: false,
            attachments: vec![AttachmentView {
                file_key: "file-key-1".to_string(),
                path: "/tmp/a.png".to_string(),
                source: "generated".to_string(),
                prompt: Some("a cat".to_string()),
            }],
        };

        let message: Message = (&view).into();
        assert_eq!(message.source_id.as_deref(), Some("msg-1"));
        assert_eq!(message.asset_ids, vec!["file-key-1"]);
        assert_eq!(
            message.content,
            vec![
                MessageContent::Text {
                    text: "你好".to_string()
                },
                MessageContent::ImageRef {
                    asset_id: "file-key-1".to_string()
                },
            ]
        );
    }

    #[test]
    fn message_domain_to_view_preserves_attachments() {
        let view = MessageView {
            id: "conv::msg-2".to_string(),
            conversation_id: "conv".to_string(),
            role: "user".to_string(),
            content: "附图".to_string(),
            create_time: None,
            sort_order: 1,
            is_starred: true,
            attachments: vec![AttachmentView {
                file_key: "upload-1".to_string(),
                path: "/tmp/b.jpg".to_string(),
                source: "upload".to_string(),
                prompt: None,
            }],
        };

        let message: Message = view.clone().into();
        let restored = message_to_view(message, view.attachments.clone());
        assert_eq!(restored.id, view.id);
        assert_eq!(restored.content, view.content);
        assert_eq!(restored.attachments, view.attachments);
        assert_eq!(restored.is_starred, view.is_starred);
    }
}
