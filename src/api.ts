import { invoke } from "@tauri-apps/api/core";
import type {
  ConversationSummary,
  DatabaseStats,
  ExportResult,
  ImportResult,
  MessageView,
  SearchHit,
  TagView,
} from "./types";

export function importExportDir(path: string) {
  return invoke<ImportResult>("import_export_dir", { path });
}

export function listConversations(options?: {
  query?: string;
  starredOnly?: boolean;
  tagId?: number | null;
  limit?: number;
  offset?: number;
}) {
  return invoke<ConversationSummary[]>("list_conversations", {
    query: options?.query || null,
    starredOnly: options?.starredOnly ?? false,
    tagId: options?.tagId ?? null,
    limit: options?.limit ?? 200,
    offset: options?.offset ?? 0,
  });
}

export function getMessages(conversationId: string) {
  return invoke<MessageView[]>("get_messages", { conversationId });
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

export function readImageDataUrl(path: string) {
  return invoke<string>("read_image_data_url", { path });
}

export function getStats() {
  return invoke<DatabaseStats>("get_stats");
}
