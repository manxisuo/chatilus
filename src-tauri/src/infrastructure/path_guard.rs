use crate::error::{AppError, AppResult};
use std::path::{Component, Path, PathBuf};

use rusqlite::{params, OptionalExtension};

use super::db::Database;

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "avif"];

pub fn is_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .is_some_and(|ext| IMAGE_EXTENSIONS.contains(&ext.as_str()))
}

/// Rejects relative paths and `..` components before any filesystem access.
pub fn has_safe_components(path: &Path) -> bool {
    if path.as_os_str().is_empty() {
        return false;
    }
    // `has_root` accepts Unix absolute paths and Windows drive/root absolute paths
    // (including `/home/...` which `is_relative()` reports as true on Windows).
    if !path.has_root() {
        return false;
    }
    path.components().all(|component| {
        matches!(
            component,
            Component::Prefix(_) | Component::RootDir | Component::Normal(_)
        )
    })
}

pub fn is_under_root(path: &Path, root: &Path) -> bool {
    path.starts_with(root)
}

/// Validates a frontend-supplied image path for read/copy.
///
/// Allowed when the file is an image and either:
/// - registered in the `assets` table or message attachments, or
/// - lives under a known import root (`imports` / `conversations.source_path`), or
/// - lives under the provided app data dir (covers `imports/` cache).
pub fn resolve_readable_image(
    db: &Database,
    app_data_dir: &Path,
    raw_path: &str,
) -> AppResult<PathBuf> {
    let path = PathBuf::from(raw_path);
    if !has_safe_components(&path) {
        return Err(AppError::msg("非法图片路径"));
    }
    if !is_image_path(&path) {
        return Err(AppError::msg("仅允许读取图片文件"));
    }

    let canonical = path
        .canonicalize()
        .map_err(|_| AppError::Msg(format!("图片文件不存在: {raw_path}")))?;
    if !canonical.is_file() {
        return Err(AppError::Msg(format!("图片文件不存在: {raw_path}")));
    }

    let app_data_canonical = app_data_dir.canonicalize().unwrap_or_else(|_| app_data_dir.to_path_buf());
    if is_under_root(&canonical, &app_data_canonical) {
        return Ok(canonical);
    }
    if let Some(dos) = strip_unc_prefix(&canonical) {
        if is_under_root(&dos, app_data_dir) || is_under_root(&dos, &app_data_canonical) {
            return Ok(canonical);
        }
    }

    if is_registered_asset(db, &path, &canonical)? {
        return Ok(canonical);
    }

    if is_under_known_import_root(db, &canonical) {
        return Ok(canonical);
    }

    Err(AppError::msg("图片路径不在允许范围内"))
}

/// Destination for "save as" is user-chosen via the system dialog; still
/// constrain to a real image filename and a safe absolute path.
pub fn validate_save_destination(raw_path: &str) -> AppResult<PathBuf> {
    let path = PathBuf::from(raw_path);
    if !has_safe_components(&path) {
        return Err(AppError::msg("非法保存路径"));
    }
    if !is_image_path(&path) {
        return Err(AppError::msg("保存路径必须是图片文件"));
    }
    Ok(path)
}

