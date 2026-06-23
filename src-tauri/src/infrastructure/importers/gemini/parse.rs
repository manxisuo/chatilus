use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::domain::ports::{ImportedAttachment, ImportedConversation, ImportedMessage};

#[derive(Debug, Clone, PartialEq)]
struct ActivityEntry {
    prompt: String,
    timestamp_text: String,
    create_time: Option<f64>,
    response_html: String,
    generated_images: usize,
    image_sources: Vec<String>,
}

pub fn parse_activity_html(html: &str) -> Vec<ImportedConversation> {
    parse_activity_entries(html)
        .into_iter()
        .filter_map(activity_to_conversation)
        .collect()
}

fn parse_activity_entries(html: &str) -> Vec<ActivityEntry> {
    html.split("<div class=\"outer-cell")
        .skip(1)
        .filter_map(parse_outer_cell)
        .collect()
}

fn parse_outer_cell(chunk: &str) -> Option<ActivityEntry> {
    let content = extract_primary_content_cell(chunk)?;
    let lines = split_html_lines(content);
    let mut lines = lines.into_iter();

    let first = lines.next()?.trim().to_string();
    if !first.to_ascii_lowercase().contains("prompted") {
        return None;
    }
    let prompt = strip_prompt_prefix(&first);
    if prompt.is_empty() {
        return None;
    }

    let mut generated_images = 0_usize;
    let mut timestamp_text = String::new();
    let mut response_parts = Vec::new();

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if generated_images == 0 {
            if let Some(count) = parse_generated_image_count(trimmed) {
                generated_images = count;
                continue;
            }
        }

        if timestamp_text.is_empty() {
            if parse_takeout_timestamp(trimmed).is_some() || looks_like_timestamp(trimmed) {
                timestamp_text = trimmed.to_string();
                continue;
            }
        }

        response_parts.push(line);
    }

    if timestamp_text.is_empty() {
        return None;
    }

    let response_html = response_parts.join("<br>");
    let image_sources = extract_image_sources(&response_html);
    let create_time = parse_takeout_timestamp(&timestamp_text);

    Some(ActivityEntry {
        prompt,
        timestamp_text,
        create_time,
        response_html,
        generated_images: generated_images.max(image_sources.len()),
        image_sources,
    })
}

fn activity_to_conversation(entry: ActivityEntry) -> Option<ImportedConversation> {
    let id = activity_id(&entry.timestamp_text, &entry.prompt);
    let title = truncate_title(&entry.prompt);
    let response_text = html_fragment_to_text(&entry.response_html);

    let image_source = if entry.generated_images > 0 || !entry.image_sources.is_empty() {
        "generated"
    } else {
        "unknown"
    };

    let attachments: Vec<ImportedAttachment> = entry
        .image_sources
        .iter()
        .map(|pointer| ImportedAttachment {
            pointer: pointer.clone(),
            source: image_source.to_string(),
            prompt: Some(entry.prompt.clone()),
            path: None,
        })
        .collect();

    let user_message = ImportedMessage {
        id: format!("{id}::user"),
        role: "user".to_string(),
        content: entry.prompt.clone(),
        create_time: entry.create_time,
        raw_json: serde_json::json!({
            "prompt": entry.prompt,
            "timestamp": entry.timestamp_text,
        })
        .to_string(),
        attachments: Vec::new(),
    };

    let assistant_message = ImportedMessage {
        id: format!("{id}::assistant"),
        role: "assistant".to_string(),
        content: response_text,
        create_time: entry.create_time,
        raw_json: serde_json::json!({
            "response_html": entry.response_html,
            "generated_images": entry.generated_images,
        })
        .to_string(),
        attachments,
    };

    Some(ImportedConversation {
        id,
        title,
        create_time: entry.create_time,
        update_time: entry.create_time,
        model: Some("gemini".to_string()),
        messages: vec![user_message, assistant_message],
    })
}

fn extract_primary_content_cell(chunk: &str) -> Option<&str> {
    let marker = "content-cell mdl-cell mdl-cell--6-col mdl-typography--body-1";
    let start = chunk.find(marker)?;
    let start = start + marker.len();
    let rest = &chunk[start..];

    if rest.starts_with(" mdl-typography--text-right") {
        return None;
    }

    let rest = rest
        .strip_prefix('>')
        .or_else(|| rest.strip_prefix("\">"))
        .unwrap_or(rest);

    let end = rest.find("</div><div class=\"content-cell")?;
    Some(&rest[..end])
}

fn split_html_lines(content: &str) -> Vec<String> {
    content
        .split("<br>")
        .flat_map(|part| part.split("<br/>"))
        .flat_map(|part| part.split("<br />"))
        .map(str::to_string)
        .collect()
}

fn strip_prompt_prefix(line: &str) -> String {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    let Some(idx) = lower.find("prompted") else {
        return trimmed.to_string();
    };
    let rest = &trimmed[idx + "prompted".len()..];
    rest.trim_start_matches(['?', '？', ':', '：', ' '])
        .trim()
        .to_string()
}

fn parse_generated_image_count(line: &str) -> Option<usize> {
    let trimmed = line.trim().trim_end_matches('.');
    let (count_text, _) = trimmed.split_once(" generated image")?;
    count_text.parse().ok()
}

fn looks_like_timestamp(line: &str) -> bool {
    let bytes = line.as_bytes();
    bytes.len() >= 12
        && bytes.get(4) == Some(&b'\xE5')
        && line.contains('日')
        && line.contains(':')
}

