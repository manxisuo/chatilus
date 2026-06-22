use rusqlite::functions::FunctionFlags;

use super::helpers::effective_source_from_attachment_json;
use super::migration::run_migrations;
use super::Database;

impl Database {
    pub(crate) fn init_schema(&self) -> Result<(), String> {
        self.conn
            .execute_batch(
                "
                PRAGMA journal_mode = WAL;
                PRAGMA foreign_keys = ON;

                CREATE TABLE IF NOT EXISTS meta (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS imports (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    source_path TEXT NOT NULL,
                    imported_at REAL NOT NULL,
                    conversation_count INTEGER NOT NULL,
                    message_count INTEGER NOT NULL,
                    source TEXT,
                    export_label TEXT,
                    importer_version TEXT
                );

                CREATE TABLE IF NOT EXISTS conversations (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    create_time REAL,
                    update_time REAL,
                    source_path TEXT NOT NULL,
                    model TEXT,
                    message_count INTEGER NOT NULL DEFAULT 0,
                    is_starred INTEGER NOT NULL DEFAULT 0
                );

                CREATE TABLE IF NOT EXISTS messages (
                    id TEXT PRIMARY KEY,
                    conversation_id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    create_time REAL,
                    sort_order INTEGER NOT NULL,
                    raw_json TEXT,
                    is_starred INTEGER NOT NULL DEFAULT 0,
                    attachments TEXT,
                    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS tags (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL UNIQUE,
                    created_at REAL NOT NULL DEFAULT (unixepoch('subsec'))
                );

                CREATE TABLE IF NOT EXISTS conversation_tags (
                    conversation_id TEXT NOT NULL,
                    tag_id INTEGER NOT NULL,
                    PRIMARY KEY (conversation_id, tag_id),
                    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
                    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
                );

                CREATE INDEX IF NOT EXISTS idx_messages_conversation
                    ON messages(conversation_id, sort_order);
                CREATE INDEX IF NOT EXISTS idx_conversations_update_time
                    ON conversations(update_time DESC);

                CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
                    message_id UNINDEXED,
                    conversation_id UNINDEXED,
                    conversation_title,
                    content,
                    tokenize = 'unicode61'
                );
                ",
            )
            .map_err(|e| format!("初始化数据库失败: {e}"))?;

        run_migrations(&self.conn)?;
        self.register_sql_functions()?;

        self.conn
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_conversations_starred
                 ON conversations(is_starred DESC, update_time DESC)",
                [],
            )
            .map_err(|e| format!("创建收藏索引失败: {e}"))?;

        Ok(())
    }

    fn register_sql_functions(&self) -> Result<(), String> {
        self.conn
            .create_scalar_function(
                "effective_image_source",
                2,
                FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
                |ctx| {
                    let role: String = ctx.get(0)?;
                    let json: String = ctx.get(1)?;
                    Ok(effective_source_from_attachment_json(&role, &json))
                },
            )
            .map_err(|e| format!("注册 SQL 函数失败: {e}"))?;
        Ok(())
    }
}
