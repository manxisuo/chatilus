use std::fs;
use std::path::Path;

use rusqlite::{params, OptionalExtension, Transaction};

use crate::domain::composite_id::{composite_conversation_id, message_storage_id};
use crate::domain::models::DataSource;
use crate::domain::ports::{
    ConversationListQuery, ConversationRepository, ImportPersistCounts, ImportedConversation,
    MessageRepository,
};
use crate::domain::ports::SearchIndexEntry;
use crate::infrastructure::importers::chatgpt::attachments::imported_attachments_to_views;
use crate::infrastructure::search::{index_message, remove_conversation_index};
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
            sql.push_str(" AND COALESCE(c.source, 'chatgpt') = ?");
            bind.push(Box::new(source.to_string()));
        }
        if query.has_images || query.has_attachments {
            sql.push_str(
                " AND EXISTS (
                    SELECT 1 FROM messages m
                    WHERE m.conversation_id = c.id
                      AND m.attachments IS NOT NULL
                      AND m.attachments != ''
                      AND m.attachments != '[]'
                )",
            );
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

    fn get_summary(&self, conversation_id: &str) -> Result<Option<ConversationSummary>, String> {
        let mut stmt = self
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
            .map_err(|e| format!("查询会话失败: {e}"))?;

        stmt.query_row(params![conversation_id], map_conversation_summary)
            .optional()
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
        source_path: &str,
        data_source: DataSource,
        conversations_deduplicated: usize,
    ) -> Result<ImportPersistCounts, String> {
        let tx = self
            .conn
            .transaction()
            .map_err(|e| format!("开启事务失败: {e}"))?;

        let mut new_conversations = 0usize;
        let mut updated_conversations = 0usize;
        let mut message_count = 0usize;

        for conversation in conversations {
            let inserted =
                upsert_conversation(&tx, conversation, source_path, data_source)?;
            if inserted {
                new_conversations += 1;
            } else {
                updated_conversations += 1;
            }
            message_count += merge_messages(
                &tx,
                conversation,
                &composite_conversation_id(data_source, &conversation.id),
            )?;
        }

        tx.commit()
            .map_err(|e| format!("提交事务失败: {e}"))?;

        Ok(ImportPersistCounts {
            new_conversations,
            updated_conversations,
            messages: message_count,
            conversations_deduplicated,
        })
    }
}

fn upsert_conversation(
    tx: &Transaction<'_>,
    conversation: &ImportedConversation,
    source_path: &str,
    data_source: DataSource,
) -> Result<bool, String> {
    let source_id = conversation.id.clone();
    let storage_id = composite_conversation_id(data_source, &source_id);
    let existing = read_conversation_metadata(tx, &storage_id)?;
    let is_new = existing.is_none();
    let metadata = pick_conversation_metadata(conversation, existing.as_ref());

    tx.execute(
        "INSERT INTO conversations (
            id, title, create_time, update_time, source_path, model, message_count, source, source_id
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(id) DO UPDATE SET
            title = excluded.title,
            create_time = excluded.create_time,
            update_time = excluded.update_time,
            source_path = excluded.source_path,
            model = excluded.model,
            source = excluded.source,
            source_id = excluded.source_id",
        params![
            storage_id,
            metadata.title,
            metadata.create_time,
            metadata.update_time,
            source_path,
            metadata.model,
            conversation.messages.len() as i64,
            data_source.as_str(),
            source_id,
        ],
    )
    .map_err(|e| format!("写入会话失败: {e}"))?;

    Ok(is_new)
}

struct ConversationMetadata {
    title: String,
    create_time: Option<f64>,
    update_time: Option<f64>,
    model: Option<String>,
}

fn read_conversation_metadata(
    tx: &Transaction<'_>,
    conversation_id: &str,
) -> Result<Option<ConversationMetadata>, String> {
    tx.query_row(
        "SELECT title, create_time, update_time, model
         FROM conversations WHERE id = ?1",
        params![conversation_id],
        |row| {
            Ok(ConversationMetadata {
                title: row.get(0)?,
                create_time: row.get(1)?,
                update_time: row.get(2)?,
                model: row.get(3)?,
            })
        },
    )
    .optional()
    .map_err(|e| format!("读取会话元数据失败: {e}"))
}

