use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use base64::Engine;
use serde_json::Value;

use crate::domain::ports::ImportedAttachment;
use crate::infrastructure::importers::codex::resolve::normalize_windows_path;

pub fn extract_message_content(
    payload: &Value,
    codex_home: Option<&Path>,
    thread_id: Option<&str>,
    cache_key: Option<&str>,
    image_index: &mut usize,
) -> Option<(String, Vec<ImportedAttachment>)> {
    let content = payload.get("content")?.as_array()?;
    let mut text_parts = Vec::new();
    let mut attachments = Vec::new();

    for item in content {
        let item_type = item.get("type").and_then(|value| value.as_str())?;
        match item_type {
            "input_text" | "output_text" => {
                if let Some(text) = item.get("text").and_then(|value| value.as_str()) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        text_parts.push(trimmed.to_string());
                    }
                    attachments.extend(extract_image_paths_from_text(text));
                }
            }
            "input_image" => {
                if let Some(image_url) = item.get("image_url").and_then(|value| value.as_str()) {
                    attachments.extend(attachment_from_image_url(
                        image_url,
                        codex_home,
                        thread_id,
                        cache_key,
                        image_index,
                    ));
                }
            }
            _ => {}
        }
    }

    let text = text_parts.join("\n\n");
    if text.trim().is_empty() && attachments.is_empty() {
        return None;
    }

    Some((text, dedupe_attachments(attachments)))
}

pub fn extract_event_msg_attachments(
    payload: &Value,
    role: &str,
    codex_home: &Path,
    thread_id: &str,
    cache_key: &str,
) -> Vec<ImportedAttachment> {
    let mut attachments = Vec::new();
    let source = if role == "user" {
        "upload"
    } else {
        "generated"
    };

    if let Some(local_images) = payload.get("local_images").and_then(|value| value.as_array()) {
        for path in local_images.iter().filter_map(|value| value.as_str()) {
            if let Some(attachment) = attachment_from_path(path, source, None) {
                attachments.push(attachment);
            }
        }
    }

    if let Some(images) = payload.get("images").and_then(|value| value.as_array()) {
        for (index, image) in images.iter().enumerate() {
            let Some(data_url) = image.as_str() else {
                continue;
            };
            if let Some(path) = materialize_data_url_image(data_url, codex_home, thread_id, cache_key, index)
            {
                attachments.push(ImportedAttachment {
                    pointer: path.display().to_string(),
                    source: source.to_string(),
                    prompt: None,
                    path: Some(path.display().to_string()),
                });
            }
        }
    }

    dedupe_attachments(attachments)
}

pub fn generated_image_attachment(
    codex_home: &Path,
    thread_id: &str,
    call_id: &str,
    prompt: Option<String>,
) -> Option<ImportedAttachment> {
    let path = codex_home
        .join("generated_images")
        .join(thread_id)
        .join(format!("{call_id}.png"));
    if !path.is_file() {
        return None;
    }
    Some(ImportedAttachment {
        pointer: path.display().to_string(),
        source: "generated".to_string(),
        prompt,
        path: Some(path.display().to_string()),
    })
}

pub fn merge_attachments(target: &mut Vec<ImportedAttachment>, incoming: Vec<ImportedAttachment>) {
    target.extend(incoming);
    *target = dedupe_attachments(std::mem::take(target));
}

pub fn dedupe_attachments(attachments: Vec<ImportedAttachment>) -> Vec<ImportedAttachment> {
    let mut seen = std::collections::HashSet::new();
    attachments
        .into_iter()
        .filter(|item| seen.insert(item.pointer.clone()))
        .collect()
}

fn attachment_from_image_url(
    image_url: &str,
    codex_home: Option<&Path>,
    thread_id: Option<&str>,
    cache_key: Option<&str>,
    image_index: &mut usize,
) -> Vec<ImportedAttachment> {
    if image_url.starts_with("data:image/") {
        let (Some(home), Some(thread), Some(key)) = (codex_home, thread_id, cache_key) else {
            return Vec::new();
        };
        let Some(path) = materialize_data_url_image(image_url, home, thread, key, *image_index) else {
            return Vec::new();
        };
        *image_index += 1;
        return vec![ImportedAttachment {
            pointer: path.display().to_string(),
            source: "upload".to_string(),
            prompt: None,
            path: Some(path.display().to_string()),
        }];
    }
    attachment_from_path(image_url, "upload", None)
        .into_iter()
        .collect()
}

