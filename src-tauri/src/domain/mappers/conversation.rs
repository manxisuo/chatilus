use crate::models::ConversationSummary;

use super::super::models::{Conversation, DataSource};

pub fn conversation_from_row(
    id: String,
    title: String,
    created_at: Option<f64>,
    updated_at: Option<f64>,
    model: Option<String>,
    message_count: i64,
    is_starred: bool,
    source_path: String,
    tags: Vec<String>,
) -> Conversation {
    Conversation {
        id: id.clone(),
        source: DataSource::ChatGpt,
        source_id: Some(id),
        title,
        created_at,
        updated_at,
        message_count,
        asset_count: 0,
        is_favorite: is_starred,
        tags,
        model,
        import_path: source_path,
        summary: None,
        raw_ref: None,
    }
}

pub fn conversation_to_summary(conversation: Conversation) -> ConversationSummary {
    ConversationSummary {
        id: conversation.id,
        title: conversation.title,
        create_time: conversation.created_at,
        update_time: conversation.updated_at,
        model: conversation.model,
        message_count: conversation.message_count,
        is_starred: conversation.is_favorite,
        source_path: conversation.import_path,
        tags: conversation.tags,
    }
}

impl From<ConversationSummary> for Conversation {
    fn from(summary: ConversationSummary) -> Self {
        conversation_from_row(
            summary.id,
            summary.title,
            summary.create_time,
            summary.update_time,
            summary.model,
            summary.message_count,
            summary.is_starred,
            summary.source_path,
            summary.tags,
        )
    }
}

impl From<Conversation> for ConversationSummary {
    fn from(conversation: Conversation) -> Self {
        conversation_to_summary(conversation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_summary_round_trip() {
        let summary = ConversationSummary {
            id: "conv-1".to_string(),
            title: "测试对话".to_string(),
            create_time: Some(1_700_000_000.0),
            update_time: Some(1_700_000_100.0),
            model: Some("gpt-4".to_string()),
            message_count: 12,
            is_starred: true,
            source_path: "/exports/chatgpt".to_string(),
            tags: vec!["工作".to_string(), "Rust".to_string()],
        };

        let domain: Conversation = summary.clone().into();
        assert_eq!(domain.source, DataSource::ChatGpt);
        assert_eq!(domain.source_id.as_deref(), Some("conv-1"));
        assert_eq!(domain.is_favorite, true);
        assert_eq!(domain.import_path, "/exports/chatgpt");

        let back: ConversationSummary = domain.into();
        assert_eq!(back, summary);
    }
}
