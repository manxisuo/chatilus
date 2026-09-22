use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection};

use crate::models::TagView;

use super::Database;

impl Database {
    pub fn list_tags(&self) -> AppResult<Vec<TagView>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT t.id, t.name, COUNT(ct.conversation_id) AS conversation_count
                 FROM tags t
                 LEFT JOIN conversation_tags ct ON ct.tag_id = t.id
                 GROUP BY t.id
                 ORDER BY t.name COLLATE NOCASE ASC",
            )
            .map_err(|e| AppError::Msg(format!("查询标签失败: {e}")))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(TagView {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    conversation_count: row.get(2)?,
                })
            })
            .map_err(|e| AppError::Msg(format!("查询标签失败: {e}")))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Msg(format!("读取标签失败: {e}")))
    }

    pub fn create_tag(&self, name: &str) -> AppResult<TagView> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(AppError::msg("标签名不能为空"));
        }

        self.conn
            .execute("INSERT INTO tags (name) VALUES (?1)", params![trimmed])
            .map_err(|e| AppError::Msg(format!("创建标签失败: {e}")))?;

        let id = self.conn.last_insert_rowid();
        Ok(TagView {
            id,
            name: trimmed.to_string(),
            conversation_count: 0,
        })
    }

    pub fn delete_tag(&self, tag_id: i64) -> AppResult<()> {
        self.conn
            .execute("DELETE FROM tags WHERE id = ?1", params![tag_id])
            .map_err(|e| AppError::Msg(format!("删除标签失败: {e}")))?;
        Ok(())
    }
}

pub(crate) fn get_conversation_tag_names(
    conn: &Connection,
    conversation_id: &str,
) -> AppResult<Vec<String>> {
    let mut stmt = conn
        .prepare(
            "SELECT t.name
             FROM tags t
             JOIN conversation_tags ct ON ct.tag_id = t.id
             WHERE ct.conversation_id = ?1
             ORDER BY t.name COLLATE NOCASE ASC",
        )
        .map_err(|e| AppError::Msg(format!("查询标签失败: {e}")))?;

    let rows = stmt
        .query_map(params![conversation_id], |row| row.get(0))
        .map_err(|e| AppError::Msg(format!("查询标签失败: {e}")))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Msg(format!("读取标签失败: {e}")))
}
