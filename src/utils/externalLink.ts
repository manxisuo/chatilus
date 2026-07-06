import { openUrl } from "@tauri-apps/plugin-opener";

export function shouldOpenExternally(href: string): boolean {
  const normalized = href.trim().toLowerCase();
  if (!normalized || normalized.startsWith("#") || normalized.startsWith("javascript:")) {
    return false;
  }
  return /^(https?:|mailto:|tel:)/.test(normalized);
}

export function openExternalHref(href: string): void {
  void openUrl(href).catch(() => {
    window.open(href, "_blank", "noopener,noreferrer");
  });
}

/** 拦截 Markdown 等内容中的外链点击，避免 WebView 整页跳转。 */
export function interceptExternalLinkClick(event: MouseEvent): void {
  const anchor = (event.target as HTMLElement | null)?.closest("a[href]");
  if (!(anchor instanceof HTMLAnchorElement)) return;

  const href = anchor.getAttribute("href");
  if (!href || !shouldOpenExternally(href)) return;

  event.preventDefault();
  event.stopPropagation();
  openExternalHref(href);
}
