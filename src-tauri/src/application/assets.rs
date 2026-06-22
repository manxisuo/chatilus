use crate::db::Database;
use crate::domain::mappers::asset_to_gallery_item;
use crate::domain::ports::{AssetListQuery, AssetRepository};
use crate::models::ImageGalleryItem;

pub fn list_images(
    db: &Database,
    limit: i64,
    offset: i64,
    include_uploads: bool,
) -> Result<Vec<ImageGalleryItem>, String> {
    AssetRepository::list(
        db,
        AssetListQuery {
            limit,
            offset,
            include_uploads,
        },
    )
    .map(|assets| assets.iter().map(asset_to_gallery_item).collect())
}
