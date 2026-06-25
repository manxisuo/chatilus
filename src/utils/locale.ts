export type AppLocale = "zh-CN" | "en";

const STORAGE_KEY = "chatlens-locale";

export function getStoredLocale(): AppLocale {
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored === "zh-CN" || stored === "en") {
    return stored;
  }
  if (typeof navigator !== "undefined" && navigator.language.startsWith("zh")) {
    return "zh-CN";
  }
  return "en";
}

export function setStoredLocale(locale: AppLocale) {
  localStorage.setItem(STORAGE_KEY, locale);
  document.documentElement.lang = locale === "zh-CN" ? "zh-CN" : "en";
}

export function intlLocale(locale: AppLocale): string {
  return locale === "zh-CN" ? "zh-CN" : "en-US";
}

export function formatDateTime(
  timestamp: number | null | undefined,
  locale: AppLocale,
  options?: Intl.DateTimeFormatOptions,
): string {
  if (!timestamp) return "—";
  const defaults: Intl.DateTimeFormatOptions = {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  };
  return new Date(timestamp * 1000).toLocaleString(intlLocale(locale), {
    ...defaults,
    ...options,
  });
}

export function formatMonthKey(key: string, locale: AppLocale): string {
  const [year, month] = key.split("-").map(Number);
  if (locale === "zh-CN") {
    return `${year}年${month}月`;
  }
  return new Date(year, month - 1, 1).toLocaleDateString("en-US", {
    month: "long",
    year: "numeric",
  });
}

export function formatDayKey(key: string, locale: AppLocale): string {
  const [, month, day] = key.split("-").map(Number);
  if (locale === "zh-CN") {
    return `${month}月${day}日`;
  }
  const year = Number(key.split("-")[0]);
  return new Date(year, month - 1, day).toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
  });
}
