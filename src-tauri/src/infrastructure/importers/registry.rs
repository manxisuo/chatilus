use std::path::{Path, PathBuf};

use crate::domain::ports::{ImportDetectResult, ImportInput, Importer};

pub struct ImporterRegistry {
    importers: Vec<Box<dyn Importer>>,
}

impl ImporterRegistry {
    pub fn new(importers: Vec<Box<dyn Importer>>) -> Self {
        Self { importers }
    }

    pub fn detect(&self, path: &Path) -> Result<ImportDetectResult, String> {
        let input = ImportInput {
            path: path.to_path_buf(),
        };

        for importer in &self.importers {
            let result = importer.detect(&input)?;
            if result.matched {
                return Ok(result);
            }
        }

        Err(format!(
            "未识别支持的导出格式: {}",
            path.display()
        ))
    }

    pub fn find_export_root(&self, search_root: &Path) -> Result<(PathBuf, ImportDetectResult), String> {
        if let Ok(result) = self.detect(search_root) {
            return Ok((search_root.to_path_buf(), result));
        }

        let mut queue = vec![search_root.to_path_buf()];
        while let Some(dir) = queue.pop() {
            let entries =
                std::fs::read_dir(&dir).map_err(|e| format!("无法读取目录 {}: {e}", dir.display()))?;

            for entry in entries {
                let entry = entry.map_err(|e| format!("读取目录项失败: {e}"))?;
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                if let Ok(result) = self.detect(&path) {
                    return Ok((path, result));
                }

                queue.push(path);
            }
        }

        Err(format!(
            "在 {} 中未找到可识别的导出数据",
            search_root.display()
        ))
    }

    pub fn by_id(&self, importer_id: &str) -> Option<&dyn Importer> {
        self.importers
            .iter()
            .find(|importer| importer.id() == importer_id)
            .map(|importer| importer.as_ref())
    }

    pub fn importer_ids(&self) -> Vec<&'static str> {
        self.importers.iter().map(|importer| importer.id()).collect()
    }
}

pub fn default_importer_registry() -> ImporterRegistry {
    use super::{ChatGptImporter, CodexImporter, CursorImporter, GeminiImporter};

    ImporterRegistry::new(vec![
        Box::new(CodexImporter::new()),
        Box::new(GeminiImporter::new()),
        Box::new(ChatGptImporter::new()),
        Box::new(CursorImporter::new()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::importers::chatgpt::find_conversation_files;

    fn sample_export_dir() -> PathBuf {
        PathBuf::from(r"D:\Personal\ChatGPT数据下载-2026年5月18日\2026-05-16-12-08-35")
    }

    #[test]
    fn registry_detects_chatgpt_export() {
        let dir = sample_export_dir();
        if !dir.is_dir() {
            return;
        }

        let registry = default_importer_registry();
        let result = registry.detect(&dir).expect("detect");
        assert!(result.matched);
        assert_eq!(result.importer_id, "chatgpt");
    }

    #[test]
    fn registry_finds_nested_export_root() {
        let registry = default_importer_registry();
        let base = std::env::temp_dir().join(format!(
            "chatlens-registry-root-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("nested/export")).expect("mkdir");
        std::fs::write(
            base.join("nested/export/conversations-000.json"),
            r#"[{"id":"c1","title":"Test","mapping":{},"current_node":null}]"#,
        )
        .expect("write json");

        let (export_root, result) = registry
            .find_export_root(&base)
            .expect("find export root");
        assert_eq!(result.importer_id, "chatgpt");
        assert!(find_conversation_files(&export_root).is_ok());
    }

    #[test]
    fn registry_rejects_unknown_directory() {
        let base = std::env::temp_dir().join(format!(
            "chatlens-registry-empty-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("mkdir");

        let registry = default_importer_registry();
        assert!(registry.detect(&base).is_err());
    }
}
