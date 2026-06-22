use rusqlite::params;

use crate::models::SearchHit;

use super::helpers::build_fts_query;
use super::Database;

impl Database {
    pub fn search_messages(&self, query: &str, limit: i64) -> Result<Vec<SearchHit>, String> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        let mut stmt = self
            .conn
            .prepare(
                "SELECT
                    f.message_id,
                    f.conversation_id,
                    f.conversation_title,
                    m.role,
                    snippet(messages_fts, 3, '【', '】', '…', 32) AS snippet,
                    m.create_time
                 FROM messages_fts f
                 JOIN messages m ON m.id = f.message_id
                 WHERE messages_fts MATCH ?1
                 ORDER BY rank
                 LIMIT ?2",
            )
            .map_err(|e| format!("搜索失败: {e}"))?;

        let fts_query = build_fts_query(trimmed);
        let rows = stmt
            .query_map(params![fts_query, limit], |row| {
                Ok(SearchHit {
                    message_id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    conversation_title: row.get(2)?,
                    role: row.get(3)?,
                    snippet: row.get(4)?,
                    create_time: row.get(5)?,
                })
            })
            .map_err(|e| format!("搜索失败: {e}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("读取搜索结果失败: {e}"))
    }
}
