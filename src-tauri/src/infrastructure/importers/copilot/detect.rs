use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const COPILOT_CSV_FILENAME: &str = "copilot-activity-history.csv";

/// Microsoft privacy export: `Conversation,Time,Author,Message` with `Human` / `AI` authors.
pub fn is_copilot_activity_csv(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_none_or(|ext| !ext.eq_ignore_ascii_case("csv"))
    {
        return false;
    }

    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let mut buf = [0_u8; 512];
    let Ok(read) = file.read(&mut buf) else {
        return false;
    };
    let text = String::from_utf8_lossy(&buf[..read]);
    let header = text.lines().next().unwrap_or("").trim_start_matches('\u{feff}');
    header_contains_copilot_columns(header)
}

pub fn header_contains_copilot_columns(header: &str) -> bool {
    let lower = header.to_ascii_lowercase();
    lower.contains("conversation")
        && lower.contains("time")
        && lower.contains("author")
        && lower.contains("message")
}

pub fn resolve_copilot_csv_path(path: &Path) -> Option<PathBuf> {
    if is_copilot_activity_csv(path) {
        return Some(path.to_path_buf());
    }

    if !path.is_dir() {
        return None;
    }

    let direct = path.join(COPILOT_CSV_FILENAME);
    if is_copilot_activity_csv(&direct) {
        return Some(direct);
    }

    let entries = fs::read_dir(path).ok()?;
    for entry in entries.flatten() {
        let candidate = entry.path();
        if is_copilot_activity_csv(&candidate) {
            return Some(candidate);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_csv_path() -> PathBuf {
        PathBuf::from(r"D:\Personal\CopilotApp\copilot-activity-history.csv")
    }

    #[test]
    fn detects_local_copilot_csv() {
        let path = sample_csv_path();
        if !path.is_file() {
            return;
        }
        assert!(is_copilot_activity_csv(&path));
        assert_eq!(
            resolve_copilot_csv_path(path.parent().unwrap()),
            Some(path.clone())
        );
    }

    #[test]
    fn recognizes_copilot_header() {
        assert!(header_contains_copilot_columns(
            "Conversation,Time,Author,Message"
        ));
        assert!(!header_contains_copilot_columns("id,title,body"));
    }
}