fn pick_conversation_metadata(
    incoming: &ImportedConversation,
    existing: Option<&ConversationMetadata>,
) -> ConversationMetadata {
    let Some(existing) = existing else {
        return ConversationMetadata {
            title: incoming.title.clone(),
            create_time: incoming.create_time,
            update_time: incoming.update_time,
            model: incoming.model.clone(),
        };
    };

    let incoming_update = incoming.update_time.unwrap_or(0.0);
    let existing_update = existing.update_time.unwrap_or(0.0);
    if incoming_update >= existing_update {
        ConversationMetadata {
            title: incoming.title.clone(),
            create_time: min_option_time(incoming.create_time, existing.create_time),
            update_time: Some(incoming_update.max(existing_update)),
            model: incoming.model.clone().or_else(|| existing.model.clone()),
        }
    } else {
        ConversationMetadata {
            title: existing.title.clone(),
            create_time: min_option_time(incoming.create_time, existing.create_time),
            update_time: existing.update_time,
            model: existing.model.clone().or_else(|| incoming.model.clone()),
        }
    }
}

fn min_option_time(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (Some(l), Some(r)) => Some(l.min(r)),
        (Some(l), None) => Some(l),
        (None, Some(r)) => Some(r),
        (None, None) => None,
    }
}

fn merge_messages(
    tx: &Transaction<'_>,
    conversation: &ImportedConversation,
    storage_id: &str,
) -> Result<usize, String> {
    let mut touched = 0usize;

    for message in &conversation.messages {
        let stored_id = message_storage_id(storage_id, &message.id);
        let attachments = imported_attachments_to_views(&message.attachments);
        let attachments_json = serde_json::to_string(&attachments)
            .map_err(|e| format!("序列化附件失败: {e}"))?;

        tx.execute(
            "INSERT INTO messages (id, conversation_id, role, content, create_time, sort_order, raw_json, attachments)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
                role = excluded.role,
                content = excluded.content,
                create_time = excluded.create_time,
                raw_json = excluded.raw_json,
                attachments = excluded.attachments",
            params![
                stored_id,
                storage_id,
                message.role,
                message.content,
                message.create_time,
                message.raw_json,
                attachments_json,
            ],
        )
        .map_err(|e| format!("写入消息失败: {e}"))?;
        touched += 1;
    }

    reorder_conversation_messages(tx, storage_id)?;
    refresh_conversation_message_count(tx, storage_id)?;
    reindex_conversation_messages(tx, storage_id, &conversation.title)?;

    Ok(touched)
}

