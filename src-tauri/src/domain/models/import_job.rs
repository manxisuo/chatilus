use serde::{Deserialize, Serialize};

use super::SourceInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportJobStatus {
    Pending,
    Extracting,
    Importing,
    Persisting,
    Done,
    Failed,
}

impl ImportJobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Extracting => "extracting",
            Self::Importing => "importing",
            Self::Persisting => "persisting",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportProgress {
    pub phase: String,
    pub progress: f64,
    pub processed: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportJob {
    pub id: String,
    pub source_path: String,
    /// 用户通过导入向导指定的 Importer；为空时回退为自动检测。
    pub importer_id: Option<String>,
    pub resolved_path: Option<String>,
    pub status: ImportJobStatus,
    pub phase: String,
    pub progress: f64,
    pub processed: usize,
    pub total: usize,
    pub error: Option<String>,
    pub source_info: Option<SourceInfo>,
}

impl ImportJob {
    pub fn new(id: impl Into<String>, source_path: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            source_path: source_path.into(),
            importer_id: None,
            resolved_path: None,
            status: ImportJobStatus::Pending,
            phase: String::new(),
            progress: 0.0,
            processed: 0,
            total: 0,
            error: None,
            source_info: None,
        }
    }

    pub fn apply_progress(&mut self, update: &ImportProgress) {
        self.phase = update.phase.clone();
        self.progress = update.progress.clamp(0.0, 1.0);
        self.processed = update.processed;
        self.total = update.total;
        self.status = match update.phase.as_str() {
            "extracting" => ImportJobStatus::Extracting,
            "parsing" | "importing" => ImportJobStatus::Importing,
            "persisting" => ImportJobStatus::Persisting,
            _ => self.status,
        };
    }
}
