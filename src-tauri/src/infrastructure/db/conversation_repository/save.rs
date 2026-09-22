use rusqlite::{params, OptionalExtension, Transaction};

use crate::domain::composite_id::{composite_conversation_id, message_storage_id};
use crate::domain::models::DataSource;
use crate::domain::ports::{
    ImportPersistCounts, ImportedConversation, SearchIndexEntry,
};
use crate::error::{AppError, AppResult};
use crate::infrastructure::attachments::{imported_attachments_to_views, merge_attachment_views};
use crate::infrastructure::db::asset_index::reindex_conversation_assets;
use crate::infrastructure::db::helpers::parse_attachments_json;
use crate::infrastructure::db::source_context_index::link_conversation_source_contexts;
use crate::infrastructure::db::Database;
use crate::infrastructure::search::{index_message, remove_conversation_index};

    pub(crate) fn save_many(
        db: &mut Database,
        conversations: &[ImportedConversation],
        source_path: &str,
        data_source: DataSource,
        conversations_deduplicated: usize,
    ) -> AppResult<ImportPersistCounts> {
        let tx = db
            .conn
            .transaction()
            .map_err(|e| AppError::Msg(format!("开启事务失败: {e}")))?;

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
                data_source.as_str(),
                source_path,
            )?;
        }

        tx.commit()
            .map_err(|e| AppError::Msg(format!("提交事务失败: {e}")))?;

        Ok(ImportPersistCounts {
            new_conversations,
            updated_conversations,
            messages: message_count,
            conversations_deduplicated,
        })
    }


fn upsert_conversation(
    tx: &Transaction<'_>,
    conversation: &ImportedConversation,
    source_path: &str,
    data_source: DataSource,
) -> AppResult<bool> {
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
    .map_err(|e| AppError::Msg(format!("写入会话失败: {e}")))?;

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
) -> AppResult<Option<ConversationMetadata>> {
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
    .map_err(|e| AppError::Msg(format!("读取会话元数据失败: {e}")))
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
    conversation_source: &str,
    _source_path: &str,
) -> AppResult<usize> {
    let mut touched = 0usize;

    for message in &conversation.messages {
        let stored_id = message_storage_id(storage_id, &message.id);
        let existing_attachments = read_message_attachments(tx, &stored_id)?;
        let incoming_attachments = imported_attachments_to_views(&message.attachments);
        let attachments = merge_attachment_views(&existing_attachments, &incoming_attachments);
        let attachments_json = serde_json::to_string(&attachments)
            .map_err(|e| AppError::Msg(format!("序列化附件失败: {e}")))?;

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
        .map_err(|e| AppError::Msg(format!("写入消息失败: {e}")))?;
        touched += 1;
    }

    reorder_conversation_messages(tx, storage_id)?;
    refresh_conversation_message_count(tx, storage_id)?;
    refresh_conversation_times_from_messages(tx, storage_id)?;
    reindex_conversation_messages(tx, storage_id, &conversation.title)?;
    reindex_conversation_assets(
        tx,
        storage_id,
        &conversation.title,
        conversation_source,
    )?;
    link_conversation_source_contexts(
        tx,
        storage_id,
        conversation_source,
        &conversation.source_contexts,
    )?;

    Ok(touched)
}

fn read_message_attachments(
    tx: &Transaction<'_>,
    message_id: &str,
) -> AppResult<Vec<crate::models::AttachmentView>> {
    let raw: Option<String> = tx
        .query_row(
            "SELECT attachments FROM messages WHERE id = ?1",
            params![message_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| AppError::Msg(format!("读取消息附件失败: {e}")))?;

    Ok(raw
        .filter(|value| !value.is_empty() && value != "[]")
        .map(|value| {
            parse_attachments_json(&value)
                .into_iter()
                .filter(|item| !item.path.trim().is_empty())
                .collect()
        })
        .unwrap_or_default())
}

fn reorder_conversation_messages(
    tx: &Transaction<'_>,
    conversation_id: &str,
) -> AppResult<()> {
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
        .map_err(|e| AppError::Msg(format!("读取消息排序失败: {e}")))?;

    let rows = stmt
        .query_map(params![conversation_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<f64>>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| AppError::Msg(format!("读取消息排序失败: {e}")))?;

    for (index, row) in rows.enumerate() {
        let (message_id, _, _) = row.map_err(|e| AppError::Msg(format!("读取消息排序失败: {e}")))?;
        tx.execute(
            "UPDATE messages SET sort_order = ?1 WHERE id = ?2",
            params![index as i64, message_id],
        )
        .map_err(|e| AppError::Msg(format!("更新消息顺序失败: {e}")))?;
    }

    Ok(())
}

fn refresh_conversation_message_count(
    tx: &Transaction<'_>,
    conversation_id: &str,
) -> AppResult<()> {
    tx.execute(
        "UPDATE conversations
         SET message_count = (
            SELECT COUNT(*) FROM messages WHERE conversation_id = ?1
         )
         WHERE id = ?1",
        params![conversation_id],
    )
    .map_err(|e| AppError::Msg(format!("更新会话消息数失败: {e}")))?;
    Ok(())
}

fn refresh_conversation_times_from_messages(
    tx: &Transaction<'_>,
    conversation_id: &str,
) -> AppResult<()> {
    let bounds = tx
        .query_row(
            "SELECT MIN(create_time), MAX(create_time)
             FROM messages
             WHERE conversation_id = ?1
               AND create_time IS NOT NULL
               AND create_time >= 788918400
               AND create_time <= 4102444800",
            params![conversation_id],
            |row| Ok((row.get::<_, Option<f64>>(0)?, row.get::<_, Option<f64>>(1)?)),
        )
        .optional()
        .map_err(|e| AppError::Msg(format!("读取消息时间范围失败: {e}")))?;

    let Some((Some(earliest), Some(latest))) = bounds else {
        return Ok(());
    };

    tx.execute(
        "UPDATE conversations
         SET create_time = CASE
                WHEN create_time IS NULL THEN ?1
                ELSE MIN(create_time, ?1)
             END,
             update_time = ?2
         WHERE id = ?3",
        params![earliest, latest, conversation_id],
    )
    .map_err(|e| AppError::Msg(format!("更新会话时间失败: {e}")))?;
    Ok(())
}

fn reindex_conversation_messages(
    tx: &Transaction<'_>,
    conversation_id: &str,
    conversation_title: &str,
) -> AppResult<()> {
    remove_conversation_index(tx, conversation_id)?;

    let mut stmt = tx
        .prepare(
            "SELECT id, content
             FROM messages
             WHERE conversation_id = ?1
             ORDER BY sort_order",
        )
        .map_err(|e| AppError::Msg(format!("读取消息索引失败: {e}")))?;

    let rows = stmt
        .query_map(params![conversation_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| AppError::Msg(format!("读取消息索引失败: {e}")))?;

    for row in rows {
        let (message_id, content) = row.map_err(|e| AppError::Msg(format!("读取消息索引失败: {e}")))?;
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

