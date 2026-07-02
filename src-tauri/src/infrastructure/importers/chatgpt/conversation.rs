use serde_json::Value;

use crate::domain::ports::{ImportedAttachment, ImportedConversation, ImportedMessage, ImportedSourceContext};
use crate::infrastructure::media::normalize_file_key;

use super::projects::{is_chatgpt_project_id, resolve_project_name, ProjectNameIndex};

pub use crate::domain::ports::{
    ImportedAttachment as ParsedAttachment, ImportedConversation as ParsedConversation,
    ImportedMessage as ParsedMessage,
};

pub fn extract_pointers_from_message_json(raw_json: &str) -> Vec<String> {
    extract_attachment_infos_from_message_json(raw_json)
        .into_iter()
        .map(|item| item.pointer)
        .collect()
}

pub fn extract_attachment_infos_from_message_json(raw_json: &str) -> Vec<ImportedAttachment> {
    let Ok(message) = serde_json::from_str::<Value>(raw_json) else {
        return Vec::new();
    };

    let role = message
        .get("author")
        .and_then(|v| v.get("role"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let mut infos = Vec::new();
    if let Some(content) = message.get("content") {
        infos.extend(extract_attachment_infos_from_content(content, role));
    }
    for pointer in extract_message_attachments(&message) {
        if attachment_pointer_seen(&infos, &pointer) {
            continue;
        }
        infos.push(ImportedAttachment {
            pointer,
            source: infer_source_from_role(role, None),
            prompt: None,
            path: None,
        });
    }
    dedupe_imported_attachments(&mut infos);
    infos
}

pub fn classify_image_source(metadata: Option<&Value>, role: &str, path: Option<&str>) -> String {
    if let Some(meta) = metadata {
        if meta.get("dalle").is_some_and(|v| !v.is_null()) {
            return "generated".to_string();
        }
        if meta.get("generation").is_some_and(|v| !v.is_null()) {
            return "generated".to_string();
        }
    }
    if path.is_some_and(|p| p.contains("dalle-generations")) {
        return "generated".to_string();
    }
    infer_source_from_role(role, path)
}

fn infer_source_from_role(role: &str, path: Option<&str>) -> String {
    if path.is_some_and(|p| p.contains("dalle-generations")) {
        return "generated".to_string();
    }
    match role {
        "user" => "upload".to_string(),
        "tool" => "generated".to_string(),
        _ => "unknown".to_string(),
    }
}

fn extract_dalle_prompt(metadata: Option<&Value>) -> Option<String> {
    metadata?
        .get("dalle")?
        .get("prompt")?
        .as_str()
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

pub fn parse_conversation(
    value: &Value,
    project_names: &ProjectNameIndex,
) -> Option<ImportedConversation> {
    let id = value
        .get("id")
        .or_else(|| value.get("conversation_id"))
        .and_then(|v| v.as_str())
        .map(str::to_string)?;

    let title = value
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("未命名对话")
        .trim()
        .to_string();

    let title = if title.is_empty() {
        "未命名对话".to_string()
    } else {
        title
    };

    let create_time = value.get("create_time").and_then(|v| v.as_f64());
    let update_time = value.get("update_time").and_then(|v| v.as_f64());
    let model = value
        .get("default_model_slug")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let mapping = value.get("mapping")?.as_object()?;
    let current_node = value.get("current_node").and_then(|v| v.as_str())?;

    let messages = linearize_messages(mapping, current_node);
    let source_contexts = extract_chatgpt_source_contexts(value, project_names);

    Some(ImportedConversation {
        id,
        title,
        create_time,
        update_time,
        model,
        messages,
        source_contexts,
    })
}

fn extract_chatgpt_source_contexts(
    value: &Value,
    project_names: &ProjectNameIndex,
) -> Vec<ImportedSourceContext> {
    let mut contexts = Vec::new();

    if let Some(context) = project_context_from_value(value, project_names) {
        contexts.push(context);
    }

    if let Some(template_id) = value
        .get("conversation_template_id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        if is_chatgpt_project_id(template_id)
            && contexts
                .iter()
                .all(|item| item.external_id.as_deref() != Some(template_id))
        {
            contexts.push(project_context_from_id(
                template_id,
                project_names,
                Some(value),
            ));
        }
    }

    if let Some(gizmo_id) = value
        .get("gizmo_id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        if is_chatgpt_project_id(gizmo_id)
            && contexts
                .iter()
                .all(|item| item.external_id.as_deref() != Some(gizmo_id))
        {
            contexts.push(project_context_from_id(gizmo_id, project_names, Some(value)));
        }
    }

    if let Some(metadata) = value.get("metadata") {
        if let Some(context) = project_context_from_value(metadata, project_names) {
            if contexts
                .iter()
                .all(|item| item.external_id != context.external_id)
            {
                contexts.push(context);
            }
        }
    }

    contexts
}

fn project_context_from_value(
    value: &Value,
    project_names: &ProjectNameIndex,
) -> Option<ImportedSourceContext> {
    let project_id = value
        .get("project_id")
        .or_else(|| value.get("projectId"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string);

    let nested = value.get("project").and_then(|project| {
        let id = project
            .get("id")
            .or_else(|| project.get("project_id"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let name = project
            .get("name")
            .or_else(|| project.get("title"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string);
        Some((id, name))
    });

    let external_id = project_id
        .or_else(|| nested.as_ref().and_then(|(id, _)| id.clone()))?;

    let name = value
        .get("project_name")
        .or_else(|| value.get("projectName"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .or_else(|| nested.and_then(|(_, name)| name))
        .or_else(|| project_names.get(&external_id).cloned())
        .unwrap_or_else(|| resolve_project_name(&external_id, project_names));

    let raw_json = serde_json::to_string(value).ok();

    Some(ImportedSourceContext {
        context_type: "project".to_string(),
        external_id: Some(external_id),
        name,
        path: None,
        raw_json,
    })
}

fn project_context_from_id(
    project_id: &str,
    project_names: &ProjectNameIndex,
    raw: Option<&Value>,
) -> ImportedSourceContext {
    ImportedSourceContext {
        context_type: "project".to_string(),
        external_id: Some(project_id.to_string()),
        name: resolve_project_name(project_id, project_names),
        path: None,
        raw_json: raw.and_then(|value| serde_json::to_string(value).ok()),
    }
}

fn linearize_messages(
    mapping: &serde_json::Map<String, Value>,
    current_node: &str,
) -> Vec<ImportedMessage> {
    let mut chain = Vec::new();
    let mut node_id = Some(current_node.to_string());

    while let Some(id) = node_id {
        let Some(node) = mapping.get(&id) else {
            break;
        };

        if let Some(message) = node.get("message") {
            if let Some(parsed) = parse_message(message) {
                if should_include(
                    &parsed.role,
                    &parsed.content,
                    parsed.attachments.len(),
                ) {
                    chain.push(parsed);
                }
            }
        }

        node_id = node
            .get("parent")
            .and_then(|v| v.as_str())
            .map(str::to_string);
    }

    chain.reverse();
    chain
}

fn should_include(role: &str, content: &str, attachment_count: usize) -> bool {
    if role == "system" {
        return false;
    }
    if role == "tool" && content.trim().is_empty() && attachment_count == 0 {
        return false;
    }
    !(content.trim().is_empty() && attachment_count == 0)
}

fn parse_message(message: &Value) -> Option<ImportedMessage> {
    let id = message.get("id").and_then(|v| v.as_str())?.to_string();
    let role = message
        .get("author")
        .and_then(|v| v.get("role"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let content_value = message.get("content")?;
    if is_internal_message(content_value, message) {
        return None;
    }
    let content = extract_content(content_value);
    let create_time = message.get("create_time").and_then(|v| v.as_f64());
    let raw_json = message.to_string();
    let mut attachments = extract_attachment_infos_from_content(content_value, &role);
    for pointer in extract_message_attachments(message) {
        if attachment_pointer_seen(&attachments, &pointer) {
            continue;
        }
        attachments.push(ImportedAttachment {
            pointer,
            source: infer_source_from_role(&role, None),
            prompt: None,
            path: None,
        });
    }
    dedupe_imported_attachments(&mut attachments);

    Some(ImportedMessage {
        id,
        role,
        content,
        create_time,
        raw_json,
        attachments,
    })
}

fn extract_content(content: &Value) -> String {
    let content_type = content
        .get("content_type")
        .and_then(|v| v.as_str())
        .unwrap_or("text");

    match content_type {
        "text" | "code" | "thoughts" | "reasoning_recap" => content
            .get("parts")
            .and_then(|v| v.as_array())
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(|part| match part {
                        Value::String(text) => Some(text.clone()),
                        Value::Object(obj) => obj
                            .get("text")
                            .and_then(|v| v.as_str())
                            .map(str::to_string),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default(),
        "multimodal_text" => content
            .get("parts")
            .and_then(|v| v.as_array())
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(|part| match part {
                        Value::String(text) => Some(text.clone()),
                        Value::Object(obj) => {
                            let part_type = obj
                                .get("content_type")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default();
                            if part_type == "image_asset_pointer" {
                                None
                            } else if part_type == "text" {
                                obj.get("text")
                                    .or_else(|| {
                                        obj.get("parts").and_then(|p| p.as_array()?.first())
                                    })
                                    .and_then(|v| v.as_str())
                                    .map(str::to_string)
                            } else {
                                None
                            }
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default(),
        "image_asset_pointer" => String::new(),
        "user_editable_context" => String::new(),
        other => format!("[{other}]"),
    }
}

fn is_internal_message(content: &Value, message: &Value) -> bool {
    let content_type = content
        .get("content_type")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if content_type == "user_editable_context" {
        return true;
    }
    message
        .get("metadata")
        .and_then(|v| v.get("is_user_system_message"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn extract_attachment_infos_from_content(content: &Value, role: &str) -> Vec<ImportedAttachment> {
    let mut infos = Vec::new();
    let Some(parts) = content.get("parts").and_then(|v| v.as_array()) else {
        return infos;
    };

    for part in parts {
        let Value::Object(obj) = part else {
            continue;
        };
        let content_type = obj
            .get("content_type")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if content_type == "image_asset_pointer" {
            if let Some(pointer) = obj.get("asset_pointer").and_then(|v| v.as_str()) {
                let metadata = obj.get("metadata");
                infos.push(ImportedAttachment {
                    pointer: pointer.to_string(),
                    source: classify_image_source(metadata, role, None),
                    prompt: extract_dalle_prompt(metadata),
                    path: None,
                });
            }
        }
    }

    infos
}

fn extract_message_attachments(message: &Value) -> Vec<String> {
    let mut pointers = Vec::new();

    if let Some(items) = message.get("attachments").and_then(|v| v.as_array()) {
        for item in items {
            collect_image_attachment_ids(item, &mut pointers);
        }
    }

    if let Some(metadata) = message.get("metadata").and_then(|v| v.as_object()) {
        if let Some(items) = metadata.get("attachments").and_then(|v| v.as_array()) {
            for item in items {
                collect_image_attachment_ids(item, &mut pointers);
            }
        }
    }

    pointers
}

fn attachment_pointer_seen(attachments: &[ImportedAttachment], pointer: &str) -> bool {
    let key = normalize_file_key(pointer);
    attachments
        .iter()
        .any(|item| normalize_file_key(&item.pointer) == key)
}

fn dedupe_imported_attachments(attachments: &mut Vec<ImportedAttachment>) {
    attachments.sort_by(|a, b| {
        normalize_file_key(&a.pointer).cmp(&normalize_file_key(&b.pointer))
    });
    attachments.dedup_by(|a, b| {
        normalize_file_key(&a.pointer) == normalize_file_key(&b.pointer)
    });
}

fn collect_image_attachment_ids(value: &Value, out: &mut Vec<String>) {
    if !is_image_attachment_item(value) {
        return;
    }
    collect_attachment_ids(value, out);
}

fn is_image_attachment_item(value: &Value) -> bool {
    match value {
        Value::String(_) => true,
        Value::Object(obj) => attachment_object_is_image(obj),
        _ => false,
    }
}

fn attachment_object_is_image(obj: &serde_json::Map<String, Value>) -> bool {
    if let Some(mime) = obj.get("mime_type").and_then(|v| v.as_str()) {
        return mime.starts_with("image/");
    }

    if obj.get("width").is_some() && obj.get("height").is_some() {
        return true;
    }

    if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
        return is_image_filename(name);
    }

    if obj.contains_key("asset_pointer") {
        return true;
    }

    // Legacy bare file id without filename metadata.
    obj.contains_key("id") || obj.contains_key("file_id")
}

fn is_image_filename(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [
        ".jpg", ".jpeg", ".png", ".gif", ".webp", ".bmp", ".svg", ".heic", ".heif", ".avif",
    ]
    .iter()
    .any(|ext| lower.ends_with(ext))
}

fn collect_attachment_ids(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(text) => out.push(text.clone()),
        Value::Object(obj) => {
            for key in ["asset_pointer", "id", "file_id", "file-service"] {
                if let Some(text) = obj.get(key).and_then(|v| v.as_str()) {
                    out.push(text.to_string());
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn skips_non_image_message_attachments() {
        let raw = r#"{
            "id": "m1",
            "author": { "role": "user" },
            "content": {
                "content_type": "text",
                "parts": ["see files"]
            },
            "metadata": {
                "attachments": [
                    {
                        "id": "file_0000000046987207a608956d303f5357",
                        "mime_type": "image/png",
                        "name": "image.png",
                        "width": 2048,
                        "height": 1206
                    },
                    {
                        "id": "file_00000000ab087207beaa4fa7ab9be5b2",
                        "name": "ROADMAP.md",
                        "size": 13487
                    },
                    {
                        "id": "file_00000000e9fc7207984ae8333d0684cd",
                        "name": "ARCHITECTURE.md",
                        "size": 14744
                    }
                ]
            },
            "create_time": 1.0
        }"#;
        let infos = extract_attachment_infos_from_message_json(raw);
        assert_eq!(infos.len(), 1);
        assert_eq!(
            infos[0].pointer,
            "file_0000000046987207a608956d303f5357"
        );
    }

    #[test]
    fn keeps_legacy_bare_attachment_id() {
        let raw = r#"{
            "id": "m1",
            "author": { "role": "user" },
            "content": { "content_type": "text", "parts": ["x"] },
            "metadata": {
                "attachments": [{ "id": "file-abc123" }]
            },
            "create_time": 1.0
        }"#;
        let infos = extract_attachment_infos_from_message_json(raw);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].pointer, "file-abc123");
    }

    #[test]
    fn dedupes_duplicate_attachment_pointers() {
        let raw = r#"{
            "id": "m1",
            "author": { "role": "user" },
            "content": {
                "content_type": "multimodal_text",
                "parts": [{
                    "content_type": "image_asset_pointer",
                    "asset_pointer": "file-service://file-abc123"
                }]
            },
            "metadata": {
                "attachments": [{ "id": "file-abc123" }]
            },
            "create_time": 1.0
        }"#;
        let infos = extract_attachment_infos_from_message_json(raw);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].pointer, "file-service://file-abc123");
    }

    #[test]
    fn extracts_project_from_conversation_template_id() {
        let value = json!({
            "id": "conv-1",
            "title": "In project",
            "conversation_template_id": "g-p-67ecb74315e081918a6c820ee7492ae7",
            "current_node": "n1",
            "mapping": {
                "n1": {
                    "id": "n1",
                    "message": {
                        "id": "m1",
                        "author": { "role": "user" },
                        "content": { "content_type": "text", "parts": ["hello"] },
                        "create_time": 1.0
                    },
                    "parent": null
                }
            }
        });
        let mut names = ProjectNameIndex::new();
        names.insert(
            "g-p-67ecb74315e081918a6c820ee7492ae7".to_string(),
            "ChatLens".to_string(),
        );
        let parsed = parse_conversation(&value, &names).expect("parse");
        assert_eq!(parsed.source_contexts.len(), 1);
        assert_eq!(parsed.source_contexts[0].name, "ChatLens");
        assert_eq!(parsed.source_contexts[0].context_type, "project");
    }

    #[test]
    fn classifies_user_upload_without_dalle_metadata() {
        let metadata = json!({
            "dalle": null,
            "generation": null
        });
        assert_eq!(
            classify_image_source(Some(&metadata), "user", None),
            "upload"
        );
    }

    #[test]
    fn classifies_dalle_generation_from_metadata() {
        let metadata = json!({
            "dalle": {
                "gen_id": "abc",
                "prompt": "a cat"
            }
        });
        assert_eq!(
            classify_image_source(Some(&metadata), "tool", None),
            "generated"
        );
    }

    #[test]
    fn skips_user_editable_context_messages() {
        let raw = r#"{
            "id": "conv-1",
            "title": "test",
            "current_node": "n1",
            "mapping": {
                "n1": {
                    "id": "n1",
                    "message": {
                        "id": "m1",
                        "author": { "role": "user" },
                        "content": {
                            "content_type": "text",
                            "parts": ["hello"]
                        },
                        "create_time": 1.0
                    },
                    "parent": "n0"
                },
                "n0": {
                    "id": "n0",
                    "message": {
                        "id": "m0",
                        "author": { "role": "user" },
                        "content": {
                            "content_type": "user_editable_context",
                            "user_profile": "secret"
                        },
                        "metadata": { "is_user_system_message": true }
                    },
                    "parent": null
                }
            }
        }"#;
        let value: Value = serde_json::from_str(raw).unwrap();
        let parsed = parse_conversation(&value, &ProjectNameIndex::new()).expect("parse");
        assert_eq!(parsed.messages.len(), 1);
        assert_eq!(parsed.messages[0].content, "hello");
    }
}
