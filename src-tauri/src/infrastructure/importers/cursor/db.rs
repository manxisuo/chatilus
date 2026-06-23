use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, Row, types::ValueRef};
use serde_json::Value;

use super::parse::{composer_meta_from_json, parse_composer_conversation, CursorComposerMeta};

pub fn resolve_cursor_db_path(input: &Path) -> Option<PathBuf> {
    let candidates = [
        input.to_path_buf(),
        input.join("state.vscdb"),
        input.join("globalStorage").join("state.vscdb"),
        input.join("User").join("globalStorage").join("state.vscdb"),
    ];

    for candidate in candidates {
        if candidate.is_file() && is_cursor_vscdb(&candidate) {
            return Some(candidate);
        }
    }

    None
}

/// Returns `Cursor/User` when `input` points at the db file or that directory tree.
pub fn resolve_cursor_user_dir(input: &Path) -> Option<PathBuf> {
    if let Some(db_path) = resolve_cursor_db_path(input) {
        return user_dir_from_db_path(&db_path);
    }

    let direct = input.join("globalStorage").join("state.vscdb");
    if direct.is_file() && is_cursor_vscdb(&direct) {
        return Some(input.to_path_buf());
    }

    None
}

fn user_dir_from_db_path(db_path: &Path) -> Option<PathBuf> {
    let global_storage = db_path.parent()?;
    if global_storage.file_name().and_then(|name| name.to_str()) != Some("globalStorage") {
        return None;
    }
    global_storage.parent().map(Path::to_path_buf)
}

pub fn is_cursor_vscdb(path: &Path) -> bool {
    open_cursor_db(path)
        .ok()
        .and_then(|conn| {
            conn.query_row(
                "SELECT 1 FROM cursorDiskKV WHERE key LIKE 'composerData:%' LIMIT 1",
                [],
                |row| row.get::<_, i32>(0),
            )
            .ok()
        })
        .is_some()
}

pub fn open_cursor_db(path: &Path) -> Result<Connection, String> {
    let uri = format!(
        "file:{}?mode=ro&immutable=1",
        path.to_string_lossy().replace('\\', "/")
    );
    Connection::open_with_flags(
        uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|e| format!("无法以只读方式打开 Cursor 数据库 {}: {e}", path.display()))
}

pub fn load_composer_index(conn: &Connection) -> Result<HashMap<String, CursorComposerMeta>, String> {
    let mut index = HashMap::new();

    let Ok(raw) = conn.query_row(
        "SELECT value FROM ItemTable WHERE key = 'composer.composerHeaders'",
        [],
        read_value_column,
    ) else {
        return Ok(index);
    };

    let text = read_json_text(raw)?;
    let value: Value =
        serde_json::from_str(&text).map_err(|e| format!("解析 composer.composerHeaders 失败: {e}"))?;

    let composers = value
        .get("allComposers")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten();

    for entry in composers {
        if let Some(meta) = composer_meta_from_json(entry) {
            index.insert(meta.composer_id.clone(), meta);
        }
    }

    Ok(index)
}

pub fn list_composer_ids(conn: &Connection) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT key FROM cursorDiskKV WHERE key LIKE 'composerData:%' ORDER BY rowid ASC")
        .map_err(|e| format!("查询 Cursor 会话列表失败: {e}"))?;

    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| format!("读取 Cursor 会话列表失败: {e}"))?;

    let mut ids = Vec::new();
    for row in rows {
        let key = row.map_err(|e| format!("读取 Cursor 会话 key 失败: {e}"))?;
        if let Some(composer_id) = key.strip_prefix("composerData:") {
            ids.push(composer_id.to_string());
        }
    }

    Ok(ids)
}

pub fn load_composer_json(conn: &Connection, composer_id: &str) -> Result<Value, String> {
    let key = format!("composerData:{composer_id}");
    let raw = conn
        .query_row(
            "SELECT value FROM cursorDiskKV WHERE key = ?1",
            [&key],
            read_value_column,
        )
        .map_err(|e| format!("读取会话 {composer_id} 失败: {e}"))?;

    let text = read_json_text(raw)?;
    serde_json::from_str(&text).map_err(|e| format!("解析会话 {composer_id} 失败: {e}"))
}

