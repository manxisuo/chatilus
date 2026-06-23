use std::collections::HashMap;

use rusqlite::ToSql;

use crate::domain::mappers::image_fields_to_asset;
use crate::domain::models::Asset;
use crate::domain::ports::{AssetListQuery, AssetRepository};
use crate::models::SourceCount;

use super::Database;

const ASSET_MONTH_SQL: &str =
    "strftime('%Y-%m', datetime(COALESCE(a.created_at, 0), 'unixepoch', 'localtime'))";

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
        list_assets(&self.conn, query.limit, query.offset, &filters)
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
        count_assets(&self.conn, &filters)
    }

    fn count_by_source(&self) -> Result<(i64, i64, i64), String> {
        count_images_by_source(&self.conn)
    }

    fn image_counts_by_conversation_source(&self) -> Result<Vec<SourceCount>, String> {
        image_counts_by_conversation_source(&self.conn)
    }

    fn count_filtered(
        &self,
        include_uploads: bool,
        conversation_source: Option<&str>,
        month: Option<&str>,
        conversation_id: Option<&str>,
    ) -> Result<i64, String> {
        let filters = ImageFilters {
            include_uploads,
            conversation_source: normalized_source(conversation_source),
            month: normalized_source(month),
            conversation_id: normalized_source(conversation_id),
        };
        count_assets(&self.conn, &filters)
    }

    fn image_counts_by_message_month(
        &self,
        conversation_source: Option<&str>,
    ) -> Result<HashMap<String, i64>, String> {
        image_counts_by_message_month(&self.conn, conversation_source)
    }
}

fn normalized_source(source: Option<&str>) -> Option<&str> {
    source.map(str::trim).filter(|value| !value.is_empty())
}

fn append_asset_filters(sql: &mut String, bind: &mut Vec<Box<dyn ToSql>>, filters: &ImageFilters) {
    if !filters.include_uploads {
        sql.push_str(" AND a.image_source != 'upload'");
    }
    if let Some(source) = filters.conversation_source {
        sql.push_str(" AND a.conversation_source = ?");
        bind.push(Box::new(source.to_string()));
    }
    if let Some(month) = filters.month {
        sql.push_str(" AND COALESCE(a.created_at, 0) > 0 AND ");
        sql.push_str(ASSET_MONTH_SQL);
        sql.push_str(" = ?");
        bind.push(Box::new(month.to_string()));
    }
    if let Some(conversation_id) = filters.conversation_id {
        sql.push_str(" AND a.conversation_id = ?");
        bind.push(Box::new(conversation_id.to_string()));
    }
}

fn list_assets(
    conn: &rusqlite::Connection,
    limit: i64,
    offset: i64,
    filters: &ImageFilters,
) -> Result<Vec<Asset>, String> {
    let mut sql = String::from(
        "SELECT
            a.message_id,
            a.conversation_id,
            a.role,
            a.created_at,
            a.conversation_title,
            a.local_path,
            a.file_key,
            a.image_source,
            a.prompt
         FROM assets a
         WHERE a.local_path != ''",
    );
    let mut bind: Vec<Box<dyn ToSql>> = Vec::new();
    append_asset_filters(&mut sql, &mut bind, filters);
    sql.push_str(" ORDER BY COALESCE(a.created_at, 0) DESC LIMIT ? OFFSET ?");
    bind.push(Box::new(limit));
    bind.push(Box::new(offset));

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("查询图片失败: {e}"))?;

    let param_refs: Vec<&dyn ToSql> = bind.iter().map(|value| value.as_ref()).collect();
    let rows = stmt
        .query_map(param_refs.as_slice(), map_asset_row)
        .map_err(|e| format!("查询图片失败: {e}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取图片失败: {e}"))
}

fn count_assets(conn: &rusqlite::Connection, filters: &ImageFilters) -> Result<i64, String> {
    let mut sql = String::from(
        "SELECT COUNT(*)
         FROM assets a
         WHERE a.local_path != ''",
    );
    let mut bind: Vec<Box<dyn ToSql>> = Vec::new();
    append_asset_filters(&mut sql, &mut bind, filters);

    let param_refs: Vec<&dyn ToSql> = bind.iter().map(|value| value.as_ref()).collect();
    conn.query_row(&sql, param_refs.as_slice(), |row| row.get(0))
        .map_err(|e| format!("统计图片失败: {e}"))
}

fn map_asset_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Asset> {
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
}

fn image_counts_by_message_month(
    conn: &rusqlite::Connection,
    conversation_source: Option<&str>,
) -> Result<HashMap<String, i64>, String> {
    let mut sql = String::from("SELECT ");
    sql.push_str(ASSET_MONTH_SQL);
    sql.push_str(
        " AS month_key, COUNT(*) AS image_count
         FROM assets a
         WHERE a.local_path != ''
           AND COALESCE(a.created_at, 0) > 0",
    );
    let mut bind: Vec<Box<dyn ToSql>> = Vec::new();
    if let Some(source) = normalized_source(conversation_source) {
        sql.push_str(" AND a.conversation_source = ?");
        bind.push(Box::new(source.to_string()));
    }
    sql.push_str(" GROUP BY month_key HAVING month_key IS NOT NULL");

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("统计月份图片失败: {e}"))?;
    let param_refs: Vec<&dyn ToSql> = bind.iter().map(|value| value.as_ref()).collect();
    let rows = stmt
        .query_map(param_refs.as_slice(), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|e| format!("统计月份图片失败: {e}"))?;

    let mut counts = HashMap::new();
    for row in rows {
        let (month, count) = row.map_err(|e| format!("统计月份图片失败: {e}"))?;
        counts.insert(month, count);
    }
    Ok(counts)
}

fn count_images_by_source(conn: &rusqlite::Connection) -> Result<(i64, i64, i64), String> {
    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM assets WHERE local_path != ''",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("统计图片失败: {e}"))?;

    let generated: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM assets WHERE local_path != '' AND image_source = 'generated'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("统计图片失败: {e}"))?;

    let upload: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM assets WHERE local_path != '' AND image_source = 'upload'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("统计图片失败: {e}"))?;

    Ok((total, generated, upload))
}

fn image_counts_by_conversation_source(conn: &rusqlite::Connection) -> Result<Vec<SourceCount>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT a.conversation_source AS source, COUNT(*) AS count
             FROM assets a
             WHERE a.local_path != ''
             GROUP BY a.conversation_source
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
    use crate::domain::ports::AssetRepository;
    use super::super::asset_index::reindex_conversation_assets;
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

        reindex_conversation_assets(
            &db.conn,
            conversation_id,
            &format!("{source} image"),
            source,
            "/export",
        )
        .expect("reindex assets");
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

        let mut db = Database::open(Path::new(&path)).expect("open db");
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
