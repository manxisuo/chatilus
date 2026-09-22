

pub(crate) const ACTIVITY_MONTH_SQL: &str =
    "strftime('%Y-%m', datetime(COALESCE(c.update_time, c.create_time), 'unixepoch', 'localtime'))";

pub(crate) const HAS_IMAGES_EXISTS_SQL: &str =
    "EXISTS (SELECT 1 FROM assets a WHERE a.conversation_id = c.id AND a.local_path != '')";

pub(crate) const HAS_ATTACHMENTS_EXISTS_SQL: &str =
    "EXISTS (
        SELECT 1 FROM messages m
        WHERE m.conversation_id = c.id
          AND m.attachments IS NOT NULL
          AND m.attachments != ''
          AND m.attachments != '[]'
    )";

pub(crate) const SOURCE_EQUALS_SQL: &str = "c.source = ?";
