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
      return "#10a37f";
    case "cursor":
      return "#6b8afd";
    case "codex":
      return "#c9a227";
    case "claude":
      return "#d97757";
    case "gemini":
      return "#4285f4";
    case "deepseek":
      return "#4f6bed";
    case "copilot":
      return "#7c8cff";
    case "grok":
      return "#a3a3a3";
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
