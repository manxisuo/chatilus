use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde_json::Value;

use crate::domain::ports::{ImportedConversation, ImportedMessage, ImportedSourceContext};

use super::db::CodexThreadRecord;
use super::resolve::normalize_windows_path;

pub fn thread_to_conversation(thread: &CodexThreadRecord) -> Option<ImportedConversation> {
    let mut conversation = parse_rollout_file(&thread.rollout_path, thread)?;
    if conversation.title.trim().is_empty() {
        conversation.title = fallback_title(thread);
    }
    if conversation.model.is_none() {
        conversation.model = thread.model.clone();
    }
  if conversation.create_time.is_none() {
        conversation.create_time = thread.created_at.map(|value| value as f64);
    }
    if conversation.update_time.is_none() {
        conversation.update_time = thread.updated_at.map(|value| value as f64);
    }
    if conversation.source_contexts.is_empty() {
        conversation.source_contexts = extract_codex_source_contexts(thread, &conversation.messages);
    }
    Some(conversation)
}

fn parse_rollout_file(path: &Path, thread: &CodexThreadRecord) -> Option<ImportedConversation> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut messages = Vec::new();
    let mut message_index = 0usize;
    let mut model = thread.model.clone();

    for line in reader.lines() {
        let line = line.ok()?;
        if line.trim().is_empty() {
            continue;
        }
        let item: Value = serde_json::from_str(&line).ok()?;
        let item_type = item.get("type").and_then(|value| value.as_str())?;

        match item_type {
            "session_meta" => {
                if let Some(payload) = item.get("payload") {
                    if model.is_none() {
                        model = payload
                            .get("model")
                            .and_then(|value| value.as_str())
                            .map(str::to_string);
                    }
                }
            }
            "turn_context" => {
                if model.is_none() {
                    model = item
                        .get("payload")
                        .and_then(|payload| payload.get("model"))
                        .and_then(|value| value.as_str())
                        .map(str::to_string);
                }
            }
            "response_item" => {
                let Some(payload) = item.get("payload") else {
                    continue;
                };
                if payload.get("type").and_then(|value| value.as_str()) != Some("message") {
                    continue;
                }
                let Some(role) = payload.get("role").and_then(|value| value.as_str()) else {
                    continue;
                };
                if role == "developer" {
                    continue;
                }
                let Some(text) = message_text(payload) else {
                    continue;
                };
                if should_skip_message_text(role, &text) {
                    continue;
                }
                let timestamp = item
                    .get("timestamp")
                    .and_then(parse_iso_timestamp)
                    .or_else(|| thread.updated_at.map(|value| value as f64));
                let message_id = payload
                    .get("id")
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("{}-{}", thread.id, message_index));
                message_index += 1;
                messages.push(ImportedMessage {
                    id: message_id,
                    role: normalize_role(role).to_string(),
                    content: text,
                    create_time: timestamp,
                    raw_json: serde_json::to_string(payload).unwrap_or_else(|_| "{}".to_string()),
                    attachments: Vec::new(),
                });
            }
            "event_msg" => {
                let Some(payload) = item.get("payload") else {
                    continue;
                };
                let payload_type = payload.get("type").and_then(|value| value.as_str());
                let (role, text) = match payload_type {
                    Some("user_message") => (
                        "user",
                        payload.get("message").and_then(|value| value.as_str())?,
                    ),
                    Some("agent_message") => (
                        "assistant",
                        payload.get("message").and_then(|value| value.as_str())?,
                    ),
                    _ => continue,
                };
                let text = text.trim();
                if text.is_empty() || should_skip_message_text(role, text) {
                    continue;
                }
                if messages
                    .last()
                    .is_some_and(|last| last.role == role && last.content.trim() == text)
                {
                    continue;
                }
                let timestamp = item
                    .get("timestamp")
                    .and_then(parse_iso_timestamp)
                    .or_else(|| thread.updated_at.map(|value| value as f64));
                let message_id = format!("{}-event-{}", thread.id, message_index);
                message_index += 1;
                messages.push(ImportedMessage {
                    id: message_id,
                    role: role.to_string(),
                    content: text.to_string(),
                    create_time: timestamp,
                    raw_json: serde_json::to_string(payload).unwrap_or_else(|_| "{}".to_string()),
                    attachments: Vec::new(),
                });
            }
            _ => {}
        }
    }

    if messages.is_empty() {
        return None;
    }

    let create_time = messages
        .iter()
        .filter_map(|message| message.create_time)
        .min_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal))
        .or_else(|| thread.created_at.map(|value| value as f64));
    let update_time = messages
        .iter()
        .filter_map(|message| message.create_time)
        .max_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal))
        .or_else(|| thread.updated_at.map(|value| value as f64));

    Some(ImportedConversation {
        id: thread.id.clone(),
        title: fallback_title(thread),
        create_time,
        update_time,
        model,
        messages,
        source_contexts: extract_codex_source_contexts(thread, &[]),
    })
}

