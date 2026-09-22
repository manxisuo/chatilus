import { describe, expect, it } from "vitest";
import {
  formatContextDisplayName,
  formatSourceContextLine,
  formatSourceContextSummary,
  sourceContextIdHint,
  sourceContextTypeLabel,
} from "../sourceContext";
import type { SourceContextView } from "../../types";

function ctx(partial: Partial<SourceContextView>): SourceContextView {
  return {
    id: "ctx-1",
    context_type: "project",
    name: "Chatilus",
    source: "chatgpt",
    external_id: null,
    path: null,
    ...partial,
  };
}

describe("sourceContext", () => {
  it("maps context type labels", () => {
    expect(sourceContextTypeLabel("project")).toBe("Project");
    expect(sourceContextTypeLabel("repository")).toBe("Repo");
    expect(sourceContextTypeLabel("custom")).toBe("custom");
  });

  it("masks ChatGPT g-p project names", () => {
    const display = formatContextDisplayName(
      ctx({ name: "g-p-67ecb74315e081918a6c820ee7492ae7" }),
    );
    expect(display.startsWith("未命名 · …")).toBe(true);
  });

  it("keeps normal project names", () => {
    expect(formatContextDisplayName(ctx({ name: "Chatilus" }))).toBe("Chatilus");
  });

  it("formats source context line", () => {
    const line = formatSourceContextLine(
      ctx({ name: "Chatilus", source: "cursor", context_type: "repository" }),
    );
    expect(line).toBe("Cursor · Repo: Chatilus");
  });

  it("returns external id hint only when useful", () => {
    expect(
      sourceContextIdHint(
        ctx({ name: "g-p-abc", external_id: "g-p-abc" }),
      ),
    ).toBe("g-p-abc");
    expect(
      sourceContextIdHint(ctx({ name: "Chatilus", external_id: "Chatilus" })),
    ).toBeNull();
  });

  it("summarizes multiple contexts", () => {
    const summary = formatSourceContextSummary([
      ctx({ name: "Chatilus", source: "chatgpt" }),
      ctx({ name: "workspace", source: "cursor", context_type: "workspace" }),
    ]);
    expect(summary).toContain("ChatGPT · Project: Chatilus");
    expect(summary).toContain("Cursor · Workspace: workspace");
    expect(formatSourceContextSummary([])).toBeNull();
  });
});
