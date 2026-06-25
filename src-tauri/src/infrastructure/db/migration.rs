use rusqlite::{params, Connection};

/// 当前数据库 schema 版本。
pub const CURRENT_SCHEMA_VERSION: i32 = 8;

impl super::Database {
    pub fn schema_version(&self) -> Result<i32, String> {
        read_schema_version(&self.conn)
    }

    pub fn app_version(&self) -> Result<Option<String>, String> {
        read_meta(&self.conn, "app_version")
    }
}

pub(super) fn run_migrations(conn: &Connection) -> Result<(), String> {
    let mut version = read_schema_version(conn)?;
    while version < CURRENT_SCHEMA_VERSION {
        version += 1;
        apply_migration(conn, version)?;
    }
    upsert_meta(conn, "app_version", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}

fn apply_migration(conn: &Connection, version: i32) -> Result<(), String> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("开启迁移事务失败 (v{version}): {e}"))?;

    let result = match version {
        1 => migrate_v1(&tx),
        2 => migrate_v2(&tx),
        3 => migrate_v3(&tx),
        4 => migrate_v4(&tx),
        5 => migrate_v5(&tx),
        6 => migrate_v6(&tx),
        7 => migrate_v7(&tx),
        8 => migrate_v8(&tx),
        _ => Err(format!("未知 schema 版本: {version}")),
    };

    match result {
        Ok(()) => tx
            .commit()
            .map_err(|e| format!("提交迁移事务失败 (v{version}): {e}")),
        Err(error) => {
            let _ = tx.rollback();
            Err(error)
        }
    }
}