fn attachment_from_path(path: &str, source: &str, prompt: Option<String>) -> Option<ImportedAttachment> {
    let normalized = normalize_windows_path(path.trim());
    if !looks_like_image_path(&normalized) {
        return None;
    }
    Some(ImportedAttachment {
        pointer: normalized.clone(),
        source: source.to_string(),
        prompt,
        path: None,
    })
}

pub fn extract_image_paths_from_text(text: &str) -> Vec<ImportedAttachment> {
    let mut attachments = Vec::new();
    let mut search_from = 0usize;
    let bytes = text.as_bytes();

    while search_from < bytes.len() {
        let Some(rel) = text[search_from..].find("path=\"") else {
            break;
        };
        let start = search_from + rel + 6;
        let Some(end_rel) = text[start..].find('"') else {
            break;
        };
        let candidate = &text[start..start + end_rel];
        if let Some(attachment) = attachment_from_path(candidate, "upload", None) {
            attachments.push(attachment);
        }
        search_from = start + end_rel + 1;
    }

    for marker in [".codex/generated_images/", ".codex\\generated_images\\", ".codex/attachments/", ".codex\\attachments\\"] {
        let lower = text.to_ascii_lowercase();
        let mut offset = 0usize;
        while let Some(rel_index) = lower[offset..].find(marker) {
            let start = offset + rel_index;
            let tail = &text[start..];
            let end = tail
                .find(|ch: char| ch.is_whitespace() || ch == '`' || ch == ')' || ch == ']')
                .unwrap_or(tail.len());
            let candidate = tail[..end].trim_matches(|ch: char| ch == '`' || ch == '(' || ch == ')');
            if let Some(attachment) = attachment_from_path(candidate, "generated", None) {
                attachments.push(attachment);
            }
            offset = start + marker.len();
        }
    }

    dedupe_attachments(attachments)
}

fn materialize_data_url_image(
    data_url: &str,
    codex_home: &Path,
    thread_id: &str,
    cache_key: &str,
    index: usize,
) -> Option<PathBuf> {
    let (mime, data) = parse_data_url(data_url)?;
    if !mime.starts_with("image/") {
        return None;
    }

    let extension = mime
        .strip_prefix("image/")
        .map(|value| match value {
            "jpeg" => "jpg",
            other => other,
        })
        .unwrap_or("png");

    let mut hasher = DefaultHasher::new();
    cache_key.hash(&mut hasher);
    index.hash(&mut hasher);
    data.hash(&mut hasher);
    let digest = format!("{:016x}", hasher.finish());

    let dir = codex_home
        .join(".chatlens-extracted")
        .join(thread_id);
    fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("{digest}.{extension}"));

    if path.is_file() {
        return Some(path);
    }

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .ok()?;
    fs::write(&path, bytes).ok()?;
    Some(path)
}

fn parse_data_url(data_url: &str) -> Option<(&str, &str)> {
    let trimmed = data_url.trim();
    let rest = trimmed.strip_prefix("data:")?;
    let (meta, data) = rest.split_once(',')?;
    if !meta.ends_with(";base64") {
        return None;
    }
    let mime = meta.strip_suffix(";base64")?;
    Some((mime, data))
}

fn looks_like_image_path(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".webp", ".gif", ".bmp"]
        .iter()
        .any(|ext| lower.ends_with(ext))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_local_image_path_from_image_tag_text() {
        let text = r#"<image name=[Image #1] path="C:\Temp\codex-clipboard-abc.png">"#;
        let attachments = extract_image_paths_from_text(text);
        assert_eq!(attachments.len(), 1);
        assert!(attachments[0].pointer.contains("codex-clipboard-abc.png"));
    }

    #[test]
    fn parses_message_content_with_text_and_path() {
        let payload = json!({
            "type": "message",
            "role": "user",
            "content": [
                { "type": "input_text", "text": "see screenshot" },
                { "type": "input_text", "text": r#"<image path="D:\shots\ui.png">"# }
            ]
        });
        let (text, attachments) = extract_message_content(&payload, None, None, None, &mut 0usize).expect("content");
        assert!(text.contains("see screenshot"));
        assert_eq!(attachments.len(), 1);
        assert!(attachments[0].pointer.contains("ui.png"));
    }

    #[test]
    fn materializes_embedded_png_to_codex_cache() {
        let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
        let data_url = format!("data:image/png;base64,{png}");
        let dir = std::env::temp_dir().join(format!(
            "chatlens-codex-cache-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let path = materialize_data_url_image(&data_url, &dir, "thread-1", "msg-1", 0)
            .expect("materialize");
        assert!(path.is_file());
        let _ = fs::remove_dir_all(dir);
    }
}
