use crate::error::AppResult;
use crate::infrastructure::db::Database;
use crate::models::TagView;

pub fn list_tags(db: &Database) -> AppResult<Vec<TagView>> {
    db.list_tags()
}

pub fn create_tag(db: &Database, name: &str) -> AppResult<TagView> {
    db.create_tag(name)
}

pub fn delete_tag(db: &Database, tag_id: i64) -> AppResult<()> {
    db.delete_tag(tag_id)
}
