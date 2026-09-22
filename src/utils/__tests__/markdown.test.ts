import { describe, expect, it } from "vitest";
import { renderMarkdown } from "../markdown";

describe("renderMarkdown", () => {
  it("renders emphasis and paragraphs", () => {
    const html = renderMarkdown("**bold** and *em*");
    expect(html).toContain("<strong>bold</strong>");
    expect(html).toContain("<em>em</em>");
  });

  it("does not render raw HTML (XSS guard)", () => {
    const html = renderMarkdown('<img src=x onerror="alert(1)">');
    expect(html).not.toContain("<img");
    expect(html).toContain("&lt;img");
  });

  it("handles empty input", () => {
    expect(renderMarkdown("")).toBe("");
    expect(renderMarkdown(undefined as unknown as string)).toBe("");
  });

  it("linkifies plain URLs", () => {
    const html = renderMarkdown("see https://example.com now");
    expect(html).toContain('href="https://example.com"');
  });
});
