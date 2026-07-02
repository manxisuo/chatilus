use crate::db::Database;
use crate::domain::mappers::asset_to_gallery_item;
use crate::domain::models::ImageKindFilter;
use crate::domain::ports::{AssetListQuery, AssetRepository};
use crate::models::ImageGalleryItem;

pub fn list_images(
    db: &Database,
    limit: i64,
    offset: i64,
    image_kind: ImageKindFilter,
    conversation_source: Option<&str>,
    month: Option<&str>,
    conversation_id: Option<&str>,
) -> Result<Vec<ImageGalleryItem>, String> {
    AssetRepository::list(
        db,
        AssetListQuery {
            limit,
            offset,
            image_kind,
            conversation_source: conversation_source.map(str::to_string),
            month: month.map(str::to_string),
            conversation_id: conversation_id.map(str::to_string),
        },
    )
    .map(|assets| assets.iter().map(asset_to_gallery_item).collect())
}

pub fn count_images(
    db: &Database,
    image_kind: ImageKindFilter,
    conversation_source: Option<&str>,
    month: Option<&str>,
    conversation_id: Option<&str>,
) -> Result<i64, String> {
    AssetRepository::count_filtered(
        db,
        image_kind,
        conversation_source,
        month,
        conversation_id,
    )
}