/// v1：纳入 meta 表追踪；补齐早期库缺失的 is_starred / attachments 列。
fn migrate_v1(conn: &Connection) -> Result<(), String> {
    if !table_has_column(conn, "conversations", "is_starred")? {
        conn.execute(
            "ALTER TABLE conversations ADD COLUMN is_starred INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| format!("迁移 v1: 添加 conversations.is_starred 失败: {e}"))?;
    }

    if !table_has_column(conn, "messages", "is_starred")? {
        conn.execute(
            "ALTER TABLE messages ADD COLUMN is_starred INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| format!("迁移 v1: 添加 messages.is_starred 失败: {e}"))?;
    }

    if !table_has_column(conn, "messages", "attachments")? {
        conn.execute("ALTER TABLE messages ADD COLUMN attachments TEXT", [])
            .map_err(|e| format!("迁移 v1: 添加 messages.attachments 失败: {e}"))?;
    }

    upsert_meta(conn, "schema_version", "1")?;
    Ok(())
}

/// v2：import_jobs 表；imports 表补充 SourceInfo 列。
fn migrate_v2(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS import_jobs (
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
        )",
        [],
    )
    .map_err(|e| format!("迁移 v2: 创建 import_jobs 失败: {e}"))?;

    if !table_has_column(conn, "imports", "source")? {
        conn.execute("ALTER TABLE imports ADD COLUMN source TEXT", [])
            .map_err(|e| format!("迁移 v2: 添加 imports.source 失败: {e}"))?;
    }
    if !table_has_column(conn, "imports", "export_label")? {
        conn.execute("ALTER TABLE imports ADD COLUMN export_label TEXT", [])
            .map_err(|e| format!("迁移 v2: 添加 imports.export_label 失败: {e}"))?;
    }
    if !table_has_column(conn, "imports", "importer_version")? {
        conn.execute("ALTER TABLE imports ADD COLUMN importer_version TEXT", [])
            .map_err(|e| format!("迁移 v2: 添加 imports.importer_version 失败: {e}"))?;
    }

    upsert_meta(conn, "schema_version", "2")?;
    Ok(())
}

/// v3：conversations 表补充 source / source_id。
fn migrate_v3(conn: &Connection) -> Result<(), String> {
    if !table_has_column(conn, "conversations", "source")? {
        conn.execute("ALTER TABLE conversations ADD COLUMN source TEXT", [])
            .map_err(|e| format!("迁移 v3: 添加 conversations.source 失败: {e}"))?;
    }
    if !table_has_column(conn, "conversations", "source_id")? {
        conn.execute("ALTER TABLE conversations ADD COLUMN source_id TEXT", [])
            .map_err(|e| format!("迁移 v3: 添加 conversations.source_id 失败: {e}"))?;
    }

    conn.execute(
        "UPDATE conversations
         SET source = COALESCE(source, 'chatgpt'),
             source_id = COALESCE(source_id, id)
         WHERE source IS NULL OR source_id IS NULL",
        [],
    )
    .map_err(|e| format!("迁移 v3: 回填 conversations.source/source_id 失败: {e}"))?;

    upsert_meta(conn, "schema_version", "3")?;
    Ok(())
}

/// v4：会话主键改为 `{source}::{source_id}`，并级联更新消息 / 标签 / FTS。
fn migrate_v4(conn: &Connection) -> Result<(), String> {
    use crate::domain::composite_id::is_composite_conversation_id;

    conn.execute(
        "UPDATE conversations
         SET source = COALESCE(source, 'chatgpt'),
             source_id = COALESCE(source_id, id)
         WHERE source IS NULL OR source_id IS NULL",
        [],
    )
    .map_err(|e| format!("迁移 v4: 回填 conversations.source/source_id 失败: {e}"))?;

    let mut stmt = conn
        .prepare("SELECT id, source, source_id FROM conversations")
        .map_err(|e| format!("迁移 v4: 读取 conversations 失败: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| format!("迁移 v4: 遍历 conversations 失败: {e}"))?;

    let mut pending = Vec::new();
    for row in rows {
        let (old_id, source, source_id) =
            row.map_err(|e| format!("迁移 v4: 读取 conversation 行失败: {e}"))?;
        if is_composite_conversation_id(&old_id) {
            continue;
        }
        let new_id = format!("{source}::{source_id}");
        if old_id == new_id {
            continue;
        }
        pending.push((old_id, new_id));
    }

    for (old_id, new_id) in pending {
        conn.execute(
            "INSERT INTO conversations (
                id, title, create_time, update_time, source_path, model, message_count,
                is_starred, source, source_id
             )
             SELECT ?1, title, create_time, update_time, source_path, model, message_count,
                    is_starred, source, source_id
             FROM conversations
             WHERE id = ?2",
            params![new_id, old_id],
        )
        .map_err(|e| format!("迁移 v4: 复制 conversations({old_id} → {new_id}) 失败: {e}"))?;

        conn.execute(
            "UPDATE messages
             SET conversation_id = ?1,
                 id = ?1 || '::' || substr(id, length(?2) + 3)
             WHERE conversation_id = ?2",
            params![new_id, old_id],
        )
        .map_err(|e| format!("迁移 v4: 更新 messages({old_id} → {new_id}) 失败: {e}"))?;

        conn.execute(
            "UPDATE conversation_tags SET conversation_id = ?1 WHERE conversation_id = ?2",
            params![new_id, old_id],
        )
        .map_err(|e| format!("迁移 v4: 更新 conversation_tags 失败: {e}"))?;

        conn.execute(
            "DELETE FROM conversations WHERE id = ?1",
            params![old_id],
        )
        .map_err(|e| format!("迁移 v4: 删除旧 conversations.id 失败: {e}"))?;
    }

    rebuild_messages_fts(conn)?;
    upsert_meta(conn, "schema_version", "4")?;
    Ok(())
}

fn rebuild_messages_fts(conn: &Connection) -> Result<(), String> {
    conn.execute("DELETE FROM messages_fts", [])
        .map_err(|e| format!("迁移 v4: 清空 messages_fts 失败: {e}"))?;
    conn.execute(
        "INSERT INTO messages_fts (message_id, conversation_id, conversation_title, content)
         SELECT m.id, m.conversation_id, c.title, m.content
         FROM messages m
         JOIN conversations c ON c.id = m.conversation_id",
        [],
    )
    .map_err(|e| format!("迁移 v4: 重建 messages_fts 失败: {e}"))?;
    Ok(())
}

/// v5：为核心查询路径补充性能索引（来源过滤、标签筛选、Timeline 预埋、导入唯一性）。
fn migrate_v5(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE INDEX IF NOT EXISTS idx_conversations_source_update
            ON conversations(source, update_time DESC);

        CREATE UNIQUE INDEX IF NOT EXISTS idx_conversations_source_source_id
            ON conversations(source, source_id);

        CREATE INDEX IF NOT EXISTS idx_messages_create_time
            ON messages(create_time DESC);

        CREATE INDEX IF NOT EXISTS idx_conversation_tags_tag
            ON conversation_tags(tag_id, conversation_id);
        ",
    )
    .map_err(|e| format!("迁移 v5: 创建性能索引失败: {e}"))?;

    upsert_meta(conn, "schema_version", "5")?;
    Ok(())
}

/// v6：物化图片资产表，消除图库 json_each / raw_json 全表扫描。
fn migrate_v6(conn: &Connection) -> Result<(), String> {
    use super::asset_index::{backfill_all_assets, create_assets_schema};

    create_assets_schema(conn)?;
    backfill_all_assets(conn)?;
    upsert_meta(conn, "schema_version", "6")?;
    Ok(())
}

/// v7：来源列表查询优化——回填 `source` 并增加 `(source, is_starred, update_time)` 复合索引。
fn migrate_v7(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "UPDATE conversations
         SET source = COALESCE(source, 'chatgpt'),
             source_id = COALESCE(source_id, id)
         WHERE source IS NULL OR source_id IS NULL",
        [],
    )
    .map_err(|e| format!("迁移 v7: 回填 conversations.source/source_id 失败: {e}"))?;

    conn.execute_batch(
        "
        CREATE INDEX IF NOT EXISTS idx_conversations_source_starred_update
            ON conversations(source, is_starred DESC, update_time DESC);
        ",
    )
    .map_err(|e| format!("迁移 v7: 创建来源列表索引失败: {e}"))?;

    upsert_meta(conn, "schema_version", "7")?;
    Ok(())
}

