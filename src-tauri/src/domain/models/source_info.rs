use serde::{Deserialize, Serialize};

use super::DataSource;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceInfo {
    pub source: DataSource,
    pub export_label: String,
    pub importer_version: String,
}

impl SourceInfo {
    pub fn new(source: DataSource, export_label: impl Into<String>, importer_version: impl Into<String>) -> Self {
        Self {
            source,
            export_label: export_label.into(),
            importer_version: importer_version.into(),
        }
    }
}
