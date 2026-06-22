use std::path::Path;

use crate::db::Database;
use crate::domain::ports::{ImportInput, ImportOptions, Importer};
use crate::infrastructure::importers::ChatGptImporter;
use crate::models::ImportResult;

pub fn import_export_dir(db: &mut Database, source_path: &Path) -> Result<ImportResult, String> {
    let importer = ChatGptImporter::new();
    let normalized = importer.import(
        &ImportInput {
            path: source_path.to_path_buf(),
        },
        &ImportOptions::default(),
    )?;
    db.persist_import(&normalized)
}
