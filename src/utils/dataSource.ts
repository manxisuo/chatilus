export const KNOWN_DATA_SOURCES = [
  "chatgpt",
  "cursor",
  "codex",
  "claude",
  "gemini",
  "deepseek",
  "copilot",
  "grok",
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
    case "deepseek":
      return "DeepSeek";
    case "copilot":
      return "Copilot";
    case "grok":
      return "Grok";
    case "chatgpt":
      return "ChatGPT";
    default:
      return source?.trim() || "Unknown";
  }
}

/** Muted accent for source dots and inline labels (not Element Plus tag colors). */
export function sourceAccentColor(source?: string | null): string {
  switch (source?.toLowerCase()) {
    case "chatgpt":
      return "#3d9a80";
    case "cursor":
      return "#6b7fd4";
    case "codex":
      return "#b8942e";
    case "claude":
      return "#c4684f";
    case "gemini":
      return "#5a8fd4";
    case "deepseek":
      return "#5a6fc9";
    case "copilot":
      return "#7a84c4";
    case "grok":
      return "#8a8a8a";
    default:
      return "#9ca3af";
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
    case "deepseek":
      return "primary";
    case "copilot":
      return "info";
    case "grok":
      return "warning";
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