fn parse_takeout_timestamp(value: &str) -> Option<f64> {
    let value = value.trim();
    let year_end = value.find('年')?;
    let month_end = value.find('月')?;
    let day_end = value.find('日')?;
    if year_end == 0 || month_end <= year_end || day_end <= month_end {
        return None;
    }

    let year: i32 = value[..year_end].parse().ok()?;
    let month: u32 = value[year_end + '年'.len_utf8()..month_end]
        .trim()
        .parse()
        .ok()?;
    let day: u32 = value[month_end + '月'.len_utf8()..day_end]
        .trim()
        .parse()
        .ok()?;

    let time_part = value[day_end + '日'.len_utf8()..].trim();
    let time_part = time_part
        .split_whitespace()
        .find(|part| part.contains(':'))?;
    let mut parts = time_part.split(':');
    let hour: u32 = parts.next()?.parse().ok()?;
    let minute: u32 = parts.next()?.parse().ok()?;
    let second: u32 = parts.next()?.parse().ok()?;

    let days = days_from_civil(year, month, day)?;
    let local_seconds = days * 86_400 + hour as i64 * 3_600 + minute as i64 * 60 + second as i64;
    // Google Takeout activity timestamps in this export use CST (UTC+8).
    Some((local_seconds - 8 * 3_600) as f64)
}

fn days_from_civil(year: i32, month: u32, day: u32) -> Option<i64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    let year = year as i64;
    let month = month as i64;
    let day = day as i64;
    let year = year - (month <= 2) as i64;
    let era = if year >= 0 {
        year / 400
    } else {
        (year - 399) / 400
    };
    let year_of_era = year - era * 400;
    let month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some(era * 146_097 + day_of_era - 719_468)
}

fn extract_image_sources(html: &str) -> Vec<String> {
    let mut images = Vec::new();
    let mut search = html;

    while let Some(img_start) = search.find("<img") {
        let rest = &search[img_start..];
        let Some(src_start) = rest.find("src=\"") else {
            search = &rest[1..];
            continue;
        };
        let value_start = src_start + "src=\"".len();
        let Some(value_end) = rest[value_start..].find('"') else {
            break;
        };
        let src = rest[value_start..value_start + value_end].trim();
        if !src.is_empty() && !images.iter().any(|existing| existing == src) {
            images.push(src.to_string());
        }
        search = &rest[value_start + value_end..];
    }

    images
}

fn html_fragment_to_text(html: &str) -> String {
    let mut text = html.to_string();
    for tag in ["<br>", "<br/>", "<br />", "</p>", "</li>"] {
        text = text.replace(tag, "\n");
    }
    text = text.replace("<p>", "");
    text = text.replace("<li>", "- ");

    while let Some(start) = text.find('<') {
        let Some(end) = text[start..].find('>') else {
            break;
        };
        text.replace_range(start..start + end + 1, "");
    }

    decode_basic_entities(&text)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn decode_basic_entities(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn activity_id(timestamp: &str, prompt: &str) -> String {
    let mut hasher = DefaultHasher::new();
    timestamp.hash(&mut hasher);
    prompt.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn truncate_title(prompt: &str) -> String {
    const MAX_LEN: usize = 80;
    let flattened = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    if flattened.chars().count() <= MAX_LEN {
        return flattened;
    }
    flattened
        .chars()
        .take(MAX_LEN)
        .chain(std::iter::once('…'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sample_activity_cell() {
        let chunk = r#"<div class="outer-cell mdl-cell mdl-cell--12-col mdl-shadow--2dp"><div class="mdl-grid"><div class="header-cell mdl-cell mdl-cell--12-col"><p class="mdl-typography--title">Gemini Apps<br></p></div><div class="content-cell mdl-cell mdl-cell--6-col mdl-typography--body-1">Prompted?can you draw pics<br>1 generated image.<br>2026年6月20日 00:28:51 CST<br><p><img alt="" src="10618735082309819750-dfbf8f6d1499c15d.png"></p>
<br></div><div class="content-cell mdl-cell mdl-cell--6-col mdl-typography--body-1 mdl-typography--text-right"></div>"#;

        let entry = parse_outer_cell(chunk).expect("entry");
        assert_eq!(entry.prompt, "can you draw pics");
        assert_eq!(entry.generated_images, 1);
        assert_eq!(entry.image_sources, vec!["10618735082309819750-dfbf8f6d1499c15d.png"]);
        assert!(entry.create_time.is_some());
    }

    #[test]
    fn builds_conversation_from_activity() {
        let html = r#"<div class="outer-cell"><div class="content-cell mdl-cell mdl-cell--6-col mdl-typography--body-1">Prompted?hello?<br>2026年6月20日 00:26:34 CST<br><p>Hi there</p>
</div><div class="content-cell mdl-cell mdl-cell--6-col mdl-typography--body-1 mdl-typography--text-right"></div>"#;
        let conversations = parse_activity_html(html);
        assert_eq!(conversations.len(), 1);
        assert_eq!(conversations[0].messages.len(), 2);
        assert_eq!(conversations[0].messages[0].role, "user");
        assert_eq!(conversations[0].messages[0].content, "hello?");
        assert_eq!(conversations[0].messages[1].role, "assistant");
        assert_eq!(conversations[0].messages[1].content, "Hi there");
    }

    #[test]
    fn parses_chinese_takeout_timestamp() {
        let ts = parse_takeout_timestamp("2026年6月20日 00:28:51 CST");
        assert!(ts.is_some());
    }
}
