use std::fs;

use crate::domain::models::{DataSource, ImportProgress};
use crate::domain::ports::{
    ImportDetectResult, ImportGuide, ImportInput, ImportMethodGuide, ImportOptions, ImportPackage,
    ImportPreview, Importer, NormalizedImportResult,
};
use crate::infrastructure::importers::chatgpt::attachments::resolve_imported_attachments;
use crate::infrastructure::media::MediaIndex;

use super::parse::parse_activity_html;
use super::resolve::{find_activity_html, resolve_gemini_export_root};

#[derive(Debug, Default, Clone, Copy)]
pub struct GeminiImporter;

impl GeminiImporter {
    pub fn new() -> Self {
        Self
    }
}

impl Importer for GeminiImporter {
    fn id(&self) -> &'static str {
        "gemini"
    }

    fn display_name(&self) -> &'static str {
        "Google Gemini Takeout"
    }

    fn source(&self) -> DataSource {
        DataSource::Gemini
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn import_guide(&self) -> ImportGuide {
        ImportGuide {
            importer_id: self.id().to_string(),
            source: self.source().as_str().to_string(),
            display_name: "Gemini".to_string(),
            description: "Google Takeout 中的 Gemini Apps 活动记录".to_string(),
            support_status: "stable".to_string(),
            support_summary: "Google Takeout".to_string(),
            recognition_hint: "ChatLens 会解析 Gemini Apps 活动记录目录中的 HTML 文件。".to_string(),
            methods: vec![ImportMethodGuide {
                id: "takeout_dir".to_string(),
                label: "选择 Gemini Apps 目录".to_string(),
                kind: "directory".to_string(),
                dialog_title: "选择 Gemini Apps 目录".to_string(),
                extensions: Vec::new(),
                hint: "选择 Google Takeout 解压后「我的活动 / Gemini Apps」文件夹，\
                       其中应包含活动记录的 .html 文件。"
                    .to_string(),
                example_path: Some("Takeout/我的活动/Gemini Apps".to_string()),
                detected_default_path: None,
                detected_default_label: None,
            }],
        }
    }

    fn detect(&self, input: &ImportInput) -> Result<ImportDetectResult, String> {
        let matched = resolve_gemini_export_root(&input.path).is_some();
        Ok(ImportDetectResult {
            matched,
            importer_id: self.id().to_string(),
            display_name: self.display_name().to_string(),
        })
    }

    fn preview(&self, input: &ImportInput) -> Result<ImportPreview, String> {
        let export_root = resolve_gemini_export_root(&input.path)
            .ok_or_else(|| format!("未找到 Gemini Takeout 活动记录: {}", input.path.display()))?;
        let conversations = load_conversations(&export_root)?;
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
        let export_root = resolve_gemini_export_root(&input.path)
            .ok_or_else(|| format!("未找到 Gemini Takeout 活动记录: {}", input.path.display()))?;

        if let Some(callback) = &options.on_progress {
            callback(ImportProgress {
                phase: "parsing".to_string(),
                progress: 0.2,
                processed: 0,
                total: 1,
            });
        }

        let media_index = MediaIndex::build(&export_root);
        let mut conversations = load_conversations(&export_root)?;

        for conversation in &mut conversations {
            for message in &mut conversation.messages {
                message.attachments = resolve_imported_attachments(
                    &message.attachments,
                    &message.role,
                    &media_index,
                );
            }
        }

        if let Some(callback) = &options.on_progress {
            callback(ImportProgress {
                phase: "parsing".to_string(),
                progress: 0.8,
                processed: 1,
                total: 1,
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

fn load_conversations(export_root: &std::path::Path) -> Result<Vec<crate::domain::ports::ImportedConversation>, String> {
    let html_path = find_activity_html(export_root)
        .ok_or_else(|| format!("未找到 Gemini 活动 HTML: {}", export_root.display()))?;
    let html = fs::read_to_string(&html_path)
        .map_err(|e| format!("读取 Gemini 活动记录失败 ({}): {e}", html_path.display()))?;
    Ok(parse_activity_html(&html))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_gemini_export_dir() -> PathBuf {
        PathBuf::from(
            r"D:\Personal\Gemini数据下载\takeout-20260622T160628Z-3-001\Takeout\我的活动\Gemini Apps",
        )
    }

    #[test]
    fn detect_matches_gemini_takeout_dir() {
        let dir = sample_gemini_export_dir();
        if !dir.is_dir() {
            return;
        }

        let importer = GeminiImporter::new();
        let result = importer
            .detect(&ImportInput { path: dir.clone() })
            .expect("detect");
        assert!(result.matched);
        assert_eq!(result.importer_id, "gemini");
    }

    #[test]
    fn import_reads_local_gemini_takeout() {
        let dir = sample_gemini_export_dir();
        if !dir.is_dir() {
            return;
        }

        let importer = GeminiImporter::new();
        let result = importer
            .import(&ImportInput { path: dir }, &ImportOptions::default())
            .expect("import");

        assert_eq!(result.source, DataSource::Gemini);
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
        assert!(with_images > 0, "expected gemini images to resolve to local paths");

        let upload_images = result
            .package
            .conversations
            .iter()
            .flat_map(|conversation| &conversation.messages)
            .flat_map(|message| &message.attachments)
            .filter(|attachment| attachment.source == "upload" && attachment.path.is_some())
            .count();
        assert!(upload_images > 0, "expected gemini user upload images to resolve");
    }
}
