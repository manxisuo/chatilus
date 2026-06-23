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
        let conversation_counts_by_source = self.conversation_counts_by_source()?;
        let image_counts_by_source = AssetRepository::image_counts_by_conversation_source(self)?;
        let last_imported_at: Option<f64> = self
            .conn
            .query_row(
                "SELECT MAX(imported_at) FROM imports",
                [],
                |row| row.get(0),
            )
            .map_err(|e| format!("统计最近导入时间失败: {e}"))?;

        Ok(DatabaseStats {
            conversation_count,
            message_count,
            image_count,
            generated_image_count,
            upload_image_count,
            starred_conversation_count,
            starred_message_count,
            tag_count,
            last_imported_at,
            db_path: self.path.clone(),
            conversation_counts_by_source,
            image_counts_by_source,
        })
    }

    fn conversation_counts_by_source(&self) -> Result<Vec<crate::models::SourceCount>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT COALESCE(source, 'chatgpt') AS source, COUNT(*) AS count
                 FROM conversations
                 GROUP BY COALESCE(source, 'chatgpt')
                 ORDER BY count DESC, source ASC",
            )
            .map_err(|e| format!("统计来源会话数失败: {e}"))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(crate::models::SourceCount {
                    source: row.get(0)?,
                    count: row.get(1)?,
                })
            })
            .map_err(|e| format!("读取来源会话统计失败: {e}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("读取来源会话统计失败: {e}"))
    }
}
