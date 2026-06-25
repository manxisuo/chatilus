export const KNOWN_DATA_SOURCES = [
  "chatgpt",
  "cursor",
  "codex",
  "claude",
  "gemini",
] as const;

export type DataSourceKey = (typeof KNOWN_DATA_SOURCES)[number];

export function sourceLabel(source?: string | null): string {
  switch (source?.toLowerCase()) {
    case "cursor":
      return "Cursor";
    case "codex":
      return "Codex";
    case "claude":
      return "Claude";
    case "gemini":
      return "Gemini";
    case "chatgpt":
      return "ChatGPT";
    default:
      return source?.trim() || "Unknown";
  }
}

export function sourceTagType(
  source?: string | null,
): "primary" | "success" | "warning" | "info" | "danger" {
  switch (source?.toLowerCase()) {
    case "cursor":
      return "primary";
    case "codex":
      return "warning";
    case "claude":
      return "warning";
    case "gemini":
      return "success";
    case "chatgpt":
      return "info";
    default:
      return "info";
  }
}

export function conversationSourceFromId(conversationId: string): string {
  const separator = conversationId.indexOf("::");
  if (separator > 0) {
    return conversationId.slice(0, separator);
  }
  return "chatgpt";
}
