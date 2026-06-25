use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;

use crate::domain::ports::{ImportedConversation, ImportedMessage};

use super::detect::header_contains_copilot_columns;

#[derive(Debug, Clone)]
struct CopilotRow {
    time: String,
    author: String,
    message: String,
}

pub fn parse_copilot_csv(path: &Path) -> Result<Vec<ImportedConversation>, String> {
    let raw = fs::read_to_string(path)
        .map_err(|e| format!("无法读取 Copilot 导出 CSV ({}): {e}", path.display()))?;
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(&raw);

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(raw.as_bytes());

    let headers = reader
        .headers()
        .map_err(|e| format!("Copilot CSV 表头无效 ({}): {e}", path.display()))?
        .clone();
    if !header_contains_copilot_columns(&headers.iter().collect::<Vec<_>>().join(",")) {
        return Err(format!(
            "不是 Copilot 活动历史 CSV（缺少 Conversation/Time/Author/Message 列）: {}",
            path.display()
        ));
    }

    let mut groups: HashMap<String, Vec<CopilotRow>> = HashMap::new();
    for (index, record) in reader.records().enumerate() {
        let record = record.map_err(|e| {
            format!(
                "Copilot CSV 第 {} 行解析失败 ({}): {e}",
                index + 2,
                path.display()
            )
        })?;

        let conversation = field(&record, &headers, "Conversation")
            .trim()
            .to_string();
        let time = field(&record, &headers, "Time").trim().to_string();
        let author = field(&record, &headers, "Author").trim().to_string();
        let message = field(&record, &headers, "Message").trim().to_string();

        if conversation.is_empty() || message.is_empty() {
            continue;
        }

        groups.entry(conversation).or_default().push(CopilotRow {
            time,
            author,
            message,
        });
    }

    let mut conversations = groups
        .into_iter()
        .filter_map(|(title, rows)| build_conversation(&title, rows))
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

fn field(record: &csv::StringRecord, headers: &csv::StringRecord, name: &str) -> String {
    headers
        .iter()
        .position(|header| header.eq_ignore_ascii_case(name))
        .and_then(|index| record.get(index))
        .unwrap_or("")
        .to_string()
}

fn build_conversation(title: &str, mut rows: Vec<CopilotRow>) -> Option<ImportedConversation> {
    if rows.is_empty() {
        return None;
    }

    rows.sort_by(|left, right| {
        let left_time = parse_copilot_timestamp(&left.time).unwrap_or(0.0);
        let right_time = parse_copilot_timestamp(&right.time).unwrap_or(0.0);
        left_time
            .partial_cmp(&right_time)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| role_rank(&left.author).cmp(&role_rank(&right.author)))
    });

    let id = conversation_id(title);
    let mut messages = Vec::new();
    let mut last_time = 0.0_f64;

    for (index, row) in rows.into_iter().enumerate() {
        let mut create_time = parse_copilot_timestamp(&row.time).unwrap_or(last_time);
        if create_time <= last_time {
            create_time = last_time + 0.001;
        }
        last_time = create_time;

        let role = map_author_role(&row.author);
        messages.push(ImportedMessage {
            id: format!("{id}::{index:04}"),
            role: role.to_string(),
            content: row.message.clone(),
            create_time: Some(create_time),
            raw_json: serde_json::json!({
                "conversation": title,
                "time": row.time,
                "author": row.author,
            })
            .to_string(),
            attachments: Vec::new(),
        });
    }

    if messages.is_empty() {
        return None;
    }

    let create_time = messages.first().and_then(|message| message.create_time);
    let update_time = messages.last().and_then(|message| message.create_time);

    Some(ImportedConversation {
        id,
        title: title.to_string(),
        create_time,
        update_time,
        model: Some("copilot".to_string()),
        messages,
        source_contexts: Vec::new(),
    })
}

fn map_author_role(author: &str) -> &'static str {
    match author.trim().eq_ignore_ascii_case("human") {
        true => "user",
        false if author.trim().eq_ignore_ascii_case("ai") => "assistant",
        false => "assistant",
    }
}

fn role_rank(author: &str) -> u8 {
    match map_author_role(author) {
        "user" => 0,
        _ => 1,
    }
}

fn conversation_id(title: &str) -> String {
    let mut hasher = DefaultHasher::new();
    title.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Parses `YYYY-MM-DDTHH:MM:SS` timestamps from the Copilot privacy export.
pub fn parse_copilot_timestamp(text: &str) -> Option<f64> {
    let text = text.trim();
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

    Some(unix_timestamp_from_utc(year, month, day, hour, minute, second))
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

    fn sample_csv_path() -> PathBuf {
        PathBuf::from(r"D:\Personal\CopilotApp\copilot-activity-history.csv")
    }

    #[test]
    fn parses_copilot_iso_timestamp() {
        assert!(parse_copilot_timestamp("2026-01-21T15:31:21").is_some());
    }

    #[test]
    fn imports_local_copilot_csv_if_present() {
        let path = sample_csv_path();
        if !path.is_file() {
            return;
        }

        let conversations = parse_copilot_csv(&path).expect("parse csv");
        assert!(!conversations.is_empty());
        assert!(conversations.iter().any(|conversation| conversation.messages.len() >= 2));

        let bash = conversations
            .iter()
            .find(|conversation| conversation.title.contains("Bash"))
            .expect("bash conversation");
        assert_eq!(bash.messages.first().map(|message| message.role.as_str()), Some("user"));
        assert!(
            bash.messages
                .windows(2)
                .all(|pair| pair[0].create_time <= pair[1].create_time)
        );
    }
}
