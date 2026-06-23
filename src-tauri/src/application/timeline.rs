use crate::db::Database;
use crate::domain::ports::{ConversationRepository, TimelineListQuery};
use crate::models::{ConversationSummary, TimelineMonthBucket};

pub fn list_timeline_months(
    db: &Database,
    source: Option<&str>,
) -> Result<Vec<TimelineMonthBucket>, String> {
    ConversationRepository::list_timeline_months(db, source)
}

pub fn list_timeline(
    db: &Database,
    source: Option<&str>,
    month: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<ConversationSummary>, String> {
    ConversationRepository::list_timeline(
        db,
        TimelineListQuery {
            source: source.map(str::to_string),
            month: month.map(str::to_string),
            limit,
            offset,
        },
    )
}
