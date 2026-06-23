use std::collections::HashMap;
use std::path::Path;

use rusqlite::ToSql;

use crate::domain::mappers::image_fields_to_asset;
use crate::domain::models::Asset;
use crate::domain::ports::{AssetListQuery, AssetRepository};
use crate::infrastructure::importers::chatgpt::{
    attachments::{imported_attachments_to_views, resolve_imported_attachments},
    extract_attachment_infos_from_message_json,
};
use crate::infrastructure::media::MediaIndex;
use crate::models::SourceCount;

use super::Database;

const IMAGE_MONTH_SQL: &str =
    "strftime('%Y-%m', datetime(COALESCE(m.create_time, 0), 'unixepoch', 'localtime'))";

struct ImageFilters<'a> {
    include_uploads: bool,
    conversation_source: Option<&'a str>,
    month: Option<&'a str>,
    conversation_id: Option<&'a str>,
}

impl AssetRepository for Database {
    fn list(&self, query: AssetListQuery) -> Result<Vec<Asset>, String> {
        let filters = ImageFilters {
            include_uploads: query.include_uploads,
            conversation_source: normalized_source(query.conversation_source.as_deref()),
            month: normalized_source(query.month.as_deref()),
            conversation_id: normalized_source(query.conversation_id.as_deref()),
        };

        let mut from_attachments =
            list_assets_from_attachments(&self.conn, query.limit, query.offset, &filters)?;

        if from_attachments.len() as i64 >= query.limit {
            return Ok(from_attachments);
        }

        let remaining = query.limit - from_attachments.len() as i64;
        let attachment_total = count_images_from_attachments(&self.conn, &filters)?;
        let hydrate_offset = query.offset.saturating_sub(attachment_total).max(0);
        let hydrated = list_assets_from_raw_json(
            &self.conn,
            remaining,
            hydrate_offset,
            &filters,
        )?;

        from_attachments.extend(hydrated);
        Ok(from_attachments)
    }

    fn count(
        &self,
        include_uploads: bool,
        conversation_source: Option<&str>,
    ) -> Result<i64, String> {
        let filters = ImageFilters {
            include_uploads,
            conversation_source: normalized_source(conversation_source),
            month: None,
            conversation_id: None,
        };
        let from_attachments = count_images_from_attachments(&self.conn, &filters)?;
        let from_raw_json = count_images_from_raw_json(&self.conn, &filters)?;
        Ok(from_attachments + from_raw_json)
    }

    fn count_by_source(&self) -> Result<(i64, i64, i64), String> {
        count_images_by_source(&self.conn)
    }

    fn image_counts_by_conversation_source(&self) -> Result<Vec<SourceCount>, String> {
        image_counts_by_conversation_source(&self.conn)
    }
}

fn normalized_source(source: Option<&str>) -> Option<&str> {
    source.map(str::trim).filter(|value| !value.is_empty())
}

fn append_attachment_filters(sql: &mut String, bind: &mut Vec<Box<dyn ToSql>>, filters: &ImageFilters) {
    if !filters.include_uploads {
        sql.push_str(" AND effective_image_source(m.role, je.value) != 'upload'");
    }
    if let Some(source) = filters.conversation_source {
        sql.push_str(" AND COALESCE(c.source, 'chatgpt') = ?");
        bind.push(Box::new(source.to_string()));
    }
    if let Some(month) = filters.month {
        sql.push_str(" AND ");
        sql.push_str(IMAGE_MONTH_SQL);
        sql.push_str(" = ?");
        bind.push(Box::new(month.to_string()));
    }
    if let Some(conversation_id) = filters.conversation_id {
        sql.push_str(" AND m.conversation_id = ?");
        bind.push(Box::new(conversation_id.to_string()));
    }
}

fn list_assets_from_attachments(
    conn: &rusqlite::Connection,
    limit: i64,
    offset: i64,
    filters: &ImageFilters,
) -> Result<Vec<Asset>, String> {
    let mut sql = String::from(
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
           AND json_extract(je.value, '$.path') IS NOT NULL",
    );
    let mut bind: Vec<Box<dyn ToSql>> = Vec::new();
    append_attachment_filters(&mut sql, &mut bind, filters);
    sql.push_str(" ORDER BY COALESCE(m.create_time, 0) DESC LIMIT ? OFFSET ?");
    bind.push(Box::new(limit));
    bind.push(Box::new(offset));

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("查询图片失败: {e}"))?;

    let param_refs: Vec<&dyn ToSql> = bind.iter().map(|value| value.as_ref()).collect();
    let rows = stmt
        .query_map(param_refs.as_slice(), |row| {
            Ok(image_fields_to_asset(
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
            ))
        })
        .map_err(|e| format!("查询图片失败: {e}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取图片失败: {e}"))
}

