use std::path::PathBuf;
use std::sync::Arc;

use super::import_guide::ImportGuide;
use super::import_package::ImportPackage;
use crate::domain::models::{DataSource, ImportProgress};

#[derive(Debug, Clone)]
pub struct ImportInput {
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportDetectResult {
    pub matched: bool,
    pub importer_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPreview {
    pub importer_id: String,
    pub display_name: String,
    pub conversation_count: usize,
    pub shard_file_count: usize,
}

#[derive(Clone, Default)]
pub struct ImportOptions {
    pub on_progress: Option<Arc<dyn Fn(ImportProgress) + Send + Sync>>,
}

#[derive(Debug, Clone)]
pub struct NormalizedImportResult {
    pub source: DataSource,
    pub source_path: String,
    pub files_processed: usize,
    pub media_files_indexed: usize,
    pub package: ImportPackage,
}

pub trait Importer: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn source(&self) -> DataSource;
    fn version(&self) -> &'static str;

    fn import_guide(&self) -> ImportGuide;

    fn detect(&self, input: &ImportInput) -> Result<ImportDetectResult, String>;
    fn preview(&self, input: &ImportInput) -> Result<ImportPreview, String>;
    fn import(
        &self,
        input: &ImportInput,
        options: &ImportOptions,
    ) -> Result<NormalizedImportResult, String>;
}
