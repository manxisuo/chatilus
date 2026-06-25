import type { SourceContextView } from "../types";
import { sourceLabel } from "./dataSource";

const CONTEXT_TYPE_LABELS: Record<string, string> = {
  project: "Project",
  repository: "Repo",
  notebook: "Notebook",
  folder: "Folder",
  workspace: "Workspace",
};

export function sourceContextTypeLabel(contextType: string): string {
  return CONTEXT_TYPE_LABELS[contextType.toLowerCase()] ?? contextType;
}

export function formatContextDisplayName(context: SourceContextView): string {
  if (
    context.context_type === "project" &&
    context.source === "chatgpt" &&
    /^g-p/.test(context.name)
  ) {
    const tail =
      context.name.length > 8 ? context.name.slice(-8) : context.name;
    return `未命名 · …${tail}`;
  }
  return context.name;
}

export function formatSourceContextLine(context: SourceContextView): string {
  const label = sourceContextTypeLabel(context.context_type);
  return `${sourceLabel(context.source)} · ${label}: ${formatContextDisplayName(context)}`;
}

export function sourceContextIdHint(
  context: SourceContextView,
): string | null {
  if (context.context_type !== "project" || !context.external_id) {
    return null;
  }
  if (
    context.name.startsWith("未命名") ||
    /^g-p/.test(context.name) ||
    context.external_id !== context.name
  ) {
    return context.external_id;
  }
  return null;
}

export function formatSourceContextSummary(
  contexts: SourceContextView[] | undefined,
): string | null {
  if (!contexts?.length) {
    return null;
  }
  return contexts.map(formatSourceContextLine).join(" · ");
}
