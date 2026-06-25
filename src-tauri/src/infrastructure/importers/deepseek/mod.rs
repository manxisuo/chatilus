mod conversation;
mod detect;
mod importer;

pub use importer::DeepSeekImporter;

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::domain::ports::ImportedConversation;

use self::conversation::parse_conversation;
use self::detect::is_deepseek_export;

pub fn parse_export_dir(export_dir: &Path) -> Result<Vec<ImportedConversation>, String> {
    let file = export_dir.join("conversations.json");
    if !file.is_file() {
        return Err(format!(
            "在 {} 中未找到 conversations.json",
            export_dir.display()
        ));
    }

    let raw = fs::read_to_string(&file).map_err(|e| format!("无法读取 {}: {e}", file.display()))?;
    let items: Vec<Value> =
        serde_json::from_str(&raw).map_err(|e| format!("JSON 解析失败 {}: {e}", file.display()))?;

    let mut conversations = Vec::new();
    for item in items {
        if let Some(parsed) = parse_conversation(&item) {
            conversations.push(parsed);
        }
    }
    Ok(conversations)
}

pub fn resolve_deepseek_export_root(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return None;
    }
    if path.is_dir() && is_deepseek_export(path) {
        return Some(path.to_path_buf());
    }
    None
}
