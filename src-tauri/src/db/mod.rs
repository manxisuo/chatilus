use std::collections::HashMap;
use std::fs;
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use rusqlite::functions::FunctionFlags;

use crate::domain::mappers::{conversation_from_row, message_from_db_fields, message_to_view};
use crate::media::MediaIndex;
use crate::models::{
    AttachmentView, ConversationSummary, DatabaseStats, ExportResult, ImageGalleryItem,
    ImportResult, MessageView, SearchHit, TagView,
};
use crate::parser::{
    classify_image_source, extract_attachment_infos_from_message_json, parse_export_dir,
    ParsedAttachment, ParsedConversation,
};

pub struct Database {
    conn: Connection,
    path: String,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("无法创建数据库目录: {e}"))?;
        }

        let conn = Connection::open(path).map_err(|e| format!("无法打开数据库: {e}"))?;
        let db = Self {
            conn,
            path: path.display().to_string(),
        };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<(), String> {
        self.conn
            .execute_batch(
                "
                PRAGMA journal_mode = WAL;
                PRAGMA foreign_keys = ON;

                CREATE TABLE IF NOT EXISTS imports (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    source_path TEXT NOT NULL,
                    imported_at REAL NOT NULL,
                    conversation_count INTEGER NOT NULL,
                    message_count INTEGER NOT NULL
                );

                CREATE TABLE IF NOT EXISTS conversations (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    create_time REAL,
                    update_time REAL,
                    source_path TEXT NOT NULL,
                    model TEXT,
                    message_count INTEGER NOT NULL DEFAULT 0,
                    is_starred INTEGER NOT NULL DEFAULT 0
                );

                CREATE TABLE IF NOT EXISTS messages (
                    id TEXT PRIMARY KEY,
                    conversation_id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    create_time REAL,
                    sort_order INTEGER NOT NULL,
                    raw_json TEXT,
                    is_starred INTEGER NOT NULL DEFAULT 0,
                    attachments TEXT,
                    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS tags (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL UNIQUE,
                    created_at REAL NOT NULL DEFAULT (unixepoch('subsec'))
                );

                CREATE TABLE IF NOT EXISTS conversation_tags (
                    conversation_id TEXT NOT NULL,
                    tag_id INTEGER NOT NULL,
                    PRIMARY KEY (conversation_id, tag_id),
                    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
                    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
                );

                CREATE INDEX IF NOT EXISTS idx_messages_conversation
                    ON messages(conversation_id, sort_order);
                CREATE INDEX IF NOT EXISTS idx_conversations_update_time
                    ON conversations(update_time DESC);

                CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
                    message_id UNINDEXED,
                    conversation_id UNINDEXED,
                    conversation_title,
                    content,
                    tokenize = 'unicode61'
                );
                ",
            )
            .map_err(|e| format!("初始化数据库失败: {e}"))?;

        self.migrate_schema()?;
        self.register_sql_functions()?;

        self.conn
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_conversations_starred
                 ON conversations(is_starred DESC, update_time DESC)",
                [],
            )
            .map_err(|e| format!("创建收藏索引失败: {e}"))?;

        Ok(())
    }

    fn register_sql_functions(&self) -> Result<(), String> {
        self.conn
            .create_scalar_function(
                "effective_image_source",
                2,
                FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
                |ctx| {
                    let role: String = ctx.get(0)?;
                    let json: String = ctx.get(1)?;
                    Ok(effective_source_from_attachment_json(&role, &json))
                },
            )
            .map_err(|e| format!("注册 SQL 函数失败: {e}"))?;
        Ok(())
    }

    fn migrate_schema(&self) -> Result<(), String> {
        let _ = self.conn.execute(
            "ALTER TABLE conversations ADD COLUMN is_starred INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE messages ADD COLUMN is_starred INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = self
            .conn
            .execute("ALTER TABLE messages ADD COLUMN attachments TEXT", []);
        Ok(())
    }

    pub fn import_export_dir(&mut self, source_path: &Path) -> Result<ImportResult, String> {
        let media_index = MediaIndex::build(source_path);
        let conversations = parse_export_dir(source_path)?;
        let files_processed = crate::parser::find_conversation_files(source_path)?.len();
        let source = source_path.display().to_string();

        let tx = self
            .conn
            .transaction()
            .map_err(|e| format!("开启事务失败: {e}"))?;

        let mut conversation_count = 0usize;
        let mut message_count = 0usize;

        for conversation in conversations {
            let inserted = upsert_conversation(&tx, &conversation, &source)?;
            if inserted {
                conversation_count += 1;
            }
            message_count += insert_messages(&tx, &conversation, &media_index)?;
        }

        tx.execute(
            "INSERT INTO imports (source_path, imported_at, conversation_count, message_count)
             VALUES (?1, unixepoch('subsec'), ?2, ?3)",
            params![source, conversation_count as i64, message_count as i64],
        )
        .map_err(|e| format!("记录导入历史失败: {e}"))?;

        tx.commit()
            .map_err(|e| format!("提交事务失败: {e}"))?;

        Ok(ImportResult {
            conversations_imported: conversation_count,
            messages_imported: message_count,
            files_processed,
            source_path: source,
            media_files_indexed: media_index.len(),
        })
    }

    pub fn list_conversations(
        &self,
        query: Option<&str>,
        starred_only: bool,
        tag_id: Option<i64>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ConversationSummary>, String> {
        let mut sql = String::from(
            "SELECT c.id, c.title, c.create_time, c.update_time, c.model, c.message_count,
                    c.is_starred, c.source_path,
                    COALESCE(GROUP_CONCAT(t.name, char(31)), '') AS tag_names
             FROM conversations c
             LEFT JOIN conversation_tags ct ON ct.conversation_id = c.id
             LEFT JOIN tags t ON t.id = ct.tag_id
             WHERE 1 = 1",
        );
        let mut bind: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(text) = query.map(str::trim).filter(|text| !text.is_empty()) {
            sql.push_str(" AND c.title LIKE ?");
            bind.push(Box::new(format!("%{text}%")));
        }
        if starred_only {
            sql.push_str(" AND c.is_starred = 1");
        }
        if let Some(tag) = tag_id {
            sql.push_str(
                " AND c.id IN (SELECT conversation_id FROM conversation_tags WHERE tag_id = ?)",
            );
            bind.push(Box::new(tag));
        }

        sql.push_str(
            " GROUP BY c.id
              ORDER BY c.is_starred DESC, COALESCE(c.update_time, c.create_time, 0) DESC
              LIMIT ? OFFSET ?",
        );
        bind.push(Box::new(limit));
        bind.push(Box::new(offset));

        let mut stmt = self
            .conn
            .prepare(&sql)
            .map_err(|e| format!("查询会话失败: {e}"))?;

        let param_refs: Vec<&dyn rusqlite::ToSql> = bind.iter().map(|value| value.as_ref()).collect();
        let rows = stmt
            .query_map(param_refs.as_slice(), map_conversation_summary)
            .map_err(|e| format!("查询会话失败: {e}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("读取会话失败: {e}"))
    }

    pub fn get_messages(&self, conversation_id: &str) -> Result<Vec<MessageView>, String> {
        let source_path: Option<String> = self
            .conn
            .query_row(
                "SELECT source_path FROM conversations WHERE id = ?1",
                params![conversation_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| format!("查询对话来源失败: {e}"))?;

        let media_index = source_path
            .as_deref()
            .filter(|path| Path::new(path).is_dir())
            .map(|path| MediaIndex::build(Path::new(path)));

        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, conversation_id, role, content, create_time, sort_order,
                        is_starred, attachments, raw_json
                 FROM messages
                 WHERE conversation_id = ?1
                 ORDER BY sort_order ASC",
            )
            .map_err(|e| format!("查询消息失败: {e}"))?;

        let rows = stmt
            .query_map(params![conversation_id], |row| {
                let id: String = row.get(0)?;
                let conversation_id: String = row.get(1)?;
                let role: String = row.get(2)?;
                let content = clean_content_placeholders(row.get(3)?);
                let create_time: Option<f64> = row.get(4)?;
                let sort_order: i64 = row.get(5)?;
                let is_starred = row.get::<_, i64>(6)? != 0;
                let attachments_raw: Option<String> = row.get(7)?;
                let raw_json: Option<String> = row.get(8)?;
                let mut attachments = attachments_raw
                    .as_deref()
                    .map(parse_attachments_json)
                    .unwrap_or_default();

                if attachments.is_empty() {
                    if let (Some(ref index), Some(ref raw)) = (&media_index, &raw_json) {
                        attachments = resolve_parsed_attachments(
                            &extract_attachment_infos_from_message_json(raw),
                            &role,
                            index,
                        );
                    }
                } else {
                    attachments = attachments
                        .into_iter()
                        .map(|attachment| enrich_attachment(attachment, &role))
                        .collect();
                }

                let message = message_from_db_fields(
                    id,
                    conversation_id,
                    &role,
                    content,
                    create_time,
                    sort_order,
                    is_starred,
                    &attachments,
                    raw_json,
                );
                Ok(message_to_view(message, attachments))
            })
            .map_err(|e| format!("查询消息失败: {e}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("读取消息失败: {e}"))
    }

    pub fn search_messages(&self, query: &str, limit: i64) -> Result<Vec<SearchHit>, String> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        let mut stmt = self
            .conn
            .prepare(
                "SELECT
                    f.message_id,
                    f.conversation_id,
                    f.conversation_title,
                    m.role,
                    snippet(messages_fts, 3, '【', '】', '…', 32) AS snippet,
                    m.create_time
                 FROM messages_fts f
                 JOIN messages m ON m.id = f.message_id
                 WHERE messages_fts MATCH ?1
                 ORDER BY rank
                 LIMIT ?2",
            )
            .map_err(|e| format!("搜索失败: {e}"))?;

        let fts_query = build_fts_query(trimmed);
        let rows = stmt
            .query_map(params![fts_query, limit], |row| {
                Ok(SearchHit {
                    message_id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    conversation_title: row.get(2)?,
                    role: row.get(3)?,
                    snippet: row.get(4)?,
                    create_time: row.get(5)?,
                })
            })
            .map_err(|e| format!("搜索失败: {e}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("读取搜索结果失败: {e}"))
    }

    pub fn set_conversation_starred(
        &self,
        conversation_id: &str,
        starred: bool,
    ) -> Result<(), String> {
        let updated = self
            .conn
            .execute(
                "UPDATE conversations SET is_starred = ?1 WHERE id = ?2",
                params![starred as i64, conversation_id],
            )
            .map_err(|e| format!("更新收藏失败: {e}"))?;

        if updated == 0 {
            return Err("对话不存在".to_string());
        }
        Ok(())
    }

    pub fn set_message_starred(&self, message_id: &str, starred: bool) -> Result<(), String> {
        let updated = self
            .conn
            .execute(
                "UPDATE messages SET is_starred = ?1 WHERE id = ?2",
                params![starred as i64, message_id],
            )
            .map_err(|e| format!("更新消息收藏失败: {e}"))?;

        if updated == 0 {
            return Err("消息不存在".to_string());
        }
        Ok(())
    }

    pub fn list_tags(&self) -> Result<Vec<TagView>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT t.id, t.name, COUNT(ct.conversation_id) AS conversation_count
                 FROM tags t
                 LEFT JOIN conversation_tags ct ON ct.tag_id = t.id
                 GROUP BY t.id
                 ORDER BY t.name COLLATE NOCASE ASC",
            )
            .map_err(|e| format!("查询标签失败: {e}"))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(TagView {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    conversation_count: row.get(2)?,
                })
            })
            .map_err(|e| format!("查询标签失败: {e}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("读取标签失败: {e}"))
    }

    pub fn create_tag(&self, name: &str) -> Result<TagView, String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("标签名不能为空".to_string());
        }

        self.conn
            .execute("INSERT INTO tags (name) VALUES (?1)", params![trimmed])
            .map_err(|e| format!("创建标签失败: {e}"))?;

        let id = self.conn.last_insert_rowid();
        Ok(TagView {
            id,
            name: trimmed.to_string(),
            conversation_count: 0,
        })
    }

    pub fn delete_tag(&self, tag_id: i64) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM tags WHERE id = ?1", params![tag_id])
            .map_err(|e| format!("删除标签失败: {e}"))?;
        Ok(())
    }

    pub fn set_conversation_tags(
        &self,
        conversation_id: &str,
        tag_ids: &[i64],
    ) -> Result<Vec<String>, String> {
        let exists: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM conversations WHERE id = ?1",
                params![conversation_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("检查对话失败: {e}"))?;
        if exists == 0 {
            return Err("对话不存在".to_string());
        }

        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|e| format!("开启事务失败: {e}"))?;

        tx.execute(
            "DELETE FROM conversation_tags WHERE conversation_id = ?1",
            params![conversation_id],
        )
        .map_err(|e| format!("清理标签失败: {e}"))?;

        for tag_id in tag_ids {
            tx.execute(
                "INSERT INTO conversation_tags (conversation_id, tag_id) VALUES (?1, ?2)",
                params![conversation_id, tag_id],
            )
            .map_err(|e| format!("设置标签失败: {e}"))?;
        }

        tx.commit()
            .map_err(|e| format!("提交标签失败: {e}"))?;

        self.get_conversation_tag_names(conversation_id)
    }

    pub fn export_conversation_markdown(
        &self,
        conversation_id: &str,
        output_path: &Path,
    ) -> Result<ExportResult, String> {
        let (title, model, create_time, update_time) = self
            .conn
            .query_row(
                "SELECT title, model, create_time, update_time FROM conversations WHERE id = ?1",
                params![conversation_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<f64>>(2)?,
                        row.get::<_, Option<f64>>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| format!("查询对话失败: {e}"))?
            .ok_or_else(|| "对话不存在".to_string())?;

        let messages = self.get_messages(conversation_id)?;
        let mut markdown = String::new();
        markdown.push_str(&format!("# {title}\n\n"));
        if let Some(model) = model {
            markdown.push_str(&format!("- 模型：`{model}`\n"));
        }
        if let Some(ts) = create_time {
            markdown.push_str(&format!("- 创建时间：{}\n", format_timestamp(ts)));
        }
        if let Some(ts) = update_time {
            markdown.push_str(&format!("- 更新时间：{}\n", format_timestamp(ts)));
        }
        markdown.push_str("\n---\n\n");

        for message in &messages {
            markdown.push_str(&format!("## {}\n\n", role_heading(&message.role)));
            if let Some(ts) = message.create_time {
                markdown.push_str(&format!("*{}\n\n", format_timestamp(ts)));
            }
            markdown.push_str(&message.content);
            markdown.push('\n');

            for attachment in &message.attachments {
                markdown.push_str(&format!(
                    "\n![{}]({})\n",
                    attachment.file_key, attachment.path
                ));
            }

            markdown.push_str("\n---\n\n");
        }

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("创建导出目录失败: {e}"))?;
        }
        fs::write(output_path, markdown).map_err(|e| format!("写入 Markdown 失败: {e}"))?;

        Ok(ExportResult {
            path: output_path.display().to_string(),
            message_count: messages.len() as i64,
        })
    }

    pub fn list_images(
        &self,
        limit: i64,
        offset: i64,
        include_uploads: bool,
    ) -> Result<Vec<ImageGalleryItem>, String> {
        let mut from_attachments =
            self.list_images_from_attachments(limit, offset, include_uploads)?;

        if from_attachments.len() as i64 >= limit {
            return Ok(from_attachments);
        }

        let remaining = limit - from_attachments.len() as i64;
        let hydrate_offset = (offset.saturating_sub(self.count_images_from_attachments(
            include_uploads,
        )?))
        .max(0);
        let hydrated =
            self.list_images_from_raw_json(remaining, hydrate_offset, include_uploads)?;

        from_attachments.extend(hydrated);
        Ok(from_attachments)
    }

    fn list_images_from_attachments(
        &self,
        limit: i64,
        offset: i64,
        include_uploads: bool,
    ) -> Result<Vec<ImageGalleryItem>, String> {
        let upload_filter = if include_uploads {
            String::new()
        } else {
            " AND effective_image_source(m.role, je.value) != 'upload'".to_string()
        };

        let sql = format!(
            "SELECT
                m.id,
                m.conversation_id,
                m.role,
                m.create_time,
                c.title,
                json_extract(je.value, '$.path') AS path,
                json_extract(je.value, '$.file_key') AS file_key,
                effective_image_source(m.role, je.value) AS source,
                json_extract(je.value, '$.prompt') AS prompt
             FROM messages m
             JOIN conversations c ON c.id = m.conversation_id
             JOIN json_each(m.attachments) AS je
             WHERE m.attachments IS NOT NULL
               AND m.attachments != '[]'
               AND json_extract(je.value, '$.path') IS NOT NULL
               {upload_filter}
             ORDER BY COALESCE(m.create_time, 0) DESC
             LIMIT ?1 OFFSET ?2"
        );

        let mut stmt = self
            .conn
            .prepare(&sql)
            .map_err(|e| format!("查询图片失败: {e}"))?;

        let rows = stmt
            .query_map(params![limit, offset], |row| {
                Ok(ImageGalleryItem {
                    message_id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    role: row.get(2)?,
                    create_time: row.get(3)?,
                    conversation_title: row.get(4)?,
                    path: row.get(5)?,
                    file_key: row.get(6)?,
                    source: row.get(7)?,
                    prompt: row.get(8)?,
                })
            })
            .map_err(|e| format!("查询图片失败: {e}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("读取图片失败: {e}"))
    }

    fn list_images_from_raw_json(
        &self,
        limit: i64,
        offset: i64,
        include_uploads: bool,
    ) -> Result<Vec<ImageGalleryItem>, String> {
        if limit <= 0 {
            return Ok(Vec::new());
        }

        let mut stmt = self
            .conn
            .prepare(
                "SELECT m.id, m.conversation_id, m.role, m.create_time, m.raw_json,
                        c.title, c.source_path
                 FROM messages m
                 JOIN conversations c ON c.id = m.conversation_id
                 WHERE m.raw_json LIKE '%image_asset_pointer%'
                   AND (m.attachments IS NULL OR m.attachments = '[]' OR m.attachments = '')
                 ORDER BY COALESCE(m.create_time, 0) DESC",
            )
            .map_err(|e| format!("查询图片失败: {e}"))?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<f64>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })
            .map_err(|e| format!("查询图片失败: {e}"))?;

        let mut media_cache: HashMap<String, MediaIndex> = HashMap::new();
        let mut flattened = Vec::new();

        for row in rows {
            let (message_id, conversation_id, role, create_time, raw_json, title, source_path) =
                row.map_err(|e| format!("读取图片失败: {e}"))?;
            let Some(raw_json) = raw_json else {
                continue;
            };

            let index = media_cache
                .entry(source_path.clone())
                .or_insert_with(|| MediaIndex::build(Path::new(&source_path)));

            let pointers = extract_attachment_infos_from_message_json(&raw_json);
            let attachments = resolve_parsed_attachments(&pointers, &role, index);
            for attachment in attachments {
                if !include_uploads && attachment.source == "upload" {
                    continue;
                }
                flattened.push(ImageGalleryItem {
                    path: attachment.path,
                    file_key: attachment.file_key,
                    conversation_id: conversation_id.clone(),
                    conversation_title: title.clone(),
                    message_id: message_id.clone(),
                    role: role.clone(),
                    create_time,
                    source: attachment.source,
                    prompt: attachment.prompt,
                });
            }
        }

        Ok(flattened
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect())
    }

    fn count_images_from_attachments(&self, include_uploads: bool) -> Result<i64, String> {
        let upload_filter = if include_uploads {
            String::new()
        } else {
            " AND effective_image_source(m.role, je.value) != 'upload'".to_string()
        };

        let sql = format!(
            "SELECT COUNT(*)
             FROM messages m
             JOIN json_each(m.attachments) AS je
             WHERE m.attachments IS NOT NULL
               AND m.attachments != '[]'
               AND json_extract(je.value, '$.path') IS NOT NULL
               {upload_filter}"
        );

        self.conn
            .query_row(&sql, [], |row| row.get(0))
            .map_err(|e| format!("统计图片失败: {e}"))
    }

    fn count_images_by_source(&self) -> Result<(i64, i64, i64), String> {
        let total: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*)
                 FROM messages m
                 JOIN json_each(m.attachments) AS je
                 WHERE m.attachments IS NOT NULL
                   AND m.attachments != '[]'
                   AND json_extract(je.value, '$.path') IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .map_err(|e| format!("统计图片失败: {e}"))?;

        let generated: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*)
                 FROM messages m
                 JOIN json_each(m.attachments) AS je
                 WHERE m.attachments IS NOT NULL
                   AND m.attachments != '[]'
                   AND json_extract(je.value, '$.path') IS NOT NULL
                   AND effective_image_source(m.role, je.value) = 'generated'",
                [],
                |row| row.get(0),
            )
            .map_err(|e| format!("统计图片失败: {e}"))?;

        let upload: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*)
                 FROM messages m
                 JOIN json_each(m.attachments) AS je
                 WHERE m.attachments IS NOT NULL
                   AND m.attachments != '[]'
                   AND json_extract(je.value, '$.path') IS NOT NULL
                   AND effective_image_source(m.role, je.value) = 'upload'",
                [],
                |row| row.get(0),
            )
            .map_err(|e| format!("统计图片失败: {e}"))?;

        Ok((total, generated, upload))
    }

    pub fn stats(&self) -> Result<DatabaseStats, String> {
        let conversation_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM conversations", [], |row| row.get(0))
            .map_err(|e| format!("统计失败: {e}"))?;
        let message_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))
            .map_err(|e| format!("统计失败: {e}"))?;
        let starred_conversation_count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM conversations WHERE is_starred = 1",
                [],
                |row| row.get(0),
            )
            .map_err(|e| format!("统计失败: {e}"))?;
        let starred_message_count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE is_starred = 1",
                [],
                |row| row.get(0),
            )
            .map_err(|e| format!("统计失败: {e}"))?;
        let tag_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM tags", [], |row| row.get(0))
            .map_err(|e| format!("统计失败: {e}"))?;
        let (image_count, generated_image_count, upload_image_count) = self.count_images_by_source()?;

        Ok(DatabaseStats {
            conversation_count,
            message_count,
            image_count,
            generated_image_count,
            upload_image_count,
            starred_conversation_count,
            starred_message_count,
            tag_count,
            db_path: self.path.clone(),
        })
    }

    fn get_conversation_tag_names(&self, conversation_id: &str) -> Result<Vec<String>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT t.name
                 FROM tags t
                 JOIN conversation_tags ct ON ct.tag_id = t.id
                 WHERE ct.conversation_id = ?1
                 ORDER BY t.name COLLATE NOCASE ASC",
            )
            .map_err(|e| format!("查询标签失败: {e}"))?;

        let rows = stmt
            .query_map(params![conversation_id], |row| row.get(0))
            .map_err(|e| format!("查询标签失败: {e}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("读取标签失败: {e}"))
    }
}