fn reorder_conversation_messages(
    tx: &Transaction<'_>,
    conversation_id: &str,
) -> Result<(), String> {
    let mut stmt = tx
        .prepare(
            "SELECT id, create_time, role
             FROM messages
             WHERE conversation_id = ?1
             ORDER BY COALESCE(create_time, 0),
                      CASE role
                        WHEN 'user' THEN 0
                        WHEN 'assistant' THEN 1
                        WHEN 'system' THEN 2
                        ELSE 3
                      END,
                      id",
        )
        .map_err(|e| format!("读取消息排序失败: {e}"))?;

    let rows = stmt
        .query_map(params![conversation_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<f64>>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| format!("读取消息排序失败: {e}"))?;

    for (index, row) in rows.enumerate() {
        let (message_id, _, _) = row.map_err(|e| format!("读取消息排序失败: {e}"))?;
        tx.execute(
            "UPDATE messages SET sort_order = ?1 WHERE id = ?2",
            params![index as i64, message_id],
        )
        .map_err(|e| format!("更新消息顺序失败: {e}"))?;
    }

    Ok(())
}

fn refresh_conversation_message_count(
    tx: &Transaction<'_>,
    conversation_id: &str,
) -> Result<(), String> {
    tx.execute(
        "UPDATE conversations
         SET message_count = (
            SELECT COUNT(*) FROM messages WHERE conversation_id = ?1
         )
         WHERE id = ?1",
        params![conversation_id],
    )
    .map_err(|e| format!("更新会话消息数失败: {e}"))?;
    Ok(())
}

fn reindex_conversation_messages(
    tx: &Transaction<'_>,
    conversation_id: &str,
    conversation_title: &str,
) -> Result<(), String> {
    remove_conversation_index(tx, conversation_id)?;

    let mut stmt = tx
        .prepare(
            "SELECT id, content
             FROM messages
             WHERE conversation_id = ?1
             ORDER BY sort_order",
        )
        .map_err(|e| format!("读取消息索引失败: {e}"))?;

    let rows = stmt
        .query_map(params![conversation_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("读取消息索引失败: {e}"))?;

    for row in rows {
        let (message_id, content) = row.map_err(|e| format!("读取消息索引失败: {e}"))?;
        index_message(
            tx,
            &SearchIndexEntry {
                message_id,
                conversation_id: conversation_id.to_string(),
                conversation_title: conversation_title.to_string(),
                content,
            },
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod merge_tests {
    use super::*;
    use crate::db::Database;
    use crate::domain::models::DataSource;
    use crate::domain::ports::{
        ConversationListQuery, ImportedAttachment, ImportedConversation, ImportedMessage,
    };

    fn sample_conversation(message_ids: &[&str]) -> ImportedConversation {
        ImportedConversation {
            id: "conv-1".to_string(),
            title: "Merge Test".to_string(),
            create_time: Some(1.0),
            update_time: Some(2.0),
            model: Some("gpt-4".to_string()),
            messages: message_ids
                .iter()
                .map(|message_id| ImportedMessage {
                    id: (*message_id).to_string(),
                    role: "user".to_string(),
                    content: format!("body-{message_id}"),
                    create_time: Some(3.0),
                    raw_json: "{}".to_string(),
                    attachments: Vec::<ImportedAttachment>::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn merge_import_keeps_existing_messages_from_other_packages() {
        let path = std::env::temp_dir().join(format!(
            "chatlens-merge-test-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let _ = std::fs::remove_file(&path);

        let mut db = Database::open(&path).expect("open db");
        ConversationRepository::save_many(
            &mut db,
            &[sample_conversation(&["m1", "m2"])],
            "/export-a",
            DataSource::ChatGpt,
            0,
        )
        .expect("first import");

        ConversationRepository::save_many(
            &mut db,
            &[sample_conversation(&["m2", "m3"])],
            "/export-b",
            DataSource::ChatGpt,
            0,
        )
        .expect("second import");

        let message_count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE conversation_id = 'chatgpt::conv-1'",
                [],
                |row| row.get(0),
            )
            .expect("count messages");
        assert_eq!(message_count, 3);

        let conversation_id: String = db
            .conn
            .query_row("SELECT id FROM conversations", [], |row| row.get(0))
            .expect("conversation id");
        assert_eq!(conversation_id, "chatgpt::conv-1");

        let source_id: String = db
            .conn
            .query_row(
                "SELECT source_id FROM conversations WHERE id = 'chatgpt::conv-1'",
                [],
                |row| row.get(0),
            )
            .expect("source id");
        assert_eq!(source_id, "conv-1");
    }

    #[test]
    fn distinct_sources_with_same_source_id_do_not_collide() {
        let path = std::env::temp_dir().join(format!(
            "chatlens-source-id-test-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let _ = std::fs::remove_file(&path);

        let mut db = Database::open(&path).expect("open db");
        let shared_source_id = "shared-composer-id";

        ConversationRepository::save_many(
            &mut db,
            &[ImportedConversation {
                id: shared_source_id.to_string(),
                title: "ChatGPT".to_string(),
                create_time: None,
                update_time: None,
                model: None,
                messages: vec![ImportedMessage {
                    id: "m1".to_string(),
                    role: "user".to_string(),
                    content: "from chatgpt".to_string(),
                    create_time: None,
                    raw_json: "{}".to_string(),
                    attachments: Vec::new(),
                }],
            }],
            "/chatgpt",
            DataSource::ChatGpt,
            0,
        )
        .expect("chatgpt import");

        ConversationRepository::save_many(
            &mut db,
            &[ImportedConversation {
                id: shared_source_id.to_string(),
                title: "Cursor".to_string(),
                create_time: None,
                update_time: None,
                model: None,
                messages: vec![ImportedMessage {
                    id: "m1".to_string(),
                    role: "user".to_string(),
                    content: "from cursor".to_string(),
                    create_time: None,
                    raw_json: "{}".to_string(),
                    attachments: Vec::new(),
                }],
            }],
            "/cursor",
            DataSource::Cursor,
            0,
        )
        .expect("cursor import");

        let conversation_count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM conversations", [], |row| row.get(0))
            .expect("conversation count");
        assert_eq!(conversation_count, 2);
    }

    #[test]
    fn list_filters_by_source() {
        let path = std::env::temp_dir().join(format!(
            "chatlens-source-filter-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let _ = std::fs::remove_file(&path);

        let mut db = Database::open(&path).expect("open db");
        ConversationRepository::save_many(
            &mut db,
            &[sample_conversation(&["m1"])],
            "/chatgpt",
            DataSource::ChatGpt,
            0,
        )
        .expect("chatgpt import");
        ConversationRepository::save_many(
            &mut db,
            &[ImportedConversation {
                id: "cursor-only".to_string(),
                title: "Cursor only".to_string(),
                create_time: None,
                update_time: None,
                model: None,
                messages: vec![ImportedMessage {
                    id: "m1".to_string(),
                    role: "user".to_string(),
                    content: "cursor".to_string(),
                    create_time: None,
                    raw_json: "{}".to_string(),
                    attachments: Vec::new(),
                }],
            }],
            "/cursor",
            DataSource::Cursor,
            0,
        )
        .expect("cursor import");

        let cursor_only = ConversationRepository::list(
            &db,
            ConversationListQuery {
                source: Some("cursor".to_string()),
                limit: 10,
                offset: 0,
                ..ConversationListQuery::default()
            },
        )
        .expect("list cursor");
        assert_eq!(cursor_only.len(), 1);
        assert_eq!(cursor_only[0].source, "cursor");
    }

    #[test]
    fn reorder_puts_user_before_assistant_when_timestamps_match() {
        let path = std::env::temp_dir().join(format!(
            "chatlens-message-order-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let _ = std::fs::remove_file(&path);

        let mut db = Database::open(&path).expect("open db");
        let conversation = ImportedConversation {
            id: "gemini-activity-1".to_string(),
            title: "draw pics".to_string(),
            create_time: Some(1_718_822_931.0),
            update_time: Some(1_718_822_931.0),
            model: Some("gemini".to_string()),
            messages: vec![
                ImportedMessage {
                    id: "abc::user".to_string(),
                    role: "user".to_string(),
                    content: "can you draw pics".to_string(),
                    create_time: Some(1_718_822_931.0),
                    raw_json: "{}".to_string(),
                    attachments: Vec::new(),
                },
                ImportedMessage {
                    id: "abc::assistant".to_string(),
                    role: "assistant".to_string(),
                    content: "sure".to_string(),
                    create_time: Some(1_718_822_931.0),
                    raw_json: "{}".to_string(),
                    attachments: Vec::new(),
                },
            ],
        };

        ConversationRepository::save_many(
            &mut db,
            &[conversation],
            "/gemini",
            DataSource::Gemini,
            0,
        )
        .expect("gemini import");

        let roles: Vec<String> = db
            .conn
            .prepare(
                "SELECT role FROM messages
                 WHERE conversation_id = 'gemini::gemini-activity-1'
                 ORDER BY sort_order",
            )
            .expect("prepare")
            .query_map([], |row| row.get(0))
            .expect("query")
            .collect::<Result<Vec<_>, _>>()
            .expect("roles");

        assert_eq!(roles, vec!["user".to_string(), "assistant".to_string()]);
    }
}