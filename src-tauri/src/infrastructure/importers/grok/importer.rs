use std::path::Path;

use crate::domain::models::{DataSource, ImportProgress};
use crate::domain::ports::{
    ImportDetectResult, ImportGuide, ImportInput, ImportMethodGuide, ImportOptions, ImportPackage,
    ImportPreview, Importer, NormalizedImportResult,
};
use crate::infrastructure::importers::chatgpt::attachments::resolve_imported_attachments;
use crate::infrastructure::media::MediaIndex;

use super::detect::resolve_grok_export_root;
use super::parse::parse_grok_export;

#[derive(Debug, Default, Clone, Copy)]
pub struct GrokImporter;

impl GrokImporter {
    pub fn new() -> Self {
        Self
    }
}

impl Importer for GrokImporter {
    fn id(&self) -> &'static str {
        "grok"
    }

    fn display_name(&self) -> &'static str {
        "Grok Export"
    }

    fn source(&self) -> DataSource {
        DataSource::Grok
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn import_guide(&self) -> ImportGuide {
        ImportGuide {
            importer_id: self.id().to_string(),
            source: self.source().as_str().to_string(),
            display_name: "Grok".to_string(),
            description: "xAI Grok 账户隐私门户导出的聊天数据".to_string(),
            support_status: "stable".to_string(),
            support_summary: "ZIP / 解压目录".to_string(),
            recognition_hint: "ChatLens 会读取 prod-grok-backend.json 中的 conversations / responses，\
                               并关联 prod-mc-asset-server 下的图片附件。"
                .to_string(),
            methods: vec![
                ImportMethodGuide {
                    id: "directory".to_string(),
                    label: "选择解压后的 Grok 导出目录".to_string(),
                    kind: "directory".to_string(),
                    dialog_title: "选择 Grok 导出目录".to_string(),
                    extensions: Vec::new(),
                    hint: "选择解压后的文件夹（含 30d/export_data/<用户ID>/prod-grok-backend.json）。"
                        .to_string(),
                    example_path: None,
                    detected_default_path: None,
                    detected_default_label: None,
                },
                ImportMethodGuide {
                    id: "zip".to_string(),
                    label: "选择 Grok 导出 ZIP".to_string(),
                    kind: "file".to_string(),
                    dialog_title: "选择 Grok 导出 ZIP".to_string(),
                    extensions: vec!["zip".to_string()],
                    hint: "直接选择从 xAI 隐私门户下载的 .zip 导出包，ChatLens 会自动解压并导入。"
                        .to_string(),
                    example_path: None,
                    detected_default_path: None,
                    detected_default_label: None,
                },
            ],
        }
    }

    fn detect(&self, input: &ImportInput) -> Result<ImportDetectResult, String> {
        let matched = resolve_grok_export_root(&input.path).is_some();
        Ok(ImportDetectResult {
            matched,
            importer_id: self.id().to_string(),
            display_name: self.display_name().to_string(),
        })
    }

    fn preview(&self, input: &ImportInput) -> Result<ImportPreview, String> {
        let export_root = resolve_grok_export_root(&input.path)
            .ok_or_else(|| format!("未找到 Grok 导出数据: {}", input.path.display()))?;
        let conversations = parse_grok_export(&export_root)?;
        Ok(ImportPreview {
            importer_id: self.id().to_string(),
            display_name: self.display_name().to_string(),
            conversation_count: conversations.len(),
            shard_file_count: 1,
        })
    }

    fn import(
        &self,
        input: &ImportInput,
        options: &ImportOptions,
    ) -> Result<NormalizedImportResult, String> {
        let export_root = resolve_grok_export_root(&input.path)
            .ok_or_else(|| format!("未找到 Grok 导出数据: {}", input.path.display()))?;

        if let Some(callback) = &options.on_progress {
            callback(ImportProgress {
                phase: "parsing".to_string(),
                progress: 0.2,
                processed: 0,
                total: 1,
            });
        }

        let media_index = MediaIndex::build(&export_root);
        let mut conversations = parse_grok_export(&export_root)?;
        for conversation in &mut conversations {
            for message in &mut conversation.messages {
                message.attachments = resolve_imported_attachments(
                    &message.attachments,
                    &message.role,
                    &media_index,
                );
            }
        }

        let total = conversations.len().max(1);
        if let Some(callback) = &options.on_progress {
            callback(ImportProgress {
                phase: "importing".to_string(),
                progress: 0.8,
                processed: total,
                total,
            });
        }

        Ok(NormalizedImportResult {
            source: self.source(),
            source_path: export_root.display().to_string(),
            files_processed: 1,
            media_files_indexed: media_index.len(),
            package: ImportPackage { conversations },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_export_root() -> PathBuf {
        PathBuf::from(r"D:\Personal\Grok\ttl\30d\export_data\4ab4af5b-b35f-4197-8957-7d7404a32f34")
    }

    #[test]
    fn detect_matches_grok_export_directory() {
        let root = sample_export_root();
        if !root.is_dir() {
            return;
        }

        let importer = GrokImporter::new();
        let result = importer
            .detect(&ImportInput { path: root.clone() })
            .expect("detect");
        assert!(result.matched);
        assert_eq!(result.importer_id, "grok");
    }

    #[test]
    fn detect_matches_nested_grok_directory() {
        let ttl = PathBuf::from(r"D:\Personal\Grok\ttl");
        if !ttl.is_dir() {
            return;
        }

        let importer = GrokImporter::new();
        let result = importer
            .detect(&ImportInput { path: ttl })
            .expect("detect");
        assert!(result.matched);
    }

    #[test]
    fn import_reads_local_grok_export() {
        let root = sample_export_root();
        if !root.is_dir() {
            return;
        }

        let importer = GrokImporter::new();
        let result = importer
            .import(&ImportInput { path: root }, &ImportOptions::default())
            .expect("import");

        assert_eq!(result.source, DataSource::Grok);
        assert!(!result.package.conversations.is_empty());
        assert!(result
            .package
            .conversations
            .iter()
            .any(|conversation| conversation.messages.len() >= 2));

        let with_images = result
            .package
            .conversations
            .iter()
            .flat_map(|conversation| &conversation.messages)
            .flat_map(|message| &message.attachments)
            .filter(|attachment| attachment.path.is_some())
            .count();
        if with_images > 0 {
            assert!(result.media_files_indexed > 0);
        }
    }
}
