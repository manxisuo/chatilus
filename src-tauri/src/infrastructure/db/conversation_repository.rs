use std::fs;
use std::path::Path;

use rusqlite::{params, OptionalExtension, Transaction};

use crate::domain::ports::{
    ConversationListQuery, ConversationRepository, ImportPersistCounts, ImportedConversation,
    MessageRepository,
};
use crate::infrastructure::importers::chatgpt::attachments::imported_attachments_to_views;
use crate::models::{ConversationSummary, ExportResult};

use super::helpers::{
    format_timestamp, map_conversation_summary, role_heading,
};
use super::tag_repository::get_conversation_tag_names;
use super::Database;

impl ConversationRepository for Database {
    fn list(&self, query: ConversationListQuery) -> Result<Vec<ConversationSummary>, String> {
        let mut sql = String::from(
            "SELECT c.id, c.title, c.create_time, c.update_time, c.model, c.message_count,
                    c.is_starred, c.source_path,
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

        sql.push_str(
            " GROUP BY c.id
              ORDER BY c.is_starred DESC, COALESCE(c.update_time, c.create_time, 0) DESC
              LIMIT ? OFFSET ?",
        );
        bind.push(Box::new(query.limit));
        bind.push(Box::new(query.offset));

        let mut stmt = self
            .conn
            .prepare(&sql)
            .map_err(|e| format!("查询会话失败: {e}"))?;

        let param_refs: Vec<&dyn rusqlite::ToSql> = bind.iter().map(|value| value.as_ref()).collect();
        let rows = stmt
            .query_map(param_refs.as_slice(), map_conversation_summary)
            .map_err(|e| format!("查询会话失败: {e}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("读取会话失败: {e}"))
    }

    fn set_starred(&self, conversation_id: &str, starred: bool) -> Result<(), String> {
        let updated = self
            .conn
            .execute(
                "UPDATE conversations SET is_starred = ?1 WHERE id = ?2",
                params![starred as i64, conversation_id],
            )
            .map_err(|e| format!("更新收藏失败: {e}"))?;

        if updated == 0 {
            return Err("对话不存在".to_string());
        }
        Ok(())
    }

    fn set_tags(
        &self,
        conversation_id: &str,
        tag_ids: &[i64],
    ) -> Result<Vec<String>, String> {
        let exists: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM conversations WHERE id = ?1",
                params![conversation_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("检查对话失败: {e}"))?;
        if exists == 0 {
            return Err("对话不存在".to_string());
        }

        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|e| format!("开启事务失败: {e}"))?;

        tx.execute(
            "DELETE FROM conversation_tags WHERE conversation_id = ?1",
            params![conversation_id],
        )
        .map_err(|e| format!("清理标签失败: {e}"))?;

        for tag_id in tag_ids {
            tx.execute(
                "INSERT INTO conversation_tags (conversation_id, tag_id) VALUES (?1, ?2)",
                params![conversation_id, tag_id],
            )
            .map_err(|e| format!("设置标签失败: {e}"))?;
        }

        tx.commit()
            .map_err(|e| format!("提交标签失败: {e}"))?;

        get_conversation_tag_names(&self.conn, conversation_id)
    }

    fn export_markdown(
        &self,
        conversation_id: &str,
        output_path: &Path,
    ) -> Result<ExportResult, String> {
        let (title, model, create_time, update_time) = self
            .conn
            .query_row(
                "SELECT title, model, create_time, update_time FROM conversations WHERE id = ?1",
                params![conversation_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<f64>>(2)?,
                        row.get::<_, Option<f64>>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| format!("查询对话失败: {e}"))?
            .ok_or_else(|| "对话不存在".to_string())?;

        let messages = MessageRepository::list_by_conversation(self, conversation_id)?;
        let mut markdown = String::new();
        markdown.push_str(&format!("# {title}\n\n"));
        if let Some(model) = model {
            markdown.push_str(&format!("- 模型：`{model}`\n"));
        }
        if let Some(ts) = create_time {
            markdown.push_str(&format!("- 创建时间：{}\n", format_timestamp(ts)));
        }
        if let Some(ts) = update_time {
            markdown.push_str(&format!("- 更新时间：{}\n", format_timestamp(ts)));
        }
        markdown.push_str("\n---\n\n");

        for message in &messages {
            markdown.push_str(&format!("## {}\n\n", role_heading(&message.role)));
            if let Some(ts) = message.create_time {
                markdown.push_str(&format!("*{}\n\n", format_timestamp(ts)));
            }
            markdown.push_str(&message.content);
            markdown.push('\n');

            for attachment in &message.attachments {
                markdown.push_str(&format!(
                    "\n![{}]({})\n",
                    attachment.file_key, attachment.path
                ));
            }

            markdown.push_str("\n---\n\n");
        }

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("创建导出目录失败: {e}"))?;
        }
        fs::write(output_path, markdown).map_err(|e| format!("写入 Markdown 失败: {e}"))?;

        Ok(ExportResult {
            path: output_path.display().to_string(),
            message_count: messages.len() as i64,
        })
    }

    fn save_many(
        &mut self,
        conversations: &[ImportedConversation],
        source: &str,
    ) -> Result<ImportPersistCounts, String> {
        let tx = self
            .conn
            .transaction()
            .map_err(|e| format!("开启事务失败: {e}"))?;

        let mut new_conversations = 0usize;
        let mut message_count = 0usize;

        for conversation in conversations {
            let inserted = upsert_conversation(&tx, conversation, source)?;
            if inserted {
                new_conversations += 1;
            }
            message_count += insert_messages(&tx, conversation)?;
        }

        tx.commit()
            .map_err(|e| format!("提交事务失败: {e}"))?;

        Ok(ImportPersistCounts {
            new_conversations,
            messages: message_count,
        })
    }
}

fn upsert_conversation(
    tx: &Transaction<'_>,
    conversation: &ImportedConversation,
    source: &str,
) -> Result<bool, String> {
    let exists: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM conversations WHERE id = ?1",
            params![conversation.id],
            |row| row.get(0),
        )
        .map_err(|e| format!("检查会话失败: {e}"))?;

