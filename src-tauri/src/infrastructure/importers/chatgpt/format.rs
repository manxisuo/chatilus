use std::fs;
use std::path::Path;

use serde_json::Value;

use super::find_conversation_files;

/// OpenAI Manifest v1: `export_manifest.json` with `version`, sharded conversations,
/// and `.dat` assets (or `conversation_asset_file_names.json`).
///
/// Some older exports also include a manifest listing legacy `.jpg`/`.png` files;
/// those are not Manifest v1 and should use the legacy ChatGPT importer.
pub fn is_manifest_v1_export(export_dir: &Path) -> bool {
    let manifest_path = export_dir.join("export_manifest.json");
    if !manifest_path.is_file() {
        return false;
    }

    let raw = match fs::read_to_string(&manifest_path) {
        Ok(value) => value,
        Err(_) => return false,
    };

    let value: Value = match serde_json::from_str(&raw) {
        Ok(value) => value,
        Err(_) => return false,
    };

    if value.get("version").is_none() || find_conversation_files(export_dir).is_err() {
        return false;
    }

    if export_dir
        .join("conversation_asset_file_names.json")
        .is_file()
    {
        return true;
    }

    has_dat_assets(export_dir, &value)
}

fn has_dat_assets(export_dir: &Path, manifest: &Value) -> bool {
    if fs::read_dir(export_dir)
        .into_iter()
        .flatten()
        .flatten()
        .any(|entry| {
            entry.path().is_file()
                && entry
                    .path()
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("dat"))
        })
    {
        return true;
    }

    manifest
        .get("export_files")
        .and_then(|files| files.as_array())
        .is_some_and(|files| {
            files.iter().any(|item| {
                item.get("path")
                    .and_then(|path| path.as_str())
                    .is_some_and(|path| path.ends_with(".dat"))
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn legacy_style_export_dir() -> PathBuf {
        PathBuf::from(r"D:\Personal\ChatGPT数据下载\2026-05-16-12-08-35")
    }

    fn sample_v1_export_dir() -> PathBuf {
        PathBuf::from(r"D:\Personal\ChatGPT数据下载\2026-06-26")
    }

    #[test]
    fn detects_manifest_v1_export() {
        let dir = sample_v1_export_dir();
        if !dir.is_dir() {
            return;
        }
        assert!(is_manifest_v1_export(&dir));
    }

    #[test]
    fn legacy_export_with_manifest_is_not_manifest_v1() {
        let dir = legacy_style_export_dir();
        if !dir.is_dir() {
            return;
        }
        assert!(!is_manifest_v1_export(&dir));
    }
}
