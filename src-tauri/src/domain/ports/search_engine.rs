#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub text: String,
    pub limit: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub message_id: String,
    pub conversation_id: String,
    pub conversation_title: String,
    pub role: String,
    pub snippet: String,
    pub created_at: Option<f64>,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchIndexEntry {
    pub message_id: String,
    pub conversation_id: String,
    pub conversation_title: String,
    pub content: String,
}

pub trait SearchEngine {
    fn search(&self, query: SearchQuery) -> Result<Vec<SearchResult>, String>;
}
