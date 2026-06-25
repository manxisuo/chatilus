use serde_json::{Map, Value};

use crate::domain::ports::{ImportedConversation, ImportedMessage};

pub fn parse_conversation(value: &Value) -> Option<ImportedConversation> {
    let id = value.get("id").and_then(|v| v.as_str())?.to_string();

    let title = value
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("未命名对话")
        .trim()
        .to_string();
    let title = if title.is_empty() {
        "未命名对话".to_string()
    } else {
        title
    };

    let create_time = value
        .get("inserted_at")
        .and_then(|v| v.as_str())
        .and_then(parse_deepseek_timestamp);
    let update_time = value
        .get("updated_at")
        .and_then(|v| v.as_str())
        .and_then(parse_deepseek_timestamp);

    let mapping = value.get("mapping")?.as_object()?;
    let mut messages = linearize_messages(mapping);
    if messages.is_empty() {
        return None;
    }
    normalize_message_times(&mut messages);

    let model = conversation_model_from_mapping(mapping);

    Some(ImportedConversation {
        id,
        title,
        create_time,
        update_time,
        model,
        messages,
        source_contexts: Vec::new(),
    })
}

fn conversation_model_from_mapping(mapping: &Map<String, Value>) -> Option<String> {
    for node in mapping.values() {
        if let Some(model) = node
            .get("message")
            .and_then(|msg| msg.get("model"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|text| !text.is_empty())
        {
            return Some(model.to_string());
        }
    }
    None
}

fn normalize_message_times(messages: &mut [ImportedMessage]) {
    let mut last_time = 0.0_f64;
    for message in messages.iter_mut() {
        let base = message.create_time.unwrap_or(last_time);
        let adjusted = if base <= last_time {
            last_time + 0.001
        } else {
            base
        };
        message.create_time = Some(adjusted);
        last_time = adjusted;
    }
}

fn linearize_messages(mapping: &Map<String, Value>) -> Vec<ImportedMessage> {
    let mut messages = Vec::new();
    if mapping.contains_key("root") {
        walk_node(mapping, "root", &mut messages);
    } else if let Some((id, _)) = mapping.iter().find(|(_, node)| {
        node.get("parent")
            .map(|parent| parent.is_null())
            .unwrap_or(true)
    }) {
        walk_node(mapping, id, &mut messages);
    }
    messages
}

fn walk_node(mapping: &Map<String, Value>, node_id: &str, out: &mut Vec<ImportedMessage>) {
    let Some(node) = mapping.get(node_id) else {
        return;
    };

    if let Some(message) = parse_node_message(node_id, node) {
        out.push(message);
    }

    let Some(children) = node.get("children").and_then(|v| v.as_array()) else {
        return;
    };
    for child in children {
        if let Some(child_id) = child.as_str() {
            walk_node(mapping, child_id, out);
        }
    }
}

fn parse_node_message(node_id: &str, node: &Value) -> Option<ImportedMessage> {
    let message = node.get("message")?;
    let fragments = message.get("fragments")?.as_array()?;
    let (role, content) = extract_role_and_content(fragments)?;
    if content.trim().is_empty() {
        return None;
    }

    let create_time = message
        .get("inserted_at")
        .and_then(|v| v.as_str())
        .and_then(parse_deepseek_timestamp);

    let raw_json = serde_json::to_string(message).unwrap_or_else(|_| "{}".to_string());

    Some(ImportedMessage {
        id: node_id.to_string(),
        role: role.to_string(),
        content,
        create_time,
        raw_json,
        attachments: Vec::new(),
    })
}

fn extract_role_and_content(fragments: &[Value]) -> Option<(&'static str, String)> {
    for fragment in fragments {
        let fragment_type = fragment.get("type").and_then(|v| v.as_str())?;
        let role = match fragment_type {
            "REQUEST" => "user",
            "RESPONSE" => "assistant",
            _ => continue,
        };
        let content = fragment
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if !content.trim().is_empty() {
            return Some((role, content));
        }
    }
    None
}

/// 解析 DeepSeek 导出的 ISO 时间（含 `+08:00` / `Z` 与毫秒）。
pub fn parse_deepseek_timestamp(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.len() < 19 {
        return None;
    }

    let (base, tz_offset_secs) = split_timezone(text)?;

    let year: i32 = base.get(0..4)?.parse().ok()?;
    if base.as_bytes().get(4) != Some(&b'-') {
        return None;
    }
    let month: u32 = base.get(5..7)?.parse().ok()?;
    if base.as_bytes().get(7) != Some(&b'-') {
        return None;
    }
    let day: u32 = base.get(8..10)?.parse().ok()?;
    if base.as_bytes().get(10) != Some(&b'T') {
        return None;
    }
    let hour: u32 = base.get(11..13)?.parse().ok()?;
    if base.as_bytes().get(13) != Some(&b':') {
        return None;
    }
    let minute: u32 = base.get(14..16)?.parse().ok()?;
    if base.as_bytes().get(16) != Some(&b':') {
        return None;
    }
    let second: u32 = base.get(17..19)?.parse().ok()?;

    let millis = if base.as_bytes().get(19) == Some(&b'.') {
        let fraction = &base[20..];
        let digits: String = fraction.chars().take(3).collect();
        let padded = format!("{digits:0<3}");
        padded.parse::<u32>().unwrap_or(0)
    } else {
        0
    };

    let local_unix =
        unix_timestamp_from_utc(year, month, day, hour, minute, second) + millis as f64 / 1000.0;
    Some(local_unix - tz_offset_secs)
}

fn split_timezone(text: &str) -> Option<(&str, f64)> {
    if let Some(idx) = text.find('+') {
        let base = &text[..idx];
        let offset = parse_tz_offset(&text[idx + 1..])?;
        return Some((base, offset));
    }
    if let Some(idx) = text.find('-') {
        if idx > 10 {
            let base = &text[..idx];
            let offset = -parse_tz_offset(&text[idx + 1..])?;
            return Some((base, offset));
        }
    }
    if text.ends_with('Z') {
        return Some((text.trim_end_matches('Z'), 0.0));
    }
    Some((text, 0.0))
}

fn parse_tz_offset(value: &str) -> Option<f64> {
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let hours: i64 = parts[0].parse().ok()?;
    let minutes: i64 = parts[1].parse().ok()?;
    Some((hours * 3600 + minutes * 60) as f64)
}

fn unix_timestamp_from_utc(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> f64 {
    fn is_leap(year: i32) -> bool {
        (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
    }

    let mut days: i64 = 0;
    for y in 1970..year {
        days += if is_leap(y) { 366 } else { 365 };
    }
    let month_days = [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for m in 1..month {
        days += month_days[m as usize] as i64;
        if m == 2 && is_leap(year) {
            days += 1;
        }
    }
    days += day as i64 - 1;
    (days * 86_400 + hour as i64 * 3_600 + minute as i64 * 60 + second as i64) as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_deepseek_iso_timestamp_with_offset() {
        let ts = parse_deepseek_timestamp("2025-02-19T20:51:25.652000+08:00");
        assert!(ts.is_some());
    }

    #[test]
    fn keeps_user_before_assistant_when_response_timestamp_is_earlier() {
        let value = json!({
            "id": "conv-2",
            "title": "Order fix",
            "inserted_at": "2026-04-29T16:22:17.405000+08:00",
            "updated_at": "2026-04-29T16:22:17.415000+08:00",
            "mapping": {
                "root": { "id": "root", "parent": null, "children": ["1"], "message": null },
                "1": {
                    "id": "1",
                    "parent": "root",
                    "children": ["2"],
                    "message": {
                        "inserted_at": "2026-04-29T16:22:17.405000+08:00",
                        "fragments": [{ "type": "REQUEST", "content": "问题" }]
                    }
                },
                "2": {
                    "id": "2",
                    "parent": "1",
                    "children": [],
                    "message": {
                        "inserted_at": "2026-04-29T16:22:17.402000+08:00",
                        "fragments": [{ "type": "RESPONSE", "content": "回答" }]
                    }
                }
            }
        });

        let parsed = parse_conversation(&value).expect("parse");
        assert_eq!(parsed.messages.len(), 2);
        assert_eq!(parsed.messages[0].role, "user");
        assert_eq!(parsed.messages[1].role, "assistant");
        let user_time = parsed.messages[0].create_time.expect("user time");
        let assistant_time = parsed.messages[1].create_time.expect("assistant time");
        assert!(user_time < assistant_time);
    }

    #[test]
    fn parses_sample_conversation() {
        let value = json!({
            "id": "conv-1",
            "title": "Hello",
            "inserted_at": "2024-12-05T15:03:01.340000+08:00",
            "updated_at": "2024-12-05T15:03:02.340000+08:00",
            "mapping": {
                "root": { "id": "root", "parent": null, "children": ["1"], "message": null },
                "1": {
                    "id": "1",
                    "parent": "root",
                    "children": ["2"],
                    "message": {
                        "model": "deepseek-chat",
                        "inserted_at": "2024-12-05T15:03:01.340000+08:00",
                        "fragments": [{ "type": "REQUEST", "content": "你好" }]
                    }
                },
                "2": {
                    "id": "2",
                    "parent": "1",
                    "children": [],
                    "message": {
                        "model": "deepseek-chat",
                        "inserted_at": "2024-12-05T15:03:02.340000+08:00",
                        "fragments": [{ "type": "RESPONSE", "content": "你好！" }]
                    }
                }
            }
        });

        let parsed = parse_conversation(&value).expect("parse");
        assert_eq!(parsed.messages.len(), 2);
        assert_eq!(parsed.messages[0].role, "user");
        assert_eq!(parsed.messages[0].content, "你好");
        assert_eq!(parsed.messages[1].role, "assistant");
        assert_eq!(parsed.model.as_deref(), Some("deepseek-chat"));
    }

    #[test]
    fn imports_local_deepseek_export_if_present() {
        let dir = std::path::PathBuf::from(r"D:\Personal\DeepSeek\deepseek_data-2026-06-24");
        if !dir.is_dir() {
            return;
        }
        let conversations = super::super::parse_export_dir(&dir).expect("parse");
        assert!(!conversations.is_empty());
        assert!(conversations.iter().any(|item| item.messages.len() >= 2));
    }
}
