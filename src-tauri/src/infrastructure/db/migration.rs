use rusqlite::{params, Connection};

/// Current database schema version.
///
/// Pre-release policy: only v1 exists. Bump this and add `migrate_vN` when shipping
/// to users who need in-place upgrades. Until then, delete `chatlens.db` and re-import
/// after any schema change (mismatch returns an error below).
pub const CURRENT_SCHEMA_VERSION: i32 = 1;

impl super::Database {
    pub fn schema_version(&self) -> Result<i32, String> {
        read_schema_version(&self.conn)
    }

    pub fn app_version(&self) -> Result<Option<String>, String> {
        read_meta(&self.conn, "app_version")
    }
}

pub(super) fn run_migrations(conn: &Connection) -> Result<(), String> {
    let version = read_schema_version(conn)?;
    match version {
        0 => upsert_meta(conn, "schema_version", &CURRENT_SCHEMA_VERSION.to_string())?,
        v if v == CURRENT_SCHEMA_VERSION => {}
        v => {
            return Err(format!(
                "数据库 schema 版本不兼容（库中为 v{v}，当前应用为 v{CURRENT_SCHEMA_VERSION}）。\
                 开发阶段请删除 chatlens.db 后重新导入；正式发布后将提供增量迁移。"
            ));
        }
    }
    upsert_meta(conn, "app_version", env!("CARGO_PKG_VERSION"))?;
    Ok(())
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

pub(super) fn upsert_meta(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rusqlite::Connection;

    use super::{read_schema_version, upsert_meta, CURRENT_SCHEMA_VERSION, run_migrations};
    use crate::db::Database;

    fn temp_db_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("chatlens-migration-{name}.db"))
    }

    #[test]
    fn fresh_db_records_schema_version() {
        let path = temp_db_path("fresh");
        let _ = std::fs::remove_file(&path);

        let db = Database::open(&path).expect("open fresh db");
        assert_eq!(
            db.schema_version().expect("schema version"),
            CURRENT_SCHEMA_VERSION
        );
        assert_eq!(
            db.app_version().expect("app version"),
            Some(env!("CARGO_PKG_VERSION").to_string())
        );

        let conn = Connection::open(&path).expect("reopen");
        assert_eq!(
            read_schema_version(&conn).expect("read version"),
            CURRENT_SCHEMA_VERSION
        );
    }

    #[test]
    fn opening_db_twice_is_idempotent() {
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

    #[test]
    fn stale_schema_version_rejects_open() {
        let path = temp_db_path("stale");
        let _ = std::fs::remove_file(&path);

        let conn = Connection::open(&path).expect("open");
        conn.execute_batch(
            "
            CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            INSERT INTO meta (key, value) VALUES ('schema_version', '9');
            ",
        )
        .expect("seed stale meta");

        match Database::open(&path) {
            Err(error) => {
                assert!(
                    error.contains("schema 版本不兼容"),
                    "unexpected error: {error}"
                );
            }
            Ok(_) => panic!("stale schema should fail"),
        }
    }

    #[test]
    fn init_creates_assets_and_source_context_tables() {
        let path = temp_db_path("full-schema");
        let _ = std::fs::remove_file(&path);

        let db = Database::open(&path).expect("open db");
        let has_assets: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'assets'",
                [],
                |row| row.get(0),
            )
            .expect("assets table");
        assert_eq!(has_assets, 1);

        db.conn
            .execute(
                "INSERT INTO source_contexts (id, source, context_type, name)
                 VALUES ('chatgpt::project::p1', 'chatgpt', 'project', 'ChatLens')",
                [],
            )
            .expect("insert context");
    }

    #[test]
    fn run_migrations_stamps_version_on_empty_meta() {
        let conn = Connection::open_in_memory().expect("open");
        conn.execute(
            "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
            [],
        )
        .expect("meta");

        run_migrations(&conn).expect("stamp version");
        let version: String = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .expect("version");
        assert_eq!(version, "1");
    }

    #[test]
    fn upsert_meta_overwrites_existing_key() {
        let conn = Connection::open_in_memory().expect("open");
        conn.execute(
            "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
            [],
        )
        .expect("meta");
        upsert_meta(&conn, "schema_version", "1").expect("insert");
        upsert_meta(&conn, "schema_version", "1").expect("update");
    }
}
