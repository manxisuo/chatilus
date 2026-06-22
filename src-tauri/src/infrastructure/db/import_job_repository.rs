use rusqlite::params;

use crate::domain::models::{ImportJob, ImportJobStatus, SourceInfo};

use super::Database;

impl Database {
    pub fn persist_import_job(&self, job: &ImportJob) -> Result<(), String> {
        let (source, export_label, importer_version) = job
            .source_info
            .as_ref()
            .map(|info| {
                (
                    Some(info.source.as_str().to_string()),
                    Some(info.export_label.clone()),
                    Some(info.importer_version.clone()),
                )
            })
            .unwrap_or((None, None, None));

        let finished_at: Option<f64> = if matches!(job.status, ImportJobStatus::Done | ImportJobStatus::Failed) {
            Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|duration| duration.as_secs_f64())
                    .unwrap_or(0.0),
            )
        } else {
            None
        };

        self.conn
            .execute(
                "INSERT INTO import_jobs (
                    id, source_path, resolved_path, status, phase, progress,
                    processed, total, error, source, export_label, importer_version,
                    created_at, finished_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, unixepoch('subsec'), ?13)",
                params![
                    job.id,
                    job.source_path,
                    job.resolved_path,
                    job.status.as_str(),
                    job.phase,
                    job.progress,
                    job.processed as i64,
                    job.total as i64,
                    job.error,
                    source,
                    export_label,
                    importer_version,
                    finished_at,
                ],
            )
            .map_err(|e| format!("保存 import_job 失败: {e}"))?;
        Ok(())
    }

    pub fn source_info_sql_values(source_info: &SourceInfo) -> (String, String, String) {
        (
            source_info.source.as_str().to_string(),
            source_info.export_label.clone(),
            source_info.importer_version.clone(),
        )
    }
}
