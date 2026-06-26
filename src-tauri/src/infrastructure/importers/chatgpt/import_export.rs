use std::path::Path;

use crate::domain::models::ImportProgress;
use crate::domain::ports::{ImportOptions, ImportedConversation};
use crate::infrastructure::attachments::resolve_imported_attachments;
use crate::infrastructure::media::MediaIndex;

use super::{find_conversation_files, load_project_name_index, parse_conversation_file};

pub struct ChatGptImportOutput {
    pub source_path: String,
    pub files_processed: usize,
    pub media_files_indexed: usize,
    pub conversations: Vec<ImportedConversation>,
}

pub fn import_chatgpt_export_dir(
    export_dir: &Path,
    options: &ImportOptions,
) -> Result<ChatGptImportOutput, String> {
    if !export_dir.is_dir() {
        return Err(format!(
            "路径不存在或不是目录: {}",
            export_dir.display()
        ));
    }

    let media_index = MediaIndex::build(export_dir);
    let project_names = load_project_name_index(export_dir);
    let shard_files = find_conversation_files(export_dir)?;
    let files_processed = shard_files.len();
    let source_path = export_dir.display().to_string();
    let total = files_processed.max(1);

    let mut conversations = Vec::new();
    for (index, file) in shard_files.iter().enumerate() {
        conversations.extend(parse_conversation_file(file, &project_names)?);
        if let Some(callback) = &options.on_progress {
            let progress = 0.2 + ((index + 1) as f64 / total as f64) * 0.6;
            callback(ImportProgress {
                phase: "parsing".to_string(),
                progress,
                processed: index + 1,
                total,
            });
        }
    }

    for conversation in &mut conversations {
        for message in &mut conversation.messages {
            message.attachments =
                resolve_imported_attachments(&message.attachments, &message.role, &media_index);
        }
    }

    Ok(ChatGptImportOutput {
        source_path,
        files_processed,
        media_files_indexed: media_index.len(),
        conversations,
    })
}
