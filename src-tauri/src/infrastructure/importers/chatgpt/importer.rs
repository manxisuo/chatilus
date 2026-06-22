use super::attachments::resolve_imported_attachments;
use super::{find_conversation_files, parse_export_dir};
use crate::domain::models::DataSource;
use crate::domain::ports::{
    ImportDetectResult, ImportInput, ImportOptions, ImportPackage, ImportPreview, Importer,
    NormalizedImportResult,
};
use crate::infrastructure::media::MediaIndex;

#[derive(Debug, Default, Clone, Copy)]
pub struct ChatGptImporter;

impl ChatGptImporter {
    pub fn new() -> Self {
        Self
    }
}

impl Importer for ChatGptImporter {
    fn id(&self) -> &'static str {
        "chatgpt"
    }

    fn display_name(&self) -> &'static str {
        "ChatGPT Export"
    }

    fn source(&self) -> DataSource {
        DataSource::ChatGpt
    }

    fn detect(&self, input: &ImportInput) -> Result<ImportDetectResult, String> {
        let matched = find_conversation_files(&input.path).is_ok();
        Ok(ImportDetectResult {
            matched,
            importer_id: self.id().to_string(),
            display_name: self.display_name().to_string(),
        })
    }

    fn preview(&self, input: &ImportInput) -> Result<ImportPreview, String> {
        let shard_file_count = find_conversation_files(&input.path)?.len();
        let conversations = parse_export_dir(&input.path)?;
        Ok(ImportPreview {
            importer_id: self.id().to_string(),
            display_name: self.display_name().to_string(),
            conversation_count: conversations.len(),
            shard_file_count,
        })
    }

    fn import(
        &self,
        input: &ImportInput,
        _options: &ImportOptions,
    ) -> Result<NormalizedImportResult, String> {
        if !input.path.is_dir() {
            return Err(format!(
                "路径不存在或不是目录: {}",
                input.path.display()
            ));
        }

        let media_index = MediaIndex::build(&input.path);
        let shard_files = find_conversation_files(&input.path)?;
        let files_processed = shard_files.len();
        let source_path = input.path.display().to_string();

        let mut conversations = parse_export_dir(&input.path)?;
        for conversation in &mut conversations {
            for message in &mut conversation.messages {
                message.attachments =
                    resolve_imported_attachments(&message.attachments, &message.role, &media_index);
            }
        }

        Ok(NormalizedImportResult {
            source: self.source(),
            source_path,
            files_processed,
            media_files_indexed: media_index.len(),
            package: ImportPackage { conversations },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_export_dir() -> PathBuf {
        PathBuf::from(r"D:\Personal\ChatGPT数据下载-2026年5月18日\2026-05-16-12-08-35")
    }

    #[test]
    fn detect_matches_chatgpt_export() {
        let dir = sample_export_dir();
        if !dir.is_dir() {
            return;
        }

        let importer = ChatGptImporter::new();
        let result = importer
            .detect(&ImportInput { path: dir })
            .expect("detect");
        assert!(result.matched);
        assert_eq!(result.importer_id, "chatgpt");
    }

    #[test]
    fn import_resolves_media_paths() {
        let dir = sample_export_dir();
        if !dir.is_dir() {
            return;
        }

        let importer = ChatGptImporter::new();
        let result = importer
            .import(&ImportInput { path: dir }, &ImportOptions::default())
            .expect("import");

        assert!(!result.package.conversations.is_empty());
        assert!(result.media_files_indexed > 0);

        let with_attachment = result
            .package
            .conversations
            .iter()
            .flat_map(|conversation| conversation.messages.iter())
            .find(|message| !message.attachments.is_empty());

        if let Some(message) = with_attachment {
            assert!(message
                .attachments
                .iter()
                .all(|attachment| attachment.path.is_some()));
        }
    }
}
