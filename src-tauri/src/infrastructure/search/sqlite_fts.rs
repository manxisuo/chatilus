use rusqlite::{params, Connection, Transaction};

use crate::domain::ports::{SearchEngine, SearchIndexEntry, SearchQuery, SearchResult};
use crate::infrastructure::db::Database;

pub fn search(conn: &Connection, query: SearchQuery) -> Result<Vec<SearchResult>, String> {
    let trimmed = query.text.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let mut stmt = conn
        .prepare(
            "SELECT
                f.message_id,
                f.conversation_id,
                f.conversation_title,
                m.role,
                snippet(messages_fts, 3, '【', '】', '…', 32) AS snippet,
                m.create_time,
                COALESCE(c.source, 'chatgpt') AS source
             FROM messages_fts f
             JOIN messages m ON m.id = f.message_id
             JOIN conversations c ON c.id = f.conversation_id
             WHERE messages_fts MATCH ?1
             ORDER BY bm25(messages_fts)
             LIMIT ?2",
        )
        .map_err(|e| format!("搜索失败: {e}"))?;

    let fts_query = build_fts_query(trimmed);
    let rows = stmt
        .query_map(params![fts_query, query.limit], |row| {
            Ok(SearchResult {
                message_id: row.get(0)?,
                conversation_id: row.get(1)?,
                conversation_title: row.get(2)?,
                role: row.get(3)?,
                snippet: row.get(4)?,
                created_at: row.get(5)?,
                source: row.get(6)?,
            })
        })
        .map_err(|e| format!("搜索失败: {e}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取搜索结果失败: {e}"))
}

pub fn index_message(tx: &Transaction<'_>, entry: &SearchIndexEntry) -> Result<(), String> {
    tx.execute(
        "INSERT INTO messages_fts (message_id, conversation_id, conversation_title, content)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            entry.message_id,
            entry.conversation_id,
            entry.conversation_title,
            entry.content,
        ],
    )
    .map_err(|e| format!("写入全文索引失败: {e}"))?;
    Ok(())
}

pub fn remove_conversation_index(
    tx: &Transaction<'_>,
    conversation_id: &str,
) -> Result<(), String> {
    tx.execute(
        "DELETE FROM messages_fts WHERE conversation_id = ?1",
        params![conversation_id],
    )
    .map_err(|e| format!("清理旧索引失败: {e}"))?;
    Ok(())
}

pub fn build_fts_query(input: &str) -> String {
    input
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ")
}

impl SearchEngine for Database {
    fn search(&self, query: SearchQuery) -> Result<Vec<SearchResult>, String> {
        search(&self.conn, query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn builds_fts_query_with_multiple_tokens() {
        assert_eq!(build_fts_query("hello world"), "\"hello\" AND \"world\"");
    }

    #[test]
    fn escapes_quotes_in_fts_query() {
        assert_eq!(build_fts_query("foo\"bar"), "\"foo\"\"bar\"");
    }

    #[test]
    fn orders_results_by_bm25() {
        let conn = Connection::open_in_memory().expect("open");
        conn.execute_batch(
            "
            CREATE TABLE conversations (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                source TEXT
            );
            CREATE TABLE messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                create_time REAL
            );
            CREATE VIRTUAL TABLE messages_fts USING fts5(
                message_id UNINDEXED,
                conversation_id UNINDEXED,
                conversation_title,
                content
            );
            ",
        )
        .expect("schema");

        conn.execute(
            "INSERT INTO conversations (id, title, source) VALUES ('c1', 'Rust', 'chatgpt')",
            [],
        )
        .expect("conversation");
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, create_time)
             VALUES ('m1', 'c1', 'user', 'hello world', 1)",
            [],
        )
        .expect("message 1");
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, create_time)
             VALUES ('m2', 'c1', 'assistant', 'rust programming language tutorial', 2)",
            [],
        )
        .expect("message 2");
        conn.execute(
            "INSERT INTO messages_fts (message_id, conversation_id, conversation_title, content)
             VALUES ('m1', 'c1', 'Rust', 'hello world'),
                    ('m2', 'c1', 'Rust', 'rust programming language tutorial')",
            [],
        )
        .expect("fts");

        let results = search(
            &conn,
            SearchQuery {
                text: "rust programming".to_string(),
                limit: 10,
            },
        )
        .expect("search");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].message_id, "m2");
    }
}
