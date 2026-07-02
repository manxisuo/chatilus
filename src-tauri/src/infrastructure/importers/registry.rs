use std::path::{Path, PathBuf};

use crate::domain::ports::{ImportDetectResult, ImportGuide, ImportInput, Importer};

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

    pub fn detect_with_importer(
        &self,
        path: &Path,
        importer_id: &str,
        user_selected: bool,
    ) -> Result<ImportDetectResult, String> {
        let importer = self
            .by_id(importer_id)
            .ok_or_else(|| format!("未知数据源: {importer_id}"))?;
        let input = ImportInput {
            path: path.to_path_buf(),
        };
        let result = if user_selected {
            self.detect_user_selected_importer(importer, &input)?
        } else {
            importer.detect(&input)?
        };
        if result.matched {
            Ok(result)
        } else {
            Err(format!(
                "路径不符合 {} 的导入要求: {}",
                importer.display_name(),
                path.display()
            ))
        }
    }

    fn detect_user_selected_importer(
        &self,
        importer: &dyn Importer,
        input: &ImportInput,
    ) -> Result<ImportDetectResult, String> {
        use super::chatgpt::find_conversation_files;

        let matched = match importer.id() {
            "chatgpt" | "chatgpt-v1" => find_conversation_files(&input.path).is_ok(),
            _ => importer.detect(input)?.matched,
        };

        Ok(ImportDetectResult {
            matched,
            importer_id: importer.id().to_string(),
            display_name: importer.display_name().to_string(),
        })
    }

    pub fn find_export_root_for_importer(
        &self,
        search_root: &Path,
        importer_id: &str,
    ) -> Result<(PathBuf, ImportDetectResult), String> {
        if let Ok(result) = self.detect_with_importer(search_root, importer_id, true) {
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

                if let Ok(result) = self.detect_with_importer(&path, importer_id, true) {
                    return Ok((path, result));
                }

                queue.push(path);
            }
        }

        let importer = self
            .by_id(importer_id)
            .ok_or_else(|| format!("未知数据源: {importer_id}"))?;
        Err(format!(
            "在 {} 中未找到符合 {} 的导出数据",
            search_root.display(),
            importer.display_name()
        ))
    }

    pub fn list_import_guides(&self) -> Vec<ImportGuide> {
        use super::import_guide_enrich::enrich_import_guide;

        const ORDER: &[&str] = &[
            "chatgpt-v1",
            "chatgpt",
            "deepseek",
            "copilot",
            "grok",
            "cursor",
            "codex",
            "gemini",
        ];
        ORDER
            .iter()
            .filter_map(|id| {
                self.by_id(id)
                    .map(|importer| enrich_import_guide(importer.import_guide()))
            })
            .collect()
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
    use super::{
        ChatGptImporter, ChatGptV1Importer, CodexImporter, CopilotImporter, CursorImporter,
        DeepSeekImporter, GeminiImporter, GrokImporter,
    };

    ImporterRegistry::new(vec![
        Box::new(CopilotImporter::new()),
        Box::new(CodexImporter::new()),
        Box::new(GeminiImporter::new()),
        Box::new(GrokImporter::new()),
        Box::new(DeepSeekImporter::new()),
        Box::new(ChatGptV1Importer::new()),
        Box::new(ChatGptImporter::new()),
        Box::new(CursorImporter::new()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::importers::chatgpt::find_conversation_files;

    fn sample_export_dir() -> PathBuf {
        PathBuf::from(r"D:\Personal\ChatGPT数据下载\2026-05-16-12-08-35")
    }

    #[test]
    fn list_import_guides_returns_all_importers_in_order() {
        let registry = default_importer_registry();
        let guides = registry.list_import_guides();
        assert_eq!(
            guides.iter().map(|guide| guide.importer_id.as_str()).collect::<Vec<_>>(),
            vec![
                "chatgpt-v1",
                "chatgpt",
                "deepseek",
                "copilot",
                "grok",
                "cursor",
                "codex",
                "gemini"
            ]
        );
        assert!(guides.iter().all(|guide| !guide.methods.is_empty()));
        assert!(guides.iter().all(|guide| !guide.support_summary.is_empty()));
        assert!(guides.iter().all(|guide| !guide.recognition_hint.is_empty()));
    }

    #[test]
    fn registry_rejects_manifest_v1_as_legacy_chatgpt() {
        let dir = PathBuf::from(r"D:\Personal\ChatGPT数据下载\2026-06-26");
        if !dir.is_dir() {
            return;
        }

        let registry = default_importer_registry();
        let importer = registry.by_id("chatgpt").expect("chatgpt importer");
        let result = importer
            .detect(&ImportInput { path: dir })
            .expect("detect");
        assert!(!result.matched);
    }

    #[test]
    fn user_selected_legacy_importer_accepts_manifest_v1_export() {
        let dir = PathBuf::from(r"D:\Personal\ChatGPT数据下载\2026-06-26");
        if !dir.is_dir() {
            return;
        }

        let registry = default_importer_registry();
        let result = registry
            .detect_with_importer(&dir, "chatgpt", true)
            .expect("detect");
        assert!(result.matched);
        assert_eq!(result.importer_id, "chatgpt");
    }

    #[test]
    fn user_selected_v1_importer_accepts_legacy_style_export() {
        let dir = sample_export_dir();
        if !dir.is_dir() {
            return;
        }

        let registry = default_importer_registry();
        let result = registry
            .detect_with_importer(&dir, "chatgpt-v1", true)
            .expect("detect");
        assert!(result.matched);
        assert_eq!(result.importer_id, "chatgpt-v1");
    }

    #[test]
    fn registry_detects_manifest_v1_export() {
        let dir = PathBuf::from(r"D:\Personal\ChatGPT数据下载\2026-06-26");
        if !dir.is_dir() {
            return;
        }

        let registry = default_importer_registry();
        let result = registry.detect(&dir).expect("detect");
        assert_eq!(result.importer_id, "chatgpt-v1");
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
