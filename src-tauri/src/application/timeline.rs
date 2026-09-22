use crate::error::AppResult;
use crate::infrastructure::db::Database;
use crate::domain::ports::{AssetRepository, ConversationRepository, TimelineListQuery};
use crate::models::{ConversationSummary, TimelineMonthBucket};

pub fn list_timeline_months(
    db: &Database,
    source: Option<&str>,
) -> AppResult<Vec<TimelineMonthBucket>> {
    let mut months = ConversationRepository::list_timeline_months(db, source)?;
    let image_counts = AssetRepository::image_counts_by_message_month(db, source)?;
    for bucket in &mut months {
        bucket.image_count = image_counts.get(&bucket.month).copied().unwrap_or(0);
    }
    Ok(months)
}

pub fn list_timeline(
    db: &Database,
    source: Option<&str>,
    month: Option<&str>,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<ConversationSummary>> {
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
