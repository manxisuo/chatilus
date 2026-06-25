use crate::domain::models::{DataSource, ImportProgress};
use crate::domain::ports::{
    ImportDetectResult, ImportGuide, ImportInput, ImportMethodGuide, ImportOptions, ImportPackage,
    ImportPreview, Importer, NormalizedImportResult, platform_display_path,
};
use crate::infrastructure::attachments::resolve_imported_attachments;
use crate::infrastructure::media::MediaIndex;

use super::db::{open_cursor_db, parse_all_conversations, resolve_cursor_db_path, resolve_cursor_user_dir};

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

    fn import_guide(&self) -> ImportGuide {
        ImportGuide {
            importer_id: self.id().to_string(),
            source: self.source().as_str().to_string(),
            display_name: "Cursor".to_string(),
            description: "Cursor IDE 本地对话数据库".to_string(),
            support_status: "stable".to_string(),
            support_summary: "本地 state.vscdb".to_string(),
            recognition_hint: "ChatLens 会读取 state.vscdb 中的工作区、会话和消息记录。".to_string(),
            methods: vec![
                ImportMethodGuide {
                    id: "vscdb".to_string(),
                    label: "选择 state.vscdb 文件".to_string(),
                    kind: "file".to_string(),
                    dialog_title: "选择 Cursor state.vscdb".to_string(),
                    extensions: vec!["vscdb".to_string()],
                    hint: "推荐直接选择 Cursor 的全局状态数据库文件。".to_string(),
                    example_path: Some(platform_display_path(
                        r"%APPDATA%\Cursor\User\globalStorage\state.vscdb",
                        "~/Library/Application Support/Cursor/User/globalStorage/state.vscdb",
                        "~/.config/Cursor/User/globalStorage/state.vscdb",
                    )),
                    detected_default_path: None,
                    detected_default_label: None,
                },
                ImportMethodGuide {
                    id: "user_dir".to_string(),
                    label: "选择 Cursor User 目录".to_string(),
                    kind: "directory".to_string(),
                    dialog_title: "选择 Cursor User 目录".to_string(),
                    extensions: Vec::new(),
                    hint: "选择包含 globalStorage/state.vscdb 的 User 目录；\
                           ChatLens 会自动定位数据库文件。"
                        .to_string(),
                    example_path: Some(platform_display_path(
                        r"%APPDATA%\Cursor\User",
                        "~/Library/Application Support/Cursor/User",
                        "~/.config/Cursor/User",
                    )),
                    detected_default_path: None,
                    detected_default_label: None,
                },
            ],
        }
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

        let mut conversations = parse_all_conversations(&conn)?;

        let user_dir = resolve_cursor_user_dir(&db_path)
            .ok_or_else(|| format!("无法定位 Cursor User 目录: {}", db_path.display()))?;
        let media_index = MediaIndex::build_cursor(&user_dir);

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
            source_path: db_path.display().to_string(),
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

        if result.media_files_indexed > 0 {
            let with_images = result
                .package
                .conversations
                .iter()
                .flat_map(|conversation| &conversation.messages)
                .flat_map(|message| &message.attachments)
                .filter(|attachment| attachment.path.is_some())
                .count();
            assert!(with_images > 0, "expected cursor images to resolve to local paths");

            let generated_images = result
                .package
                .conversations
                .iter()
                .flat_map(|conversation| &conversation.messages)
                .flat_map(|message| &message.attachments)
                .filter(|attachment| {
                    attachment.source == "generated" && attachment.path.is_some()
                })
                .count();
            assert!(
                generated_images > 0,
                "expected cursor generated images to resolve to local paths"
            );
        }
    }
}
