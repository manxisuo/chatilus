use crate::db::Database;
use crate::domain::ports::{AssetListQuery, AssetRepository};
use crate::models::ImageGalleryItem;

pub fn list_images(
    db: &Database,
    limit: i64,
    offset: i64,
    include_uploads: bool,
) -> Result<Vec<ImageGalleryItem>, String> {
    AssetRepository::list_images(
        db,
        AssetListQuery {
            limit,
            offset,
            include_uploads,
        },
    )
}
