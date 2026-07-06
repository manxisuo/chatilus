use crate::domain::models::{DataSource, ImportProgress};
use crate::domain::ports::{
    ImportDetectResult, ImportGuide, ImportInput, ImportMethodGuide, ImportOptions, ImportPackage,
    ImportPreview, Importer, NormalizedImportResult,
};

use super::detect::resolve_copilot_csv_path;
use super::parse::parse_copilot_csv;

#[derive(Debug, Default, Clone, Copy)]
pub struct CopilotImporter;

impl CopilotImporter {
    pub fn new() -> Self {
        Self
    }
}

impl Importer for CopilotImporter {
    fn id(&self) -> &'static str {
        "copilot"
    }

    fn display_name(&self) -> &'static str {
        "Microsoft Copilot Export"
    }

    fn source(&self) -> DataSource {
        DataSource::Copilot
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn import_guide(&self) -> ImportGuide {
        ImportGuide {
            importer_id: self.id().to_string(),
            source: self.source().as_str().to_string(),
            display_name: "Copilot".to_string(),
            description: "Microsoft 账户隐私门户导出的 Copilot 活动历史".to_string(),
            support_status: "experimental".to_string(),
            support_summary: "CSV".to_string(),
            recognition_hint: "ChatLens 会读取 copilot-activity-history.csv 中的 Conversation / \
                               Time / Author / Message 列，并按会话标题聚合消息。"
                .to_string(),
            methods: vec![
                ImportMethodGuide {
                    id: "csv".to_string(),
                    label: "选择 Copilot 活动历史 CSV".to_string(),
                    kind: "file".to_string(),
                    dialog_title: "选择 copilot-activity-history.csv".to_string(),
                    extensions: vec!["csv".to_string()],
                    hint: "从 https://account.microsoft.com/privacy/copilot 下载导出包后，\
                           选择其中的 copilot-activity-history.csv。"
                        .to_string(),
                    example_path: Some("copilot-activity-history.csv".to_string()),
                    detected_default_path: None,
                    detected_default_label: None,
                },
                ImportMethodGuide {
                    id: "directory".to_string(),
                    label: "选择包含 CSV 的文件夹".to_string(),
                    kind: "directory".to_string(),
                    dialog_title: "选择 Copilot 导出目录".to_string(),
                    extensions: Vec::new(),
                    hint: "选择解压后的 Copilot 导出文件夹，ChatLens 会自动查找 \
                           copilot-activity-history.csv。"
                        .to_string(),
                    example_path: None,
                    detected_default_path: None,
                    detected_default_label: None,
                },
            ],
        }
    }

    fn detect(&self, input: &ImportInput) -> Result<ImportDetectResult, String> {
        let matched = resolve_copilot_csv_path(&input.path).is_some();
        Ok(ImportDetectResult {
            matched,
            importer_id: self.id().to_string(),
            display_name: self.display_name().to_string(),
        })
    }

    fn preview(&self, input: &ImportInput) -> Result<ImportPreview, String> {
        let csv_path = resolve_copilot_csv_path(&input.path)
            .ok_or_else(|| format!("未找到 Copilot 活动历史 CSV: {}", input.path.display()))?;
        let conversations = parse_copilot_csv(&csv_path)?;
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
        let csv_path = resolve_copilot_csv_path(&input.path)
            .ok_or_else(|| format!("未找到 Copilot 活动历史 CSV: {}", input.path.display()))?;

        if let Some(callback) = &options.on_progress {
            callback(ImportProgress {
                phase: "parsing".to_string(),
                progress: 0.2,
                processed: 0,
                total: 1,
            });
        }

        let conversations = parse_copilot_csv(&csv_path)?;
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
            source_path: csv_path.display().to_string(),
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

    fn sample_csv_path() -> PathBuf {
        PathBuf::from(r"D:\Personal\CopilotApp\copilot-activity-history.csv")
    }

    #[test]
    fn detect_matches_copilot_csv() {
        let path = sample_csv_path();
        if !path.is_file() {
            return;
        }

        let importer = CopilotImporter::new();
        let result = importer
            .detect(&ImportInput { path: path.clone() })
            .expect("detect");
        assert!(result.matched);
        assert_eq!(result.importer_id, "copilot");
    }

    #[test]
    fn import_reads_local_copilot_csv() {
        let path = sample_csv_path();
        if !path.is_file() {
            return;
        }

        let importer = CopilotImporter::new();
        let result = importer
            .import(&ImportInput { path }, &ImportOptions::default())
            .expect("import");

        assert_eq!(result.source, DataSource::Copilot);
        assert!(!result.package.conversations.is_empty());
        assert!(result
            .package
            .conversations
            .iter()
            .any(|conversation| conversation.messages.len() >= 2));
    }
}
