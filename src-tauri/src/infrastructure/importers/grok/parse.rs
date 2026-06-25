use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::domain::ports::{ImportedAttachment, ImportedConversation, ImportedMessage};

use super::detect::GROK_BACKEND_FILENAME;

const ASSET_SERVER_DIR: &str = "prod-mc-asset-server";

pub fn parse_grok_export(export_root: &Path) -> Result<Vec<ImportedConversation>, String> {
    let backend_path = export_root.join(GROK_BACKEND_FILENAME);
    let raw = fs::read_to_string(&backend_path)
        .map_err(|e| format!("无法读取 Grok 导出文件 ({}): {e}", backend_path.display()))?;
    let value: Value = serde_json::from_str(&raw)
        .map_err(|e| format!("Grok 导出 JSON 无效 ({}): {e}", backend_path.display()))?;

    let Some(items) = value.get("conversations").and_then(|v| v.as_array()) else {
        return Err(format!(
            "Grok 导出缺少 conversations 数组: {}",
            backend_path.display()
        ));
    };

    let mut conversations = items
        .iter()
        .filter_map(|item| build_conversation(item, export_root))
        .collect::<Vec<_>>();

    conversations.sort_by(|left, right| {
        right
            .update_time
            .partial_cmp(&left.update_time)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.title.cmp(&right.title))
    });

    Ok(conversations)
}

fn build_conversation(item: &Value, export_root: &Path) -> Option<ImportedConversation> {
    let conversation = item.get("conversation")?;
    let id = conversation.get("id").and_then(|v| v.as_str())?.to_string();
    let title = conversation
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Untitled")
        .trim()
        .to_string();
    let create_time = conversation
        .get("create_time")
        .and_then(parse_grok_timestamp);
    let update_time = conversation
        .get("modify_time")
        .and_then(parse_grok_timestamp)
        .or(create_time);
    let model = conversation
        .get("media_types")
        .and_then(|v| v.as_array())
        .and_then(|types| types.first())
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let mut responses: Vec<&Value> = item
        .get("responses")
        .and_then(|v| v.as_array())
        .map(|responses| responses.iter().collect())
        .unwrap_or_default();

    responses.sort_by(|left, right| {
        let left_time = response_create_time(left).unwrap_or(0.0);
        let right_time = response_create_time(right).unwrap_or(0.0);
        left_time
            .partial_cmp(&right_time)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| response_id(left).cmp(&response_id(right)))
    });

    let mut messages = Vec::new();
    let mut last_time = 0.0_f64;

    for (index, entry) in responses.into_iter().enumerate() {
        let response = entry.get("response")?;
        let message_id = response
            .get("_id")
            .and_then(|v| v.as_str())
            .unwrap_or(&format!("{id}-{index:04}"))
            .to_string();
        let sender = response.get("sender").and_then(|v| v.as_str())?;
        let role = map_sender_role(sender);
        let card_attachments = response
            .get("card_attachments_json")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_string))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut content = sanitize_grok_message(
            response.get("message").and_then(|v| v.as_str()).unwrap_or(""),
            &card_attachments,
        );
        let attachments = extract_attachments(response, export_root, &card_attachments);

        if content.trim().is_empty() && attachments.is_empty() {
            continue;
        }

        if content.trim().is_empty() && !attachments.is_empty() {
            if let Some(prompt) = attachments
                .iter()
                .find_map(|attachment| attachment.prompt.clone())
            {
                content = prompt;
            }
        }

        let mut create_time = response_create_time(entry).unwrap_or(last_time);
        if create_time <= last_time {
            create_time = last_time + 0.001;
        }
        last_time = create_time;

        messages.push(ImportedMessage {
            id: message_id,
            role: role.to_string(),
            content,
            create_time: Some(create_time),
            raw_json: serde_json::to_string(response).unwrap_or_else(|_| "{}".to_string()),
            attachments,
        });
    }

    if messages.is_empty() {
        return None;
    }

    let conversation_model = messages
        .iter()
        .find_map(|message| {
            serde_json::from_str::<Value>(&message.raw_json)
                .ok()
                .and_then(|value| value.get("model").and_then(|v| v.as_str()).map(str::to_string))
        })
        .or(model)
        .or_else(|| Some("grok".to_string()));

    Some(ImportedConversation {
        id,
        title,
        create_time: messages
            .first()
            .and_then(|message| message.create_time)
            .or(create_time),
        update_time: messages
            .last()
            .and_then(|message| message.create_time)
            .or(update_time),
        model: conversation_model,
        messages,
        source_contexts: Vec::new(),
    })
}

