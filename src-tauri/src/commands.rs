use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{AppHandle, Manager, State};

use crate::db::Database;
use crate::models::{
    ConversationSummary, DatabaseStats, ExportResult, ImportResult, MessageView, SearchHit,
    TagView,
};

pub struct AppState {
    pub db: Mutex<Database>,
}

pub fn init_state(app: &AppHandle) -> Result<AppState, String> {
    let db_path = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法获取应用数据目录: {e}"))?
        .join("chatlens.db");

    let db = Database::open(&db_path)?;
    Ok(AppState {
        db: Mutex::new(db),
    })
}

#[tauri::command]
pub fn import_export_dir(
    state: State<'_, AppState>,
    path: String,
) -> Result<ImportResult, String> {
    let export_path = PathBuf::from(path);
    if !export_path.is_dir() {
        return Err(format!("路径不存在或不是目录: {}", export_path.display()));
    }

    let mut db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    db.import_export_dir(&export_path)
}

#[tauri::command]
pub fn list_conversations(
    state: State<'_, AppState>,
    query: Option<String>,
    starred_only: Option<bool>,
    tag_id: Option<i64>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<ConversationSummary>, String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    db.list_conversations(
        query.as_deref(),
        starred_only.unwrap_or(false),
        tag_id,
        limit.unwrap_or(200),
        offset.unwrap_or(0),
    )
}

#[tauri::command]
pub fn get_messages(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<MessageView>, String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    db.get_messages(&conversation_id)
}

#[tauri::command]
pub fn search_messages(
    state: State<'_, AppState>,
    query: String,
    limit: Option<i64>,
) -> Result<Vec<SearchHit>, String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    db.search_messages(&query, limit.unwrap_or(100))
}

#[tauri::command]
pub fn set_conversation_starred(
    state: State<'_, AppState>,
    conversation_id: String,
    starred: bool,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    db.set_conversation_starred(&conversation_id, starred)
}

#[tauri::command]
pub fn set_message_starred(
    state: State<'_, AppState>,
    message_id: String,
    starred: bool,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    db.set_message_starred(&message_id, starred)
}

#[tauri::command]
pub fn list_tags(state: State<'_, AppState>) -> Result<Vec<TagView>, String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    db.list_tags()
}

#[tauri::command]
pub fn create_tag(state: State<'_, AppState>, name: String) -> Result<TagView, String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    db.create_tag(&name)
}

#[tauri::command]
pub fn delete_tag(state: State<'_, AppState>, tag_id: i64) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    db.delete_tag(tag_id)
}

#[tauri::command]
pub fn set_conversation_tags(
    state: State<'_, AppState>,
    conversation_id: String,
    tag_ids: Vec<i64>,
) -> Result<Vec<String>, String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    db.set_conversation_tags(&conversation_id, &tag_ids)
}

#[tauri::command]
pub fn export_conversation_markdown(
    state: State<'_, AppState>,
    conversation_id: String,
    output_path: String,
) -> Result<ExportResult, String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    db.export_conversation_markdown(&conversation_id, &PathBuf::from(output_path))
}

#[tauri::command]
pub fn read_image_data_url(path: String) -> Result<String, String> {
    let file_path = PathBuf::from(&path);
    if !file_path.is_file() {
        return Err(format!("图片文件不存在: {path}"));
    }

    let bytes = std::fs::read(&file_path).map_err(|e| format!("读取图片失败: {e}"))?;
    let mime = mime_from_path(&file_path);
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
    Ok(format!("data:{mime};base64,{encoded}"))
}

fn mime_from_path(path: &PathBuf) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "image/jpeg",
    }
}

#[tauri::command]
pub fn get_stats(state: State<'_, AppState>) -> Result<DatabaseStats, String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    db.stats()
}
