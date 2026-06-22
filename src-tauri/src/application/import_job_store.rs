use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::domain::models::{ImportJob, ImportJobStatus, ImportProgress, SourceInfo};
use crate::models::{ImportJobView, ImportResult};

#[derive(Default)]
pub struct ImportJobStore {
    jobs: HashMap<String, ImportJobState>,
}

struct ImportJobState {
    job: ImportJob,
    result: Option<ImportResult>,
}

impl ImportJobStore {
    pub fn insert(&mut self, job: ImportJob) {
        let id = job.id.clone();
        self.jobs.insert(
            id,
            ImportJobState {
                job,
                result: None,
            },
        );
    }

    pub fn update_progress(&mut self, job_id: &str, progress: &ImportProgress) {
        if let Some(state) = self.jobs.get_mut(job_id) {
            state.job.apply_progress(progress);
        }
    }

    pub fn set_resolved_path(&mut self, job_id: &str, resolved_path: String) {
        if let Some(state) = self.jobs.get_mut(job_id) {
            state.job.resolved_path = Some(resolved_path);
        }
    }

    pub fn set_source_info(&mut self, job_id: &str, source_info: SourceInfo) {
        if let Some(state) = self.jobs.get_mut(job_id) {
            state.job.source_info = Some(source_info);
        }
    }

    pub fn mark_done(&mut self, job_id: &str, result: ImportResult) {
        if let Some(state) = self.jobs.get_mut(job_id) {
            state.job.status = ImportJobStatus::Done;
            state.job.progress = 1.0;
            state.job.phase = "done".to_string();
            state.job.error = None;
            state.result = Some(result);
        }
    }

    pub fn mark_failed(&mut self, job_id: &str, error: String) {
        if let Some(state) = self.jobs.get_mut(job_id) {
            state.job.status = ImportJobStatus::Failed;
            state.job.error = Some(error);
        }
    }

    pub fn get_job(&self, job_id: &str) -> Option<ImportJob> {
        self.jobs.get(job_id).map(|state| state.job.clone())
    }

    pub fn get_view(&self, job_id: &str) -> Option<ImportJobView> {
        self.jobs.get(job_id).map(|state| ImportJobView {
            id: state.job.id.clone(),
            source_path: state.job.source_path.clone(),
            resolved_path: state.job.resolved_path.clone(),
            status: state.job.status.as_str().to_string(),
            phase: state.job.phase.clone(),
            progress: state.job.progress,
            processed: state.job.processed,
            total: state.job.total,
            error: state.job.error.clone(),
            source: state
                .job
                .source_info
                .as_ref()
                .map(|info| info.source.as_str().to_string()),
            export_label: state
                .job
                .source_info
                .as_ref()
                .map(|info| info.export_label.clone()),
            importer_version: state
                .job
                .source_info
                .as_ref()
                .map(|info| info.importer_version.clone()),
            result: state.result.clone(),
        })
    }
}

pub type SharedImportJobStore = Arc<Mutex<ImportJobStore>>;

pub fn new_import_job_store() -> SharedImportJobStore {
    Arc::new(Mutex::new(ImportJobStore::default()))
}