fn map_conversation_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<ConversationSummary> {
    let tag_names: String = row.get(8)?;
    let tags = if tag_names.is_empty() {
        Vec::new()
    } else {
        tag_names.split('\x1f').map(str::to_string).collect()
    };

    Ok(conversation_from_row(
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get::<_, i64>(6)? != 0,
        row.get(7)?,
        tags,
    )
    .into())
}

fn clean_content_placeholders(content: String) -> String {
    content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed != "[image_asset_pointer]"
                && trimmed != "[multimodal_text]"
                && trimmed != "[user_editable_context]"
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn parse_attachments_json(raw: &str) -> Vec<AttachmentView> {
    serde_json::from_str(raw).unwrap_or_default()
}

fn upsert_conversation(
    tx: &rusqlite::Transaction<'_>,
    conversation: &ParsedConversation,
    source: &str,
) -> Result<bool, String> {
    let exists: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM conversations WHERE id = ?1",
            params![conversation.id],
            |row| row.get(0),
        )
        .map_err(|e| format!("检查会话失败: {e}"))?;

    tx.execute(
        "INSERT INTO conversations (id, title, create_time, update_time, source_path, model, message_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(id) DO UPDATE SET
            title = excluded.title,
            create_time = excluded.create_time,
            update_time = excluded.update_time,
            source_path = excluded.source_path,
            model = excluded.model,
            message_count = excluded.message_count",
        params![
            conversation.id,
            conversation.title,
            conversation.create_time,
            conversation.update_time,
            source,
            conversation.model,
            conversation.messages.len() as i64,
        ],
    )
    .map_err(|e| format!("写入会话失败: {e}"))?;

    tx.execute(
        "DELETE FROM messages WHERE conversation_id = ?1",
        params![conversation.id],
    )
    .map_err(|e| format!("清理旧消息失败: {e}"))?;

    tx.execute(
        "DELETE FROM messages_fts WHERE conversation_id = ?1",
        params![conversation.id],
    )
    .map_err(|e| format!("清理旧索引失败: {e}"))?;

    Ok(exists == 0)
}

