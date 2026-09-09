import { save } from "@tauri-apps/plugin-dialog";
import { saveImageCopy } from "../api";

function sanitizeFileName(name: string): string {
  return name.replace(/[\\/:*?"<>|]/g, "_").trim() || "image";
}

function extensionFromPath(path: string): string {
  const match = path.match(/\.([a-zA-Z0-9]{1,8})$/);
  return match?.[1]?.toLowerCase() ?? "png";
}

function defaultFileName(path: string, fileKey?: string): string {
  const raw = (fileKey && fileKey.trim()) || path.split(/[/\\]/).pop() || "image";
  const base = sanitizeFileName(raw.replace(/\.[a-zA-Z0-9]{1,8}$/, "") || "image");
  const ext = extensionFromPath(path);
  return `${base}.${ext}`;
}

/** Open a save dialog and copy the image file to the chosen location. Returns true if saved. */
export async function promptAndSaveImage(
  sourcePath: string,
  options?: { fileKey?: string; dialogTitle?: string },
): Promise<boolean> {
  const ext = extensionFromPath(sourcePath);
  const outputPath = await save({
    defaultPath: defaultFileName(sourcePath, options?.fileKey),
    filters: [
      {
        name: "Image",
        extensions: [ext, "png", "jpg", "jpeg", "webp", "gif"].filter(
          (item, index, list) => list.indexOf(item) === index,
        ),
      },
    ],
    title: options?.dialogTitle ?? "Save image",
  });

  if (!outputPath) return false;

  await saveImageCopy(sourcePath, outputPath);
  return true;
}
