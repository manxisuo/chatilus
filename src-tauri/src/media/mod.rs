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

    pub fn resolve(&self, pointer: &str) -> Option<PathBuf> {
        let key = normalize_file_key(pointer);
        if let Some(path) = self.by_key.get(&key) {
            return Some(path.clone());
        }

        self.by_key
            .iter()
            .find(|(candidate, _)| {
                candidate.starts_with(&key)
                    || key.starts_with(candidate.as_str())
                    || path_matches_pointer(candidate, &key)
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

fn path_matches_pointer(candidate: &str, pointer_key: &str) -> bool {
    candidate
        .strip_prefix("file-")
        .zip(pointer_key.strip_prefix("file-"))
        .is_some_and(|(a, b)| a == b || a.starts_with(b) || b.starts_with(a))
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
}
