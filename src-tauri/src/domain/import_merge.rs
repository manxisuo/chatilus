use std::cmp::Ordering;
use std::collections::HashMap;

use crate::domain::ports::{ImportedConversation, ImportedMessage};

/// 合并同一导入包内重复出现的对话（按 `id` 去重）。
pub fn dedup_import_package(
    conversations: Vec<ImportedConversation>,
) -> (Vec<ImportedConversation>, usize) {
    let mut merged: HashMap<String, ImportedConversation> = HashMap::new();
    let mut duplicates = 0usize;

    for conversation in conversations {
        if let Some(existing) = merged.remove(&conversation.id) {
            duplicates += 1;
            merged.insert(
                conversation.id.clone(),
                merge_imported_conversations(existing, conversation),
            );
        } else {
            merged.insert(conversation.id.clone(), conversation);
        }
    }

    let mut result: Vec<_> = merged.into_values().collect();
    result.sort_by(|left, right| compare_conversation_order(left, right));
    (result, duplicates)
}

pub fn merge_imported_conversations(
    left: ImportedConversation,
    right: ImportedConversation,
) -> ImportedConversation {
    let (base, other) = if prefer_conversation(&left, &right) {
        (left, right)
    } else {
        (right, left)
    };

    let mut messages: HashMap<String, ImportedMessage> = base
        .messages
        .into_iter()
        .map(|message| (message.id.clone(), message))
        .collect();

    for message in other.messages {
        match messages.get(&message.id) {
            Some(existing) if !prefer_message(&message, existing) => {}
            _ => {
                messages.insert(message.id.clone(), message);
            }
        }
    }

    let mut merged_messages: Vec<_> = messages.into_values().collect();
    sort_messages(&mut merged_messages);

    ImportedConversation {
        id: base.id,
        title: base.title,
        create_time: pick_earlier_time(base.create_time, other.create_time),
        update_time: pick_later_time(base.update_time, other.update_time),
        model: base.model.or(other.model),
        messages: merged_messages,
    }
}

fn prefer_conversation(left: &ImportedConversation, right: &ImportedConversation) -> bool {
    match (
        left.update_time.unwrap_or(0.0),
        right.update_time.unwrap_or(0.0),
    ) {
        (l, r) if l != r => l >= r,
        _ => left.messages.len() >= right.messages.len(),
    }
}

fn prefer_message(candidate: &ImportedMessage, existing: &ImportedMessage) -> bool {
    match (
        candidate.create_time.unwrap_or(0.0),
        existing.create_time.unwrap_or(0.0),
    ) {
        (c, e) if c != e => c >= e,
        _ => candidate.content.len() >= existing.content.len(),
    }
}

fn sort_messages(messages: &mut [ImportedMessage]) {
    messages.sort_by(|left, right| {
        left.create_time
            .partial_cmp(&right.create_time)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn compare_conversation_order(left: &ImportedConversation, right: &ImportedConversation) -> Ordering {
    right
        .update_time
        .unwrap_or(0.0)
        .partial_cmp(&left.update_time.unwrap_or(0.0))
        .unwrap_or(Ordering::Equal)
        .then_with(|| left.title.cmp(&right.title))
}

fn pick_earlier_time(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (Some(l), Some(r)) => Some(l.min(r)),
        (Some(l), None) => Some(l),
        (None, Some(r)) => Some(r),
        (None, None) => None,
    }
}

fn pick_later_time(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (Some(l), Some(r)) => Some(l.max(r)),
        (Some(l), None) => Some(l),
        (None, Some(r)) => Some(r),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ports::ImportedAttachment;

    fn sample_conversation(id: &str, update_time: f64, message_ids: &[&str]) -> ImportedConversation {
        ImportedConversation {
            id: id.to_string(),
            title: format!("Title {id}"),
            create_time: Some(update_time - 10.0),
            update_time: Some(update_time),
            model: Some("gpt-4".to_string()),
            messages: message_ids
                .iter()
                .enumerate()
                .map(|(index, message_id)| ImportedMessage {
                    id: (*message_id).to_string(),
                    role: "user".to_string(),
                    content: format!("content-{message_id}"),
                    create_time: Some(update_time + index as f64),
                    raw_json: "{}".to_string(),
                    attachments: Vec::<ImportedAttachment>::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn dedup_merges_duplicate_conversations_in_package() {
        let first = sample_conversation("c1", 100.0, &["m1"]);
        let second = sample_conversation("c1", 200.0, &["m2"]);
        let (merged, duplicates) = dedup_import_package(vec![first, second]);

        assert_eq!(duplicates, 1);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].messages.len(), 2);
        assert_eq!(merged[0].update_time, Some(200.0));
    }

    #[test]
    fn merge_keeps_distinct_messages_by_source_id() {
        let left = sample_conversation("c1", 100.0, &["m1", "m2"]);
        let right = sample_conversation("c1", 150.0, &["m2", "m3"]);
        let merged = merge_imported_conversations(left, right);

        assert_eq!(merged.messages.len(), 3);
        assert!(merged.messages.iter().any(|message| message.id == "m1"));
        assert!(merged.messages.iter().any(|message| message.id == "m3"));
    }
}