pub fn load_bubbles_by_ids(
    conn: &Connection,
    composer_id: &str,
    bubble_ids: &[String],
) -> Result<HashMap<String, Value>, String> {
    if bubble_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let prefix = format!("bubbleId:{composer_id}:");
    let mut bubbles = HashMap::new();

    for chunk in bubble_ids.chunks(200) {
        let keys: Vec<String> = chunk
            .iter()
            .map(|bubble_id| format!("{prefix}{bubble_id}"))
            .collect();
        let placeholders = std::iter::repeat_n("?", keys.len())
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!("SELECT key, value FROM cursorDiskKV WHERE key IN ({placeholders})");
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| format!("查询会话 {composer_id} 消息失败: {e}"))?;

        let params: Vec<&dyn rusqlite::ToSql> =
            keys.iter().map(|key| key as &dyn rusqlite::ToSql).collect();
        let rows = stmt
            .query_map(params.as_slice(), |row| {
                let key: String = row.get(0)?;
                let raw = match row.get_ref(1)? {
                    ValueRef::Text(text) => text.to_vec(),
                    ValueRef::Blob(blob) => blob.to_vec(),
                    ValueRef::Null => return Ok(None),
                    _ => {
                        return Err(rusqlite::Error::InvalidColumnType(
                            1,
                            "value".to_string(),
                            rusqlite::types::Type::Blob,
                        ));
                    }
                };
                Ok(Some((key, raw)))
            })
            .map_err(|e| format!("读取会话 {composer_id} 消息失败: {e}"))?;

        for row in rows {
            let Some((key, raw)) = row.map_err(|e| format!("读取消息行失败: {e}"))? else {
                continue;
            };
            let Some(bubble_id) = key.strip_prefix(&prefix) else {
                continue;
            };
            let text = read_json_text(raw)?;
            let value: Value =
                serde_json::from_str(&text).map_err(|e| format!("解析消息 {bubble_id} 失败: {e}"))?;
            bubbles.insert(bubble_id.to_string(), value);
        }
    }

    Ok(bubbles)
}

fn header_bubble_ids(composer_json: &Value) -> Vec<String> {
    composer_json
        .get("fullConversationHeadersOnly")
        .and_then(|value| value.as_array())
        .map(|headers| {
            headers
                .iter()
                .filter_map(|header| {
                    header
                        .get("bubbleId")
                        .and_then(|value| value.as_str())
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn parse_all_conversations(conn: &Connection) -> Result<Vec<crate::domain::ports::ImportedConversation>, String> {
    let index = load_composer_index(conn)?;
    let composer_ids = list_composer_ids(conn)?;
    let mut conversations = Vec::new();

    for composer_id in composer_ids {
        let Ok(composer_json) = load_composer_json(conn, &composer_id) else {
            continue;
        };
        let meta = index
            .get(&composer_id)
            .cloned()
            .or_else(|| composer_meta_from_json(&composer_json));
        let Some(meta) = meta else {
            continue;
        };
        let bubble_ids = header_bubble_ids(&composer_json);
        let bubbles = load_bubbles_by_ids(conn, &composer_id, &bubble_ids)?;
        if let Some(conversation) = parse_composer_conversation(&composer_id, &composer_json, &meta, &bubbles) {
            conversations.push(conversation);
        }
    }

    Ok(conversations)
}

fn read_value_column(row: &Row<'_>) -> rusqlite::Result<Vec<u8>> {
    match row.get_ref(0)? {
        ValueRef::Text(text) => Ok(text.to_vec()),
        ValueRef::Blob(blob) => Ok(blob.to_vec()),
        ValueRef::Null => Ok(Vec::new()),
        _ => Err(rusqlite::Error::InvalidColumnType(
            0,
            "value".to_string(),
            rusqlite::types::Type::Blob,
        )),
    }
}

fn read_json_text(raw: Vec<u8>) -> Result<String, String> {
    if raw.is_empty() {
        return Err("Cursor 数据为空".to_string());
    }
    String::from_utf8(raw).map_err(|e| format!("Cursor 数据不是有效 UTF-8: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_cursor_db() -> PathBuf {
        std::env::var("APPDATA")
            .map(|appdata| {
                PathBuf::from(appdata)
                    .join("Cursor")
                    .join("User")
                    .join("globalStorage")
                    .join("state.vscdb")
            })
            .unwrap_or_default()
    }

    #[test]
    fn detects_local_cursor_global_db() {
        let db = sample_cursor_db();
        if !db.is_file() {
            return;
        }

        assert!(is_cursor_vscdb(&db));
        let resolved = resolve_cursor_db_path(db.parent().unwrap());
        assert_eq!(resolved.as_deref(), Some(db.as_path()));
    }

    #[test]
    fn parses_local_cursor_conversations() {
        let db = sample_cursor_db();
        if !db.is_file() {
            return;
        }

        let conn = open_cursor_db(&db).expect("open");
        let conversations = parse_all_conversations(&conn).expect("parse");
        assert!(!conversations.is_empty());
        assert!(conversations.iter().any(|conversation| !conversation.messages.is_empty()));
    }
}
