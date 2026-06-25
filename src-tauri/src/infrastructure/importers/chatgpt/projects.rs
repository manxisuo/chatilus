use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

pub type ProjectNameIndex = HashMap<String, String>;

/// ChatGPT Project 在导出里可能表现为 `project_id`，或 `conversation_template_id` / `gizmo_id` 的 `g-p…` 形式。
pub fn is_chatgpt_project_id(id: &str) -> bool {
    id.starts_with("g-p")
}

pub fn load_project_name_index(export_dir: &Path) -> ProjectNameIndex {
    let path = export_dir.join("projects.json");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return HashMap::new();
    };
    let Ok(items) = serde_json::from_str::<Vec<Value>>(&raw) else {
        return HashMap::new();
    };

    let mut index = ProjectNameIndex::new();
    for item in items {
        let Some(id) = item
            .get("id")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|text| !text.is_empty())
        else {
            continue;
        };
        let Some(name) = item
            .get("name")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
        else {
            continue;
        };

        index.insert(id.to_string(), name.clone());
        if let Some(gizmo_id) = item
            .get("bound_gpt_id")
            .or_else(|| item.get("gizmo_id"))
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|text| !text.is_empty())
        {
            index.entry(gizmo_id.to_string()).or_insert(name);
        }
    }

    index
}

pub fn resolve_project_name(project_id: &str, project_names: &ProjectNameIndex) -> String {
    project_names
        .get(project_id)
        .cloned()
        .unwrap_or_else(|| fallback_project_name(project_id))
}

fn fallback_project_name(project_id: &str) -> String {
    let suffix = project_id.rsplit('-').next().unwrap_or(project_id);
    let tail = if suffix.len() > 8 {
        &suffix[suffix.len() - 8..]
    } else {
        suffix
    };
    format!("未命名 · …{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_project_gizmo_ids() {
        assert!(is_chatgpt_project_id("g-p-67ecb74315e081918a6c820ee7492ae7"));
        assert!(is_chatgpt_project_id("g-pmuQfob8d"));
        assert!(!is_chatgpt_project_id("g-m5lMeGifF"));
    }

    #[test]
    fn resolves_name_from_index() {
        let mut index = ProjectNameIndex::new();
        index.insert("g-p-abc".to_string(), "ChatLens".to_string());
        assert_eq!(resolve_project_name("g-p-abc", &index), "ChatLens");
        assert_eq!(
            resolve_project_name("g-p-unknown-long-id-value", &index),
            "未命名 · …value"
        );
    }
}