/// v8：来源侧项目/仓库/笔记本上下文，并为 Workspace 映射预留表结构。
fn migrate_v8(conn: &Connection) -> Result<(), String> {
    use super::source_context_index::create_source_context_schema;

    create_source_context_schema(conn)?;
    upsert_meta(conn, "schema_version", "8")?;
    Ok(())
}

#[cfg(test)]
mod migrate_v8_tests {
    #[test]
    fn migrate_v8_creates_source_context_tables() {
        let path = std::env::temp_dir().join(format!(
            "chatlens-migration-v8-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let _ = std::fs::remove_file(&path);
        let db = crate::db::Database::open(&path).expect("open db");
        assert_eq!(db.schema_version().expect("version"), 8);

        db.conn
            .execute(
                "INSERT INTO source_contexts (id, source, context_type, name)
                 VALUES ('chatgpt::project::p1', 'chatgpt', 'project', 'ChatLens')",
                [],
            )
            .expect("insert context");
    }
}

#[cfg(test)]
mod migrate_v6_tests {
    use super::*;
    use rusqlite::params;
    use rusqlite::Connection;

    #[test]
    fn migrate_v6_materializes_assets_from_attachments() {
        let conn = Connection::open_in_memory().expect("open");
        conn.execute_batch(
            "
            CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            INSERT INTO meta (key, value) VALUES ('schema_version', '5');

            CREATE TABLE conversations (
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

            CREATE TABLE messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                create_time REAL,
                sort_order INTEGER NOT NULL,
                raw_json TEXT,
                is_starred INTEGER NOT NULL DEFAULT 0,
                attachments TEXT
            );
            ",
        )
        .expect("seed");

        conn.execute(
            "INSERT INTO conversations (
                id, title, create_time, update_time, source_path, model, message_count, source, source_id
             ) VALUES ('chatgpt::c1', 'Image chat', 1, 2, '/export', NULL, 1, 'chatgpt', 'c1')",
            [],
        )
        .expect("conversation");

        let attachments = serde_json::json!([{
            "path": "/tmp/test.png",
            "file_key": "test.png",
            "source": "generated"
        }])
        .to_string();

        conn.execute(
            "INSERT INTO messages (
                id, conversation_id, role, content, create_time, sort_order, raw_json, attachments
             ) VALUES ('chatgpt::c1::m1', 'chatgpt::c1', 'assistant', 'image', 1, 0, '{}', ?1)",
            params![attachments],
        )
        .expect("message");

        migrate_v6(&conn).expect("migrate v6");

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM assets", [], |row| row.get(0))
            .expect("count assets");
        assert_eq!(count, 1);

        let path: String = conn
            .query_row("SELECT local_path FROM assets", [], |row| row.get(0))
            .expect("path");
        assert_eq!(path, "/tmp/test.png");
    }
}

#[cfg(test)]
mod migrate_v7_tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn migrate_v7_backfills_source_and_creates_list_index() {
        let conn = Connection::open_in_memory().expect("open");
        conn.execute_batch(
            "
            CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            INSERT INTO meta (key, value) VALUES ('schema_version', '6');

            CREATE TABLE conversations (
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
            ",
        )
        .expect("seed");

        conn.execute(
            "INSERT INTO conversations (
                id, title, create_time, update_time, source_path, message_count, source, source_id
             ) VALUES ('legacy', 'Legacy', 1, 2, '/export', 1, NULL, NULL)",
            [],
        )
        .expect("legacy conversation");

