use std::fs;
use std::path::Path;

use rusqlite::{params, Connection};

use crate::domain::import_merge::dedup_import_package;
use crate::domain::models::SourceInfo;
use crate::domain::ports::{ConversationRepository, NormalizedImportResult};
use crate::models::ImportResult;

mod asset_index;
mod asset_repository;
mod conversation_repository;
mod helpers;
mod import_job_repository;
mod message_repository;
mod migration;
mod schema;
mod stats;
mod tag_repository;

pub struct Database {
    pub(crate) conn: Connection,
    pub(crate) path: String,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("无法创建数据库目录: {e}"))?;
        }

        let conn = Connection::open(path).map_err(|e| format!("无法打开数据库: {e}"))?;
        let db = Self {
            conn,
            path: path.display().to_string(),
        };
        db.init_schema()?;
        Ok(db)
    }

    pub fn persist_import(
        &mut self,
        imported: &NormalizedImportResult,
        source_info: &SourceInfo,
    ) -> Result<ImportResult, String> {
        let source = imported.source_path.clone();
        let files_processed = imported.files_processed;
        let media_files_indexed = imported.media_files_indexed;
        let (source_key, export_label, importer_version) =
            Database::source_info_sql_values(source_info);

        let (conversations, conversations_deduplicated) =
            dedup_import_package(imported.package.conversations.clone());

        let counts = ConversationRepository::save_many(
            self,
            &conversations,
            &source,
            source_info.source,
            conversations_deduplicated,
        )?;

        self.conn
            .execute(
                "INSERT INTO imports (
                    source_path, imported_at, conversation_count, message_count,
                    source, export_label, importer_version
                 ) VALUES (?1, unixepoch('subsec'), ?2, ?3, ?4, ?5, ?6)",
                params![
                    source,
                    counts.new_conversations as i64,
                    counts.messages as i64,
                    source_key,
                    export_label,
                    importer_version,
                ],
            )
            .map_err(|e| format!("记录导入历史失败: {e}"))?;

        Ok(ImportResult {
            conversations_imported: counts.new_conversations,
            conversations_updated: counts.updated_conversations,
            conversations_deduplicated: counts.conversations_deduplicated,
            messages_imported: counts.messages,
            files_processed,
            source_path: source,
            media_files_indexed,
        })
    }
}