pub fn extract_codex_source_contexts(
    thread: &CodexThreadRecord,
    _messages: &[ImportedMessage],
) -> Vec<ImportedSourceContext> {
    let mut contexts = Vec::new();

    if let Some(cwd) = thread
        .cwd
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        let name = workspace_folder_name(cwd).unwrap_or_else(|| cwd.to_string());
        contexts.push(ImportedSourceContext {
            context_type: "folder".to_string(),
            external_id: None,
            name,
            path: Some(cwd.to_string()),
            raw_json: None,
        });
    }

    if let Some(name) = repository_name_from_git_url(thread.git_origin_url.as_deref()) {
        push_repository_context(
            &mut contexts,
            thread.git_origin_url.clone(),
            name,
            thread.cwd.clone(),
        );
    }

    contexts
}

fn push_repository_context(
    contexts: &mut Vec<ImportedSourceContext>,
    external_id: Option<String>,
    name: String,
    path: Option<String>,
) {
    if contexts.iter().any(|item| {
        item.context_type == "repository" && item.name == name && item.path == path
    }) {
        return;
    }
    contexts.push(ImportedSourceContext {
        context_type: "repository".to_string(),
        external_id,
        name,
        path,
        raw_json: None,
    });
}

fn fallback_title(thread: &CodexThreadRecord) -> String {
    let raw = thread
        .title
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .or_else(|| {
            thread
                .first_user_message
                .as_deref()
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_string)
        });

    raw.map(|text| truncate_title(&text, 80))
        .unwrap_or_else(|| "Codex Session".to_string())
}

fn truncate_title(text: &str, max_chars: usize) -> String {
    let trimmed = text.replace('\r', " ").replace('\n', " ");
    if trimmed.chars().count() <= max_chars {
        return trimmed;
    }
    trimmed.chars().take(max_chars).collect::<String>() + "…"
}

fn message_text(payload: &Value) -> Option<String> {
    let content = payload.get("content")?.as_array()?;
    let mut parts = Vec::new();
    for item in content {
        let item_type = item.get("type").and_then(|value| value.as_str())?;
        if !matches!(item_type, "input_text" | "output_text") {
            continue;
        }
        if let Some(text) = item.get("text").and_then(|value| value.as_str()) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                parts.push(trimmed.to_string());
            }
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("\n\n"))
}

fn should_skip_message_text(role: &str, text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return true;
    }
    if role == "user" {
        return trimmed.starts_with("<environment_context")
            || trimmed.starts_with("<permissions")
            || trimmed.starts_with("# AGENTS.md");
    }
    false
}

fn normalize_role(role: &str) -> &'static str {
    match role {
        "assistant" => "assistant",
        "system" => "system",
        _ => "user",
    }
}

fn parse_iso_timestamp(value: &Value) -> Option<f64> {
    let text = value.as_str()?.trim();
    if text.len() < 19 {
        return None;
    }

    let year: i32 = text.get(0..4)?.parse().ok()?;
    if text.as_bytes().get(4) != Some(&b'-') {
        return None;
    }
    let month: u32 = text.get(5..7)?.parse().ok()?;
    if text.as_bytes().get(7) != Some(&b'-') {
        return None;
    }
    let day: u32 = text.get(8..10)?.parse().ok()?;
    if text.as_bytes().get(10) != Some(&b'T') {
        return None;
    }
    let hour: u32 = text.get(11..13)?.parse().ok()?;
    if text.as_bytes().get(13) != Some(&b':') {
        return None;
    }
    let minute: u32 = text.get(14..16)?.parse().ok()?;
    if text.as_bytes().get(16) != Some(&b':') {
        return None;
    }
    let second: u32 = text.get(17..19)?.parse().ok()?;
    let millis = if text.as_bytes().get(19) == Some(&b'.') {
        text.get(20..)
            .and_then(|fraction| fraction.strip_suffix('Z'))
            .map(|fraction| {
                let digits: String = fraction.chars().take(3).collect();
                let padded = format!("{digits:0<3}");
                padded.parse::<u32>().unwrap_or(0)
            })
            .unwrap_or(0)
    } else {
        0
    };

    Some(unix_timestamp_from_utc(year, month, day, hour, minute, second) + millis as f64 / 1000.0)
}

