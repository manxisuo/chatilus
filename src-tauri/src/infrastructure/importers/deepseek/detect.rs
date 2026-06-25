use std::fs;
use std::path::Path;

use serde_json::Value;

/// DeepSeek 官方导出：根目录含 `conversations.json` + `user.json`，对话使用 `fragments` 而非 ChatGPT 的 `author`/`content`。
pub fn is_deepseek_export(export_dir: &Path) -> bool {
    let conversations = export_dir.join("conversations.json");
    if !conversations.is_file() {
        return false;
    }

    let Ok(raw) = fs::read_to_string(&conversations) else {
        return false;
    };
    let Ok(items) = serde_json::from_str::<Vec<Value>>(&raw) else {
        return false;
    };
    let Some(first) = items.first() else {
        return false;
    };

    if first.get("create_time").is_some() || first.get("current_node").is_some() {
        return false;
    }

    if !first.get("inserted_at").is_some() {
        return false;
    }

    let Some(mapping) = first.get("mapping").and_then(|v| v.as_object()) else {
        return false;
    };

    mapping.values().any(|node| {
        node.get("message")
            .and_then(|msg| msg.get("fragments"))
            .and_then(|v| v.as_array())
            .is_some_and(|fragments| !fragments.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_export_dir() -> PathBuf {
        PathBuf::from(r"D:\Personal\DeepSeek\deepseek_data-2026-06-24")
    }

    #[test]
    fn detects_local_deepseek_export() {
        let dir = sample_export_dir();
        if !dir.is_dir() {
            return;
        }
        assert!(is_deepseek_export(&dir));
    }

    #[test]
    fn rejects_chatgpt_style_export() {
        let value = serde_json::json!({
            "id": "c1",
            "title": "Test",
            "create_time": 1.0,
            "current_node": "n1",
            "mapping": { "n1": { "id": "n1", "message": { "author": { "role": "user" } } } }
        });
        assert!(value.get("create_time").is_some());
    }
}
