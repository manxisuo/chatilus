use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

pub const GROK_BACKEND_FILENAME: &str = "prod-grok-backend.json";

/// xAI / Grok privacy export: `prod-grok-backend.json` with `conversations[].conversation` + `responses`.
pub fn is_grok_backend_json(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_none_or(|name| !name.eq_ignore_ascii_case(GROK_BACKEND_FILENAME))
    {
        return false;
    }

    let Ok(raw) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return false;
    };

    value
        .get("conversations")
        .and_then(|items| items.as_array())
        .is_some_and(|items| items.iter().any(is_grok_conversation_item))
}

fn is_grok_conversation_item(item: &Value) -> bool {
    item.get("conversation")
        .and_then(|conversation| conversation.get("id"))
        .and_then(|id| id.as_str())
        .is_some_and(|id| !id.is_empty())
        && item
            .get("responses")
            .and_then(|responses| responses.as_array())
            .is_some_and(|responses| {
                responses.iter().any(|entry| {
                    entry
                        .get("response")
                        .and_then(|response| response.get("sender"))
                        .and_then(|sender| sender.as_str())
                        .is_some()
                })
            })
}

pub fn resolve_grok_export_root(path: &Path) -> Option<PathBuf> {
    if is_grok_backend_json(path) {
        return path.parent().map(Path::to_path_buf);
    }

    if path.is_dir() {
        let direct = path.join(GROK_BACKEND_FILENAME);
        if is_grok_backend_json(&direct) {
            return Some(path.to_path_buf());
        }

        if let Some(found) = find_grok_backend_in_dir(path, 4) {
            return found.parent().map(Path::to_path_buf);
        }
    }

    None
}

fn find_grok_backend_in_dir(dir: &Path, depth: u8) -> Option<PathBuf> {
    if depth == 0 {
        return None;
    }

    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let candidate = entry.path();
        if candidate.is_file() && is_grok_backend_json(&candidate) {
            return Some(candidate);
        }
        if candidate.is_dir() {
            if let Some(found) = find_grok_backend_in_dir(&candidate, depth - 1) {
                return Some(found);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_export_root() -> PathBuf {
        PathBuf::from(r"D:\Personal\Grok\ttl\30d\export_data\4ab4af5b-b35f-4197-8957-7d7404a32f34")
    }

    #[test]
    fn detects_local_grok_export() {
        let root = sample_export_root();
        if !root.is_dir() {
            return;
        }
        assert!(is_grok_backend_json(&root.join(GROK_BACKEND_FILENAME)));
        assert_eq!(resolve_grok_export_root(&root), Some(root));
    }

    #[test]
    fn resolves_nested_grok_export_directory() {
        let ttl = PathBuf::from(r"D:\Personal\Grok\ttl");
        if !ttl.is_dir() {
            return;
        }
        let resolved = resolve_grok_export_root(&ttl);
        assert!(resolved.is_some());
        assert!(is_grok_backend_json(
            &resolved.unwrap().join(GROK_BACKEND_FILENAME)
        ));
    }
}
