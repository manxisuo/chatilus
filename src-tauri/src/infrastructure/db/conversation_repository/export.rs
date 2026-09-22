use std::fs;
use std::path::Path;

use rusqlite::{params, OptionalExtension};

use crate::domain::ports::MessageRepository;
use crate::error::{AppError, AppResult};
use crate::infrastructure::db::helpers::{format_timestamp, role_heading};
use crate::infrastructure::db::Database;
use crate::models::ExportResult;

    pub(crate) fn export_markdown(db: &Database, conversation_id: &str, output_path: &Path) -> AppResult<ExportResult> {
        let (title, model, create_time, update_time) = db
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
            .map_err(|e| AppError::Msg(format!("查询对话失败: {e}")))?
            .ok_or_else(|| AppError::msg("对话不存在"))?;

        let messages = MessageRepository::list_by_conversation(db, conversation_id)?;
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
            fs::create_dir_all(parent).map_err(|e| AppError::Msg(format!("创建导出目录失败: {e}")))?;
        }
        fs::write(output_path, markdown).map_err(|e| AppError::Msg(format!("写入 Markdown 失败: {e}")))?;

        Ok(ExportResult {
            path: output_path.display().to_string(),
            message_count: messages.len() as i64,
        })
    }