fn extract_attachments(
    response: &Value,
    export_root: &Path,
    card_attachments: &[String],
) -> Vec<ImportedAttachment> {
    let mut attachments = Vec::new();

    if let Some(file_attachments) = response.get("file_attachments").and_then(|v| v.as_array()) {
        for file_id in file_attachments.iter().filter_map(|v| v.as_str()) {
            if let Some(path) = resolve_asset_content_path(export_root, file_id) {
                attachments.push(ImportedAttachment {
                    pointer: file_id.to_string(),
                    source: "upload".to_string(),
                    prompt: None,
                    path: Some(path.display().to_string()),
                });
            }
        }
    }

    let mut seen = std::collections::HashSet::new();
    for card in select_final_generated_cards(card_attachments) {
        let image_uuid = card.image_uuid;
        if !seen.insert(image_uuid.clone()) {
            continue;
        }

        let prompt = card.prompt;

        if let Some(path) = resolve_asset_content_path(export_root, &image_uuid) {
            attachments.push(ImportedAttachment {
                pointer: image_uuid,
                source: "generated".to_string(),
                prompt: prompt.clone(),
                path: Some(path.display().to_string()),
            });
            continue;
        }

        if let Some(image_url) = card.image_url.as_deref() {
            if let Some(path) = resolve_generated_image_path(export_root, image_url) {
                attachments.push(ImportedAttachment {
                    pointer: image_uuid,
                    source: "generated".to_string(),
                    prompt,
                    path: Some(path.display().to_string()),
                });
            }
        }
    }

    attachments
}

struct GeneratedCardSelection {
    image_uuid: String,
    image_url: Option<String>,
    prompt: Option<String>,
}

fn select_final_generated_cards(card_attachments: &[String]) -> Vec<GeneratedCardSelection> {
    let mut best_by_uuid = std::collections::HashMap::<String, (i64, GeneratedCardSelection)>::new();

    for card in card_attachments {
        let Ok(value) = serde_json::from_str::<Value>(card) else {
            continue;
        };
        let chunk = value.get("image_chunk").unwrap_or(&value);
        let Some(image_uuid) = chunk
            .get("imageUuid")
            .and_then(|v| v.as_str())
            .or_else(|| value.get("id").and_then(|v| v.as_str()))
            .map(str::to_string)
        else {
            continue;
        };

        let progress = chunk
            .get("progress")
            .and_then(|v| v.as_i64())
            .or_else(|| {
                chunk
                    .get("progress")
                    .and_then(|v| v.as_f64())
                    .map(|value| value as i64)
            })
            .unwrap_or(0);
        let image_url = chunk.get("imageUrl").and_then(|v| v.as_str()).map(str::to_string);
        let prompt = chunk
            .get("imagePrompt")
            .and_then(|v| v.get("prompt"))
            .and_then(|v| v.as_str())
            .or_else(|| value.get("prompt").and_then(|v| v.as_str()))
            .map(str::to_string);

        let candidate = GeneratedCardSelection {
            image_uuid: image_uuid.clone(),
            image_url: image_url.clone(),
            prompt,
        };

        let replace = match best_by_uuid.get(&image_uuid) {
            None => true,
            Some((current_progress, current)) => {
                progress > *current_progress
                    || (progress == *current_progress
                        && is_preferred_generated_url(
                            current.image_url.as_deref(),
                            image_url.as_deref(),
                        ))
            }
        };
        if replace {
            best_by_uuid.insert(image_uuid, (progress, candidate));
        }
    }

    best_by_uuid
        .into_values()
        .map(|(_, selection)| selection)
        .filter(|selection| {
            selection.image_url.is_some() || selection.prompt.is_some()
        })
        .collect()
}

fn is_preferred_generated_url(current: Option<&str>, candidate: Option<&str>) -> bool {
    match (current, candidate) {
        (Some(current), Some(candidate)) => current.contains("-part-") && !candidate.contains("-part-"),
        (None, Some(_)) => true,
        _ => false,
    }
}

fn resolve_asset_content_path(export_root: &Path, asset_id: &str) -> Option<PathBuf> {
    let path = export_root
        .join(ASSET_SERVER_DIR)
        .join(asset_id)
        .join("content");
    path.is_file().then_some(path)
}

