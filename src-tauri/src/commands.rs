use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

use crate::application::{self, new_import_job_store, spawn_import_job, SharedImportJobStore};
use crate::db::Database;
use crate::domain::models::ImportJob;
use crate::domain::ports::ImportGuide;
use crate::infrastructure::importers::default_importer_registry;
use crate::models::{
    ConversationSummary, DatabaseStats, ExportResult, ImageGalleryItem, ImportJobView,
    ImportResult, MessageView, SearchHit, TagView, TimelineMonthBucket,
};

pub struct AppState {
    pub db_path: PathBuf,
    pub db: Mutex<Database>,
    pub import_jobs: SharedImportJobStore,
}

pub fn init_state(app: &AppHandle) -> Result<AppState, String> {
    let db_path = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法获取应用数据目录: {e}"))?
        .join("chatlens.db");

    let db = Database::open(&db_path)?;
    Ok(AppState {
        db_path,
        db: Mutex::new(db),
        import_jobs: new_import_job_store(),
    })
}

#[tauri::command]
pub fn import_export_dir(
    state: State<'_, AppState>,
    path: String,
) -> Result<ImportResult, String> {
    let export_path = PathBuf::from(path);
    if !export_path.exists() {
        return Err(format!("路径不存在: {}", export_path.display()));
    }

    let mut db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    application::import_export_dir(&mut db, &export_path)
}

#[tauri::command]
pub fn list_import_guides() -> Result<Vec<ImportGuide>, String> {
    Ok(default_importer_registry().list_import_guides())
}

#[tauri::command]
pub fn start_import(
    state: State<'_, AppState>,
    app: AppHandle,
    path: String,
    importer_id: Option<String>,
) -> Result<String, String> {
    let input_path = PathBuf::from(&path);
    if !input_path.exists() {
        return Err(format!("路径不存在: {}", input_path.display()));
    }

    if let Some(ref id) = importer_id {
        if default_importer_registry().by_id(id).is_none() {
            return Err(format!("未知数据源: {id}"));
        }
    }

    let job_id = Uuid::new_v4().to_string();
    let mut job = ImportJob::new(job_id.clone(), path);
    job.importer_id = importer_id;
    spawn_import_job(app, state.import_jobs.clone(), state.db_path.clone(), job);
    Ok(job_id)
}

#[tauri::command]
pub fn get_import_job(state: State<'_, AppState>, job_id: String) -> Result<ImportJobView, String> {
    let registry = state
        .import_jobs
        .lock()
        .map_err(|_| "导入任务锁失败".to_string())?;
    registry
        .get_view(&job_id)
        .ok_or_else(|| format!("未找到导入任务: {job_id}"))
}

#[tauri::command]
pub fn list_conversations(
    state: State<'_, AppState>,
    query: Option<String>,
    starred_only: Option<bool>,
    tag_id: Option<i64>,
    source: Option<String>,
    has_images: Option<bool>,
    has_code: Option<bool>,
    has_attachments: Option<bool>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<ConversationSummary>, String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    application::list_conversations(
        &db,
        query.as_deref(),
        starred_only.unwrap_or(false),
        tag_id,
        source.as_deref(),
        has_images.unwrap_or(false),
        has_code.unwrap_or(false),
        has_attachments.unwrap_or(false),
        limit.unwrap_or(200),
        offset.unwrap_or(0),
    )
}

#[tauri::command]
pub fn get_conversation(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Option<ConversationSummary>, String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    application::get_conversation(&db, &conversation_id)
}

#[tauri::command]
pub fn get_messages(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<MessageView>, String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    application::get_messages(&db, &conversation_id)
}

#[tauri::command]
pub fn search_messages(
    state: State<'_, AppState>,
    query: String,
    limit: Option<i64>,
) -> Result<Vec<SearchHit>, String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    application::search_messages(&db, &query, limit.unwrap_or(100))
}

#[tauri::command]
pub fn set_conversation_starred(
    state: State<'_, AppState>,
    conversation_id: String,
    starred: bool,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    application::set_conversation_starred(&db, &conversation_id, starred)
}

#[tauri::command]
pub fn list_starred_messages(
    state: State<'_, AppState>,
    source: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<SearchHit>, String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    application::list_starred_messages(
        &db,
        source.as_deref(),
        limit.unwrap_or(200),
        offset.unwrap_or(0),
    )
}

#[tauri::command]
pub fn set_message_starred(
    state: State<'_, AppState>,
    message_id: String,
    starred: bool,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    application::set_message_starred(&db, &message_id, starred)
}

#[tauri::command]
pub fn list_tags(state: State<'_, AppState>) -> Result<Vec<TagView>, String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    application::list_tags(&db)
}

#[tauri::command]
pub fn create_tag(state: State<'_, AppState>, name: String) -> Result<TagView, String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    application::create_tag(&db, &name)
}

#[tauri::command]
pub fn delete_tag(state: State<'_, AppState>, tag_id: i64) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    application::delete_tag(&db, tag_id)
}

#[tauri::command]
pub fn set_conversation_tags(
    state: State<'_, AppState>,
    conversation_id: String,
    tag_ids: Vec<i64>,
) -> Result<Vec<String>, String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    application::set_conversation_tags(&db, &conversation_id, &tag_ids)
}

#[tauri::command]
pub fn export_conversation_markdown(
    state: State<'_, AppState>,
    conversation_id: String,
    output_path: String,
) -> Result<ExportResult, String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    application::export_conversation_markdown(&db, &conversation_id, &PathBuf::from(output_path))
}

#[tauri::command]
pub fn list_images(
    state: State<'_, AppState>,
    limit: Option<i64>,
    offset: Option<i64>,
    image_kind: Option<String>,
    conversation_source: Option<String>,
    month: Option<String>,
    conversation_id: Option<String>,
) -> Result<Vec<ImageGalleryItem>, String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    let image_kind = image_kind
        .as_deref()
        .and_then(crate::domain::models::ImageKindFilter::parse)
        .unwrap_or(crate::domain::models::ImageKindFilter::All);
    application::list_images(
        &db,
        limit.unwrap_or(60),
        offset.unwrap_or(0),
        image_kind,
        conversation_source.as_deref(),
        month.as_deref(),
        conversation_id.as_deref(),
    )
}

#[tauri::command]
pub fn count_images(
    state: State<'_, AppState>,
    image_kind: Option<String>,
    conversation_source: Option<String>,
    month: Option<String>,
    conversation_id: Option<String>,
) -> Result<i64, String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    let image_kind = image_kind
        .as_deref()
        .and_then(crate::domain::models::ImageKindFilter::parse)
        .unwrap_or(crate::domain::models::ImageKindFilter::All);
    application::count_images(
        &db,
        image_kind,
        conversation_source.as_deref(),
        month.as_deref(),
        conversation_id.as_deref(),
    )
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
pub fn list_timeline_months(
    state: State<'_, AppState>,
    source: Option<String>,
) -> Result<Vec<TimelineMonthBucket>, String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    application::list_timeline_months(&db, source.as_deref())
}

#[tauri::command]
pub fn list_timeline(
    state: State<'_, AppState>,
    source: Option<String>,
    month: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<ConversationSummary>, String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    application::list_timeline(
        &db,
        source.as_deref(),
        month.as_deref(),
        limit.unwrap_or(100),
        offset.unwrap_or(0),
    )
}

#[tauri::command]
pub fn get_stats(state: State<'_, AppState>) -> Result<DatabaseStats, String> {
    let db = state.db.lock().map_err(|_| "数据库锁失败".to_string())?;
    db.stats()
}
