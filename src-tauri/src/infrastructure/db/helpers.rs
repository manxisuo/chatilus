use crate::domain::mappers::conversation_from_row;
use crate::infrastructure::importers::chatgpt::classify_image_source;
use crate::models::{AttachmentView, ConversationSummary};

pub(crate) fn map_conversation_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<ConversationSummary> {
    let tag_names: String = row.get(8)?;
    let tags = if tag_names.is_empty() {
        Vec::new()
    } else {
        tag_names.split('\x1f').map(str::to_string).collect()
    };

    Ok(conversation_from_row(
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get::<_, i64>(6)? != 0,
        row.get(7)?,
        tags,
    )
    .into())
}

pub(crate) fn clean_content_placeholders(content: String) -> String {
    content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed != "[image_asset_pointer]"
                && trimmed != "[multimodal_text]"
                && trimmed != "[user_editable_context]"
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

pub(crate) fn parse_attachments_json(raw: &str) -> Vec<AttachmentView> {
    serde_json::from_str(raw).unwrap_or_default()
}

pub(crate) fn effective_source_from_attachment_json(role: &str, json: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return classify_image_source(None, role, None);
    };
    let stored = value.get("source").and_then(|v| v.as_str());
    let path = value.get("path").and_then(|v| v.as_str());
    if let Some(source) = stored.filter(|s| !s.is_empty() && *s != "unknown") {
        if path.is_some_and(|p| p.contains("dalle-generations")) {
            return "generated".to_string();
        }
        return source.to_string();
    }
    classify_image_source(None, role, path)
}

pub(crate) fn build_fts_query(input: &str) -> String {
    input
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ")
}

pub(crate) fn role_heading(role: &str) -> &'static str {
    match role {
        "user" => "用户",
        "assistant" => "助手",
        "tool" => "工具",
        _ => "消息",
    }
}

pub(crate) fn format_timestamp(ts: f64) -> String {
    let millis = (ts * 1000.0) as i64;
    if let Some(dt) = timestamp_millis_to_rfc3339(millis) {
        return dt;
    }
    ts.to_string()
}

fn timestamp_millis_to_rfc3339(millis: i64) -> Option<String> {
    let seconds = millis / 1000;
    let days = seconds / 86_400;
    if days < 0 {
        return None;
    }

    let day_seconds = seconds % 86_400;
    let hour = day_seconds / 3600;
    let minute = (day_seconds % 3600) / 60;
    let second = day_seconds % 60;

    let (year, month, day) = civil_from_days(days);
    Some(format!(
        "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02} UTC"
    ))
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year, m as i64, d as i64)
}
