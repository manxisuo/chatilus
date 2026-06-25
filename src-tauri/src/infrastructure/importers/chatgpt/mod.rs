mod conversation;
mod importer;
mod projects;

pub use conversation::{
    classify_image_source, extract_attachment_infos_from_message_json,
    extract_pointers_from_message_json, parse_conversation,
    ParsedAttachment, ParsedConversation, ParsedMessage,
};
pub use projects::{load_project_name_index, ProjectNameIndex};
pub use importer::ChatGptImporter;

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::domain::ports::ImportedConversation;

pub fn find_conversation_files(export_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files: Vec<PathBuf> = fs::read_dir(export_dir)
        .map_err(|e| format!("无法读取目录 {}: {e}", export_dir.display()))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name == "conversations.json"
                            || (name.starts_with("conversations-") && name.ends_with(".json"))
                    })
        })
        .collect();

    files.sort();
    if files.is_empty() {
        return Err(format!(
            "在 {} 中未找到 conversations.json 或 conversations-*.json",
            export_dir.display()
        ));
    }

    Ok(files)
}

pub fn parse_conversation_file(
    file: &Path,
    project_names: &ProjectNameIndex,
) -> Result<Vec<ImportedConversation>, String> {
    let raw =
        fs::read_to_string(file).map_err(|e| format!("无法读取 {}: {e}", file.display()))?;
    let items: Vec<Value> = serde_json::from_str(&raw)
        .map_err(|e| format!("JSON 解析失败 {}: {e}", file.display()))?;

    let mut conversations = Vec::new();
    for item in items {
        if let Some(parsed) = conversation::parse_conversation(&item, project_names) {
            conversations.push(parsed);
        }
    }
    Ok(conversations)
}

pub fn parse_export_dir(export_dir: &Path) -> Result<Vec<ImportedConversation>, String> {
    let project_names = projects::load_project_name_index(export_dir);
    let files = find_conversation_files(export_dir)?;
    let mut conversations = Vec::new();

    for file in &files {
        conversations.extend(parse_conversation_file(file, &project_names)?);
    }

    Ok(conversations)
}
