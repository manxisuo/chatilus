use crate::domain::ports::AssetRepository;
use crate::models::DatabaseStats;

use super::Database;

impl Database {
    pub fn stats(&self) -> Result<DatabaseStats, String> {
        let conversation_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM conversations", [], |row| row.get(0))
            .map_err(|e| format!("统计失败: {e}"))?;
        let message_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))
            .map_err(|e| format!("统计失败: {e}"))?;
        let starred_conversation_count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM conversations WHERE is_starred = 1",
                [],
                |row| row.get(0),
            )
            .map_err(|e| format!("统计失败: {e}"))?;
        let starred_message_count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE is_starred = 1",
                [],
                |row| row.get(0),
            )
            .map_err(|e| format!("统计失败: {e}"))?;
        let tag_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM tags", [], |row| row.get(0))
            .map_err(|e| format!("统计失败: {e}"))?;
        let (image_count, generated_image_count, upload_image_count) =
            AssetRepository::count_by_source(self)?;

        Ok(DatabaseStats {
            conversation_count,
            message_count,
            image_count,
            generated_image_count,
            upload_image_count,
            starred_conversation_count,
            starred_message_count,
            tag_count,
            db_path: self.path.clone(),
        })
    }
}
