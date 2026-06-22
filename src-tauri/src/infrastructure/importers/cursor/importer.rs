use crate::domain::models::{DataSource, ImportProgress};
use crate::domain::ports::{
    ImportDetectResult, ImportInput, ImportOptions, ImportPackage, ImportPreview, Importer,
    NormalizedImportResult,
};

use super::db::{open_cursor_db, parse_all_conversations, resolve_cursor_db_path};

#[derive(Debug, Default, Clone, Copy)]
pub struct CursorImporter;

impl CursorImporter {
    pub fn new() -> Self {
        Self
    }
}

impl Importer for CursorImporter {
    fn id(&self) -> &'static str {
        "cursor"
    }

    fn display_name(&self) -> &'static str {
        "Cursor Local Data"
    }

    fn source(&self) -> DataSource {
        DataSource::Cursor
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn detect(&self, input: &ImportInput) -> Result<ImportDetectResult, String> {
        let matched = resolve_cursor_db_path(&input.path).is_some();
        Ok(ImportDetectResult {
            matched,
            importer_id: self.id().to_string(),
            display_name: self.display_name().to_string(),
        })
    }

    fn preview(&self, input: &ImportInput) -> Result<ImportPreview, String> {
        let db_path = resolve_cursor_db_path(&input.path)
            .ok_or_else(|| format!("未找到 Cursor state.vscdb: {}", input.path.display()))?;
        let conn = open_cursor_db(&db_path)?;
        let conversations = parse_all_conversations(&conn)?;
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
        let db_path = resolve_cursor_db_path(&input.path)
            .ok_or_else(|| format!("未找到 Cursor state.vscdb: {}", input.path.display()))?;
        let conn = open_cursor_db(&db_path)?;

        if let Some(callback) = &options.on_progress {
            callback(ImportProgress {
                phase: "parsing".to_string(),
                progress: 0.2,
                processed: 0,
                total: 1,
            });
        }

        let conversations = parse_all_conversations(&conn)?;

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
            source_path: db_path.display().to_string(),
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

    fn sample_cursor_root() -> PathBuf {
        std::env::var("APPDATA")
            .map(|appdata| PathBuf::from(appdata).join("Cursor").join("User"))
            .unwrap_or_default()
    }

    #[test]
    fn detect_matches_cursor_user_dir() {
        let root = sample_cursor_root();
        if !root.is_dir() {
            return;
        }

        let importer = CursorImporter::new();
        let result = importer
            .detect(&ImportInput { path: root })
            .expect("detect");
        assert!(result.matched);
        assert_eq!(result.importer_id, "cursor");
    }

    #[test]
    fn import_reads_local_cursor_data() {
        let root = sample_cursor_root();
        if !root.is_dir() {
            return;
        }

        let importer = CursorImporter::new();
        let result = importer
            .import(&ImportInput { path: root }, &ImportOptions::default())
            .expect("import");

        assert_eq!(result.source, DataSource::Cursor);
        assert!(!result.package.conversations.is_empty());
    }
}
