export interface ConversationSummary {
  id: string;
  title: string;
  create_time: number | null;
  update_time: number | null;
  model: string | null;
  message_count: number;
  is_starred: boolean;
  source_path: string;
  tags: string[];
}

export interface AttachmentView {
  file_key: string;
  path: string;
  source: "generated" | "upload" | "unknown";
  prompt?: string | null;
}

export interface MessageView {
  id: string;
  conversation_id: string;
  role: string;
  content: string;
  create_time: number | null;
  sort_order: number;
  is_starred: boolean;
  attachments: AttachmentView[];
}

export interface SearchHit {
  message_id: string;
  conversation_id: string;
  conversation_title: string;
  role: string;
  snippet: string;
  create_time: number | null;
}

export interface TagView {
  id: number;
  name: string;
  conversation_count: number;
}

export interface ImportResult {
  conversations_imported: number;
  conversations_updated: number;
  conversations_deduplicated: number;
  messages_imported: number;
  files_processed: number;
  source_path: string;
  media_files_indexed: number;
}

export interface ImportProgressEvent {
  job_id: string;
  phase: string;
  progress: number;
  processed: number;
  total: number;
}

export interface ImportJobView {
  id: string;
  source_path: string;
  resolved_path?: string | null;
  status: string;
  phase: string;
  progress: number;
  processed: number;
  total: number;
  error?: string | null;
  source?: string | null;
  export_label?: string | null;
  importer_version?: string | null;
  result?: ImportResult | null;
}

export interface ImageGalleryItem {
  path: string;
  file_key: string;
  conversation_id: string;
  conversation_title: string;
  message_id: string;
  role: string;
  create_time: number | null;
  source: "generated" | "upload" | "unknown";
  prompt?: string | null;
}

export interface DatabaseStats {
  conversation_count: number;
  message_count: number;
  image_count: number;
  generated_image_count: number;
  upload_image_count: number;
  starred_conversation_count: number;
  starred_message_count: number;
  tag_count: number;
  db_path: string;
}

export interface ExportResult {
  path: string;
  message_count: number;
}
