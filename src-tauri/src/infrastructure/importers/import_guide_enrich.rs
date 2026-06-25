use std::path::PathBuf;

use crate::domain::ports::{ImportGuide, ImportMethodGuide};
use crate::infrastructure::importers::codex::{default_codex_home, resolve_codex_state_db};

fn cursor_default_vscdb() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .ok()
            .map(|appdata| {
                PathBuf::from(appdata)
                    .join("Cursor")
                    .join("User")
                    .join("globalStorage")
                    .join("state.vscdb")
            })
            .filter(|path| path.is_file())
    } else if cfg!(target_os = "macos") {
        std::env::var("HOME").ok().and_then(|home| {
            let path = PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("Cursor")
                .join("User")
                .join("globalStorage")
                .join("state.vscdb");
            path.is_file().then_some(path)
        })
    } else {
        std::env::var("HOME").ok().and_then(|home| {
            let path = PathBuf::from(home)
                .join(".config")
                .join("Cursor")
                .join("User")
                .join("globalStorage")
                .join("state.vscdb");
            path.is_file().then_some(path)
        })
    }
}

fn cursor_default_user_dir() -> Option<PathBuf> {
    cursor_default_vscdb().and_then(|db| {
        db.parent()
            .and_then(|global_storage| global_storage.parent())
            .map(PathBuf::from)
            .filter(|path| path.is_dir())
    })
}

fn attach_detected_default(
    method: &mut ImportMethodGuide,
    path: Option<PathBuf>,
    label: &str,
) {
    let Some(path) = path else {
        return;
    };
    if !path.exists() {
        return;
    }
    method.detected_default_path = Some(path.display().to_string());
    method.detected_default_label = Some(label.to_string());
}

pub fn enrich_import_guide(mut guide: ImportGuide) -> ImportGuide {
    match guide.importer_id.as_str() {
        "cursor" => {
            for method in &mut guide.methods {
                match method.id.as_str() {
                    "vscdb" => attach_detected_default(
                        method,
                        cursor_default_vscdb(),
                        "检测到本机默认 Cursor 数据库",
                    ),
                    "user_dir" => attach_detected_default(
                        method,
                        cursor_default_user_dir(),
                        "检测到本机默认 Cursor User 目录",
                    ),
                    _ => {}
                }
            }
        }
        "codex" => {
            let home = default_codex_home();
            let home_exists = home.is_dir() && resolve_codex_state_db(&home).is_some();
            let state_db = resolve_codex_state_db(&home);

            for method in &mut guide.methods {
                match method.id.as_str() {
                    "home_dir" if home_exists => {
                        attach_detected_default(
                            method,
                            Some(home.clone()),
                            "检测到本机默认 Codex 数据目录",
                        );
                    }
                    "state_db" => {
                        attach_detected_default(
                            method,
                            state_db.clone(),
                            "检测到本机默认 Codex 数据库",
                        );
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    guide
}
