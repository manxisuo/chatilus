use rusqlite::{params, OptionalExtension};

use super::shared::*;
use crate::domain::ports::ConversationListQuery;
use crate::error::{AppError, AppResult};
use crate::infrastructure::db::helpers::map_conversation_summary;
use crate::infrastructure::db::source_context_index::attach_source_contexts;
use crate::infrastructure::db::Database;
use crate::models::ConversationSummary;

    pub(crate) fn list(db: &Database, query: ConversationListQuery) -> AppResult<Vec<ConversationSummary>> {
        let mut sql = String::from(
            "SELECT c.id, c.title, c.create_time, c.update_time, c.model, c.message_count,
                    c.is_starred, c.source_path, COALESCE(c.source, 'chatgpt') AS source,
                    COALESCE(GROUP_CONCAT(t.name, char(31)), '') AS tag_names
             FROM conversations c
             LEFT JOIN conversation_tags ct ON ct.conversation_id = c.id
             LEFT JOIN tags t ON t.id = ct.tag_id
             WHERE 1 = 1",
        );
        let mut bind: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(text) = query
            .text_query
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
        {
            sql.push_str(" AND c.title LIKE ?");
            bind.push(Box::new(format!("%{text}%")));
        }
        if query.starred_only {
            sql.push_str(" AND c.is_starred = 1");
        }
        if let Some(tag) = query.tag_id {
            sql.push_str(
                " AND c.id IN (SELECT conversation_id FROM conversation_tags WHERE tag_id = ?)",
            );
            bind.push(Box::new(tag));
        }
        if let Some(source) = query
            .source
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            sql.push_str(" AND ");
            sql.push_str(SOURCE_EQUALS_SQL);
            bind.push(Box::new(source.to_string()));
        }
        if query.has_images && query.has_attachments {
            sql.push_str(" AND (");
            sql.push_str(HAS_IMAGES_EXISTS_SQL);
            sql.push_str(" OR ");
            sql.push_str(HAS_ATTACHMENTS_EXISTS_SQL);
            sql.push_str(")");
        } else if query.has_images {
            sql.push_str(" AND ");
            sql.push_str(HAS_IMAGES_EXISTS_SQL);
        } else if query.has_attachments {
            sql.push_str(" AND ");
            sql.push_str(HAS_ATTACHMENTS_EXISTS_SQL);
        }
        if query.has_code {
            sql.push_str(
                " AND EXISTS (
                    SELECT 1 FROM messages m
                    WHERE m.conversation_id = c.id
                      AND m.content LIKE '%```%'
                )",
            );
        }

        sql.push_str(
            " GROUP BY c.id
              ORDER BY c.is_starred DESC, COALESCE(c.update_time, c.create_time, 0) DESC
              LIMIT ? OFFSET ?",
        );
        bind.push(Box::new(query.limit));
        bind.push(Box::new(query.offset));

        let mut stmt = db
            .conn
            .prepare(&sql)
            .map_err(|e| AppError::Msg(format!("查询会话失败: {e}")))?;

        let param_refs: Vec<&dyn rusqlite::ToSql> = bind.iter().map(|value| value.as_ref()).collect();
        let rows = stmt
            .query_map(param_refs.as_slice(), map_conversation_summary)
            .map_err(|e| AppError::Msg(format!("查询会话失败: {e}")))?;

        let mut summaries = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Msg(format!("读取会话失败: {e}")))?;
        attach_source_contexts(&db.conn, &mut summaries)?;
        Ok(summaries)
    }

    pub(crate) fn get_summary(db: &Database, conversation_id: &str) -> AppResult<Option<ConversationSummary>> {
        let mut stmt = db
            .conn
            .prepare(
                "SELECT c.id, c.title, c.create_time, c.update_time, c.model, c.message_count,
                        c.is_starred, c.source_path, COALESCE(c.source, 'chatgpt') AS source,
                        COALESCE(GROUP_CONCAT(t.name, char(31)), '') AS tag_names
                 FROM conversations c
                 LEFT JOIN conversation_tags ct ON ct.conversation_id = c.id
                 LEFT JOIN tags t ON t.id = ct.tag_id
                 WHERE c.id = ?1
                 GROUP BY c.id",
            )
            .map_err(|e| AppError::Msg(format!("查询会话失败: {e}")))?;

        let mut summary = stmt
            .query_row(params![conversation_id], map_conversation_summary)
            .optional()
            .map_err(|e| AppError::Msg(format!("读取会话失败: {e}")))?;

        if let Some(item) = summary.as_mut() {
            attach_source_contexts(&db.conn, std::slice::from_mut(item))?;
        }
        Ok(summary)
    }

