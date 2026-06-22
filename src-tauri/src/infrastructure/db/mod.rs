use std::fs;
use std::path::Path;

use rusqlite::{params, Connection};

use crate::domain::ports::{ConversationRepository, NormalizedImportResult};
use crate::models::ImportResult;

mod asset_repository;
mod conversation_repository;
mod helpers;
mod message_repository;
mod schema;
mod search;
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
    ) -> Result<ImportResult, String> {
        let source = imported.source_path.clone();
        let files_processed = imported.files_processed;
        let media_files_indexed = imported.media_files_indexed;

        let counts = ConversationRepository::save_many(
            self,
            &imported.package.conversations,
            &source,
        )?;

        self.conn
            .execute(
                "INSERT INTO imports (source_path, imported_at, conversation_count, message_count)
                 VALUES (?1, unixepoch('subsec'), ?2, ?3)",
                params![
                    source,
                    counts.new_conversations as i64,
                    counts.messages as i64
                ],
            )
            .map_err(|e| format!("记录导入历史失败: {e}"))?;

        Ok(ImportResult {
            conversations_imported: counts.new_conversations,
            messages_imported: counts.messages,
            files_processed,
            source_path: source,
            media_files_indexed,
        })
    }
}
