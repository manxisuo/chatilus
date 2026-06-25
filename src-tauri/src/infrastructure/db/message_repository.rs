use rusqlite::params;

use crate::domain::mappers::{message_from_db_fields, message_to_view};
use crate::domain::ports::MessageRepository;
use crate::models::MessageView;

use super::helpers::{clean_content_placeholders, message_snippet, parse_attachments_json};
use super::Database;

impl MessageRepository for Database {
    fn list_by_conversation(&self, conversation_id: &str) -> Result<Vec<MessageView>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, conversation_id, role, content, create_time, sort_order,
                        is_starred, attachments, raw_json
                 FROM messages
                 WHERE conversation_id = ?1
                 ORDER BY sort_order ASC",
            )
            .map_err(|e| format!("查询消息失败: {e}"))?;

        let rows = stmt
            .query_map(params![conversation_id], |row| {
                let id: String = row.get(0)?;
                let conversation_id: String = row.get(1)?;
                let role: String = row.get(2)?;
                let content = clean_content_placeholders(row.get(3)?);
                let create_time: Option<f64> = row.get(4)?;
                let sort_order: i64 = row.get(5)?;
                let is_starred = row.get::<_, i64>(6)? != 0;
                let attachments_raw: Option<String> = row.get(7)?;
                let raw_json: Option<String> = row.get(8)?;
                let attachments = attachments_raw
                    .as_deref()
                    .map(parse_attachments_json)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|item| !item.path.trim().is_empty())
                    .collect::<Vec<_>>();

                let message = message_from_db_fields(
                    id,
                    conversation_id,
                    &role,
                    content,
                    create_time,
                    sort_order,
                    is_starred,
                    &attachments,
                    raw_json,
                );
                Ok(message_to_view(message, attachments))
            })
            .map_err(|e| format!("查询消息失败: {e}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("读取消息失败: {e}"))
    }

    fn list_starred(
        &self,
        source: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<crate::models::SearchHit>, String> {
        let mut sql = String::from(
            "SELECT m.id, m.conversation_id, c.title, m.role, m.content, m.create_time,
                    COALESCE(c.source, 'chatgpt') AS source
             FROM messages m
             JOIN conversations c ON c.id = m.conversation_id
             WHERE m.is_starred = 1",
        );
        let mut bind: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(source) = source.map(str::trim).filter(|value| !value.is_empty()) {
            sql.push_str(" AND c.source = ?");
            bind.push(Box::new(source.to_string()));
        }

        sql.push_str(
            " ORDER BY COALESCE(m.create_time, 0) DESC, m.id DESC
              LIMIT ? OFFSET ?",
        );
        bind.push(Box::new(limit));
        bind.push(Box::new(offset));

        let mut stmt = self
            .conn
            .prepare(&sql)
            .map_err(|e| format!("查询收藏消息失败: {e}"))?;

        let param_refs: Vec<&dyn rusqlite::ToSql> = bind.iter().map(|value| value.as_ref()).collect();
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| {
                let content = clean_content_placeholders(row.get(4)?);
                Ok(crate::models::SearchHit {
                    message_id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    conversation_title: row.get(2)?,
                    role: row.get(3)?,
                    snippet: message_snippet(&content, 160),
                    create_time: row.get(5)?,
                    source: row.get(6)?,
                })
            })
            .map_err(|e| format!("查询收藏消息失败: {e}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("读取收藏消息失败: {e}"))
    }

    fn set_starred(&self, message_id: &str, starred: bool) -> Result<(), String> {
        let updated = self
            .conn
            .execute(
                "UPDATE messages SET is_starred = ?1 WHERE id = ?2",
                params![starred as i64, message_id],
            )
            .map_err(|e| format!("更新消息收藏失败: {e}"))?;

        if updated == 0 {
            return Err("消息不存在".to_string());
        }
        Ok(())
    }
}