fn insert_messages(
    tx: &rusqlite::Transaction<'_>,
    conversation: &ParsedConversation,
    media_index: &MediaIndex,
) -> Result<usize, String> {
    for (index, message) in conversation.messages.iter().enumerate() {
        let stored_id = format!("{}::{}", conversation.id, message.id);
        let attachments = resolve_parsed_attachments(&message.attachments, &message.role, media_index);
        let attachments_json = serde_json::to_string(&attachments)
            .map_err(|e| format!("序列化附件失败: {e}"))?;

        tx.execute(
            "INSERT INTO messages (id, conversation_id, role, content, create_time, sort_order, raw_json, attachments)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                stored_id,
                conversation.id,
                message.role,
                message.content,
                message.create_time,
                index as i64,
                message.raw_json,
                attachments_json,
            ],
        )
        .map_err(|e| format!("写入消息失败: {e}"))?;

        tx.execute(
            "INSERT INTO messages_fts (message_id, conversation_id, conversation_title, content)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                stored_id,
                conversation.id,
                conversation.title,
                message.content,
            ],
        )
        .map_err(|e| format!("写入全文索引失败: {e}"))?;
    }

    Ok(conversation.messages.len())
}

fn resolve_parsed_attachments(
    parsed: &[ParsedAttachment],
    role: &str,
    media_index: &MediaIndex,
) -> Vec<AttachmentView> {
    parsed
        .iter()
        .filter_map(|item| {
            media_index.resolve(&item.pointer).map(|path| {
                let path_str = path.display().to_string();
                let source = if item.source == "unknown" {
                    classify_image_source(None, role, Some(&path_str))
                } else if path_str.contains("dalle-generations") {
                    "generated".to_string()
                } else {
                    item.source.clone()
                };
                AttachmentView {
                    file_key: item.pointer.clone(),
                    path: path_str,
                    source,
                    prompt: item.prompt.clone(),
                }
            })
        })
        .collect()
}

