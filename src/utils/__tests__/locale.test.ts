import { describe, expect, it } from "vitest";
import {
  formatDateTime,
  formatDayKey,
  formatMonthKey,
  intlLocale,
} from "../locale";

describe("locale helpers", () => {
  it("maps app locale to intl locale", () => {
    expect(intlLocale("zh-CN")).toBe("zh-CN");
    expect(intlLocale("en")).toBe("en-US");
  });

  it("formats missing timestamps as em dash", () => {
    expect(formatDateTime(null, "zh-CN")).toBe("—");
    expect(formatDateTime(undefined, "en")).toBe("—");
  });

  it("formats month keys in Chinese", () => {
    expect(formatMonthKey("2024-06", "zh-CN")).toBe("2024年6月");
  });

  it("formats month keys in English", () => {
    expect(formatMonthKey("2024-06", "en")).toBe("June 2024");
  });

  it("formats day keys in Chinese", () => {
    expect(formatDayKey("2024-06-15", "zh-CN")).toBe("6月15日");
  });
});
