use crate::db::Database;
use crate::models::TagView;

pub fn list_tags(db: &Database) -> Result<Vec<TagView>, String> {
    db.list_tags()
}

pub fn create_tag(db: &Database, name: &str) -> Result<TagView, String> {
    db.create_tag(name)
}

pub fn delete_tag(db: &Database, tag_id: i64) -> Result<(), String> {
    db.delete_tag(tag_id)
}
