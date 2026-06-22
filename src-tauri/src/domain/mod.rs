pub mod composite_id;
pub mod import_merge;
pub mod mappers;
pub mod models;
pub mod ports;

#[allow(unused_imports)]
pub use models::{Asset, AssetType, Conversation, DataSource, Message, MessageContent, MessageRole};
#[allow(unused_imports)]
pub use ports::{
    AssetListQuery, AssetRepository, ConversationListQuery, ConversationRepository,
    ImportDetectResult, ImportInput, ImportOptions, ImportPackage, ImportPreview, Importer,
    ImportedAttachment, ImportedConversation, ImportedMessage, ImportPersistCounts,
    MessageRepository, NormalizedImportResult, SearchEngine, SearchIndexEntry, SearchQuery,
    SearchResult,
};