fn is_registered_asset(db: &Database, original: &Path, canonical: &Path) -> AppResult<bool> {
    let keys = path_match_keys(original, canonical);

    for key in &keys {
        let found: Option<i64> = db
            .conn
            .query_row(
                "SELECT 1 FROM assets WHERE lower(local_path) = ?1 LIMIT 1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| AppError::Msg(format!("校验图片注册信息失败: {e}")))?;
        if found.is_some() {
            return Ok(true);
        }
    }

    // Message attachments that never got materialized into `assets`.
    for key in &keys {
        let found: Option<i64> = db
            .conn
            .query_row(
                "SELECT 1 FROM messages
                 WHERE attachments IS NOT NULL
                   AND lower(attachments) LIKE '%' || ?1 || '%'
                 LIMIT 1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| AppError::Msg(format!("校验附件注册信息失败: {e}")))?;
        if found.is_some() {
            return Ok(true);
        }
    }

    Ok(false)
}

fn is_under_known_import_root(db: &Database, canonical: &Path) -> bool {
    let dos_owned = strip_unc_prefix(canonical);
    let dos = dos_owned.as_deref().unwrap_or(canonical);

    let mut roots: Vec<String> = Vec::new();
    if let Ok(mut stmt) = db.conn.prepare("SELECT source_path FROM imports") {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for row in rows.flatten() {
                roots.push(row);
            }
        }
    }
    if let Ok(mut stmt) =
        db.conn
            .prepare("SELECT DISTINCT source_path FROM conversations WHERE source_path IS NOT NULL")
    {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for row in rows.flatten() {
                roots.push(row);
            }
        }
    }

    for root in roots {
        let root_path = PathBuf::from(&root);
        if root.trim().is_empty() {
            continue;
        }
        if is_under_root(canonical, &root_path) || is_under_root(dos, &root_path) {
            return true;
        }
        if let Ok(root_canonical) = root_path.canonicalize() {
            if is_under_root(canonical, &root_canonical) {
                return true;
            }
            if let Some(root_dos) = strip_unc_prefix(&root_canonical) {
                if is_under_root(dos, &root_dos) || is_under_root(canonical, &root_dos) {
                    return true;
                }
            }
        }
    }

    false
}

/// Builds lowercase string keys covering DOS and UNC spellings of a path.
fn path_match_keys(original: &Path, canonical: &Path) -> Vec<String> {
    let mut keys = Vec::new();
    let mut push = |path: &Path| {
        let key = path.to_string_lossy().to_lowercase();
        if !key.is_empty() && !keys.contains(&key) {
            keys.push(key);
        }
    };

    push(original);
    push(canonical);
    if let Some(dos) = strip_unc_prefix(canonical) {
        push(&dos);
    }
    if let Ok(canonical_original) = original.canonicalize() {
        push(&canonical_original);
        if let Some(dos) = strip_unc_prefix(&canonical_original) {
            push(&dos);
        }
    }
    keys
}

