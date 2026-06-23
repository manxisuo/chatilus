use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct MediaIndex {
    by_key: HashMap<String, PathBuf>,
}

impl MediaIndex {
    pub fn build(export_dir: &Path) -> Self {
        let mut by_key = HashMap::new();
        index_directory(export_dir, export_dir, &mut by_key, 0);
        Self { by_key }
    }

    /// Indexes images under `Cursor/User/workspaceStorage/*/images/` and
    /// `%USERPROFILE%/.cursor/projects/*/assets/`.
    pub fn build_cursor(user_dir: &Path) -> Self {
        let mut by_key = HashMap::new();
        let workspace_storage = user_dir.join("workspaceStorage");
        if workspace_storage.is_dir() {
            index_cursor_workspace_images(&workspace_storage, &mut by_key);
        }
        if let Some(projects_dir) = cursor_projects_dir() {
            index_cursor_projects_assets(&projects_dir, &mut by_key);
        }
        Self { by_key }
    }

    pub fn resolve(&self, pointer: &str) -> Option<PathBuf> {
        if let Some(path) = resolve_existing_file_pointer(pointer) {
            return Some(path);
        }

        let key = normalize_file_key(pointer);
        if let Some(path) = self.by_key.get(&key) {
            return Some(path.clone());
        }

        if is_uuid(&key) {
            if let Some(path) = self.by_key.get(&format!("image-{key}")) {
                return Some(path.clone());
            }
        }

        self.by_key
            .iter()
            .find(|(candidate, _)| {
                candidate.starts_with(&key)
                    || key.starts_with(candidate.as_str())
                    || path_matches_pointer(candidate, &key)
                    || (is_uuid(&key) && candidate.starts_with(&format!("{key}-")))
            })
            .map(|(_, path)| path.clone())
    }

    pub fn len(&self) -> usize {
        self.by_key.len()
    }
}

fn index_directory(
    export_dir: &Path,
    current: &Path,
    index: &mut HashMap<String, PathBuf>,
    depth: u8,
) {
    let entries = match fs::read_dir(current) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                register_file_keys(name, &path, index);
            }
            continue;
        }

        if path.is_dir() && depth < 3 {
            index_directory(export_dir, &path, index, depth + 1);
        }
    }
}

fn register_file_keys(name: &str, path: &PathBuf, index: &mut HashMap<String, PathBuf>) {
    if let Some(key) = pointer_key_from_filename(name) {
        index.entry(key).or_insert_with(|| path.clone());
    }

    if let Some(key) = file_key_from_hash_name(name) {
        index.entry(key).or_insert_with(|| path.clone());
    }

    if is_image_filename(name) {
        index.entry(name.to_string()).or_insert_with(|| path.clone());
    }
}

fn is_image_filename(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".jpg", ".jpeg", ".png", ".gif", ".webp", ".bmp"]
        .iter()
        .any(|ext| lower.ends_with(ext))
}

/// `file-UZjt2wbMlSFQEdh6nzVVybgI-1000036297.jpg` → `file-UZjt2wbMlSFQEdh6nzVVybgI`
fn pointer_key_from_filename(name: &str) -> Option<String> {
    let base = name.split('#').next()?;
    let prefix = if base.starts_with("file-") {
        "file-"
    } else if base.starts_with("file_") {
        "file_"
    } else {
        return None;
    };

    let rest = &base[prefix.len()..];
    if rest.is_empty() {
        return None;
    }

    let id_end = rest.find('-').unwrap_or(rest.len());
    let id = &rest[..id_end];
    if id.is_empty() {
        return None;
    }

    Some(format!("{prefix}{id}"))
}

fn file_key_from_hash_name(name: &str) -> Option<String> {
    if let Some(idx) = name.find("#file_") {
        let tail = &name[idx + 1..];
        let key = tail.split('#').next()?.split('.').next()?;
        return Some(key.to_string());
    }
    None
}

fn index_cursor_workspace_images(workspace_storage: &Path, index: &mut HashMap<String, PathBuf>) {
    let entries = match fs::read_dir(workspace_storage) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let images_dir = entry.path().join("images");
        if !images_dir.is_dir() {
            continue;
        }

        let image_entries = match fs::read_dir(&images_dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for image_entry in image_entries.flatten() {
            let path = image_entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !is_image_filename(name) {
                continue;
            }

            for key in cursor_image_keys_from_filename(name) {
                insert_prefer_newer(index, key, path.clone());
            }
        }
    }
}

