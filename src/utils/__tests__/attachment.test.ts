import { describe, expect, it } from "vitest";
import { attachmentCacheKey, galleryItemCacheKey } from "../attachment";

describe("attachment cache keys", () => {
  it("builds attachment cache key from file_key and path", () => {
    expect(
      attachmentCacheKey({ file_key: "file-1", path: "C:\\a\\b.png" }),
    ).toBe("file-1:C:\\a\\b.png");
  });

  it("builds gallery cache key from message_id and path", () => {
    expect(
      galleryItemCacheKey({ message_id: "m1", path: "/tmp/x.png" }),
    ).toBe("m1:/tmp/x.png");
  });
});
