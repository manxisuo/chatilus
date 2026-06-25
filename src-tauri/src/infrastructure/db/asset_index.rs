use std::path::Path;

use rusqlite::{params, Connection};

use crate::infrastructure::importers::chatgpt::{
    attachments::{imported_attachments_to_views, resolve_imported_attachments},
    extract_attachment_infos_from_message_json,
};
use crate::infrastructure::media::MediaIndex;
use crate::models::AttachmentView;

use super::helpers::parse_attachments_json;

pub(crate) fn create_assets_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS assets (
            message_id TEXT NOT NULL,
            file_key TEXT NOT NULL,
            conversation_id TEXT NOT NULL,
            conversation_source TEXT NOT NULL,
            role TEXT NOT NULL,
            conversation_title TEXT NOT NULL,
            local_path TEXT NOT NULL,
            image_source TEXT NOT NULL,
            prompt TEXT,
            created_at REAL,
            PRIMARY KEY (message_id, file_key),
            FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE,
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_assets_created_at
            ON assets(created_at DESC);

        CREATE INDEX IF NOT EXISTS idx_assets_source_created
            ON assets(conversation_source, created_at DESC);

        CREATE INDEX IF NOT EXISTS idx_assets_conversation
            ON assets(conversation_id, created_at DESC);
        ",
    )
    .map_err(|e| format!("创建 assets 表失败: {e}"))
}

pub(crate) fn reindex_conversation_assets(
    conn: &Connection,
    conversation_id: &str,
    conversation_title: &str,
    conversation_source: &str,
    source_path: &str,
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM assets WHERE conversation_id = ?1",
        params![conversation_id],
    )
    .map_err(|e| format!("清理图片索引失败: {e}"))?;

    let mut stmt = conn
        .prepare(
            "SELECT m.id, m.role, m.create_time, m.attachments, m.raw_json
             FROM messages m
             WHERE m.conversation_id = ?1",
        )
        .map_err(|e| format!("读取消息附件失败: {e}"))?;

    let rows = stmt
        .query_map(params![conversation_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<f64>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })
        .map_err(|e| format!("读取消息附件失败: {e}"))?;

    let mut media_index: Option<MediaIndex> = None;

    for row in rows {
        let (message_id, role, create_time, attachments, raw_json) =
            row.map_err(|e| format!("读取消息附件失败: {e}"))?;

        let attachment_views = resolve_message_attachments(
            &role,
            attachments.as_deref(),
            raw_json.as_deref(),
            source_path,
            conversation_source,
            &mut media_index,
        );

        for attachment in attachment_views {
            if attachment.path.trim().is_empty() {
                continue;
            }
            insert_asset_row(
                conn,
                &message_id,
                conversation_id,
                conversation_title,
                conversation_source,
                &role,
                create_time,
                &attachment,
            )?;
        }
    }

    Ok(())
}

pub(crate) fn backfill_all_assets(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.title, COALESCE(c.source, 'chatgpt'), c.source_path
             FROM conversations c",
        )
        .map_err(|e| format!("读取会话失败: {e}"))?;

    let conversations = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| format!("读取会话失败: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取会话失败: {e}"))?;

    for (conversation_id, title, source, source_path) in conversations {
        reindex_conversation_assets(&conn, &conversation_id, &title, &source, &source_path)?;
    }

    Ok(())
}

fn resolve_message_attachments(
    role: &str,
    attachments: Option<&str>,
    raw_json: Option<&str>,
    source_path: &str,
    conversation_source: &str,
    media_index: &mut Option<MediaIndex>,
) -> Vec<AttachmentView> {
    if let Some(attachments) = attachments.filter(|value| !value.is_empty() && *value != "[]") {
        return parse_attachments_json(attachments)
            .into_iter()
            .filter(|item| !item.path.trim().is_empty())
            .collect();
    }

    let Some(raw_json) = raw_json.filter(|value| value.contains("image_asset_pointer")) else {
        return Vec::new();
    };

    let index = media_index.get_or_insert_with(|| build_media_index(conversation_source, source_path));
    let pointers = extract_attachment_infos_from_message_json(raw_json);
    imported_attachments_to_views(&resolve_imported_attachments(&pointers, role, index))
        .into_iter()
        .filter(|item| !item.path.trim().is_empty())
        .collect()
}

fn build_media_index(conversation_source: &str, source_path: &str) -> MediaIndex {
    let path = Path::new(source_path);
    match conversation_source {
        "codex" if path.is_dir() => MediaIndex::build_codex(path),
        "cursor" if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "state.vscdb") =>
        {
            path.parent()
                .and_then(|global_storage| global_storage.parent())
                .map(|user_dir| MediaIndex::build_cursor(user_dir))
                .unwrap_or_else(|| MediaIndex::build(path))
        }
        _ => MediaIndex::build(path),
    }
}

fn insert_asset_row(
    conn: &Connection,
    message_id: &str,
    conversation_id: &str,
    conversation_title: &str,
    conversation_source: &str,
    role: &str,
    create_time: Option<f64>,
    attachment: &AttachmentView,
) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO assets (
            message_id, file_key, conversation_id, conversation_source, role,
            conversation_title, local_path, image_source, prompt, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            message_id,
            attachment.file_key,
            conversation_id,
            conversation_source,
            role,
            conversation_title,
            attachment.path,
            attachment.source,
            attachment.prompt,
            create_time,
        ],
    )
    .map_err(|e| format!("写入图片索引失败: {e}"))?;
    Ok(())
}
