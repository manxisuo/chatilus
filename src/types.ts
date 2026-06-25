export interface ConversationSummary {
  id: string;
  title: string;
  create_time: number | null;
  update_time: number | null;
  model: string | null;
  message_count: number;
  is_starred: boolean;
  source_path: string;
  source?: string;
  tags: string[];
  activity_month?: string | null;
  latest_message_id?: string | null;
  has_images?: boolean;
  source_contexts?: SourceContextView[];
}

export interface SourceContextView {
  id: string;
  source: string;
  context_type: string;
  external_id?: string | null;
  name: string;
  path?: string | null;
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
  source?: string;
}

export interface TagView {
  id: number;
  name: string;
  conversation_count: number;
}

export interface ImportMethodGuide {
  id: string;
  label: string;
  kind: "directory" | "file";
  dialog_title: string;
  extensions?: string[];
  hint: string;
  example_path?: string | null;
  detected_default_path?: string | null;
  detected_default_label?: string | null;
}

export interface ImportGuide {
  importer_id: string;
  source: string;
  display_name: string;
  description: string;
  support_status: "stable" | "experimental";
  support_summary: string;
  recognition_hint: string;
  methods: ImportMethodGuide[];
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

export interface SourceCount {
  source: string;
  count: number;
}

export interface TimelineMonthBucket {
  month: string;
  conversation_count: number;
  image_count?: number;
  source_counts?: SourceCount[];
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
  last_imported_at: number | null;
  db_path: string;
  conversation_counts_by_source?: SourceCount[];
  image_counts_by_source?: SourceCount[];
}

export interface ExportResult {
  path: string;
  message_count: number;
}
