use crate::domain::models::{DataSource, ImportProgress};
use crate::domain::ports::{
    ImportDetectResult, ImportGuide, ImportInput, ImportMethodGuide, ImportOptions, ImportPackage,
    ImportPreview, Importer, NormalizedImportResult,
};

use super::{is_deepseek_export, parse_export_dir, resolve_deepseek_export_root};

#[derive(Debug, Default, Clone, Copy)]
pub struct DeepSeekImporter;

impl DeepSeekImporter {
    pub fn new() -> Self {
        Self
    }
}

impl Importer for DeepSeekImporter {
    fn id(&self) -> &'static str {
        "deepseek"
    }

    fn display_name(&self) -> &'static str {
        "DeepSeek Export"
    }

    fn source(&self) -> DataSource {
        DataSource::DeepSeek
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn import_guide(&self) -> ImportGuide {
        ImportGuide {
            importer_id: self.id().to_string(),
            source: self.source().as_str().to_string(),
            display_name: "DeepSeek".to_string(),
            description: "DeepSeek 官方数据导出（设置 → 数据导出）".to_string(),
            support_status: "experimental".to_string(),
            support_summary: "ZIP / 解压目录".to_string(),
            recognition_hint: "Chatilus 会读取 conversations.json 中的 mapping / fragments 对话树，\
                               并识别 user.json。"
                .to_string(),
            methods: vec![
                ImportMethodGuide {
                    id: "directory".to_string(),
                    label: "选择解压后的导出目录".to_string(),
                    kind: "directory".to_string(),
                    dialog_title: "选择 DeepSeek 导出目录".to_string(),
                    extensions: Vec::new(),
                    hint: "选择已解压的文件夹，应包含 conversations.json 与 user.json。"
                        .to_string(),
                    example_path: None,
                    detected_default_path: None,
                    detected_default_label: None,
                },
                ImportMethodGuide {
                    id: "zip".to_string(),
                    label: "选择原始 ZIP 导出包".to_string(),
                    kind: "file".to_string(),
                    dialog_title: "选择 DeepSeek 导出 ZIP".to_string(),
                    extensions: vec!["zip".to_string()],
                    hint: "直接选择从 DeepSeek 下载的 .zip 导出包，Chatilus 会自动解压并导入。"
                        .to_string(),
                    example_path: None,
                    detected_default_path: None,
                    detected_default_label: None,
                },
            ],
        }
    }

    fn detect(&self, input: &ImportInput) -> Result<ImportDetectResult, String> {
        let matched = resolve_deepseek_export_root(&input.path)
            .or_else(|| {
                input
                    .path
                    .is_dir()
                    .then(|| is_deepseek_export(&input.path))
                    .filter(|ok| *ok)
                    .map(|_| input.path.clone())
            })
            .is_some();
        Ok(ImportDetectResult {
            matched,
            importer_id: self.id().to_string(),
            display_name: self.display_name().to_string(),
        })
    }

    fn preview(&self, input: &ImportInput) -> Result<ImportPreview, String> {
        let export_root = resolve_deepseek_export_root(&input.path)
            .ok_or_else(|| format!("未找到 DeepSeek 导出: {}", input.path.display()))?;
        let conversations = parse_export_dir(&export_root)?;
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
        let export_root = resolve_deepseek_export_root(&input.path)
            .ok_or_else(|| format!("未找到 DeepSeek 导出: {}", input.path.display()))?;

        if let Some(callback) = &options.on_progress {
            callback(ImportProgress {
                phase: "parsing".to_string(),
                progress: 0.2,
                processed: 0,
                total: 1,
            });
        }

        let conversations = parse_export_dir(&export_root)?;
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
            media_files_indexed: 0,
            package: ImportPackage { conversations },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_export_dir() -> PathBuf {
        PathBuf::from(r"D:\Personal\DeepSeek\deepseek_data-2026-06-24")
    }

    #[test]
    fn detect_matches_deepseek_export() {
        let dir = sample_export_dir();
        if !dir.is_dir() {
            return;
        }

        let importer = DeepSeekImporter::new();
        let result = importer
            .detect(&ImportInput { path: dir })
            .expect("detect");
        assert!(result.matched);
        assert_eq!(result.importer_id, "deepseek");
    }

    #[test]
    fn import_reads_local_deepseek_export() {
        let dir = sample_export_dir();
        if !dir.is_dir() {
            return;
        }

        let importer = DeepSeekImporter::new();
        let result = importer
            .import(&ImportInput { path: dir }, &ImportOptions::default())
            .expect("import");

        assert_eq!(result.source, DataSource::DeepSeek);
        assert!(!result.package.conversations.is_empty());
        assert!(result
            .package
            .conversations
            .iter()
            .any(|conversation| conversation.messages.len() >= 2));
    }
}
