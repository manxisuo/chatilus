use std::fs;
use std::path::{Path, PathBuf};

const ACTIVITY_MARKERS: &[&str] = &[
    "outer-cell mdl-cell",
    "Prompted",
    "Gemini Apps",
];

pub fn resolve_gemini_export_root(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return is_gemini_activity_html_file(path)
            .then(|| path.parent().map(|parent| parent.to_path_buf()))
            .flatten();
    }

    if !path.is_dir() {
        return None;
    }

    if let Some(html) = find_activity_html(path) {
        if is_gemini_activity_html_file(&html) {
            return Some(path.to_path_buf());
        }
    }

    None
}

pub fn find_activity_html(dir: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    let mut candidates = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let is_html = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("html"));
        if !is_html {
            continue;
        }
        candidates.push(path);
    }

    candidates.sort_by_key(|path| {
        std::cmp::Reverse(
            fs::metadata(path)
                .map(|meta| meta.len())
                .unwrap_or_default(),
        )
    });

    candidates.into_iter().find(|path| is_gemini_activity_html_file(path))
}

pub fn is_gemini_activity_html_file(path: &Path) -> bool {
    let Ok(content) = fs::read_to_string(path) else {
        return false;
    };

    ACTIVITY_MARKERS
        .iter()
        .all(|marker| content.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_gemini_export_dir() -> PathBuf {
        PathBuf::from(
            r"D:\Personal\Gemini数据下载\takeout-20260622T160628Z-3-001\Takeout\我的活动\Gemini Apps",
        )
    }

    #[test]
    fn detects_local_gemini_takeout_dir() {
        let dir = sample_gemini_export_dir();
        if !dir.is_dir() {
            return;
        }

        assert!(resolve_gemini_export_root(&dir).is_some());
        assert!(find_activity_html(&dir).is_some());
    }
}
