use super::attachments::resolve_imported_attachments;
use super::{find_conversation_files, load_project_name_index, parse_conversation_file};
use crate::domain::models::{DataSource, ImportProgress};
use crate::domain::ports::{
    ImportDetectResult, ImportGuide, ImportInput, ImportMethodGuide, ImportOptions, ImportPackage,
    ImportPreview, Importer, NormalizedImportResult,
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

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn import_guide(&self) -> ImportGuide {
        ImportGuide {
            importer_id: self.id().to_string(),
            source: self.source().as_str().to_string(),
            display_name: "ChatGPT".to_string(),
            description: "OpenAI 官方数据导出（Settings → Data controls → Export）".to_string(),
            support_status: "stable".to_string(),
            support_summary: "ZIP / 解压目录".to_string(),
            recognition_hint: "ChatLens 会查找 conversations.json 或 conversations-*.json；\
                               可选还有 user.json、projects.json 及图片附件。"
                .to_string(),
            methods: vec![
                ImportMethodGuide {
                    id: "directory".to_string(),
                    label: "选择解压后的导出目录".to_string(),
                    kind: "directory".to_string(),
                    dialog_title: "选择 ChatGPT 导出目录".to_string(),
                    extensions: Vec::new(),
                    hint: "选择已解压的文件夹，应包含 conversations.json 或 conversations-*.json。"
                        .to_string(),
                    example_path: None,
                    detected_default_path: None,
                    detected_default_label: None,
                },
                ImportMethodGuide {
                    id: "zip".to_string(),
                    label: "选择原始 ZIP 导出包".to_string(),
                    kind: "file".to_string(),
                    dialog_title: "选择 ChatGPT 导出 ZIP".to_string(),
                    extensions: vec!["zip".to_string()],
                    hint: "直接选择从 OpenAI 下载的 .zip 导出包，ChatLens 会自动解压并导入。"
                        .to_string(),
                    example_path: None,
                    detected_default_path: None,
                    detected_default_label: None,
                },
            ],
        }
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
        let conversations = super::parse_export_dir(&input.path)?;
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
        options: &ImportOptions,
    ) -> Result<NormalizedImportResult, String> {
        if !input.path.is_dir() {
            return Err(format!(
                "路径不存在或不是目录: {}",
                input.path.display()
            ));
        }

        let media_index = MediaIndex::build(&input.path);
        let project_names = load_project_name_index(&input.path);
        let shard_files = find_conversation_files(&input.path)?;
        let files_processed = shard_files.len();
        let source_path = input.path.display().to_string();
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
