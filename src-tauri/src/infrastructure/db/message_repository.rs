use std::path::Path;

use rusqlite::{params, OptionalExtension};

use crate::domain::mappers::{message_from_db_fields, message_to_view};
use crate::domain::ports::MessageRepository;
use crate::infrastructure::importers::chatgpt::{
    attachments::{
        enrich_attachment_view, imported_attachments_to_views, resolve_imported_attachments,
    },
    extract_attachment_infos_from_message_json,
};
use crate::infrastructure::media::MediaIndex;
use crate::models::MessageView;

use super::helpers::{clean_content_placeholders, parse_attachments_json};
use super::Database;

impl MessageRepository for Database {
    fn list_by_conversation(&self, conversation_id: &str) -> Result<Vec<MessageView>, String> {
        let source_path: Option<String> = self
            .conn
            .query_row(
                "SELECT source_path FROM conversations WHERE id = ?1",
                params![conversation_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| format!("查询对话来源失败: {e}"))?;

        let media_index = source_path
            .as_deref()
            .filter(|path| Path::new(path).is_dir())
            .map(|path| MediaIndex::build(Path::new(path)));

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
                let mut attachments = attachments_raw
                    .as_deref()
                    .map(parse_attachments_json)
                    .unwrap_or_default();

                if attachments.is_empty() {
                    if let (Some(ref index), Some(ref raw)) = (&media_index, &raw_json) {
                        attachments = imported_attachments_to_views(&resolve_imported_attachments(
                            &extract_attachment_infos_from_message_json(raw),
                            &role,
                            index,
                        ));
                    }
                } else {
                    attachments = attachments
                        .into_iter()
                        .map(|attachment| enrich_attachment_view(attachment, &role))
                        .collect();
                }

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