    tx.execute(
        "INSERT INTO conversations (id, title, create_time, update_time, source_path, model, message_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(id) DO UPDATE SET
            title = excluded.title,
            create_time = excluded.create_time,
            update_time = excluded.update_time,
            source_path = excluded.source_path,
            model = excluded.model,
            message_count = excluded.message_count",
        params![
            conversation.id,
            conversation.title,
            conversation.create_time,
            conversation.update_time,
            source,
            conversation.model,
            conversation.messages.len() as i64,
        ],
    )
    .map_err(|e| format!("写入会话失败: {e}"))?;

    tx.execute(
        "DELETE FROM messages WHERE conversation_id = ?1",
        params![conversation.id],
    )
    .map_err(|e| format!("清理旧消息失败: {e}"))?;

    tx.execute(
        "DELETE FROM messages_fts WHERE conversation_id = ?1",
        params![conversation.id],
    )
    .map_err(|e| format!("清理旧索引失败: {e}"))?;

    Ok(exists == 0)
}

fn insert_messages(
    tx: &Transaction<'_>,
    conversation: &ImportedConversation,
) -> Result<usize, String> {
    for (index, message) in conversation.messages.iter().enumerate() {
        let stored_id = format!("{}::{}", conversation.id, message.id);
        let attachments = imported_attachments_to_views(&message.attachments);
        let attachments_json = serde_json::to_string(&attachments)
            .map_err(|e| format!("序列化附件失败: {e}"))?;

        tx.execute(
            "INSERT INTO messages (id, conversation_id, role, content, create_time, sort_order, raw_json, attachments)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                stored_id,
                conversation.id,
                message.role,
                message.content,
                message.create_time,
                index as i64,
                message.raw_json,
                attachments_json,
            ],
        )
        .map_err(|e| format!("写入消息失败: {e}"))?;

        tx.execute(
            "INSERT INTO messages_fts (message_id, conversation_id, conversation_title, content)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                stored_id,
                conversation.id,
                conversation.title,
                message.content,
            ],
        )
        .map_err(|e| format!("写入全文索引失败: {e}"))?;
    }

    Ok(conversation.messages.len())
}