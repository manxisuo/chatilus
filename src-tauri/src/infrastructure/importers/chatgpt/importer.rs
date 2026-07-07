use super::{find_conversation_files, format::is_manifest_v1_export, import_export::import_chatgpt_export_dir};
use crate::domain::models::DataSource;
use crate::domain::ports::{
    ImportDetectResult, ImportGuide, ImportInput, ImportMethodGuide, ImportOptions, ImportPackage,
    ImportPreview, Importer, NormalizedImportResult,
};

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
            recognition_hint: "Chatilus 会查找 conversations.json 或 conversations-*.json；\
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
                    hint: "直接选择从 OpenAI 下载的 .zip 导出包，Chatilus 会自动解压并导入。"
                        .to_string(),
                    example_path: None,
                    detected_default_path: None,
                    detected_default_label: None,
                },
            ],
        }
    }

    fn detect(&self, input: &ImportInput) -> Result<ImportDetectResult, String> {
        if is_manifest_v1_export(&input.path) {
            return Ok(ImportDetectResult {
                matched: false,
                importer_id: self.id().to_string(),
                display_name: self.display_name().to_string(),
            });
        }
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
        if !options.user_selected && is_manifest_v1_export(&input.path) {
            return Err(format!(
                "该目录为 ChatGPT Manifest v1 导出，请使用「ChatGPT (Manifest v1)」导入: {}",
                input.path.display()
            ));
        }

        let output = import_chatgpt_export_dir(&input.path, options)?;

        Ok(NormalizedImportResult {
            source: self.source(),
            source_path: output.source_path,
            files_processed: output.files_processed,
            media_files_indexed: output.media_files_indexed,
            package: ImportPackage {
                conversations: output.conversations,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_export_dir() -> PathBuf {
        PathBuf::from(r"D:\Personal\ChatGPT数据下载\2026-05-16-12-08-35")
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
    fn user_selected_import_accepts_manifest_v1_export() {
        let dir = PathBuf::from(r"D:\Personal\ChatGPT数据下载\2026-06-26");
        if !dir.is_dir() {
            return;
        }

        let importer = ChatGptImporter::new();
        let result = importer.import(
            &ImportInput { path: dir },
            &ImportOptions {
                user_selected: true,
                ..ImportOptions::default()
            },
        );
        assert!(result.is_ok(), "expected import to succeed: {:?}", result.err());
    }

    #[test]
    fn import_resolves_media_paths() {
        let dir = sample_export_dir();
        if !dir.is_dir() {
            return;
        }

        let importer = ChatGptImporter::new();
        let result = importer
            .import(
                &ImportInput { path: dir },
                &ImportOptions {
                    user_selected: true,
                    ..ImportOptions::default()
                },
            )
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