fn unix_timestamp_from_utc(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> f64 {
    fn is_leap(year: i32) -> bool {
        (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
    }

    let mut days: i64 = 0;
    for y in 1970..year {
        days += if is_leap(y) { 366 } else { 365 };
    }
    let month_days = [
        0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
    ];
    for m in 1..month {
        days += month_days[m as usize] as i64;
        if m == 2 && is_leap(year) {
            days += 1;
        }
    }
    days += day as i64 - 1;
    (days * 86_400 + hour as i64 * 3_600 + minute as i64 * 60 + second as i64) as f64
}

fn workspace_folder_name(path: &str) -> Option<String> {
    let normalized = normalize_windows_path(path);
    let trimmed = normalized.trim_end_matches(['\\', '/']);
    let name = trimmed
        .rsplit(['\\', '/'])
        .next()
        .filter(|value| !value.is_empty())?;
    Some(name.to_string())
}

fn repository_name_from_git_url(url: Option<&str>) -> Option<String> {
    let url = url?.trim();
    if url.is_empty() {
        return None;
    }
    let slug = url
        .trim_end_matches(".git")
        .rsplit(['/', ':'])
        .next()
        .filter(|value| !value.is_empty())?;
    Some(slug.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn sample_thread(rollout_path: &Path) -> CodexThreadRecord {
        CodexThreadRecord {
            id: "019e9145-9162-7092-8ee8-68d444f97b67".to_string(),
            title: Some("hello".to_string()),
            cwd: Some(r"D:\test".to_string()),
            model: Some("gpt-5.5".to_string()),
            rollout_path: rollout_path.to_path_buf(),
            source: Some("vscode".to_string()),
            created_at: Some(1_780_553_716),
            updated_at: Some(1_780_553_774),
            git_branch: None,
            git_origin_url: None,
            first_user_message: Some("hello".to_string()),
        }
    }

    #[test]
    fn parses_rollout_messages() {
        let path = std::env::temp_dir().join(format!(
            "chatlens-codex-rollout-{}.jsonl",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let mut file = std::fs::File::create(&path).expect("create rollout");
        writeln!(
            file,
            r#"{{"timestamp":"2026-06-04T06:15:17.382Z","type":"session_meta","payload":{{"id":"019e9145-9162-7092-8ee8-68d444f97b67","cwd":"D:\\test"}}}}"#
        )
        .expect("write meta");
        writeln!(
            file,
            r#"{{"timestamp":"2026-06-04T06:15:38.385Z","type":"response_item","payload":{{"type":"message","role":"user","content":[{{"type":"input_text","text":"hello\n"}}]}}}}"#
        )
        .expect("write user");
        writeln!(
            file,
            r#"{{"timestamp":"2026-06-04T06:16:13.614Z","type":"response_item","payload":{{"type":"message","role":"assistant","content":[{{"type":"output_text","text":"Hello. How can I help?"}}]}}}}"#
        )
        .expect("write assistant");

        let thread = sample_thread(&path);
        let conversation = thread_to_conversation(&thread).expect("conversation");
        assert_eq!(conversation.messages.len(), 2);
        assert_eq!(conversation.messages[0].role, "user");
        assert_eq!(conversation.messages[0].content, "hello");
        assert_eq!(conversation.messages[1].role, "assistant");
        assert_eq!(conversation.source_contexts.len(), 1);
        assert!(conversation
            .source_contexts
            .iter()
            .any(|item| item.context_type == "folder" && item.name == "test"));
    }

    #[test]
    fn truncates_long_codex_thread_title() {
        let long = "这是一个空目录，我准备做一个新项目。".repeat(10);
        let thread = CodexThreadRecord {
            title: Some(long.clone()),
            ..sample_thread(&std::path::PathBuf::from("unused.jsonl"))
        };
        let title = fallback_title(&thread);
        assert!(title.chars().count() <= 81);
        assert!(title.ends_with('…'));
    }

    #[test]
    fn skips_environment_context_user_messages() {
        assert!(should_skip_message_text(
            "user",
            "<environment_context>\n  <cwd>D:\\test</cwd>\n</environment_context>"
        ));
        assert!(!should_skip_message_text("user", "hello"));
    }

    #[test]
    fn imports_local_codex_if_present() {
        let home = super::super::resolve::default_codex_home();
        if !home.is_dir() {
            return;
        }
        let (conn, _) = super::super::db::open_codex_db(&home).expect("open db");
        let threads = super::super::db::list_threads(&conn).expect("list");
        if threads.is_empty() {
            return;
        }
        let conversation = thread_to_conversation(&threads[0]).expect("conversation");
        assert!(!conversation.messages.is_empty());
    }
}
