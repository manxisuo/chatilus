use crate::db::Database;
use crate::models::SearchHit;

pub fn search_messages(db: &Database, query: &str, limit: i64) -> Result<Vec<SearchHit>, String> {
    db.search_messages(query, limit)
}