fn enrich_attachment(mut attachment: AttachmentView, role: &str) -> AttachmentView {
    if attachment.source.is_empty() || attachment.source == "unknown" {
        attachment.source =
            classify_image_source(None, role, Some(&attachment.path));
    } else if attachment.path.contains("dalle-generations") {
        attachment.source = "generated".to_string();
    }
    attachment
}

fn effective_source_from_attachment_json(role: &str, json: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return classify_image_source(None, role, None);
    };
    let stored = value.get("source").and_then(|v| v.as_str());
    let path = value.get("path").and_then(|v| v.as_str());
    if let Some(source) = stored.filter(|s| !s.is_empty() && *s != "unknown") {
        if path.is_some_and(|p| p.contains("dalle-generations")) {
            return "generated".to_string();
        }
        return source.to_string();
    }
    classify_image_source(None, role, path)
}

fn build_fts_query(input: &str) -> String {
    input
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn role_heading(role: &str) -> &'static str {
    match role {
        "user" => "用户",
        "assistant" => "助手",
        "tool" => "工具",
        _ => "消息",
    }
}

fn format_timestamp(ts: f64) -> String {
    let millis = (ts * 1000.0) as i64;
    if let Some(dt) = timestamp_millis_to_rfc3339(millis) {
        return dt;
    }
    ts.to_string()
}

fn timestamp_millis_to_rfc3339(millis: i64) -> Option<String> {
    let seconds = millis / 1000;
    let days = seconds / 86_400;
    if days < 0 {
        return None;
    }

    let day_seconds = seconds % 86_400;
    let hour = day_seconds / 3600;
    let minute = (day_seconds % 3600) / 60;
    let second = day_seconds % 60;

    let (year, month, day) = civil_from_days(days);
    Some(format!(
        "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02} UTC"
    ))
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year, m as i64, d as i64)
}
