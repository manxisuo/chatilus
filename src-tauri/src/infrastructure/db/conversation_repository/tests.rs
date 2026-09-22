    use super::*;
use crate::domain::ports::ConversationRepository;
    use crate::infrastructure::db::Database;
    use crate::domain::models::DataSource;
    use crate::domain::ports::{
        ConversationListQuery, ImportedAttachment, ImportedConversation, ImportedMessage,
        TimelineListQuery,
    };
    use crate::infrastructure::db::helpers::parse_attachments_json;

    fn sample_conversation(message_ids: &[&str]) -> ImportedConversation {
        ImportedConversation {
            id: "conv-1".to_string(),
            title: "Merge Test".to_string(),
            create_time: Some(1.0),
            update_time: Some(2.0),
            model: Some("gpt-4".to_string()),
            messages: message_ids
                .iter()
                .map(|message_id| ImportedMessage {
                    id: (*message_id).to_string(),
                    role: "user".to_string(),
                    content: format!("body-{message_id}"),
                    create_time: Some(3.0),
                    raw_json: "{}".to_string(),
                    attachments: Vec::<ImportedAttachment>::new(),
                })
                .collect(),
            source_contexts: Vec::new(),
        }
    }

    #[test]
    fn merge_import_keeps_existing_messages_from_other_packages() {
        let path = std::env::temp_dir().join(format!(
            "chatlens-merge-test-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let _ = std::fs::remove_file(&path);

        let mut db = Database::open(&path).expect("open db");
        ConversationRepository::save_many(
            &mut db,
            &[sample_conversation(&["m1", "m2"])],
            "/export-a",
            DataSource::ChatGpt,
            0,
        )
        .expect("first import");

        ConversationRepository::save_many(
            &mut db,
            &[sample_conversation(&["m2", "m3"])],
            "/export-b",
            DataSource::ChatGpt,
            0,
        )
        .expect("second import");

        let message_count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE conversation_id = 'chatgpt::conv-1'",
                [],
                |row| row.get(0),
            )
            .expect("count messages");
        assert_eq!(message_count, 3);

        let conversation_id: String = db
            .conn
            .query_row("SELECT id FROM conversations", [], |row| row.get(0))
            .expect("conversation id");
        assert_eq!(conversation_id, "chatgpt::conv-1");

        let source_id: String = db
            .conn
            .query_row(
                "SELECT source_id FROM conversations WHERE id = 'chatgpt::conv-1'",
                [],
                |row| row.get(0),
            )
            .expect("source id");
        assert_eq!(source_id, "conv-1");
    }

    #[test]
    fn merge_import_preserves_existing_attachments_when_incoming_is_empty() {
        let path = std::env::temp_dir().join(format!(
            "chatlens-attachment-merge-test-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let _ = std::fs::remove_file(&path);

        let mut db = Database::open(&path).expect("open db");
        let legacy_image_path = r"C:\legacy-export\file-abc.png".to_string();

        ConversationRepository::save_many(
            &mut db,
            &[ImportedConversation {
                id: "conv-attachments".to_string(),
                title: "Images".to_string(),
                create_time: Some(1.0),
                update_time: Some(2.0),
                model: None,
                messages: vec![ImportedMessage {
                    id: "m1".to_string(),
                    role: "user".to_string(),
                    content: "see image".to_string(),
                    create_time: Some(3.0),
                    raw_json: "{}".to_string(),
                    attachments: vec![ImportedAttachment {
                        pointer: "file-abc".to_string(),
                        source: "upload".to_string(),
                        prompt: None,
                        path: Some(legacy_image_path.clone()),
                    }],
                }],
                source_contexts: Vec::new(),
            }],
            "/legacy-export",
            DataSource::ChatGpt,
            0,
        )
        .expect("legacy import");

        ConversationRepository::save_many(
            &mut db,
            &[ImportedConversation {
                id: "conv-attachments".to_string(),
                title: "Images".to_string(),
                create_time: Some(1.0),
                update_time: Some(4.0),
                model: None,
                messages: vec![ImportedMessage {
                    id: "m1".to_string(),
                    role: "user".to_string(),
                    content: "see image".to_string(),
                    create_time: Some(3.0),
                    raw_json: "{}".to_string(),
                    attachments: Vec::new(),
                }],
                source_contexts: Vec::new(),
            }],
            "/manifest-v1-export",
            DataSource::ChatGpt,
            0,
        )
        .expect("manifest v1 import");

        let attachments_json: String = db
            .conn
            .query_row(
                "SELECT attachments FROM messages WHERE id = 'chatgpt::conv-attachments::m1'",
                [],
                |row| row.get(0),
            )
            .expect("attachments");
        let attachments = parse_attachments_json(&attachments_json);
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].file_key, "file-abc");
        assert_eq!(attachments[0].path, legacy_image_path);

        let asset_count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM assets WHERE conversation_id = 'chatgpt::conv-attachments'",
                [],
                |row| row.get(0),
            )
            .expect("asset count");
        assert_eq!(asset_count, 1);
    }

    #[test]
    fn distinct_sources_with_same_source_id_do_not_collide() {
        let path = std::env::temp_dir().join(format!(
            "chatlens-source-id-test-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let _ = std::fs::remove_file(&path);

        let mut db = Database::open(&path).expect("open db");
        let shared_source_id = "shared-composer-id";

        ConversationRepository::save_many(
            &mut db,
            &[ImportedConversation {
                id: shared_source_id.to_string(),
                title: "ChatGPT".to_string(),
                create_time: None,
                update_time: None,
                model: None,
                messages: vec![ImportedMessage {
                    id: "m1".to_string(),
                    role: "user".to_string(),
                    content: "from chatgpt".to_string(),
                    create_time: None,
                    raw_json: "{}".to_string(),
                    attachments: Vec::new(),
                }],
                source_contexts: Vec::new(),
            }],
            "/chatgpt",
            DataSource::ChatGpt,
            0,
        )
        .expect("chatgpt import");

        ConversationRepository::save_many(
            &mut db,
            &[ImportedConversation {
                id: shared_source_id.to_string(),
                title: "Cursor".to_string(),
                create_time: None,
                update_time: None,
                model: None,
                messages: vec![ImportedMessage {
                    id: "m1".to_string(),
                    role: "user".to_string(),
                    content: "from cursor".to_string(),
                    create_time: None,
                    raw_json: "{}".to_string(),
                    attachments: Vec::new(),
                }],
                source_contexts: Vec::new(),
            }],
            "/cursor",
            DataSource::Cursor,
            0,
        )
        .expect("cursor import");

        let conversation_count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM conversations", [], |row| row.get(0))
            .expect("conversation count");
        assert_eq!(conversation_count, 2);
    }

    #[test]
    fn list_filters_by_source() {
        let path = std::env::temp_dir().join(format!(
            "chatlens-source-filter-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let _ = std::fs::remove_file(&path);

        let mut db = Database::open(&path).expect("open db");
        ConversationRepository::save_many(
            &mut db,
            &[sample_conversation(&["m1"])],
            "/chatgpt",
            DataSource::ChatGpt,
            0,
        )
        .expect("chatgpt import");
        ConversationRepository::save_many(
            &mut db,
            &[ImportedConversation {
                id: "cursor-only".to_string(),
                title: "Cursor only".to_string(),
                create_time: None,
                update_time: None,
                model: None,
                messages: vec![ImportedMessage {
                    id: "m1".to_string(),
                    role: "user".to_string(),
                    content: "cursor".to_string(),
                    create_time: None,
                    raw_json: "{}".to_string(),
                    attachments: Vec::new(),
                }],
                source_contexts: Vec::new(),
            }],
            "/cursor",
            DataSource::Cursor,
            0,
        )
        .expect("cursor import");

        let cursor_only = ConversationRepository::list(
            &db,
            ConversationListQuery {
                source: Some("cursor".to_string()),
                limit: 10,
                offset: 0,
                ..ConversationListQuery::default()
            },
        )
        .expect("list cursor");
        assert_eq!(cursor_only.len(), 1);
        assert_eq!(cursor_only[0].source, "cursor");
    }

    #[test]
    fn reorder_puts_user_before_assistant_when_timestamps_match() {
        let path = std::env::temp_dir().join(format!(
            "chatlens-message-order-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let _ = std::fs::remove_file(&path);

        let mut db = Database::open(&path).expect("open db");
        let conversation = ImportedConversation {
            id: "gemini-activity-1".to_string(),
            title: "draw pics".to_string(),
            create_time: Some(1_718_822_931.0),
            update_time: Some(1_718_822_931.0),
            model: Some("gemini".to_string()),
            messages: vec![
                ImportedMessage {
                    id: "abc::user".to_string(),
                    role: "user".to_string(),
                    content: "can you draw pics".to_string(),
                    create_time: Some(1_718_822_931.0),
                    raw_json: "{}".to_string(),
                    attachments: Vec::new(),
                },
                ImportedMessage {
                    id: "abc::assistant".to_string(),
                    role: "assistant".to_string(),
                    content: "sure".to_string(),
                    create_time: Some(1_718_822_931.0),
                    raw_json: "{}".to_string(),
                    attachments: Vec::new(),
                },
            ],
            source_contexts: Vec::new(),
        };

        ConversationRepository::save_many(
            &mut db,
            &[conversation],
            "/gemini",
            DataSource::Gemini,
            0,
        )
        .expect("gemini import");

        let roles: Vec<String> = db
            .conn
            .prepare(
                "SELECT role FROM messages
                 WHERE conversation_id = 'gemini::gemini-activity-1'
                 ORDER BY sort_order",
            )
            .expect("prepare")
            .query_map([], |row| row.get(0))
            .expect("query")
            .collect::<Result<Vec<_>, _>>()
            .expect("roles");

        assert_eq!(roles, vec!["user".to_string(), "assistant".to_string()]);
    }

    #[test]
    fn timeline_orders_by_activity_time_without_starred_boost() {
        let path = std::env::temp_dir().join(format!(
            "chatlens-timeline-order-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let _ = std::fs::remove_file(&path);

        let mut db = Database::open(&path).expect("open db");
        ConversationRepository::save_many(
            &mut db,
            &[ImportedConversation {
                id: "older-starred".to_string(),
                title: "Older starred".to_string(),
                create_time: Some(100.0),
                update_time: Some(100.0),
                model: None,
                messages: vec![ImportedMessage {
                    id: "m1".to_string(),
                    role: "user".to_string(),
                    content: "old".to_string(),
                    create_time: Some(100.0),
                    raw_json: "{}".to_string(),
                    attachments: Vec::new(),
                }],
                source_contexts: Vec::new(),
            }],
            "/a",
            DataSource::ChatGpt,
            0,
        )
        .expect("older import");

        ConversationRepository::save_many(
            &mut db,
            &[ImportedConversation {
                id: "newer-plain".to_string(),
                title: "Newer plain".to_string(),
                create_time: Some(200.0),
                update_time: Some(200.0),
                model: None,
                messages: vec![ImportedMessage {
                    id: "m1".to_string(),
                    role: "user".to_string(),
                    content: "new".to_string(),
                    create_time: Some(200.0),
                    raw_json: "{}".to_string(),
                    attachments: Vec::new(),
                }],
                source_contexts: Vec::new(),
            }],
            "/b",
            DataSource::Cursor,
            0,
        )
        .expect("newer import");

        db.conn
            .execute(
                "UPDATE conversations SET is_starred = 1 WHERE id = 'chatgpt::older-starred'",
                [],
            )
            .expect("star older");

        let timeline = ConversationRepository::list_timeline(
            &db,
            TimelineListQuery {
                limit: 10,
                offset: 0,
                ..TimelineListQuery::default()
            },
        )
        .expect("timeline");

        assert_eq!(timeline.len(), 2);
        assert_eq!(timeline[0].id, "cursor::newer-plain");
        assert_eq!(timeline[1].id, "chatgpt::older-starred");
    }

    #[test]
    fn timeline_month_buckets_and_filter() {
        let path = std::env::temp_dir().join(format!(
            "chatlens-timeline-months-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let _ = std::fs::remove_file(&path);

        let mut db = Database::open(&path).expect("open db");
        ConversationRepository::save_many(
            &mut db,
            &[ImportedConversation {
                id: "june-chatgpt".to_string(),
                title: "June ChatGPT".to_string(),
                create_time: Some(1_718_208_000.0),
                update_time: Some(1_718_208_000.0),
                model: None,
                messages: vec![ImportedMessage {
                    id: "m1".to_string(),
                    role: "user".to_string(),
                    content: "june".to_string(),
                    create_time: Some(1_718_208_000.0),
                    raw_json: "{}".to_string(),
                    attachments: Vec::new(),
                }],
                source_contexts: Vec::new(),
            }],
            "/chatgpt",
            DataSource::ChatGpt,
            0,
        )
        .expect("june chatgpt");

        ConversationRepository::save_many(
            &mut db,
            &[ImportedConversation {
                id: "july-cursor".to_string(),
                title: "July Cursor".to_string(),
                create_time: Some(1_720_886_400.0),
                update_time: Some(1_720_886_400.0),
                model: None,
                messages: vec![ImportedMessage {
                    id: "m1".to_string(),
                    role: "user".to_string(),
                    content: "july".to_string(),
                    create_time: Some(1_720_886_400.0),
                    raw_json: "{}".to_string(),
                    attachments: Vec::new(),
                }],
                source_contexts: Vec::new(),
            }],
            "/cursor",
            DataSource::Cursor,
            0,
        )
        .expect("july cursor");

        let months = ConversationRepository::list_timeline_months(&db, None).expect("months");
        assert_eq!(months.len(), 2);
        assert_eq!(months[0].month, "2024-07");
        assert_eq!(months[1].month, "2024-06");
        let june_sources: Vec<_> = months[1]
            .source_counts
            .iter()
            .map(|item| item.source.as_str())
            .collect();
        assert!(june_sources.contains(&"chatgpt"));
        assert!(months[0]
            .source_counts
            .iter()
            .any(|item| item.source == "cursor"));

        let cursor_months =
            ConversationRepository::list_timeline_months(&db, Some("cursor")).expect("cursor months");
        assert_eq!(cursor_months.len(), 1);
        assert_eq!(cursor_months[0].month, "2024-07");

        let june_only = ConversationRepository::list_timeline(
            &db,
            TimelineListQuery {
                month: Some("2024-06".to_string()),
                limit: 10,
                offset: 0,
                ..TimelineListQuery::default()
            },
        )
        .expect("june timeline");
        assert_eq!(june_only.len(), 1);
        assert_eq!(june_only[0].title, "June ChatGPT");
    }

    #[test]
    fn import_persists_and_loads_source_contexts() {
        use crate::domain::ports::ImportedSourceContext;

        let path = std::env::temp_dir().join(format!(
            "chatlens-source-context-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let _ = std::fs::remove_file(&path);

        let mut db = Database::open(&path).expect("open db");
        ConversationRepository::save_many(
            &mut db,
            &[ImportedConversation {
                id: "conv-project".to_string(),
                title: "In Project".to_string(),
                create_time: Some(1.0),
                update_time: Some(2.0),
                model: None,
                messages: vec![ImportedMessage {
                    id: "m1".to_string(),
                    role: "user".to_string(),
                    content: "hello".to_string(),
                    create_time: Some(1.0),
                    raw_json: "{}".to_string(),
                    attachments: Vec::new(),
                }],
                source_contexts: vec![ImportedSourceContext {
                    context_type: "project".to_string(),
                    external_id: Some("proj-1".to_string()),
                    name: "ChatLens".to_string(),
                    path: None,
                    raw_json: None,
                }],
            }],
            "/chatgpt",
            DataSource::ChatGpt,
            0,
        )
        .expect("save");

        let summary = ConversationRepository::get_summary(&db, "chatgpt::conv-project")
            .expect("get")
            .expect("summary");
        assert_eq!(summary.source_contexts.len(), 1);
        assert_eq!(summary.source_contexts[0].name, "ChatLens");
        assert_eq!(summary.source_contexts[0].context_type, "project");
    }
