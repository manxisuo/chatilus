use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedAttachment {
    pub pointer: String,
    pub source: String,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedConversation {
    pub id: String,
    pub title: String,
    pub create_time: Option<f64>,
    pub update_time: Option<f64>,
    pub model: Option<String>,
    pub messages: Vec<ParsedMessage>,
}

#[derive(Debug, Clone)]
pub struct ParsedMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub create_time: Option<f64>,
    pub raw_json: String,
    pub attachments: Vec<ParsedAttachment>,
}

pub fn extract_pointers_from_message_json(raw_json: &str) -> Vec<String> {
    extract_attachment_infos_from_message_json(raw_json)
        .into_iter()
        .map(|item| item.pointer)
        .collect()
}

pub fn extract_attachment_infos_from_message_json(raw_json: &str) -> Vec<ParsedAttachment> {
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
        if infos.iter().any(|item| item.pointer == pointer) {
            continue;
        }
        infos.push(ParsedAttachment {
            pointer,
            source: infer_source_from_role(role, None),
            prompt: None,
        });
    }
    infos.sort_by(|a, b| a.pointer.cmp(&b.pointer));
    infos.dedup_by(|a, b| a.pointer == b.pointer);
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

pub fn parse_conversation(value: &Value) -> Option<ParsedConversation> {
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

    Some(ParsedConversation {
        id,
        title,
        create_time,
        update_time,
        model,
        messages,
    })
}

fn linearize_messages(
    mapping: &serde_json::Map<String, Value>,
    current_node: &str,
) -> Vec<ParsedMessage> {
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

fn parse_message(message: &Value) -> Option<ParsedMessage> {
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
        if attachments.iter().any(|item| item.pointer == pointer) {
            continue;
        }
        attachments.push(ParsedAttachment {
            pointer,
            source: infer_source_from_role(&role, None),
            prompt: None,
        });
    }
    attachments.sort_by(|a, b| a.pointer.cmp(&b.pointer));
    attachments.dedup_by(|a, b| a.pointer == b.pointer);

    Some(ParsedMessage {
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

fn extract_attachment_infos_from_content(content: &Value, role: &str) -> Vec<ParsedAttachment> {
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
                infos.push(ParsedAttachment {
                    pointer: pointer.to_string(),
                    source: classify_image_source(metadata, role, None),
                    prompt: extract_dalle_prompt(metadata),
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
            collect_attachment_ids(item, &mut pointers);
        }
    }

    if let Some(metadata) = message.get("metadata").and_then(|v| v.as_object()) {
        if let Some(items) = metadata.get("attachments").and_then(|v| v.as_array()) {
            for item in items {
                collect_attachment_ids(item, &mut pointers);
            }
        }
    }

    pointers
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
        let parsed = parse_conversation(&value).expect("parse");
        assert_eq!(parsed.messages.len(), 1);
        assert_eq!(parsed.messages[0].content, "hello");
    }
}
