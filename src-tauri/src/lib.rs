mod application;
mod commands;
mod db;
mod domain;
mod infrastructure;
mod models;

use commands::{
    create_tag, delete_tag, export_conversation_markdown, get_conversation, get_import_job,
    get_messages, get_stats, list_starred_messages, list_timeline, list_timeline_months,
    import_export_dir, init_state, list_conversations, count_images, list_images, list_tags,
    list_import_guides, read_image_data_url,
    search_messages, set_conversation_starred, set_conversation_tags, set_message_starred,
    start_import,
};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = init_state(app.handle())?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            import_export_dir,
            list_import_guides,
            start_import,
            get_import_job,
            list_conversations,
            get_conversation,
            list_images,
            count_images,
            get_messages,
            list_starred_messages,
            list_timeline,
            list_timeline_months,
            search_messages,
            set_conversation_starred,
            set_message_starred,
            list_tags,
            create_tag,
            delete_tag,
            set_conversation_tags,
            export_conversation_markdown,
            read_image_data_url,
            get_stats,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::application;
    use crate::db::Database;
    use crate::infrastructure::importers::chatgpt::parse_export_dir;

    fn sample_export_dir() -> PathBuf {
        PathBuf::from(r"D:\Personal\ChatGPT数据下载-2026年5月18日\2026-05-16-12-08-35")
    }

    #[test]
    fn parse_smallest_shard() {
        let conversations = parse_export_dir(&sample_export_dir()).expect("parse export");
        assert!(!conversations.is_empty());

        let with_messages: Vec<_> = conversations
            .iter()
            .filter(|item| !item.messages.is_empty())
            .collect();
        assert!(!with_messages.is_empty());
    }

    #[test]
    fn import_into_sqlite() {
        let dir = std::env::temp_dir().join("chatlens-test-v02.db");
        let _ = std::fs::remove_file(&dir);

        let mut db = Database::open(&dir).expect("open db");
        let result = application::import_export_dir(&mut db, &sample_export_dir())
            .expect("import export");

        assert!(result.conversations_imported > 0);
        assert!(result.messages_imported > 0);
        assert_eq!(result.files_processed, 14);
        assert!(result.media_files_indexed > 0);

        let stats = db.stats().expect("stats");
        assert_eq!(
            stats.conversation_count as usize,
            result.conversations_imported
        );

        let hits = application::search_messages(&db, "uuid", 5).expect("search");
        assert!(!hits.is_empty());

        let conv_id = application::list_conversations(
            &db, None, false, None, None, false, false, false, 1, 0,
        )
            .expect("list")[0]
            .id
            .clone();
        application::set_conversation_starred(&db, &conv_id, true).expect("star");
        let tag = application::create_tag(&db, "测试标签").expect("tag");
        application::set_conversation_tags(&db, &conv_id, &[tag.id])
            .expect("set tags");

        let export_path = std::env::temp_dir().join("chatlens-export-test.md");
        let export = application::export_conversation_markdown(&db, &conv_id, &export_path)
            .expect("export");
        assert!(export.message_count > 0);
        assert!(export_path.exists());
    }
}
