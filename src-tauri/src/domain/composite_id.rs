use super::models::DataSource;

/// 会话在库内的稳定主键：`{source}::{source_id}`。
pub fn composite_conversation_id(source: DataSource, source_id: &str) -> String {
    format!("{}::{}", source.as_str(), source_id)
}

pub fn is_composite_conversation_id(id: &str) -> bool {
    id.split_once("::")
        .is_some_and(|(prefix, rest)| !rest.is_empty() && is_known_source_prefix(prefix))
}

pub fn parse_composite_conversation_id(id: &str) -> Option<(DataSource, String)> {
    let (prefix, rest) = id.split_once("::")?;
    if rest.is_empty() || !is_known_source_prefix(prefix) {
        return None;
    }
    Some((DataSource::parse(prefix), rest.to_string()))
}

fn is_known_source_prefix(prefix: &str) -> bool {
    matches!(prefix, "chatgpt" | "cursor" | "claude" | "gemini")
}

pub fn message_storage_id(conversation_storage_id: &str, message_source_id: &str) -> String {
    format!("{conversation_storage_id}::{message_source_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_composite_conversation_id() {
        assert_eq!(
            composite_conversation_id(DataSource::Cursor, "abc-123"),
            "cursor::abc-123"
        );
    }

    #[test]
    fn detects_composite_conversation_id() {
        assert!(is_composite_conversation_id("chatgpt::conv-1"));
        assert!(is_composite_conversation_id("cursor::uuid"));
        assert!(!is_composite_conversation_id("conv-1"));
        assert!(!is_composite_conversation_id("unknown::x"));
    }

    #[test]
    fn builds_message_storage_id() {
        assert_eq!(
            message_storage_id("cursor::conv-1", "bubble-1"),
            "cursor::conv-1::bubble-1"
        );
    }
}
