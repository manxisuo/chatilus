use rusqlite::params;

use crate::error::{AppError, AppResult};
use crate::infrastructure::db::tag_repository::get_conversation_tag_names;
use crate::infrastructure::db::Database;

    pub(crate) fn set_starred(db: &Database, conversation_id: &str, starred: bool) -> AppResult<()> {
        let updated = db
            .conn
            .execute(
                "UPDATE conversations SET is_starred = ?1 WHERE id = ?2",
                params![starred as i64, conversation_id],
            )
            .map_err(|e| AppError::Msg(format!("更新收藏失败: {e}")))?;

        if updated == 0 {
            return Err(AppError::msg("对话不存在"));
        }
        Ok(())
    }

    pub(crate) fn set_tags(db: &Database, conversation_id: &str, tag_ids: &[i64]) -> AppResult<Vec<String>> {
        let exists: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM conversations WHERE id = ?1",
                params![conversation_id],
                |row| row.get(0),
            )
            .map_err(|e| AppError::Msg(format!("检查对话失败: {e}")))?;
        if exists == 0 {
            return Err(AppError::msg("对话不存在"));
        }

        let tx = db
            .conn
            .unchecked_transaction()
            .map_err(|e| AppError::Msg(format!("开启事务失败: {e}")))?;

        tx.execute(
            "DELETE FROM conversation_tags WHERE conversation_id = ?1",
            params![conversation_id],
        )
        .map_err(|e| AppError::Msg(format!("清理标签失败: {e}")))?;

        for tag_id in tag_ids {
            tx.execute(
                "INSERT INTO conversation_tags (conversation_id, tag_id) VALUES (?1, ?2)",
                params![conversation_id, tag_id],
            )
            .map_err(|e| AppError::Msg(format!("设置标签失败: {e}")))?;
        }

        tx.commit()
            .map_err(|e| AppError::Msg(format!("提交标签失败: {e}")))?;

        get_conversation_tag_names(&db.conn, conversation_id)
    }