fn path_matches_pointer(candidate: &str, pointer_key: &str) -> bool {
    candidate
        .strip_prefix("file-")
        .zip(pointer_key.strip_prefix("file-"))
        .is_some_and(|(a, b)| a == b || a.starts_with(b) || b.starts_with(a))
}

fn cursor_projects_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(|home| {
        PathBuf::from(home).join(".cursor").join("projects")
    })
}

fn index_cursor_projects_assets(projects_dir: &Path, index: &mut HashMap<String, PathBuf>) {
    let entries = match fs::read_dir(projects_dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let assets_dir = entry.path().join("assets");
        if !assets_dir.is_dir() {
            continue;
        }

        let asset_entries = match fs::read_dir(&assets_dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for asset_entry in asset_entries.flatten() {
            let path = asset_entry.path();
            if !path.is_file() {
                continue;
            }
            register_cursor_asset_path(&path, index);
        }
    }
}

fn register_cursor_asset_path(path: &Path, index: &mut HashMap<String, PathBuf>) {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return;
    };
    if !is_image_filename(name) || name.starts_with("c__Users_") {
        return;
    }

    let path_buf = path.to_path_buf();
    insert_prefer_newer(index, name.to_string(), path_buf.clone());

    let path_str = path.to_string_lossy();
    insert_prefer_newer(index, path_str.to_string(), path_buf.clone());
    insert_prefer_newer(
        index,
        normalize_windows_path_key(&path_str),
        path_buf.clone(),
    );
    insert_prefer_newer(
        index,
        normalize_windows_path_key(&path_str.replace('\\', "/")),
        path_buf,
    );
}

fn normalize_windows_path_key(path: &str) -> String {
    path.replace('/', "\\")
}

fn resolve_existing_file_pointer(pointer: &str) -> Option<PathBuf> {
    let trimmed = pointer.trim();
    if trimmed.is_empty() {
        return None;
    }

    let path = Path::new(trimmed);
    if path.is_file() {
        return Some(path.to_path_buf());
    }

    None
}

fn insert_prefer_newer(index: &mut HashMap<String, PathBuf>, key: String, path: PathBuf) {
    index
        .entry(key)
        .and_modify(|existing| {
            if file_is_newer(&path, existing) {
                *existing = path.clone();
            }
        })
        .or_insert(path);
}

fn file_is_newer(candidate: &Path, existing: &Path) -> bool {
    let candidate_mtime = fs::metadata(candidate).and_then(|meta| meta.modified()).ok();
    let existing_mtime = fs::metadata(existing).and_then(|meta| meta.modified()).ok();
    match (candidate_mtime, existing_mtime) {
        (Some(candidate_mtime), Some(existing_mtime)) => candidate_mtime > existing_mtime,
        (Some(_), None) => true,
        _ => false,
    }
}

fn cursor_image_keys_from_filename(name: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let base = name
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(name);

    if let Some(uuid) = base.strip_prefix("image-").filter(|value| is_uuid(value)) {
        keys.push(uuid.to_string());
        keys.push(format!("image-{uuid}"));
        return keys;
    }

    if let Some(uuid) = base.strip_prefix("ChatLens-").filter(|value| is_uuid(value)) {
        keys.push(uuid.to_string());
        keys.push(format!("ChatLens-{uuid}"));
        return keys;
    }

    if base.len() >= 36 {
        let prefix = &base[..36];
        if is_uuid(prefix) {
            keys.push(prefix.to_string());
        }
    }

    keys
}

fn is_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 36 {
        return false;
    }

    for (index, byte) in bytes.iter().enumerate() {
        match index {
            8 | 13 | 18 | 23 => {
                if *byte != b'-' {
                    return false;
                }
            }
            _ => {
                if !byte.is_ascii_hexdigit() {
                    return false;
                }
            }
        }
    }

    true
}

