use std::collections::HashMap;

use crate::models::{AttachmentView, ImageGalleryItem};

use super::super::models::{Asset, AssetType, DataSource};

pub fn attachment_view_to_asset(
    attachment: &AttachmentView,
    conversation_id: &str,
    message_id: &str,
    role: &str,
    created_at: Option<f64>,
) -> Asset {
    let mut metadata = HashMap::new();
    metadata.insert(
        "image_source".into(),
        serde_json::Value::String(attachment.source.clone()),
    );
    metadata.insert("role".into(), serde_json::Value::String(role.to_string()));
    if let Some(prompt) = &attachment.prompt {
        metadata.insert("prompt".into(), serde_json::Value::String(prompt.clone()));
    }

    Asset {
        id: attachment.file_key.clone(),
        source: DataSource::ChatGpt,
        conversation_id: Some(conversation_id.to_string()),
        message_id: Some(message_id.to_string()),
        asset_type: AssetType::Image,
        title: attachment.prompt.clone(),
        mime_type: None,
        uri: None,
        local_path: Some(attachment.path.clone()),
        content: None,
        metadata,
        created_at,
        is_favorite: false,
        tags: Vec::new(),
    }
}

pub fn asset_to_attachment_view(asset: &Asset) -> AttachmentView {
    let source = asset
        .metadata
        .get("image_source")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let prompt = asset
        .title
        .clone()
        .or_else(|| {
            asset
                .metadata
                .get("prompt")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        });

    AttachmentView {
        file_key: asset.id.clone(),
        path: asset.local_path.clone().unwrap_or_default(),
        source,
        prompt,
    }
}

pub fn image_fields_to_asset(
    message_id: String,
    conversation_id: String,
    role: String,
    create_time: Option<f64>,
    conversation_title: String,
    path: String,
    file_key: String,
    source: String,
    prompt: Option<String>,
) -> Asset {
    let mut metadata = HashMap::new();
    metadata.insert(
        "image_source".into(),
        serde_json::Value::String(source.clone()),
    );
    metadata.insert("role".into(), serde_json::Value::String(role));
    metadata.insert(
        "conversation_title".into(),
        serde_json::Value::String(conversation_title),
    );
    if let Some(prompt) = &prompt {
        metadata.insert("prompt".into(), serde_json::Value::String(prompt.clone()));
    }

    Asset {
        id: file_key,
        source: DataSource::ChatGpt,
        conversation_id: Some(conversation_id),
        message_id: Some(message_id),
        asset_type: AssetType::Image,
        title: prompt,
        mime_type: None,
        uri: None,
        local_path: Some(path),
        content: None,
        metadata,
        created_at: create_time,
        is_favorite: false,
        tags: Vec::new(),
    }
}

pub fn gallery_item_to_asset(item: &ImageGalleryItem) -> Asset {
    image_fields_to_asset(
        item.message_id.clone(),
        item.conversation_id.clone(),
        item.role.clone(),
        item.create_time,
        item.conversation_title.clone(),
        item.path.clone(),
        item.file_key.clone(),
        item.source.clone(),
        item.prompt.clone(),
    )
}

pub fn asset_to_gallery_item(asset: &Asset) -> ImageGalleryItem {
    let source = asset
        .metadata
        .get("image_source")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    ImageGalleryItem {
        path: asset.local_path.clone().unwrap_or_default(),
        file_key: asset.id.clone(),
        conversation_id: asset.conversation_id.clone().unwrap_or_default(),
        conversation_title: asset
            .metadata
            .get("conversation_title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        message_id: asset.message_id.clone().unwrap_or_default(),
        role: asset
            .metadata
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string(),
        create_time: asset.created_at,
        source,
        prompt: asset.title.clone().or_else(|| {
            asset
                .metadata
                .get("prompt")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        }),
    }
}

impl From<Asset> for ImageGalleryItem {
    fn from(asset: Asset) -> Self {
        asset_to_gallery_item(&asset)
    }
}

pub fn assets_to_attachment_views(assets: &[Asset]) -> Vec<AttachmentView> {
    assets.iter().map(asset_to_attachment_view).collect()
}

impl From<&AttachmentView> for Asset {
    fn from(attachment: &AttachmentView) -> Self {
        attachment_view_to_asset(attachment, "", "", "unknown", None)
    }
}

impl From<&ImageGalleryItem> for Asset {
    fn from(item: &ImageGalleryItem) -> Self {
        gallery_item_to_asset(item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attachment_view_asset_round_trip() {
        let attachment = AttachmentView {
            file_key: "key-1".to_string(),
            path: "/data/file-1.png".to_string(),
            source: "generated".to_string(),
            prompt: Some("sunset".to_string()),
        };

        let asset = attachment_view_to_asset(&attachment, "conv-1", "msg-1", "tool", Some(42.0));
        assert_eq!(asset.asset_type, AssetType::Image);
        assert_eq!(asset.conversation_id.as_deref(), Some("conv-1"));

        let back = asset_to_attachment_view(&asset);
        assert_eq!(back.file_key, attachment.file_key);
        assert_eq!(back.path, attachment.path);
        assert_eq!(back.source, attachment.source);
        assert_eq!(back.prompt, attachment.prompt);
    }

    #[test]
    fn gallery_item_asset_round_trip() {
        let item = ImageGalleryItem {
            path: "/img.png".to_string(),
            file_key: "fk".to_string(),
            conversation_id: "c1".to_string(),
            conversation_title: "标题".to_string(),
            message_id: "m1".to_string(),
            role: "assistant".to_string(),
            create_time: Some(1.0),
            source: "upload".to_string(),
            prompt: Some("prompt".to_string()),
        };

        let asset = gallery_item_to_asset(&item);
        assert_eq!(asset.id, "fk");
        assert_eq!(asset.local_path.as_deref(), Some("/img.png"));

        let back = asset_to_gallery_item(&asset);
        assert_eq!(back, item);
    }
}
