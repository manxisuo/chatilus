use std::collections::HashMap;

use serde_json::Value;

use crate::domain::ports::{ImportedConversation, ImportedMessage};

#[derive(Debug, Clone)]
pub struct CursorComposerMeta {
    pub composer_id: String,
    pub name: String,
    pub created_at: Option<f64>,
    pub updated_at: Option<f64>,
    pub model: Option<String>,
}

pub fn composer_meta_from_json(value: &Value) -> Option<CursorComposerMeta> {
    let composer_id = value
        .get("composerId")
        .and_then(|v| v.as_str())
        .map(str::to_string)?;

    let name = value
        .get("name")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .or_else(|| {
            value
                .get("text")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "Untitled".to_string());

    let model = value
        .get("modelConfig")
        .and_then(|config| config.get("modelName"))
        .and_then(|v| v.as_str())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .or_else(|| {
            value
                .get("unifiedMode")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        });

    Some(CursorComposerMeta {
        composer_id,
        name,
        created_at: timestamp_value_to_seconds(value.get("createdAt")),
        updated_at: timestamp_value_to_seconds(value.get("lastUpdatedAt")),
        model,
    })
}

pub fn parse_composer_conversation(
    composer_id: &str,
    composer_json: &Value,
    meta: &CursorComposerMeta,
    bubbles: &HashMap<String, Value>,
) -> Option<ImportedConversation> {
    let headers = composer_json
        .get("fullConversationHeadersOnly")
        .and_then(|v| v.as_array())
        .filter(|items| !items.is_empty())?;

    let mut messages = Vec::new();
    for header in headers {
        let bubble_id = header.get("bubbleId").and_then(|v| v.as_str())?;
        let bubble = bubbles.get(bubble_id)?;
        if let Some(message) = bubble_to_message(bubble_id, bubble) {
            messages.push(message);
        }
    }

    if messages.is_empty() {
        return None;
    }

    let create_time = meta
        .created_at
        .or_else(|| messages.first().and_then(|message| message.create_time));
    let update_time = meta
        .updated_at
        .or_else(|| messages.last().and_then(|message| message.create_time));

    Some(ImportedConversation {
        id: composer_id.to_string(),
        title: meta.name.clone(),
        create_time,
        update_time,
        model: meta.model.clone(),
        messages,
    })
}

fn bubble_to_message(bubble_id: &str, bubble: &Value) -> Option<ImportedMessage> {
    let text = bubble
        .get("text")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or_default();
    if text.is_empty() {
        return None;
    }

    let role = infer_bubble_role(bubble, text);

    let create_time = timestamp_value_to_seconds(bubble.get("createdAt"));

    let raw_json = serde_json::to_string(bubble).unwrap_or_else(|_| "{}".to_string());

    Some(ImportedMessage {
        id: bubble_id.to_string(),
        role: role.to_string(),
        content: text.to_string(),
        create_time,
        raw_json,
        attachments: Vec::new(),
    })
}

fn infer_bubble_role(bubble: &Value, text: &str) -> &'static str {
    if is_cursor_system_notification(text) {
        return "system";
    }

    if is_cursor_assistant_payload(text) {
        return "assistant";
    }

    match bubble.get("type").and_then(|v| v.as_i64()) {
        Some(1) => "user",
        Some(2) => "assistant",
        _ => "unknown",
    }
}

fn is_cursor_system_notification(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("<system_notification>")
}

fn is_cursor_assistant_payload(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("<assistant_message>")
}

fn timestamp_value_to_seconds(value: Option<&Value>) -> Option<f64> {
    let value = value?;

    if let Some(number) = value.as_i64() {
        return Some(normalize_epoch_seconds(number as f64));
    }
    if let Some(number) = value.as_f64() {
        return Some(normalize_epoch_seconds(number));
    }
    if let Some(text) = value.as_str() {
        return parse_rfc3339_seconds(text);
    }

    None
}

fn normalize_epoch_seconds(value: f64) -> f64 {
    if value > 1_000_000_000_000.0 {
        value / 1000.0
    } else {
        value
    }
}

