import type { AttachmentView, ImageGalleryItem } from "../types";

export function attachmentCacheKey(
  attachment: Pick<AttachmentView, "file_key" | "path">,
): string {
  return `${attachment.file_key}:${attachment.path}`;
}

export function galleryItemCacheKey(
  item: Pick<ImageGalleryItem, "message_id" | "path">,
): string {
  return `${item.message_id}:${item.path}`;
}
