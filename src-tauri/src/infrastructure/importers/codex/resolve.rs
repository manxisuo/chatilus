use std::path::{Path, PathBuf};

pub fn default_codex_home() -> PathBuf {
    std::env::var("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("USERPROFILE")
                .or_else(|_| std::env::var("HOME"))
                .map(|home| PathBuf::from(home).join(".codex"))
                .unwrap_or_else(|_| PathBuf::from(".codex"))
        })
}

pub fn resolve_codex_home(input: &Path) -> Option<PathBuf> {
    if let Some(home) = codex_home_from_state_db(input) {
        return Some(home);
    }

    let candidates = [input.to_path_buf(), input.join(".codex")];

    for candidate in candidates {
        if candidate.is_dir() && looks_like_codex_home(&candidate) {
            return Some(candidate);
        }
    }

    None
}

pub fn resolve_codex_state_db(input: &Path) -> Option<PathBuf> {
    if input.is_file() && is_codex_state_db(input) {
        return Some(input.to_path_buf());
    }

    let home = resolve_codex_home(input)?;
    find_state_db_in_dir(&home)
}

fn codex_home_from_state_db(path: &Path) -> Option<PathBuf> {
    if !path.is_file() || !is_codex_state_db(path) {
        return None;
    }
    let parent = path.parent()?;
    if parent.file_name().and_then(|name| name.to_str()) == Some("sqlite") {
        return parent.parent().map(Path::to_path_buf);
    }
    Some(parent.to_path_buf())
}

fn looks_like_codex_home(path: &Path) -> bool {
    path.join("sessions").is_dir()
        || find_state_db_in_dir(path).is_some()
        || path.join("config.toml").is_file()
}

fn find_state_db_in_dir(dir: &Path) -> Option<PathBuf> {
    for candidate in [dir.to_path_buf(), dir.join("sqlite")] {
        if let Some(path) = newest_state_db(&candidate) {
            return Some(path);
        }
    }
    None
}

fn newest_state_db(dir: &Path) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }

    let mut matches = Vec::new();
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path.file_name()?.to_str()?;
        if name.starts_with("state_") && name.ends_with(".sqlite") && is_codex_state_db(&path) {
            matches.push(path);
        }
    }

    matches.sort();
    matches.pop()
}

pub fn is_codex_state_db(path: &Path) -> bool {
    let uri = format!(
        "file:{}?mode=ro&immutable=1",
        path.to_string_lossy().replace('\\', "/")
    );
    let Ok(conn) = rusqlite::Connection::open_with_flags(
        uri,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    ) else {
        return false;
    };

    let Ok(_) = conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'threads' LIMIT 1",
        [],
        |row| row.get::<_, i32>(0),
    ) else {
        return false;
    };

    conn.query_row(
        "SELECT rollout_path FROM threads WHERE rollout_path IS NOT NULL AND rollout_path != '' LIMIT 1",
        [],
        |row| row.get::<_, String>(0),
    )
    .is_ok()
}

pub fn normalize_windows_path(path: &str) -> String {
    let trimmed = path.trim();
    let stripped = trimmed
        .strip_prefix(r"\\?\")
        .or_else(|| trimmed.strip_prefix(r"\\?\"))
        .unwrap_or(trimmed);
    stripped.replace('/', "\\")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_extended_windows_path_prefix() {
        assert_eq!(
            normalize_windows_path(r"\\?\D:\Code\ChatLens"),
            r"D:\Code\ChatLens"
        );
    }

    #[test]
    fn detects_local_codex_home_if_present() {
        let home = default_codex_home();
        if !home.is_dir() {
            return;
        }
        assert!(resolve_codex_home(&home).is_some());
        assert!(resolve_codex_state_db(&home).is_some());
    }
}
