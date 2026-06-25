use rusqlite::functions::FunctionFlags;

use super::asset_index::create_assets_schema;
use super::helpers::effective_source_from_attachment_json;
use super::migration::run_migrations;
use super::source_context_index::create_source_context_schema;
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

                CREATE TABLE IF NOT EXISTS import_jobs (
                    id TEXT PRIMARY KEY,
                    source_path TEXT NOT NULL,
                    resolved_path TEXT,
                    status TEXT NOT NULL,
                    phase TEXT NOT NULL DEFAULT '',
                    progress REAL NOT NULL DEFAULT 0,
                    processed INTEGER NOT NULL DEFAULT 0,
                    total INTEGER NOT NULL DEFAULT 0,
                    error TEXT,
                    source TEXT,
                    export_label TEXT,
                    importer_version TEXT,
                    created_at REAL NOT NULL,
                    finished_at REAL
                );

                CREATE TABLE IF NOT EXISTS conversations (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    create_time REAL,
                    update_time REAL,
                    source_path TEXT NOT NULL,
                    model TEXT,
                    message_count INTEGER NOT NULL DEFAULT 0,
                    is_starred INTEGER NOT NULL DEFAULT 0,
                    source TEXT,
                    source_id TEXT
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
                CREATE INDEX IF NOT EXISTS idx_conversations_starred
                    ON conversations(is_starred DESC, update_time DESC);
                CREATE INDEX IF NOT EXISTS idx_conversations_source_update
                    ON conversations(source, update_time DESC);
                CREATE UNIQUE INDEX IF NOT EXISTS idx_conversations_source_source_id
                    ON conversations(source, source_id);
                CREATE INDEX IF NOT EXISTS idx_conversations_source_starred_update
                    ON conversations(source, is_starred DESC, update_time DESC);
                CREATE INDEX IF NOT EXISTS idx_messages_create_time
                    ON messages(create_time DESC);
                CREATE INDEX IF NOT EXISTS idx_conversation_tags_tag
                    ON conversation_tags(tag_id, conversation_id);

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

        create_assets_schema(&self.conn)?;
        create_source_context_schema(&self.conn)?;
        run_migrations(&self.conn)?;
        self.register_sql_functions()?;

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
