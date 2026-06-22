use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Emitter};

use crate::application::{import_job_store::SharedImportJobStore, run_import_resolved};
use crate::db::Database;
use crate::domain::models::{ImportJob, ImportProgress, SourceInfo};
use crate::infrastructure::archive::resolve_import_path;
use crate::infrastructure::importers::default_importer_registry;
use crate::models::{ImportJobView, ImportProgressEvent};

pub fn spawn_import_job(
    app: AppHandle,
    store: SharedImportJobStore,
    db_path: PathBuf,
    job: ImportJob,
) {
    let job_id = job.id.clone();
    let source_path = job.source_path.clone();

    {
        let mut registry = store
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        registry.insert(job);
    }

    std::thread::spawn(move || {
        let progress_store = store.clone();
        let progress_app = app.clone();
        let progress_job_id = job_id.clone();
        let progress = Arc::new(move |update: ImportProgress| {
            if let Ok(mut registry) = progress_store.lock() {
                registry.update_progress(&progress_job_id, &update);
            }
            let _ = progress_app.emit(
                "import-progress",
                ImportProgressEvent {
                    job_id: progress_job_id.clone(),
                    phase: update.phase.clone(),
                    progress: update.progress,
                    processed: update.processed,
                    total: update.total,
                },
            );
        });

        let outcome = (|| {
            let resolved = resolve_import_path(
                std::path::Path::new(&source_path),
                Some(progress.clone()),
            )?;

            let importer_registry = default_importer_registry();
            let importer = importer_registry
                .by_id(&resolved.importer_id)
                .ok_or_else(|| format!("未找到 Importer: {}", resolved.importer_id))?;

            if let Ok(mut registry) = store.lock() {
                registry.set_resolved_path(
                    &job_id,
                    resolved.export_dir.display().to_string(),
                );
                registry.set_source_info(
                    &job_id,
                    SourceInfo::new(
                        importer.source(),
                        resolved.export_label.clone(),
                        importer.version(),
                    ),
                );
            }

            let mut db = Database::open(&db_path)?;
            run_import_resolved(&mut db, resolved, Some(progress.clone()))
        })();

        let mut registry = store
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        match outcome {
            Ok(result) => {
                registry.mark_done(&job_id, result.clone());
                if let Some(job) = registry.get_job(&job_id) {
                    let _ = Database::open(&db_path).and_then(|db| db.persist_import_job(&job));
                }
                let _ = app.emit(
                    "import-complete",
                    registry
                        .get_view(&job_id)
                        .unwrap_or_else(|| ImportJobView::failed(&job_id, &source_path, "unknown")),
                );
            }
            Err(error) => {
                registry.mark_failed(&job_id, error.clone());
                if let Some(job) = registry.get_job(&job_id) {
                    let _ = Database::open(&db_path).and_then(|db| db.persist_import_job(&job));
                }
                let _ = app.emit(
                    "import-complete",
                    registry.get_view(&job_id).unwrap_or_else(|| {
                        ImportJobView::failed(&job_id, &source_path, error.as_str())
                    }),
                );
            }
        }
    });
}
