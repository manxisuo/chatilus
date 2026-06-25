use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, Row};

use super::resolve::{normalize_windows_path, resolve_codex_state_db};

#[derive(Debug, Clone)]
pub struct CodexThreadRecord {
    pub id: String,
    pub title: Option<String>,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub rollout_path: PathBuf,
    pub source: Option<String>,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub git_branch: Option<String>,
    pub git_origin_url: Option<String>,
    pub first_user_message: Option<String>,
}

pub fn open_codex_db(input: &Path) -> Result<(Connection, PathBuf), String> {
    let db_path = resolve_codex_state_db(input)
        .ok_or_else(|| format!("未找到 Codex state 数据库: {}", input.display()))?;
    let uri = format!(
        "file:{}?mode=ro&immutable=1",
        db_path.to_string_lossy().replace('\\', "/")
    );
    let conn = Connection::open_with_flags(
        uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|e| format!("无法以只读方式打开 Codex 数据库 {}: {e}", db_path.display()))?;
    Ok((conn, db_path))
}

pub fn list_threads(conn: &Connection) -> Result<Vec<CodexThreadRecord>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, title, cwd, model, rollout_path, source, created_at, updated_at,
                    git_branch, git_origin_url, first_user_message
             FROM threads
             WHERE rollout_path IS NOT NULL AND rollout_path != ''
             ORDER BY updated_at DESC",
        )
        .map_err(|e| format!("查询 Codex 会话列表失败: {e}"))?;

    let rows = stmt
        .query_map([], map_thread_row)
        .map_err(|e| format!("读取 Codex 会话列表失败: {e}"))?;

    let mut threads = Vec::new();
    for row in rows {
        let thread = row.map_err(|e| format!("解析 Codex 会话行失败: {e}"))?;
        if thread.rollout_path.is_file() {
            threads.push(thread);
        }
    }
    Ok(threads)
}

fn map_thread_row(row: &Row<'_>) -> rusqlite::Result<CodexThreadRecord> {
    let rollout_path = row.get::<_, String>(4)?;
    Ok(CodexThreadRecord {
        id: row.get(0)?,
        title: row.get::<_, Option<String>>(1)?,
        cwd: row
            .get::<_, Option<String>>(2)?
            .map(|value| normalize_windows_path(&value)),
        model: row.get(3)?,
        rollout_path: PathBuf::from(normalize_windows_path(&rollout_path)),
        source: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        git_branch: row.get(8)?,
        git_origin_url: row.get(9)?,
        first_user_message: row.get(10)?,
    })
}