fn list_assets_from_raw_json(
    conn: &rusqlite::Connection,
    limit: i64,
    offset: i64,
    filters: &ImageFilters,
) -> Result<Vec<Asset>, String> {
    if limit <= 0 {
        return Ok(Vec::new());
    }

    let mut sql = String::from(
        "SELECT m.id, m.conversation_id, m.role, m.create_time, m.raw_json,
                c.title, c.source_path, COALESCE(c.source, 'chatgpt') AS conversation_source
         FROM messages m
         JOIN conversations c ON c.id = m.conversation_id
         WHERE m.raw_json LIKE '%image_asset_pointer%'
           AND (m.attachments IS NULL OR m.attachments = '[]' OR m.attachments = '')",
    );
    let mut bind: Vec<Box<dyn ToSql>> = Vec::new();
    if let Some(source) = filters.conversation_source {
        sql.push_str(" AND COALESCE(c.source, 'chatgpt') = ?");
        bind.push(Box::new(source.to_string()));
    }
    if let Some(month) = filters.month {
        sql.push_str(" AND ");
        sql.push_str(IMAGE_MONTH_SQL);
        sql.push_str(" = ?");
        bind.push(Box::new(month.to_string()));
    }
    if let Some(conversation_id) = filters.conversation_id {
        sql.push_str(" AND m.conversation_id = ?");
        bind.push(Box::new(conversation_id.to_string()));
    }
    sql.push_str(" ORDER BY COALESCE(m.create_time, 0) DESC");

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("查询图片失败: {e}"))?;

    let param_refs: Vec<&dyn ToSql> = bind.iter().map(|value| value.as_ref()).collect();
    let rows = stmt
        .query_map(param_refs.as_slice(), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<f64>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
            ))
        })
        .map_err(|e| format!("查询图片失败: {e}"))?;

    let mut media_cache: HashMap<String, MediaIndex> = HashMap::new();
    let mut flattened = Vec::new();

    for row in rows {
        let (
            message_id,
            conversation_id,
            role,
            create_time,
            raw_json,
            title,
            source_path,
            conversation_source,
        ) = row.map_err(|e| format!("读取图片失败: {e}"))?;
        if let Some(source) = filters.conversation_source {
            if conversation_source != source {
                continue;
            }
        }
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
            if !filters.include_uploads && attachment.source == "upload" {
                continue;
            }
            flattened.push(image_fields_to_asset(
                message_id.clone(),
                conversation_id.clone(),
                role.clone(),
                create_time,
                title.clone(),
                attachment.path,
                attachment.file_key,
                attachment.source,
                attachment.prompt,
            ));
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
    filters: &ImageFilters,
) -> Result<i64, String> {
    let mut sql = String::from(
        "SELECT COUNT(*)
         FROM messages m
         JOIN conversations c ON c.id = m.conversation_id
         JOIN json_each(m.attachments) AS je
         WHERE m.attachments IS NOT NULL
           AND m.attachments != '[]'
           AND json_extract(je.value, '$.path') IS NOT NULL",
    );
    let mut bind: Vec<Box<dyn ToSql>> = Vec::new();
    append_attachment_filters(&mut sql, &mut bind, filters);

    let param_refs: Vec<&dyn ToSql> = bind.iter().map(|value| value.as_ref()).collect();
    conn.query_row(&sql, param_refs.as_slice(), |row| row.get(0))
        .map_err(|e| format!("统计图片失败: {e}"))
}

