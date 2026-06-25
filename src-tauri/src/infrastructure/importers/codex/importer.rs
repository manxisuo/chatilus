use crate::domain::models::{DataSource, ImportProgress};
use crate::domain::ports::{
    ImportDetectResult, ImportGuide, ImportInput, ImportMethodGuide, ImportOptions, ImportPackage,
    ImportPreview, Importer, NormalizedImportResult, platform_display_path,
};

use super::db::{list_threads, open_codex_db};
use super::parse::thread_to_conversation;
use super::resolve::{is_codex_state_db, resolve_codex_home, resolve_codex_state_db};

#[derive(Debug, Default, Clone, Copy)]
pub struct CodexImporter;

impl CodexImporter {
    pub fn new() -> Self {
        Self
    }
}

impl Importer for CodexImporter {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn display_name(&self) -> &'static str {
        "Codex Local Data"
    }

    fn source(&self) -> DataSource {
        DataSource::Codex
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn import_guide(&self) -> ImportGuide {
        ImportGuide {
            importer_id: self.id().to_string(),
            source: self.source().as_str().to_string(),
            display_name: "Codex".to_string(),
            description: "OpenAI Codex CLI 本地会话数据".to_string(),
            support_status: "experimental".to_string(),
            support_summary: "本地 ~/.codex".to_string(),
            recognition_hint: "ChatLens 会读取 state_*.sqlite 中的线程索引，\
                               并解析 sessions 目录下的 rollout JSONL 会话记录。"
                .to_string(),
            methods: vec![
                ImportMethodGuide {
                    id: "home_dir".to_string(),
                    label: "选择 Codex 数据目录".to_string(),
                    kind: "directory".to_string(),
                    dialog_title: "选择 Codex 数据目录".to_string(),
                    extensions: Vec::new(),
                    hint: "选择含 sessions 子目录与 state_*.sqlite 的 Codex 数据目录。".to_string(),
                    example_path: Some(platform_display_path(
                        r"%USERPROFILE%\.codex",
                        "~/.codex",
                        "~/.codex",
                    )),
                    detected_default_path: None,
                    detected_default_label: None,
                },
                ImportMethodGuide {
                    id: "state_db".to_string(),
                    label: "选择 state SQLite 文件".to_string(),
                    kind: "file".to_string(),
                    dialog_title: "选择 Codex state SQLite".to_string(),
                    extensions: vec!["sqlite".to_string()],
                    hint: "直接选择 state_*.sqlite 文件（通常在 .codex 目录下）。".to_string(),
                    example_path: Some(platform_display_path(
                        r"%USERPROFILE%\.codex\state_5.sqlite",
                        "~/.codex/state_5.sqlite",
                        "~/.codex/state_5.sqlite",
                    )),
                    detected_default_path: None,
                    detected_default_label: None,
                },
            ],
        }
    }

    fn detect(&self, input: &ImportInput) -> Result<ImportDetectResult, String> {
        let matched = resolve_codex_state_db(&input.path).is_some();
        Ok(ImportDetectResult {
            matched,
            importer_id: self.id().to_string(),
            display_name: self.display_name().to_string(),
        })
    }

    fn preview(&self, input: &ImportInput) -> Result<ImportPreview, String> {
        let (conn, _) = open_codex_db(&input.path)?;
        let threads = list_threads(&conn)?;
        Ok(ImportPreview {
            importer_id: self.id().to_string(),
            display_name: self.display_name().to_string(),
            conversation_count: threads.len(),
            shard_file_count: threads.len(),
        })
    }

    fn import(
        &self,
        input: &ImportInput,
        options: &ImportOptions,
    ) -> Result<NormalizedImportResult, String> {
        let (conn, db_path) = open_codex_db(&input.path)?;
        let threads = list_threads(&conn)?;
        let total = threads.len().max(1);
        let mut conversations = Vec::new();

        for (index, thread) in threads.iter().enumerate() {
            if let Some(conversation) = thread_to_conversation(thread) {
                conversations.push(conversation);
            }
            if let Some(callback) = &options.on_progress {
                callback(ImportProgress {
                    phase: "parsing".to_string(),
                    progress: 0.2 + ((index + 1) as f64 / total as f64) * 0.6,
                    processed: index + 1,
                    total,
                });
            }
        }

        if let Some(callback) = &options.on_progress {
            callback(ImportProgress {
                phase: "parsing".to_string(),
                progress: 0.85,
                processed: total,
                total,
            });
        }

        Ok(NormalizedImportResult {
            source: self.source(),
            source_path: resolve_codex_home(&input.path)
                .unwrap_or_else(|| db_path.parent().unwrap_or(&db_path).to_path_buf())
                .display()
                .to_string(),
            files_processed: threads.len(),
            media_files_indexed: 0,
            package: ImportPackage { conversations },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_codex_home() -> PathBuf {
        std::env::var("CODEX_HOME")
            .map(PathBuf::from)
            .or_else(|_| {
                std::env::var("USERPROFILE").map(|home| PathBuf::from(home).join(".codex"))
            })
            .unwrap_or_default()
    }

    #[test]
    fn detect_matches_codex_home() {
        let home = sample_codex_home();
        if !home.is_dir() {
            return;
        }

        let importer = CodexImporter::new();
        let result = importer
            .detect(&ImportInput { path: home.clone() })
            .expect("detect");
        assert!(result.matched);
        assert_eq!(result.importer_id, "codex");
    }

    #[test]
    fn detect_matches_codex_state_db_file() {
        let home = sample_codex_home();
        let db = home.join("state_5.sqlite");
        if !is_codex_state_db(&db) {
            return;
        }

        let importer = CodexImporter::new();
        let result = importer
            .detect(&ImportInput { path: db })
            .expect("detect");
        assert!(result.matched);
    }

    #[test]
    fn import_reads_local_codex_data() {
        let home = sample_codex_home();
        if !home.is_dir() {
            return;
        }

        let importer = CodexImporter::new();
        let result = importer
            .import(&ImportInput { path: home }, &ImportOptions::default())
            .expect("import");

        assert_eq!(result.source, DataSource::Codex);
        assert!(!result.package.conversations.is_empty());
        assert!(result
            .package
            .conversations
            .iter()
            .any(|conversation| conversation.messages.len() >= 1));
    }
}
