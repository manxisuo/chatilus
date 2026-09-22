import { describe, expect, it } from "vitest";
import { conversationSourceFromId, sourceLabel, sourceTagType } from "../dataSource";

// sanity: conversationSourceFromId is the bridge used by Timeline / search hit labels
describe("source id bridge used by UI", () => {
  it("supports every known source prefix", () => {
    for (const source of [
      "chatgpt",
      "cursor",
      "codex",
      "claude",
      "gemini",
      "deepseek",
      "copilot",
      "grok",
    ]) {
      expect(conversationSourceFromId(`${source}::x`)).toBe(source);
      expect(sourceLabel(source)).not.toBe("Unknown");
      expect(sourceTagType(source)).toBeTruthy();
    }
  });
});
