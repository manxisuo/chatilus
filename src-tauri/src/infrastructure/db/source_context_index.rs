use std::collections::HashMap;

use rusqlite::{params, Connection};

use crate::domain::ports::ImportedSourceContext;
use crate::models::SourceContextView;

pub(crate) fn create_source_context_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS source_contexts (
            id TEXT PRIMARY KEY,
            source TEXT NOT NULL,
            context_type TEXT NOT NULL,
            external_id TEXT,
            name TEXT NOT NULL,
            path TEXT,
            raw_json TEXT,
            created_at REAL NOT NULL DEFAULT (unixepoch('subsec')),
            updated_at REAL NOT NULL DEFAULT (unixepoch('subsec'))
        );

        CREATE INDEX IF NOT EXISTS idx_source_contexts_source_type
            ON source_contexts(source, context_type);

        CREATE INDEX IF NOT EXISTS idx_source_contexts_name
            ON source_contexts(name);

        CREATE TABLE IF NOT EXISTS conversation_source_contexts (
            conversation_id TEXT NOT NULL,
            source_context_id TEXT NOT NULL,
            PRIMARY KEY (conversation_id, source_context_id),
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
            FOREIGN KEY (source_context_id) REFERENCES source_contexts(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_conversation_source_contexts_context
            ON conversation_source_contexts(source_context_id);

        CREATE TABLE IF NOT EXISTS workspaces (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            icon TEXT,
            color TEXT,
            created_at REAL NOT NULL DEFAULT (unixepoch('subsec')),
            updated_at REAL NOT NULL DEFAULT (unixepoch('subsec'))
        );

        CREATE TABLE IF NOT EXISTS workspace_items (
            workspace_id TEXT NOT NULL,
            item_type TEXT NOT NULL,
            item_id TEXT NOT NULL,
            added_by TEXT NOT NULL DEFAULT 'manual',
            created_at REAL NOT NULL DEFAULT (unixepoch('subsec')),
            PRIMARY KEY (workspace_id, item_type, item_id),
            FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS workspace_source_context_mappings (
            workspace_id TEXT NOT NULL,
            source_context_id TEXT NOT NULL,
            mapping_type TEXT NOT NULL DEFAULT 'manual',
            confidence REAL,
            created_at REAL NOT NULL DEFAULT (unixepoch('subsec')),
            PRIMARY KEY (workspace_id, source_context_id),
            FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
            FOREIGN KEY (source_context_id) REFERENCES source_contexts(id) ON DELETE CASCADE
        );
        ",
    )
    .map_err(|e| format!("创建 source_contexts 表失败: {e}"))
}

pub(crate) fn link_conversation_source_contexts(
    conn: &Connection,
    conversation_id: &str,
    source: &str,
    contexts: &[ImportedSourceContext],
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM conversation_source_contexts WHERE conversation_id = ?1",
        params![conversation_id],
    )
    .map_err(|e| format!("清理来源上下文关联失败: {e}"))?;

    for context in contexts {
        let context_id = source_context_storage_id(source, context);
        upsert_source_context(conn, source, &context_id, context)?;
        conn.execute(
            "INSERT OR IGNORE INTO conversation_source_contexts (conversation_id, source_context_id)
             VALUES (?1, ?2)",
            params![conversation_id, context_id],
        )
        .map_err(|e| format!("写入来源上下文关联失败: {e}"))?;
    }

    Ok(())
}

pub(crate) fn attach_source_contexts(
    conn: &Connection,
    summaries: &mut [crate::models::ConversationSummary],
) -> Result<(), String> {
    if summaries.is_empty() {
        return Ok(());
    }

    let ids: Vec<&str> = summaries.iter().map(|item| item.id.as_str()).collect();
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let sql = format!(
        "SELECT csc.conversation_id, sc.id, sc.source, sc.context_type, sc.external_id,
                sc.name, sc.path
         FROM conversation_source_contexts csc
         JOIN source_contexts sc ON sc.id = csc.source_context_id
         WHERE csc.conversation_id IN ({placeholders})
         ORDER BY sc.context_type, sc.name"
    );

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("查询来源上下文失败: {e}"))?;

    let params: Vec<&dyn rusqlite::ToSql> = ids
        .iter()
        .map(|id| id as &dyn rusqlite::ToSql)
        .collect();

    let rows = stmt
        .query_map(params.as_slice(), |row| {
            Ok((
                row.get::<_, String>(0)?,
                SourceContextView {
                    id: row.get(1)?,
                    source: row.get(2)?,
                    context_type: row.get(3)?,
                    external_id: row.get(4)?,
                    name: row.get(5)?,
                    path: row.get(6)?,
                },
            ))
        })
        .map_err(|e| format!("查询来源上下文失败: {e}"))?;

    let mut by_conversation: HashMap<String, Vec<SourceContextView>> = HashMap::new();
    for row in rows {
        let (conversation_id, context) =
            row.map_err(|e| format!("读取来源上下文失败: {e}"))?;
        by_conversation
            .entry(conversation_id)
            .or_default()
            .push(context);
    }

    for summary in summaries {
        summary.source_contexts = by_conversation.remove(&summary.id).unwrap_or_default();
    }

    Ok(())
}

fn upsert_source_context(
    conn: &Connection,
    source: &str,
    context_id: &str,
    context: &ImportedSourceContext,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO source_contexts (
            id, source, context_type, external_id, name, path, raw_json, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, unixepoch('subsec'), unixepoch('subsec'))
         ON CONFLICT(id) DO UPDATE SET
            external_id = COALESCE(excluded.external_id, source_contexts.external_id),
            name = excluded.name,
            path = COALESCE(excluded.path, source_contexts.path),
            raw_json = COALESCE(excluded.raw_json, source_contexts.raw_json),
            updated_at = unixepoch('subsec')",
        params![
            context_id,
            source,
            context.context_type,
            context.external_id,
            context.name,
            context.path,
            context.raw_json,
        ],
    )
    .map_err(|e| format!("写入来源上下文失败: {e}"))?;
    Ok(())
}

pub(crate) fn source_context_storage_id(
    source: &str,
    context: &ImportedSourceContext,
) -> String {
    let facet = context
        .external_id
        .as_deref()
        .filter(|value| !value.is_empty())
        .or_else(|| context.path.as_deref().filter(|value| !value.is_empty()))
        .unwrap_or(context.name.as_str());
    let safe_facet = facet.replace("::", "__");
    format!("{source}::{context_type}::{safe_facet}", context_type = context.context_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_id_uses_external_id_when_present() {
        let context = ImportedSourceContext {
            context_type: "project".to_string(),
            external_id: Some("proj-1".to_string()),
            name: "ChatLens".to_string(),
            path: None,
            raw_json: None,
        };
        assert_eq!(
            source_context_storage_id("chatgpt", &context),
            "chatgpt::project::proj-1"
        );
    }
}
