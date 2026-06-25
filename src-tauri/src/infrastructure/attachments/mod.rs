use std::path::Path;

use crate::domain::ports::ImportedAttachment;
use crate::infrastructure::importers::chatgpt::classify_image_source;
use crate::infrastructure::media::MediaIndex;
use crate::models::AttachmentView;

/// Resolves import-time attachment pointers to local file paths via `MediaIndex`.
pub fn resolve_imported_attachments(
    attachments: &[ImportedAttachment],
    role: &str,
    media_index: &MediaIndex,
) -> Vec<ImportedAttachment> {
    attachments
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
        .collect()
}

pub fn imported_attachments_to_views(attachments: &[ImportedAttachment]) -> Vec<AttachmentView> {
    attachments
        .iter()
        .filter_map(|item| {
            item.path.as_ref().map(|path| AttachmentView {
                file_key: item.pointer.clone(),
                path: path.clone(),
                source: item.source.clone(),
                prompt: item.prompt.clone(),
            })
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
