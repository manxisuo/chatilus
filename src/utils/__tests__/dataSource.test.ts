import { describe, expect, it } from "vitest";
import {
  conversationSourceFromId,
  sourceLabel,
  sourceTagType,
  KNOWN_DATA_SOURCES,
} from "../dataSource";

describe("dataSource", () => {
  it("maps known sources to display labels", () => {
    expect(sourceLabel("chatgpt")).toBe("ChatGPT");
    expect(sourceLabel("Cursor")).toBe("Cursor");
    expect(sourceLabel("deepseek")).toBe("DeepSeek");
  });

  it("falls back for unknown or empty sources", () => {
    expect(sourceLabel(null)).toBe("Unknown");
    expect(sourceLabel("  ")).toBe("Unknown");
    expect(sourceLabel("my-tool")).toBe("my-tool");
  });

  it("assigns stable tag types", () => {
    expect(sourceTagType("cursor")).toBe("primary");
    expect(sourceTagType("chatgpt")).toBe("info");
    expect(sourceTagType("unknown")).toBe("info");
  });

  it("parses composite conversation ids", () => {
    expect(conversationSourceFromId("cursor::abc-123")).toBe("cursor");
    expect(conversationSourceFromId("chatgpt::conv-1")).toBe("chatgpt");
    expect(conversationSourceFromId("legacy-id")).toBe("chatgpt");
  });

  it("lists all supported sources", () => {
    expect(KNOWN_DATA_SOURCES).toContain("chatgpt");
    expect(KNOWN_DATA_SOURCES).toContain("cursor");
    expect(KNOWN_DATA_SOURCES.length).toBeGreaterThanOrEqual(8);
  });
});