fn strip_unc_prefix(path: &Path) -> Option<PathBuf> {
    let text = path.to_string_lossy();
    let stripped = text
        .strip_prefix(r"\\?\")
        .or_else(|| text.strip_prefix("//?/"))
        .or_else(|| text.strip_prefix(r"\\.\"))
        .or_else(|| text.strip_prefix("//./"))?;
    if stripped.is_empty() {
        return None;
    }
    Some(PathBuf::from(stripped))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_image_extensions() {
        assert!(is_image_path(Path::new(r"C:\a\b.PNG")));
        assert!(is_image_path(Path::new("/tmp/x.jpeg")));
        assert!(!is_image_path(Path::new(r"C:\a\b.txt")));
        assert!(is_image_path(Path::new("/tmp/x.jpg")));
        assert!(!is_image_path(Path::new(r"C:\a\b")));
    }

    #[test]
    fn rejects_unsafe_components() {
        assert!(!has_safe_components(Path::new("relative/a.png")));
        assert!(!has_safe_components(Path::new(r"C:\a\..\..\Windows\a.png")));
        assert!(!has_safe_components(Path::new(r"C:a.png")));
        assert!(has_safe_components(Path::new(r"C:\data\imports\a.png")));
        assert!(has_safe_components(Path::new("/home/user/export/file.png")));
    }

    #[test]
    fn save_destination_requires_image_and_absolute_path() {
        assert!(validate_save_destination(r"C:\Users\me\Pictures\out.png").is_ok());
        assert!(validate_save_destination(r"C:\Users\me\Pictures\out.txt").is_err());
        assert!(validate_save_destination("out.png").is_err());
        assert!(validate_save_destination(r"C:\a\..\out.png").is_err());
    }

    #[test]
    fn registered_asset_paths_are_allowed() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        // Keep the image outside app_data_dir so only the registry branch can allow it.
        let image_dir = std::env::temp_dir().join(format!("chatlens-export-sim-{stamp}"));
        std::fs::create_dir_all(&image_dir).expect("mkdir");
        let path = image_dir.join(format!("chatlens-path-guard-{stamp}.png"));
        std::fs::write(&path, b"fake-png").expect("write image");
        let canonical = path.canonicalize().expect("canonical");

        let db_path = std::env::temp_dir().join(format!("chatlens-path-guard-{stamp}.db"));
        let db = Database::open(&db_path).expect("open db");
        db.conn
            .execute(
                "INSERT INTO conversations (id, title, source_path, message_count)
                 VALUES ('chatgpt::c1', 't', '/export', 1)",
                [],
            )
            .expect("insert conversation");
        db.conn
            .execute(
                "INSERT INTO messages (id, conversation_id, role, content, sort_order, attachments)
                 VALUES ('chatgpt::c1::m1', 'chatgpt::c1', 'user', 'x', 0, ?1)",
                params![format!(
                    r#"[{{"path":"{}"}}"#,
                    path.display().to_string().replace('\\', "\\\\")
                )],
            )
            .expect("insert message");
        // Store the non-canonical form, matching what importers persist.
        db.conn
            .execute(
                "INSERT INTO assets (
                    message_id, file_key, conversation_id, conversation_source,
                    role, conversation_title, local_path, image_source, created_at
                 ) VALUES ('chatgpt::c1::m1','k1','chatgpt::c1','chatgpt','user','t', ?1, 'upload', 1)",
                params![path.display().to_string().to_lowercase()],
            )
            .expect("insert asset");

        let app_data = std::env::temp_dir().join(format!("chatlens-appdata-{stamp}"));
        std::fs::create_dir_all(&app_data).expect("mkdir app data");
        let resolved = resolve_readable_image(&db, &app_data, &path.display().to_string())
            .expect("should allow registered asset");
        assert_eq!(resolved, canonical);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&image_dir);
        let _ = std::fs::remove_dir_all(&app_data);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn rejects_non_image_and_missing_files() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let db_path = std::env::temp_dir().join(format!("chatlens-path-guard-neg-{stamp}.db"));
        let db = Database::open(&db_path).expect("open db");
        let app_data = std::env::temp_dir();

        let secret = std::env::temp_dir().join(format!("chatlens-secret-{stamp}.txt"));
        std::fs::write(&secret, b"password").expect("write secret");
        let err = resolve_readable_image(&db, &app_data, &secret.display().to_string())
            .expect_err("must reject non-image").to_string();
        assert!(err.contains("图片") || err.contains("允许"));

        let missing = std::env::temp_dir().join(format!("chatlens-missing-{stamp}.png"));
        assert!(resolve_readable_image(&db, &app_data, &missing.display().to_string()).is_err());

        let _ = std::fs::remove_file(&secret);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn unregistered_image_outside_roots_is_rejected() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let db_path = std::env::temp_dir().join(format!("chatlens-path-guard-out-{stamp}.db"));
        let db = Database::open(&db_path).expect("open db");

        // Outside app data and not registered.
        let rogue_dir = std::env::temp_dir().join(format!("chatlens-rogue-{stamp}"));
        std::fs::create_dir_all(&rogue_dir).expect("mkdir");
        let rogue = rogue_dir.join("secret.png");
        std::fs::write(&rogue, b"not-really-png").expect("write");

        // app_data_dir is a sibling empty dir that does not contain rogue.
        let other_app_data = std::env::temp_dir().join(format!("chatlens-other-app-{stamp}"));
        std::fs::create_dir_all(&other_app_data).expect("mkdir");

        let result = resolve_readable_image(&db, &other_app_data, &rogue.display().to_string());
        assert!(result.is_err());

        let _ = std::fs::remove_file(&rogue);
        let _ = std::fs::remove_dir_all(&rogue_dir);
        let _ = std::fs::remove_dir_all(&other_app_data);
        let _ = std::fs::remove_file(&db_path);
    }
}