fn resolve_generated_image_path(export_root: &Path, image_url: &str) -> Option<PathBuf> {
    let relative = image_url.trim_start_matches('/');
    let direct = export_root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    if direct.is_file() {
        return Some(direct);
    }

    let file_name = Path::new(relative)
        .file_name()
        .and_then(|name| name.to_str())?;
    let asset_root = export_root.join(ASSET_SERVER_DIR);
    let entries = fs::read_dir(&asset_root).ok()?;
    for entry in entries.flatten() {
        let candidate = entry.path().join(file_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

fn sanitize_grok_message(text: &str, card_attachments: &[String]) -> String {
    let trimmed = text.trim();
    if trimmed.starts_with("<grok:render") {
        return extract_prompt_from_cards(card_attachments).unwrap_or_default();
    }

    strip_grok_render_blocks(trimmed)
}

fn strip_grok_render_blocks(text: &str) -> String {
    let mut result = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("<grok:render") {
        result.push_str(&rest[..start]);
        if let Some(end) = rest[start..].find("</grok:render>") {
            rest = &rest[start + end + "</grok:render>".len()..];
        } else if let Some(end) = rest[start..].find('>') {
            rest = &rest[start + end + 1..];
        } else {
            break;
        }
    }
    result.push_str(rest);
    result.trim().to_string()
}

fn extract_prompt_from_cards(card_attachments: &[String]) -> Option<String> {
    for card in card_attachments {
        let Ok(value) = serde_json::from_str::<Value>(card) else {
            continue;
        };
        if let Some(prompt) = value
            .get("image_chunk")
            .and_then(|chunk| chunk.get("imagePrompt"))
            .and_then(|prompt| prompt.get("prompt"))
            .and_then(|prompt| prompt.as_str())
            .map(str::trim)
            .filter(|prompt| !prompt.is_empty())
        {
            return Some(prompt.to_string());
        }
        if let Some(prompt) = value
            .get("prompt")
            .and_then(|prompt| prompt.as_str())
            .map(str::trim)
            .filter(|prompt| !prompt.is_empty())
        {
            return Some(prompt.to_string());
        }
    }
    None
}

fn map_sender_role(sender: &str) -> &'static str {
    match sender.trim().eq_ignore_ascii_case("human") {
        true => "user",
        false => "assistant",
    }
}

fn response_id(entry: &Value) -> String {
    entry
        .get("response")
        .and_then(|response| response.get("_id"))
        .and_then(|id| id.as_str())
        .unwrap_or("")
        .to_string()
}

fn response_create_time(entry: &Value) -> Option<f64> {
    entry
        .get("response")
        .and_then(|response| response.get("create_time"))
        .and_then(parse_grok_timestamp)
}

pub fn parse_grok_timestamp(value: &Value) -> Option<f64> {
    if let Some(number) = value.as_f64() {
        return Some(normalize_epoch_seconds(number));
    }
    if let Some(text) = value.as_str() {
        return parse_iso_timestamp(text);
    }
    if let Some(number) = value
        .get("$date")
        .and_then(|date| {
            date.get("$numberLong")
                .and_then(|number| number.as_str().and_then(|text| text.parse::<f64>().ok()))
                .or_else(|| date.get("$numberLong").and_then(|number| number.as_f64()))
                .or_else(|| {
                    date.as_str()
                        .and_then(parse_iso_timestamp)
                })
        })
    {
        return Some(normalize_epoch_seconds(number));
    }
    None
}

fn parse_iso_timestamp(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.len() < 19 {
        return None;
    }
    let year: i32 = text.get(0..4)?.parse().ok()?;
    let month: u32 = text.get(5..7)?.parse().ok()?;
    let day: u32 = text.get(8..10)?.parse().ok()?;
    let hour: u32 = text.get(11..13)?.parse().ok()?;
    let minute: u32 = text.get(14..16)?.parse().ok()?;
    let second: u32 = text.get(17..19)?.parse().ok()?;
    Some(unix_timestamp_from_utc(year, month, day, hour, minute, second))
}

fn normalize_epoch_seconds(value: f64) -> f64 {
    if value > 1_000_000_000_000.0 {
        value / 1000.0
    } else {
        value
    }
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
    use std::path::PathBuf;

    use crate::infrastructure::importers::grok::resolve_grok_export_root;

    fn sample_export_root() -> PathBuf {
        PathBuf::from(r"D:\Personal\Grok\ttl\30d\export_data\4ab4af5b-b35f-4197-8957-7d7404a32f34")
    }

    #[test]
    fn parses_grok_mongo_timestamp() {
        let value = serde_json::json!({
            "$date": { "$numberLong": "1782392772097" }
        });
        let ts = parse_grok_timestamp(&value).expect("timestamp");
        assert!((ts - 1_782_392_772.097).abs() < 1.0);
    }

    #[test]
    fn grok_message_timestamps_are_reasonable() {
        let root = PathBuf::from(
            r"C:\Users\manxi\AppData\Roaming\com.manxi.chatlens\imports\720f08bb-5c35-4197-af4a-d5e9b2b31efb",
        );
        if !root.is_dir() {
            return;
        }

        let conversations = parse_grok_export(
            &resolve_grok_export_root(&root).expect("export root"),
        )
        .expect("parse");

        for conversation in &conversations {
            for message in &conversation.messages {
                let ts = message.create_time.expect("create_time");
                assert!(
                    ts > 1_000_000_000.0,
                    "conversation {:?} message {:?} has epoch-ish timestamp {}",
                    conversation.title,
                    message.id,
                    ts
                );
            }
        }
    }

    #[test]
    fn imports_local_grok_export_if_present() {
        let root = sample_export_root();
        if !root.is_dir() {
            return;
        }

        let conversations = parse_grok_export(&root).expect("parse grok export");
        assert!(!conversations.is_empty());
        assert!(conversations.iter().any(|conversation| conversation.messages.len() >= 2));

        let desk = conversations
            .iter()
            .find(|conversation| conversation.title.contains("Cyberpunk"))
            .expect("desk conversation");
        assert!(
            desk.messages
                .iter()
                .any(|message| !message.attachments.is_empty()),
            "expected uploaded image attachment"
        );
    }
}
