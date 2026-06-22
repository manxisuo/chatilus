use std::path::Path;
use std::sync::Arc;

use crate::db::Database;
use crate::domain::models::{DataSource, ImportProgress, SourceInfo};
use crate::domain::ports::{ImportInput, ImportOptions, Importer};
use crate::infrastructure::archive::{chatgpt_importer_version, resolve_import_path, ResolvedImportPath};
use crate::infrastructure::importers::ChatGptImporter;
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
    let _cleanup = resolved.cleanup;
    let source_info = SourceInfo::new(
        DataSource::ChatGpt,
        resolved.export_label,
        chatgpt_importer_version(),
    );

    let importer = ChatGptImporter::new();
    let normalized = importer.import(
        &ImportInput {
            path: resolved.export_dir,
        },
        &ImportOptions {
            on_progress: on_progress.clone(),
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
