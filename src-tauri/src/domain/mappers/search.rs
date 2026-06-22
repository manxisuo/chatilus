use crate::domain::ports::SearchResult;
use crate::models::SearchHit;

impl From<SearchResult> for SearchHit {
    fn from(result: SearchResult) -> Self {
        SearchHit {
            message_id: result.message_id,
            conversation_id: result.conversation_id,
            conversation_title: result.conversation_title,
            role: result.role,
            snippet: result.snippet,
            create_time: result.created_at,
            source: result.source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_result_maps_to_hit() {
        let result = SearchResult {
            message_id: "m1".to_string(),
            conversation_id: "c1".to_string(),
            conversation_title: "标题".to_string(),
            role: "user".to_string(),
            snippet: "匹配片段".to_string(),
            created_at: Some(1.0),
            source: "cursor".to_string(),
        };

        let hit: SearchHit = result.into();
        assert_eq!(hit.message_id, "m1");
        assert_eq!(hit.create_time, Some(1.0));
        assert_eq!(hit.source, "cursor");
    }
}