fn count_images_from_raw_json(
    conn: &rusqlite::Connection,
    filters: &ImageFilters,
) -> Result<i64, String> {
    let mut sql = String::from(
        "SELECT m.id, m.role, m.raw_json, c.source_path, COALESCE(c.source, 'chatgpt') AS conversation_source
         FROM messages m
         JOIN conversations c ON c.id = m.conversation_id
         WHERE m.raw_json LIKE '%image_asset_pointer%'
           AND (m.attachments IS NULL OR m.attachments = '[]' OR m.attachments = '')",
    );
    let mut bind: Vec<Box<dyn ToSql>> = Vec::new();
    if let Some(source) = filters.conversation_source {
        sql.push_str(" AND COALESCE(c.source, 'chatgpt') = ?");
        bind.push(Box::new(source.to_string()));
    }

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("统计图片失败: {e}"))?;

    let param_refs: Vec<&dyn ToSql> = bind.iter().map(|value| value.as_ref()).collect();
    let rows = stmt
        .query_map(param_refs.as_slice(), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|e| format!("统计图片失败: {e}"))?;

    let mut media_cache: HashMap<String, MediaIndex> = HashMap::new();
    let mut count = 0_i64;

    for row in rows {
        let (message_id, role, raw_json, source_path, conversation_source) =
            row.map_err(|e| format!("统计图片失败: {e}"))?;
        let _ = message_id;
        if let Some(source) = filters.conversation_source {
            if conversation_source != source {
                continue;
            }
        }
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
            if !filters.include_uploads && attachment.source == "upload" {
                continue;
            }
            count += 1;
        }
    }

    Ok(count)
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

fn image_counts_by_conversation_source(conn: &rusqlite::Connection) -> Result<Vec<SourceCount>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT COALESCE(c.source, 'chatgpt') AS source, COUNT(*) AS count
             FROM messages m
             JOIN conversations c ON c.id = m.conversation_id
             JOIN json_each(m.attachments) AS je
             WHERE m.attachments IS NOT NULL
               AND m.attachments != '[]'
               AND json_extract(je.value, '$.path') IS NOT NULL
             GROUP BY COALESCE(c.source, 'chatgpt')
             ORDER BY count DESC, source ASC",
        )
        .map_err(|e| format!("统计来源图片数失败: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(SourceCount {
                source: row.get(0)?,
                count: row.get(1)?,
            })
        })
        .map_err(|e| format!("读取来源图片统计失败: {e}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取来源图片统计失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use rusqlite::params;
    use std::path::Path;

    fn insert_conversation_with_image(
        db: &Database,
        conversation_id: &str,
        source: &str,
        path: &str,
    ) {
        db.conn
            .execute(
                "INSERT INTO conversations (
                    id, title, create_time, update_time, source_path, model, message_count, source, source_id
                 ) VALUES (?1, ?2, 1, 2, '/export', NULL, 1, ?3, ?4)",
                params![conversation_id, format!("{source} image"), source, conversation_id],
            )
            .expect("insert conversation");

        let attachments = serde_json::json!([{
            "path": path,
            "file_key": "test.png",
            "source": "generated"
        }])
        .to_string();

        db.conn
            .execute(
                "INSERT INTO messages (
                    id, conversation_id, role, content, create_time, sort_order, raw_json, attachments
                 ) VALUES (?1, ?2, 'assistant', 'image', 1, 0, '{}', ?3)",
                params![format!("{conversation_id}::msg-1"), conversation_id, attachments],
            )
            .expect("insert message");
    }

    #[test]
    fn list_filters_by_conversation_source() {
        let path = std::env::temp_dir().join(format!(
            "chatlens-asset-source-test-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let _ = std::fs::remove_file(&path);

        let db = Database::open(Path::new(&path)).expect("open db");
        insert_conversation_with_image(&db, "chatgpt::cg-1", "chatgpt", "/tmp/chatgpt.png");
        insert_conversation_with_image(&db, "cursor::cu-1", "cursor", "/tmp/cursor.png");

        let all = AssetRepository::list(
            &db,
            AssetListQuery {
                limit: 10,
                offset: 0,
                include_uploads: true,
                conversation_source: None,
                month: None,
                conversation_id: None,
            },
        )
        .expect("list all");
        assert_eq!(all.len(), 2);

        let chatgpt_only = AssetRepository::list(
            &db,
            AssetListQuery {
                limit: 10,
                offset: 0,
                include_uploads: true,
                conversation_source: Some("chatgpt".to_string()),
                month: None,
                conversation_id: None,
            },
        )
        .expect("list chatgpt");
        assert_eq!(chatgpt_only.len(), 1);
        assert_eq!(
            chatgpt_only[0].conversation_id.as_deref(),
            Some("chatgpt::cg-1")
        );

        let cursor_count = AssetRepository::count(&db, true, Some("cursor")).expect("count");
        assert_eq!(cursor_count, 1);
    }
}
