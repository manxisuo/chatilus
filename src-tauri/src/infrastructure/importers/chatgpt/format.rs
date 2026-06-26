use std::path::Path;

use serde_json::Value;

use super::find_conversation_files;

/// OpenAI export manifest v1: flat directory with `export_manifest.json` and sharded conversations.
pub fn is_manifest_v1_export(export_dir: &Path) -> bool {
    let manifest_path = export_dir.join("export_manifest.json");
    if !manifest_path.is_file() {
        return false;
    }

    let raw = match std::fs::read_to_string(&manifest_path) {
        Ok(value) => value,
        Err(_) => return false,
    };

    let value: Value = match serde_json::from_str(&raw) {
        Ok(value) => value,
        Err(_) => return false,
    };

    value.get("version").is_some() && find_conversation_files(export_dir).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
}