        migrate_v7(&conn).expect("migrate v7");

        let source: String = conn
            .query_row("SELECT source FROM conversations WHERE id = 'legacy'", [], |row| {
                row.get(0)
            })
            .expect("source");
        assert_eq!(source, "chatgpt");

        let indexes: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master
                 WHERE type = 'index' AND name LIKE 'idx_%'
                 ORDER BY name",
            )
            .expect("prepare")
            .query_map([], |row| row.get(0))
            .expect("query")
            .filter_map(Result::ok)
            .collect();

        assert!(indexes.contains(&"idx_conversations_source_starred_update".to_string()));

        let version: String = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .expect("schema version");
        assert_eq!(version, "7");
    }
}

#[cfg(test)]
mod migrate_v5_tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn migrate_v5_creates_performance_indexes() {
        let conn = Connection::open_in_memory().expect("open");
        conn.execute_batch(
            "
            CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            INSERT INTO meta (key, value) VALUES ('schema_version', '4');

            CREATE TABLE conversations (
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

            CREATE TABLE messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                create_time REAL,
                sort_order INTEGER NOT NULL,
                raw_json TEXT,
                is_starred INTEGER NOT NULL DEFAULT 0,
                attachments TEXT
            );

            CREATE TABLE conversation_tags (
                conversation_id TEXT NOT NULL,
                tag_id INTEGER NOT NULL,
                PRIMARY KEY (conversation_id, tag_id)
            );
            ",
        )
        .expect("seed");

        migrate_v5(&conn).expect("migrate v5");

        let indexes: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master
                 WHERE type = 'index' AND name LIKE 'idx_%'
                 ORDER BY name",
            )
            .expect("prepare")
            .query_map([], |row| row.get(0))
            .expect("query")
            .filter_map(Result::ok)
            .collect();

        assert!(indexes.contains(&"idx_conversations_source_source_id".to_string()));
        assert!(indexes.contains(&"idx_conversations_source_update".to_string()));
        assert!(indexes.contains(&"idx_conversation_tags_tag".to_string()));
        assert!(indexes.contains(&"idx_messages_create_time".to_string()));

        let version: String = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .expect("schema version");
        assert_eq!(version, "5");
    }
}

#[cfg(test)]
mod migrate_v4_tests {
    use super::*;
    use crate::domain::composite_id::is_composite_conversation_id;
    use rusqlite::Connection;

    #[test]
    fn migrates_legacy_conversation_ids_to_composite_form() {
        let conn = Connection::open_in_memory().expect("open");
        conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;
            CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            INSERT INTO meta (key, value) VALUES ('schema_version', '3');

            CREATE TABLE conversations (
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

            CREATE TABLE messages (
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

            CREATE TABLE tags (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, created_at REAL NOT NULL DEFAULT 0);
            CREATE TABLE conversation_tags (
                conversation_id TEXT NOT NULL,
                tag_id INTEGER NOT NULL,
                PRIMARY KEY (conversation_id, tag_id),
                FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
            );

            CREATE VIRTUAL TABLE messages_fts USING fts5(
                message_id UNINDEXED,
                conversation_id UNINDEXED,
                conversation_title,
                content,
                tokenize = 'unicode61'
            );

            INSERT INTO conversations (id, title, source_path, message_count, source, source_id)
            VALUES ('conv-1', 'Legacy', '/export', 1, 'chatgpt', 'conv-1');
            INSERT INTO messages (id, conversation_id, role, content, sort_order)
            VALUES ('conv-1::m1', 'conv-1', 'user', 'hello', 0);
            INSERT INTO messages_fts (message_id, conversation_id, conversation_title, content)
            VALUES ('conv-1::m1', 'conv-1', 'Legacy', 'hello');
            ",
        )
        .expect("seed");

        migrate_v4(&conn).expect("migrate v4");

        let conversation_id: String = conn
            .query_row("SELECT id FROM conversations", [], |row| row.get(0))
            .expect("conversation id");
        assert_eq!(conversation_id, "chatgpt::conv-1");
        assert!(is_composite_conversation_id(&conversation_id));

        let message_id: String = conn
            .query_row("SELECT id FROM messages", [], |row| row.get(0))
            .expect("message id");
        assert_eq!(message_id, "chatgpt::conv-1::m1");

        let fts_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM messages_fts", [], |row| row.get(0))
            .expect("fts count");
        assert_eq!(fts_count, 1);
    }
}

fn read_schema_version(conn: &Connection) -> Result<i32, String> {
    match read_meta(conn, "schema_version")? {
        Some(value) => value
            .parse()
            .map_err(|e| format!("解析 schema_version 失败 ({value}): {e}")),
        None => Ok(0),
    }
}

fn read_meta(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    if !table_exists(conn, "meta")? {
        return Ok(None);
    }

    conn.query_row(
        "SELECT value FROM meta WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .map(Some)
    .or_else(|error| {
        if matches!(error, rusqlite::Error::QueryReturnedNoRows) {
            Ok(None)
        } else {
            Err(format!("读取 meta.{key} 失败: {error}"))
        }
    })
}

fn upsert_meta(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|e| format!("写入 meta.{key} 失败: {e}"))?;
    Ok(())
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
        params![table],
        |_| Ok(()),
    )
    .map(|_| true)
    .or_else(|error| {
        if matches!(error, rusqlite::Error::QueryReturnedNoRows) {
            Ok(false)
        } else {
            Err(format!("检查表 {table} 是否存在失败: {error}"))
        }
    })
}