fn parse_rfc3339_seconds(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(number) = trimmed.parse::<f64>() {
        return Some(normalize_epoch_seconds(number));
    }

    let without_z = trimmed.strip_suffix('Z')?;
    let (date, time) = without_z.split_once('T')?;
    let (year, month, day) = parse_date(date)?;
    let (hour, minute, second, millis) = parse_time(time)?;

    let days = days_from_civil(year, month, day)?;
    let seconds = hour * 3600 + minute * 60 + second;
    let unix = days * 86_400 + seconds as i64;
    Some(unix as f64 + millis as f64 / 1000.0)
}

fn parse_date(value: &str) -> Option<(i32, u32, u32)> {
    let mut parts = value.split('-');
    let year = parts.next()?.parse().ok()?;
    let month = parts.next()?.parse().ok()?;
    let day = parts.next()?.parse().ok()?;
    Some((year, month, day))
}

fn parse_time(value: &str) -> Option<(u32, u32, u32, u32)> {
    let (base, millis) = match value.split_once('.') {
        Some((base, frac)) => (base, frac.parse::<u32>().unwrap_or(0)),
        None => (value, 0),
    };
    let mut parts = base.split(':');
    let hour = parts.next()?.parse().ok()?;
    let minute = parts.next()?.parse().ok()?;
    let second = parts.next()?.parse().ok()?;
    Some((hour, minute, second, millis))
}

fn days_from_civil(year: i32, month: u32, day: u32) -> Option<i64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    let year = year as i64;
    let month = month as i64;
    let day = day as i64;
    let year = year - (month <= 2) as i64;
    let era = if year >= 0 { year / 400 } else { (year - 399) / 400 };
    let year_of_era = year - era * 400;
    let month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some(era * 146097 + day_of_era - 71_9468)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn classifies_system_notification_as_system_role() {
        let bubble = json!({
            "bubbleId": "sys-1",
            "type": 1,
            "createdAt": "2026-06-07T01:39:00.000Z",
            "text": "<timestamp>Sunday, Jun 7, 2026, 1:39 AM (UTC+8)</timestamp>\n<system_notification>\nThe following task has finished.\n</system_notification>"
        });

        let message = bubble_to_message("sys-1", &bubble).expect("message");
        assert_eq!(message.role, "system");
    }

    #[test]
    fn parses_cursor_bubble_message() {
        let bubble = json!({
            "bubbleId": "abc",
            "type": 1,
            "createdAt": "2026-06-21T10:40:31.724Z",
            "text": "hello"
        });

        let message = bubble_to_message("abc", &bubble).expect("message");
        assert_eq!(message.role, "user");
        assert_eq!(message.content, "hello");
        assert!(message.create_time.is_some());
    }

    #[test]
    fn skips_empty_bubbles() {
        let bubble = json!({
            "bubbleId": "abc",
            "type": 2,
            "text": "   "
        });
        assert!(bubble_to_message("abc", &bubble).is_none());
    }

    #[test]
    fn builds_conversation_from_headers() {
        let composer_id = "composer-1";
        let composer_json = json!({
            "composerId": composer_id,
            "name": "Demo",
            "createdAt": 1_782_019_407_449_i64,
            "lastUpdatedAt": 1_782_146_697_457_i64,
            "fullConversationHeadersOnly": [
                { "bubbleId": "b1", "type": 1 },
                { "bubbleId": "b2", "type": 2 }
            ]
        });
        let meta = composer_meta_from_json(&composer_json).expect("meta");
        let mut bubbles = HashMap::new();
        bubbles.insert(
            "b1".to_string(),
            json!({"type": 1, "text": "hi", "createdAt": "2026-06-21T10:40:31.724Z"}),
        );
        bubbles.insert(
            "b2".to_string(),
            json!({"type": 2, "text": "there", "createdAt": "2026-06-21T10:41:00.000Z"}),
        );

        let conversation =
            parse_composer_conversation(composer_id, &composer_json, &meta, &bubbles).expect("conversation");
        assert_eq!(conversation.title, "Demo");
        assert_eq!(conversation.messages.len(), 2);
    }
}
