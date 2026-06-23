import { setTheme as setAppTheme } from "@tauri-apps/api/app";
import hljsDark from "highlight.js/styles/github-dark.css?url";
import hljsLight from "highlight.js/styles/github.css?url";

export type AppearanceMode = "light" | "dark" | "system";

const STORAGE_KEY = "chatlens-appearance";

let hljsLink: HTMLLinkElement | null = null;
let systemMediaQuery: MediaQueryList | null = null;
let systemListener: ((event: MediaQueryListEvent) => void) | null = null;

export function getStoredAppearance(): AppearanceMode {
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored === "light" || stored === "dark" || stored === "system") {
    return stored;
  }
  return "system";
}

function resolveTheme(mode: AppearanceMode): "light" | "dark" {
  if (mode === "system") {
    return window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "dark"
      : "light";
  }
  return mode;
}

function ensureHljsLink() {
  if (hljsLink) return hljsLink;
  hljsLink = document.getElementById("hljs-theme") as HTMLLinkElement | null;
  if (!hljsLink) {
    hljsLink = document.createElement("link");
    hljsLink.id = "hljs-theme";
    hljsLink.rel = "stylesheet";
    document.head.appendChild(hljsLink);
  }
  return hljsLink;
}

function updateHighlightTheme(theme: "light" | "dark") {
  const link = ensureHljsLink();
  link.href = theme === "dark" ? hljsDark : hljsLight;
}

async function syncNativeTheme(mode: AppearanceMode) {
  try {
    await setAppTheme(mode === "system" ? null : mode);
  } catch {
    // Web preview or unsupported platform.
  }
}

function teardownSystemListener() {
  if (systemListener && systemMediaQuery) {
    systemMediaQuery.removeEventListener("change", systemListener);
  }
  systemListener = null;
  systemMediaQuery = null;
}

function setupSystemListener(mode: AppearanceMode) {
  teardownSystemListener();
  if (mode !== "system") return;

  systemMediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
  systemListener = () => {
    void applyAppearance("system");
  };
  systemMediaQuery.addEventListener("change", systemListener);
}

export function applyAppearance(mode: AppearanceMode) {
  const resolved = resolveTheme(mode);
  const root = document.documentElement;

  root.dataset.appearance = mode;
  root.dataset.theme = resolved;
  root.classList.toggle("dark", resolved === "dark");
  localStorage.setItem(STORAGE_KEY, mode);
  updateHighlightTheme(resolved);
  void syncNativeTheme(mode);
}

export function setAppearance(mode: AppearanceMode) {
  applyAppearance(mode);
  setupSystemListener(mode);
}

export function initAppearance() {
  const mode = getStoredAppearance();
  applyAppearance(mode);
  setupSystemListener(mode);
}
