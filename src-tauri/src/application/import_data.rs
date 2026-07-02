use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::db::Database;
use crate::domain::models::{ImportProgress, SourceInfo};
use crate::domain::ports::{ImportInput, ImportOptions};
use crate::infrastructure::archive::resolve_import_path;
use crate::infrastructure::archive::ResolvedImportPath;
use crate::infrastructure::importers::default_importer_registry;
use crate::models::ImportResult;

pub fn import_export_dir(db: &mut Database, source_path: &Path) -> Result<ImportResult, String> {
    let resolved = resolve_import_path(source_path, None)?;
    run_import_resolved(db, resolved, None)
}

pub fn run_import(
    db: &mut Database,
    input_path: &Path,
    on_progress: Option<Arc<dyn Fn(ImportProgress) + Send + Sync>>,
) -> Result<ImportResult, String> {
    let resolved = resolve_import_path(input_path, on_progress.clone())?;
    run_import_resolved(db, resolved, on_progress)
}

pub fn run_import_resolved(
    db: &mut Database,
    resolved: ResolvedImportPath,
    on_progress: Option<Arc<dyn Fn(ImportProgress) + Send + Sync>>,
) -> Result<ImportResult, String> {
    let export_dir = if resolved.cleanup.is_some() {
        persist_export_dir(&db.path, &resolved.export_label, &resolved.export_dir)?
    } else {
        resolved.export_dir.clone()
    };
    let _cleanup = resolved.cleanup;

    let registry = default_importer_registry();
    let importer = registry
        .by_id(&resolved.importer_id)
        .ok_or_else(|| format!("未找到 Importer: {}", resolved.importer_id))?;

    let source_info = SourceInfo::new(
        importer.source(),
        resolved.export_label,
        importer.version(),
    );

    let normalized = importer.import(
        &ImportInput { path: export_dir },
        &ImportOptions {
            on_progress: on_progress.clone(),
            user_selected: resolved.user_selected,
        },
    )?;

    if let Some(callback) = &on_progress {
        let total = normalized.package.conversations.len().max(1);
        callback(ImportProgress {
            phase: "persisting".to_string(),
            progress: 0.85,
            processed: 0,
            total,
        });
    }

    let result = db.persist_import(&normalized, &source_info)?;

    if let Some(callback) = &on_progress {
        let total = normalized.package.conversations.len().max(1);
        callback(ImportProgress {
            phase: "done".to_string(),
            progress: 1.0,
            processed: total,
            total,
        });
    }

    Ok(result)
}

fn imports_cache_root(db_path: &str) -> PathBuf {
    Path::new(db_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("imports")
}

fn persist_export_dir(
    db_path: &str,
    export_label: &str,
    source_dir: &Path,
) -> Result<PathBuf, String> {
    let safe_label = export_label
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let dest = imports_cache_root(db_path).join(safe_label);
    if dest.exists() {
        fs::remove_dir_all(&dest)
            .map_err(|e| format!("清理旧导入缓存失败 ({}): {e}", dest.display()))?;
    }
    copy_dir_recursive(source_dir, &dest)?;
    Ok(dest)
}

fn copy_dir_recursive(source: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest)
        .map_err(|e| format!("创建导入缓存目录失败 ({}): {e}", dest.display()))?;

    for entry in fs::read_dir(source)
        .map_err(|e| format!("读取导出目录失败 ({}): {e}", source.display()))?
    {
        let entry = entry.map_err(|e| format!("读取导出目录项失败: {e}"))?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("创建导入缓存目录失败 ({}): {e}", parent.display()))?;
            }
            fs::copy(&from, &to).map_err(|e| {
                format!(
                    "复制导出文件失败 ({} -> {}): {e}",
                    from.display(),
                    to.display()
                )
            })?;
        }
    }

    Ok(())
}
