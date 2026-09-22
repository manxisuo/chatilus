use std::collections::HashMap;

use rusqlite::params;

use super::shared::*;
use crate::domain::ports::TimelineListQuery;
use crate::error::{AppError, AppResult};
use crate::infrastructure::db::helpers::map_conversation_summary;
use crate::infrastructure::db::source_context_index::attach_source_contexts;
use crate::infrastructure::db::Database;
use crate::models::{ConversationSummary, SourceCount, TimelineMonthBucket};


fn map_timeline_conversation(row: &rusqlite::Row<'_>) -> rusqlite::Result<ConversationSummary> {
    let mut summary = map_conversation_summary(row)?;
    summary.activity_month = row.get(10)?;
    summary.latest_message_id = row.get(11)?;
    summary.has_images = row.get::<_, i64>(12)? != 0;
    Ok(summary)
}

fn timeline_month_source_counts(
    conn: &rusqlite::Connection,
    source: Option<&str>,
) -> AppResult<HashMap<String, Vec<SourceCount>>> {
    let mut sql = String::from("SELECT ");
    sql.push_str(ACTIVITY_MONTH_SQL);
    sql.push_str(
        " AS month_key,
                COALESCE(c.source, 'chatgpt') AS source,
                COUNT(*) AS count
         FROM conversations c
         WHERE COALESCE(c.update_time, c.create_time, 0) > 0",
    );
    let mut bind: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(source) = source.map(str::trim).filter(|value| !value.is_empty()) {
        sql.push_str(" AND ");
        sql.push_str(SOURCE_EQUALS_SQL);
        bind.push(Box::new(source.to_string()));
    }

    sql.push_str(
        " GROUP BY month_key, source
          HAVING month_key IS NOT NULL
          ORDER BY month_key DESC, count DESC, source ASC",
    );

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| AppError::Msg(format!("查询时间线月份来源失败: {e}")))?;
    let param_refs: Vec<&dyn rusqlite::ToSql> = bind.iter().map(|value| value.as_ref()).collect();
    let rows = stmt
        .query_map(param_refs.as_slice(), |row| {
            Ok((
                row.get::<_, String>(0)?,
                SourceCount {
                    source: row.get(1)?,
                    count: row.get(2)?,
                },
            ))
        })
        .map_err(|e| AppError::Msg(format!("查询时间线月份来源失败: {e}")))?;

    let mut counts: HashMap<String, Vec<SourceCount>> = HashMap::new();
    for row in rows {
        let (month, entry) = row.map_err(|e| AppError::Msg(format!("读取时间线月份来源失败: {e}")))?;
        counts.entry(month).or_default().push(entry);
    }
    Ok(counts)
}

    pub(crate) fn list_timeline(db: &Database, query: TimelineListQuery) -> AppResult<Vec<ConversationSummary>> {
        let mut sql = String::from(
            "SELECT c.id, c.title, c.create_time, c.update_time, c.model, c.message_count,
                    c.is_starred, c.source_path, COALESCE(c.source, 'chatgpt') AS source,
                    COALESCE(GROUP_CONCAT(t.name, char(31)), '') AS tag_names,
                    ",
        );
        sql.push_str(ACTIVITY_MONTH_SQL);
        sql.push_str(
            " AS activity_month,
                    (SELECT m.id FROM messages m
                     WHERE m.conversation_id = c.id
                     ORDER BY m.sort_order DESC
                     LIMIT 1) AS latest_message_id,
                    CASE WHEN ");
        sql.push_str(HAS_IMAGES_EXISTS_SQL);
        sql.push_str(
            " THEN 1 ELSE 0 END AS has_images
             FROM conversations c
             LEFT JOIN conversation_tags ct ON ct.conversation_id = c.id
             LEFT JOIN tags t ON t.id = ct.tag_id
             WHERE COALESCE(c.update_time, c.create_time, 0) > 0",
        );
        let mut bind: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

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
        if let Some(month) = query
            .month
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            sql.push_str(" AND ");
            sql.push_str(ACTIVITY_MONTH_SQL);
            sql.push_str(" = ?");
            bind.push(Box::new(month.to_string()));
        }

        sql.push_str(
            " GROUP BY c.id
              ORDER BY COALESCE(c.update_time, c.create_time, 0) DESC
              LIMIT ? OFFSET ?",
        );
        bind.push(Box::new(query.limit));
        bind.push(Box::new(query.offset));

        let mut stmt = db
            .conn
            .prepare(&sql)
            .map_err(|e| AppError::Msg(format!("查询时间线失败: {e}")))?;

        let param_refs: Vec<&dyn rusqlite::ToSql> = bind.iter().map(|value| value.as_ref()).collect();
        let rows = stmt
            .query_map(param_refs.as_slice(), map_timeline_conversation)
            .map_err(|e| AppError::Msg(format!("查询时间线失败: {e}")))?;

        let mut summaries = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Msg(format!("读取时间线失败: {e}")))?;
        attach_source_contexts(&db.conn, &mut summaries)?;
        Ok(summaries)
    }

    pub(crate) fn list_timeline_months(db: &Database, source: Option<&str>) -> AppResult<Vec<TimelineMonthBucket>> {
        let mut sql = String::from("SELECT ");
        sql.push_str(ACTIVITY_MONTH_SQL);
        sql.push_str(
            " AS month_key,
                    COUNT(*) AS conversation_count
             FROM conversations c
             WHERE COALESCE(c.update_time, c.create_time, 0) > 0",
        );
        let mut bind: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(source) = source.map(str::trim).filter(|value| !value.is_empty()) {
            sql.push_str(" AND ");
            sql.push_str(SOURCE_EQUALS_SQL);
            bind.push(Box::new(source.to_string()));
        }

        sql.push_str(
            " GROUP BY month_key
              HAVING month_key IS NOT NULL
              ORDER BY month_key DESC",
        );

        let mut stmt = db
            .conn
            .prepare(&sql)
            .map_err(|e| AppError::Msg(format!("查询时间线月份失败: {e}")))?;

        let param_refs: Vec<&dyn rusqlite::ToSql> = bind.iter().map(|value| value.as_ref()).collect();
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok(TimelineMonthBucket {
                    month: row.get(0)?,
                    conversation_count: row.get(1)?,
                    image_count: 0,
                    source_counts: Vec::new(),
                })
            })
            .map_err(|e| AppError::Msg(format!("查询时间线月份失败: {e}")))?;

        let mut months = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Msg(format!("读取时间线月份失败: {e}")))?;
        let source_counts = timeline_month_source_counts(&db.conn, source)?;
        for bucket in &mut months {
            bucket.source_counts = source_counts
                .get(&bucket.month)
                .cloned()
                .unwrap_or_default();
        }
        Ok(months)
    }

