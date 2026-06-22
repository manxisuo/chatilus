use crate::db::Database;
use crate::domain::ports::{SearchEngine, SearchQuery};
use crate::models::SearchHit;

pub fn search_messages(db: &Database, query: &str, limit: i64) -> Result<Vec<SearchHit>, String> {
    SearchEngine::search(
        db,
        SearchQuery {
            text: query.to_string(),
            limit,
        },
    )
    .map(|results| results.into_iter().map(SearchHit::from).collect())
}
