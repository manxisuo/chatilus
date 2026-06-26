use std::collections::HashSet;
use std::path::Path;

use crate::domain::ports::ImportedAttachment;
use crate::infrastructure::importers::chatgpt::classify_image_source;
use crate::infrastructure::media::{normalize_file_key, MediaIndex};
use crate::models::AttachmentView;

/// Resolves import-time attachment pointers to local file paths via `MediaIndex`.
pub fn resolve_imported_attachments(
    attachments: &[ImportedAttachment],
    role: &str,
    media_index: &MediaIndex,
) -> Vec<ImportedAttachment> {
    let resolved = attachments
        .iter()
        .filter_map(|item| {
            if let Some(path) = item.path.as_ref().filter(|path| Path::new(path).is_file()) {
                let source = attachment_source(role, path, &item.source);
                return Some(ImportedAttachment {
                    pointer: item.pointer.clone(),
                    source,
                    prompt: item.prompt.clone(),
                    path: Some(path.clone()),
                });
            }

            media_index.resolve(&item.pointer).map(|path| {
                let path_str = path.display().to_string();
                ImportedAttachment {
                    pointer: item.pointer.clone(),
                    source: attachment_source(role, &path_str, &item.source),
                    prompt: item.prompt.clone(),
                    path: Some(path_str),
                }
            })
        })
        .collect();
    dedupe_resolved_attachments(resolved)
}

pub fn imported_attachments_to_views(attachments: &[ImportedAttachment]) -> Vec<AttachmentView> {
    let mut seen_paths = HashSet::new();
    attachments
        .iter()
        .filter_map(|item| {
            item.path.as_ref().map(|path| AttachmentView {
                file_key: normalize_file_key(&item.pointer),
                path: path.clone(),
                source: item.source.clone(),
                prompt: item.prompt.clone(),
            })
        })
        .filter(|view| seen_paths.insert(view.path.to_ascii_lowercase()))
        .collect()
}

fn dedupe_resolved_attachments(attachments: Vec<ImportedAttachment>) -> Vec<ImportedAttachment> {
    let mut seen_keys = HashSet::new();
    let mut seen_paths = HashSet::new();
    attachments
        .into_iter()
        .filter(|item| {
            if !seen_keys.insert(normalize_file_key(&item.pointer)) {
                return false;
            }
            if let Some(path) = item.path.as_ref() {
                if !seen_paths.insert(path.to_ascii_lowercase()) {
                    return false;
                }
            }
            true
        })
        .collect()
}

fn attachment_source(role: &str, path: &str, stored: &str) -> String {
    if stored == "unknown" {
        classify_image_source(None, role, Some(path))
    } else if path.contains("dalle-generations") {
        "generated".to_string()
    } else {
        stored.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedupes_same_asset_with_different_pointer_schemes() {
        let path = r"C:\export\file-abc.dat";
        let views = imported_attachments_to_views(&[
            ImportedAttachment {
                pointer: "file-service://file-abc".to_string(),
                source: "upload".to_string(),
                prompt: None,
                path: Some(path.to_string()),
            },
            ImportedAttachment {
                pointer: "file-abc".to_string(),
                source: "upload".to_string(),
                prompt: None,
                path: Some(path.to_string()),
            },
        ]);
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].file_key, "file-abc");
    }

    #[test]
    fn skips_unresolved_pointers() {
        let index = MediaIndex::build(std::path::Path::new("."));
        let resolved = resolve_imported_attachments(
            &[ImportedAttachment {
                pointer: "file-missing".to_string(),
                source: "unknown".to_string(),
                prompt: None,
                path: None,
            }],
            "user",
            &index,
        );
        assert!(resolved.is_empty());
    }
}
