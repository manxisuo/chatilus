import { describe, expect, it } from "vitest";
import { LAYOUT_BREAKPOINTS, LAYOUT_WIDTHS } from "../layout";

describe("layout constants", () => {
  it("keeps breakpoints ordered", () => {
    expect(LAYOUT_BREAKPOINTS.hideRightPanel).toBeGreaterThan(
      LAYOUT_BREAKPOINTS.compactLeftPanel,
    );
    expect(LAYOUT_BREAKPOINTS.compactLeftPanel).toBeGreaterThan(
      LAYOUT_BREAKPOINTS.singleColumn,
    );
  });

  it("keeps positive widths", () => {
    expect(LAYOUT_WIDTHS.sidebar).toBeGreaterThan(0);
    expect(LAYOUT_WIDTHS.mainMin).toBeGreaterThan(0);
  });
});