pub fn normalize_file_key(pointer: &str) -> String {
    pointer
        .trim()
        .strip_prefix("file-service://")
        .or_else(|| pointer.strip_prefix("sediment://"))
        .unwrap_or(pointer)
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn normalize_pointer() {
        assert_eq!(
            normalize_file_key("file-service://file-abc123"),
            "file-abc123"
        );
    }

    #[test]
    fn pointer_key_from_jpg_name() {
        assert_eq!(
            pointer_key_from_filename("file-UZjt2wbMlSFQEdh6nzVVybgI-1000036297.jpg"),
            Some("file-UZjt2wbMlSFQEdh6nzVVybgI".to_string())
        );
    }

    #[test]
    fn pointer_key_from_extensionless_name() {
        assert_eq!(
            pointer_key_from_filename(
                "file-jYOPKSz8VPyhHyfrastQCE3H-a58a2745-a096-44f3-8760-672a20fcd42a653372494411067361"
            ),
            Some("file-jYOPKSz8VPyhHyfrastQCE3H".to_string())
        );
    }

    #[test]
    fn resolves_real_export_files() {
        let dir = PathBuf::from(r"D:\Personal\ChatGPT数据下载-2026年5月18日\2026-05-16-12-08-35");
        if !dir.is_dir() {
            return;
        }

        let index = MediaIndex::build(&dir);
        assert!(index.len() > 0);
        assert!(index
            .resolve("file-service://file-UZjt2wbMlSFQEdh6nzVVybgI")
            .is_some());
        assert!(index
            .resolve("file-service://file-jYOPKSz8VPyhHyfrastQCE3H")
            .is_some());
    }

    #[test]
    fn resolves_gemini_takeout_image_by_filename() {
        let dir = PathBuf::from(
            r"D:\Personal\Gemini数据下载\takeout-20260622T160628Z-3-001\Takeout\我的活动\Gemini Apps",
        );
        if !dir.is_dir() {
            return;
        }

        let index = MediaIndex::build(&dir);
        assert!(index.len() > 50);

        let sample_name = fs::read_dir(&dir)
            .expect("read dir")
            .flatten()
            .find_map(|entry| {
                let name = entry.file_name().to_str()?.to_string();
                if is_image_filename(&name) {
                    Some(name)
                } else {
                    None
                }
            })
            .expect("sample image");

        assert!(index.resolve(&sample_name).is_some());
    }

    #[test]
    fn cursor_image_keys_from_upload_filename() {
        let keys = cursor_image_keys_from_filename(
            "f708e67e-8c7c-4f74-ba96-5a567f3bcf1e-006c0ffd-276e-481e-be70-ca62e5ae4a9c.png",
        );
        assert_eq!(keys, vec!["f708e67e-8c7c-4f74-ba96-5a567f3bcf1e".to_string()]);
    }

    #[test]
    fn cursor_image_keys_from_generated_filename() {
        let keys =
            cursor_image_keys_from_filename("image-2b162585-c39b-419e-af86-a0f412a8152c.png");
        assert_eq!(
            keys,
            vec![
                "2b162585-c39b-419e-af86-a0f412a8152c".to_string(),
                "image-2b162585-c39b-419e-af86-a0f412a8152c".to_string(),
            ]
        );
    }

    #[test]
    fn resolves_local_cursor_workspace_images() {
        let user_dir = std::env::var("APPDATA")
            .map(|appdata| PathBuf::from(appdata).join("Cursor").join("User"))
            .unwrap_or_default();
        if !user_dir.is_dir() {
            return;
        }

        let index = MediaIndex::build_cursor(&user_dir);
        if index.len() == 0 {
            return;
        }

        assert!(index
            .resolve("f708e67e-8c7c-4f74-ba96-5a567f3bcf1e")
            .is_some()
            || index
                .resolve("2b162585-c39b-419e-af86-a0f412a8152c")
                .is_some());
    }

    #[test]
    fn resolves_local_cursor_project_assets() {
        let user_dir = std::env::var("APPDATA")
            .map(|appdata| PathBuf::from(appdata).join("Cursor").join("User"))
            .unwrap_or_default();
        if !user_dir.is_dir() {
            return;
        }

        let index = MediaIndex::build_cursor(&user_dir);
        let resolved = index.resolve("java-thread-state-transitions.png");
        if resolved.is_some() {
            assert!(resolved.unwrap().is_file());
        }
    }
}
