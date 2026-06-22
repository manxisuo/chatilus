use std::collections::HashMap;
use std::path::Path;

use rusqlite::params;

use crate::domain::ports::{AssetListQuery, AssetRepository};
use crate::infrastructure::importers::chatgpt::{
    attachments::{imported_attachments_to_views, resolve_imported_attachments},
    extract_attachment_infos_from_message_json,
};
use crate::infrastructure::media::MediaIndex;
use crate::models::ImageGalleryItem;

use super::Database;

impl AssetRepository for Database {
    fn list_images(&self, query: AssetListQuery) -> Result<Vec<ImageGalleryItem>, String> {
        let mut from_attachments =
            list_images_from_attachments(&self.conn, query.limit, query.offset, query.include_uploads)?;

        if from_attachments.len() as i64 >= query.limit {
            return Ok(from_attachments);
        }

        let remaining = query.limit - from_attachments.len() as i64;
        let hydrate_offset = (query
            .offset
            .saturating_sub(count_images_from_attachments(
                &self.conn,
                query.include_uploads,
            )?))
        .max(0);
        let hydrated = list_images_from_raw_json(
            &self.conn,
            remaining,
            hydrate_offset,
            query.include_uploads,
        )?;

        from_attachments.extend(hydrated);
        Ok(from_attachments)
    }

    fn count_by_source(&self) -> Result<(i64, i64, i64), String> {
        count_images_by_source(&self.conn)
    }
}

fn list_images_from_attachments(
    conn: &rusqlite::Connection,
    limit: i64,
    offset: i64,
    include_uploads: bool,
) -> Result<Vec<ImageGalleryItem>, String> {
    let upload_filter = if include_uploads {
        String::new()
    } else {
        " AND effective_image_source(m.role, je.value) != 'upload'".to_string()
    };

    let sql = format!(
        "SELECT
            m.id,
            m.conversation_id,
            m.role,
            m.create_time,
            c.title,
            json_extract(je.value, '$.path') AS path,
            json_extract(je.value, '$.file_key') AS file_key,
            effective_image_source(m.role, je.value) AS source,
            json_extract(je.value, '$.prompt') AS prompt
         FROM messages m
         JOIN conversations c ON c.id = m.conversation_id
         JOIN json_each(m.attachments) AS je
         WHERE m.attachments IS NOT NULL
           AND m.attachments != '[]'
           AND json_extract(je.value, '$.path') IS NOT NULL
           {upload_filter}
         ORDER BY COALESCE(m.create_time, 0) DESC
         LIMIT ?1 OFFSET ?2"
    );

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("查询图片失败: {e}"))?;

    let rows = stmt
        .query_map(params![limit, offset], |row| {
            Ok(ImageGalleryItem {
                message_id: row.get(0)?,
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                create_time: row.get(3)?,
                conversation_title: row.get(4)?,
                path: row.get(5)?,
                file_key: row.get(6)?,
                source: row.get(7)?,
                prompt: row.get(8)?,
            })
        })
        .map_err(|e| format!("查询图片失败: {e}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取图片失败: {e}"))
}

fn list_images_from_raw_json(
    conn: &rusqlite::Connection,
    limit: i64,
    offset: i64,
    include_uploads: bool,
) -> Result<Vec<ImageGalleryItem>, String> {
    if limit <= 0 {
        return Ok(Vec::new());
    }

    let mut stmt = conn
        .prepare(
            "SELECT m.id, m.conversation_id, m.role, m.create_time, m.raw_json,
                    c.title, c.source_path
             FROM messages m
             JOIN conversations c ON c.id = m.conversation_id
             WHERE m.raw_json LIKE '%image_asset_pointer%'
               AND (m.attachments IS NULL OR m.attachments = '[]' OR m.attachments = '')
             ORDER BY COALESCE(m.create_time, 0) DESC",
        )
        .map_err(|e| format!("查询图片失败: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<f64>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(|e| format!("查询图片失败: {e}"))?;

    let mut media_cache: HashMap<String, MediaIndex> = HashMap::new();
    let mut flattened = Vec::new();

    for row in rows {
        let (message_id, conversation_id, role, create_time, raw_json, title, source_path) =
            row.map_err(|e| format!("读取图片失败: {e}"))?;
        let Some(raw_json) = raw_json else {
            continue;
        };

        let index = media_cache
            .entry(source_path.clone())
            .or_insert_with(|| MediaIndex::build(Path::new(&source_path)));

        let pointers = extract_attachment_infos_from_message_json(&raw_json);
        let attachments = imported_attachments_to_views(&resolve_imported_attachments(
            &pointers, &role, index,
        ));
        for attachment in attachments {
            if !include_uploads && attachment.source == "upload" {
                continue;
            }
            flattened.push(ImageGalleryItem {
                path: attachment.path,
                file_key: attachment.file_key,
                conversation_id: conversation_id.clone(),
                conversation_title: title.clone(),
                message_id: message_id.clone(),
                role: role.clone(),
                create_time,
                source: attachment.source,
                prompt: attachment.prompt,
            });
        }
    }

    Ok(flattened
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .collect())
}

fn count_images_from_attachments(
    conn: &rusqlite::Connection,
    include_uploads: bool,
) -> Result<i64, String> {
    let upload_filter = if include_uploads {
        String::new()
    } else {
        " AND effective_image_source(m.role, je.value) != 'upload'".to_string()
    };

    let sql = format!(
        "SELECT COUNT(*)
         FROM messages m
         JOIN json_each(m.attachments) AS je
         WHERE m.attachments IS NOT NULL
           AND m.attachments != '[]'
           AND json_extract(je.value, '$.path') IS NOT NULL
           {upload_filter}"
    );

    conn.query_row(&sql, [], |row| row.get(0))
        .map_err(|e| format!("统计图片失败: {e}"))
}

fn count_images_by_source(conn: &rusqlite::Connection) -> Result<(i64, i64, i64), String> {
    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM messages m
             JOIN json_each(m.attachments) AS je
             WHERE m.attachments IS NOT NULL
               AND m.attachments != '[]'
               AND json_extract(je.value, '$.path') IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("统计图片失败: {e}"))?;

    let generated: i64 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM messages m
             JOIN json_each(m.attachments) AS je
             WHERE m.attachments IS NOT NULL
               AND m.attachments != '[]'
               AND json_extract(je.value, '$.path') IS NOT NULL
               AND effective_image_source(m.role, je.value) = 'generated'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("统计图片失败: {e}"))?;

    let upload: i64 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM messages m
             JOIN json_each(m.attachments) AS je
             WHERE m.attachments IS NOT NULL
               AND m.attachments != '[]'
               AND json_extract(je.value, '$.path') IS NOT NULL
               AND effective_image_source(m.role, je.value) = 'upload'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("统计图片失败: {e}"))?;

    Ok((total, generated, upload))
}