fn table_has_column(conn: &Connection, table: &str, column: &str) -> Result<bool, String> {
    if !table_exists(conn, table)? {
        return Ok(false);
    }

    let sql = format!("PRAGMA table_info({table})");
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("读取 {table} 列信息失败: {e}"))?;

    let names = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| format!("遍历 {table} 列信息失败: {e}"))?;

    for name in names {
        if name.map_err(|e| format!("读取 {table} 列名失败: {e}"))? == column {
            return Ok(true);
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rusqlite::Connection;

    use super::{read_schema_version, table_has_column, CURRENT_SCHEMA_VERSION};
    use crate::db::Database;

    fn temp_db_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("chatlens-migration-{name}.db"))
    }

    fn create_legacy_v0_db(path: &PathBuf) {
        let _ = std::fs::remove_file(path);
        let conn = Connection::open(path).expect("open legacy db");
        conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;

            CREATE TABLE imports (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_path TEXT NOT NULL,
                imported_at REAL NOT NULL,
                conversation_count INTEGER NOT NULL,
                message_count INTEGER NOT NULL
            );

            CREATE TABLE conversations (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                create_time REAL,
                update_time REAL,
                source_path TEXT NOT NULL,
                model TEXT,
                message_count INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                create_time REAL,
                sort_order INTEGER NOT NULL,
                raw_json TEXT,
                FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
            );
            ",
        )
        .expect("create legacy schema");
    }

    #[test]
    fn fresh_db_records_schema_version() {
        let path = temp_db_path("fresh");
        let _ = std::fs::remove_file(&path);

        let db = Database::open(&path).expect("open fresh db");
        assert_eq!(db.schema_version().expect("schema version"), CURRENT_SCHEMA_VERSION);
        assert_eq!(
            db.app_version().expect("app version"),
            Some(env!("CARGO_PKG_VERSION").to_string())
        );

        let conn = Connection::open(&path).expect("reopen");
        assert!(table_has_column(&conn, "meta", "key").expect("meta.key"));
        assert_eq!(
            read_schema_version(&conn).expect("read version"),
            CURRENT_SCHEMA_VERSION
        );
    }

    #[test]
    fn legacy_db_migrates_to_current_version() {
        let path = temp_db_path("legacy");
        create_legacy_v0_db(&path);

        let conn = Connection::open(&path).expect("open legacy");
        assert!(!table_has_column(&conn, "conversations", "is_starred").expect("check starred"));
        assert!(!table_has_column(&conn, "messages", "attachments").expect("check attachments"));

        let db = Database::open(&path).expect("migrate legacy db");
        assert_eq!(db.schema_version().expect("schema version"), CURRENT_SCHEMA_VERSION);

        let conn = Connection::open(&path).expect("reopen migrated");
        assert!(table_has_column(&conn, "conversations", "is_starred").expect("starred added"));
        assert!(table_has_column(&conn, "messages", "is_starred").expect("msg starred added"));
        assert!(table_has_column(&conn, "messages", "attachments").expect("attachments added"));
    }

    #[test]
    fn migrations_are_idempotent() {
        let path = temp_db_path("idempotent");
        let _ = std::fs::remove_file(&path);

        let db = Database::open(&path).expect("first open");
        assert_eq!(
            db.schema_version().expect("version"),
            CURRENT_SCHEMA_VERSION
        );

        let db = Database::open(&path).expect("second open");
        assert_eq!(
            db.schema_version().expect("version"),
            CURRENT_SCHEMA_VERSION
        );
    }
}
