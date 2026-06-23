import { invoke } from "@tauri-apps/api/core";
import type {
  ConversationSummary,
  DatabaseStats,
  ExportResult,
  ImageGalleryItem,
  ImportJobView,
  ImportResult,
  MessageView,
  SearchHit,
  TagView,
  TimelineMonthBucket,
} from "./types";

export function importExportDir(path: string) {
  return invoke<ImportResult>("import_export_dir", { path });
}

export function startImport(path: string) {
  return invoke<string>("start_import", { path });
}

export function getImportJob(jobId: string) {
  return invoke<ImportJobView>("get_import_job", { jobId });
}

export function listConversations(options?: {
  query?: string;
  starredOnly?: boolean;
  tagId?: number | null;
  source?: string | null;
  hasImages?: boolean;
  hasCode?: boolean;
  hasAttachments?: boolean;
  limit?: number;
  offset?: number;
}) {
  return invoke<ConversationSummary[]>("list_conversations", {
    query: options?.query || null,
    starredOnly: options?.starredOnly ?? false,
    tagId: options?.tagId ?? null,
    source: options?.source ?? null,
    hasImages: options?.hasImages ?? false,
    hasCode: options?.hasCode ?? false,
    hasAttachments: options?.hasAttachments ?? false,
    limit: options?.limit ?? 200,
    offset: options?.offset ?? 0,
  });
}

export function getConversation(conversationId: string) {
  return invoke<ConversationSummary | null>("get_conversation", { conversationId });
}

export function getMessages(conversationId: string) {
  return invoke<MessageView[]>("get_messages", { conversationId });
}

export function listStarredMessages(options?: {
  source?: string | null;
  limit?: number;
  offset?: number;
}) {
  return invoke<SearchHit[]>("list_starred_messages", {
    source: options?.source ?? null,
    limit: options?.limit ?? 200,
    offset: options?.offset ?? 0,
  });
}

export function listTimelineMonths(source?: string | null) {
  return invoke<TimelineMonthBucket[]>("list_timeline_months", {
    source: source ?? null,
  });
}

export function listTimeline(options?: {
  source?: string | null;
  month?: string | null;
  limit?: number;
  offset?: number;
}) {
  return invoke<ConversationSummary[]>("list_timeline", {
    source: options?.source ?? null,
    month: options?.month ?? null,
    limit: options?.limit ?? 100,
    offset: options?.offset ?? 0,
  });
}

export function searchMessages(query: string, limit = 100) {
  return invoke<SearchHit[]>("search_messages", { query, limit });
}

export function setConversationStarred(conversationId: string, starred: boolean) {
  return invoke<void>("set_conversation_starred", { conversationId, starred });
}

export function setMessageStarred(messageId: string, starred: boolean) {
  return invoke<void>("set_message_starred", { messageId, starred });
}

export function listTags() {
  return invoke<TagView[]>("list_tags");
}

export function createTag(name: string) {
  return invoke<TagView>("create_tag", { name });
}

export function deleteTag(tagId: number) {
  return invoke<void>("delete_tag", { tagId });
}

export function setConversationTags(conversationId: string, tagIds: number[]) {
  return invoke<string[]>("set_conversation_tags", { conversationId, tagIds });
}

export function exportConversationMarkdown(conversationId: string, outputPath: string) {
  return invoke<ExportResult>("export_conversation_markdown", {
    conversationId,
    outputPath,
  });
}

export function listImages(
  limit = 60,
  offset = 0,
  includeUploads = true,
  conversationSource?: string | null,
) {
  return invoke<ImageGalleryItem[]>("list_images", {
    limit,
    offset,
    includeUploads,
    conversationSource: conversationSource ?? null,
  });
}

export function readImageDataUrl(path: string) {
  return invoke<string>("read_image_data_url", { path });
}

export function getStats() {
  return invoke<DatabaseStats>("get_stats");
}
